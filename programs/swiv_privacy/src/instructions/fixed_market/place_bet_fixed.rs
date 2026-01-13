use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, TokenAccount, Transfer};
use crate::state::{GlobalConfig, FixedMarket, AssetConfig, UserBet, MarketType, BetStatus, MarketMode};
use crate::constants::{SEED_GLOBAL_CONFIG, SEED_ASSET_CONFIG, SEED_FIXED_MARKET, SEED_BET};
use crate::errors::CustomError;
use crate::utils::infinite_math::{calculate_time_decay_factor, calculate_max_potential_profit}; 
use crate::events::BetPlaced;
use ephemeral_rollups_sdk::access_control::{
    CreateGroupCpiBuilder, CreatePermissionCpiBuilder
};

#[derive(Accounts)]
#[instruction(
    amount: u64,
    declared_multiplier_bps: u64, 
    commitment: [u8; 32], 
    request_id: String
)]
pub struct PlaceBetFixed<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        seeds = [SEED_GLOBAL_CONFIG],
        bump,
        constraint = !global_config.paused @ CustomError::Paused
    )]
    pub global_config: Box<Account<'info, GlobalConfig>>,

    #[account(
        mut,
        seeds = [SEED_FIXED_MARKET, fixed_market.name.as_bytes()],
        bump = fixed_market.bump
    )]
    pub fixed_market: Box<Account<'info, FixedMarket>>,

    #[account(
        seeds = [SEED_ASSET_CONFIG, asset_config.symbol.as_bytes()],
        bump = asset_config.bump
    )]
    pub asset_config: Box<Account<'info, AssetConfig>>,

    #[account(
        mut,
        seeds = [b"fixed_vault", fixed_market.key().as_ref()],
        bump,
        token::authority = fixed_market,
    )]
    pub market_vault: Box<Account<'info, TokenAccount>>,

    #[account(mut)]
    pub user_token_account: Box<Account<'info, TokenAccount>>,

    /// CHECK: Validated against GlobalConfig.treasury_wallet
    #[account(
        mut, 
        token::authority = global_config.treasury_wallet
    )]
    pub treasury_wallet: Box<Account<'info, TokenAccount>>,

    #[account(
        init,
        payer = user,
        space = UserBet::SPACE,
        seeds = [SEED_BET, fixed_market.key().as_ref(), user.key().as_ref(), request_id.as_bytes()], 
        bump
    )]
    pub user_bet: Box<Account<'info, UserBet>>,

    // --- 2. ADD PERMISSION ACCOUNTS ---
    
    /// The Permission Group PDA (derived from seeds)
    /// CHECK: Checked via CPI to MagicBlock program
    #[account(mut)]
    pub group: UncheckedAccount<'info>,

    /// The Permission PDA
    /// CHECK: Checked via CPI to MagicBlock program
    #[account(mut)]
    pub permission: UncheckedAccount<'info>,

    /// The MagicBlock Access Control Program
    /// CHECK: Checked via CPI
    pub permission_program: UncheckedAccount<'info>,
    
    // ----------------------------------

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

pub fn place_bet_fixed(
    ctx: Context<PlaceBetFixed>,
    amount: u64,
    declared_multiplier_bps: u64,
    commitment: [u8; 32], 
    _request_id: String, 
) -> Result<()> {
    let market = &mut ctx.accounts.fixed_market;
    let global_config = &ctx.accounts.global_config;
    let clock = Clock::get()?;

    require!(clock.unix_timestamp >= market.start_time, CustomError::DurationTooShort);
    require!(clock.unix_timestamp < market.end_time, CustomError::DurationTooShort); 

    let mut fee_amount = 0;
    let mut net_deposit = amount;

    if market.mode != MarketMode::Parimutuel {
        fee_amount = amount
            .checked_mul(global_config.house_fee_bps).unwrap()
            .checked_div(10000).unwrap();
        
        net_deposit = amount.checked_sub(fee_amount).unwrap();
    }

    // --- LOGIC BRANCH: HOUSE VS PARIMUTUEL ---
    let final_multiplier = if market.mode == MarketMode::House {
        let decay_factor = calculate_time_decay_factor(
            market.start_time,
            market.end_time,
            clock.unix_timestamp
        )?; 

        let adjusted_multiplier = declared_multiplier_bps
            .checked_mul(decay_factor).unwrap()
            .checked_div(10_000).unwrap();
        
        let max_potential_profit = calculate_max_potential_profit(net_deposit, adjusted_multiplier)?;
        let max_liability_this_bet = net_deposit + max_potential_profit;
        
        let new_vault_balance = market.vault_balance + net_deposit;
        let new_locked_total = market.locked_for_payouts + max_liability_this_bet;
        
        require!(new_locked_total <= new_vault_balance, CustomError::SolvencyRisk);

        market.locked_for_payouts = new_locked_total;
        adjusted_multiplier
    } else {
        0 
    };

    // --- TRANSFERS ---
    token::transfer(
        CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            Transfer {
                from: ctx.accounts.user_token_account.to_account_info(),
                to: ctx.accounts.market_vault.to_account_info(),
                authority: ctx.accounts.user.to_account_info(),
            },
        ),
        net_deposit,
    )?;

    if fee_amount > 0 {
        token::transfer(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.user_token_account.to_account_info(),
                    to: ctx.accounts.treasury_wallet.to_account_info(),
                    authority: ctx.accounts.user.to_account_info(),
                },
            ),
            fee_amount,
        )?;
    }

    market.vault_balance = market.vault_balance.checked_add(net_deposit).unwrap();

    // 5. Initialize UserBet
    {
        let user_bet = &mut ctx.accounts.user_bet;
        user_bet.owner = ctx.accounts.user.key();
        user_bet.market_identifier = market.name.clone();
        user_bet.market_type = MarketType::Fixed;
        user_bet.deposit = net_deposit; 
        user_bet.end_timestamp = market.end_time;
        
        user_bet.payout_multiplier = final_multiplier;
        user_bet.creation_ts = clock.unix_timestamp; 
        user_bet.update_count = 0;                   
        user_bet.calculated_weight = 0;
        user_bet.is_weight_added = false;
        user_bet.entry_price = 0; 
        user_bet.volatility_factor_at_entry = ctx.accounts.asset_config.volatility_factor;
        user_bet.status = BetStatus::Active;
        
        // --- COMMIT-REVEAL SETUP ---
        user_bet.commitment = commitment;
        user_bet.is_revealed = false;
        user_bet.prediction_low = 0; 
        user_bet.prediction_high = 0;
        user_bet.prediction_target = 0;
        
        user_bet.bump = ctx.bumps.user_bet;
    }

    // --- 3. AUTOMATICALLY SETUP PERMISSIONS ---
    let permission_program = &ctx.accounts.permission_program;
    let group = &ctx.accounts.group;
    let permission = &ctx.accounts.permission;
    let user = &ctx.accounts.user;
    let system_program = &ctx.accounts.system_program;
    
    let user_bet_info = ctx.accounts.user_bet.to_account_info();
    let group_id = user_bet_info.key();

    // Create Group (ID = UserBet Key)
    CreateGroupCpiBuilder::new(permission_program.to_account_info().as_ref())
        .group(group.to_account_info().as_ref())
        .id(group_id)
        .members(vec![])
        .payer(user.to_account_info().as_ref())
        .system_program(system_program.to_account_info().as_ref())
        .invoke()?;

    // Create Permission
    CreatePermissionCpiBuilder::new(permission_program.to_account_info().as_ref())
        .permission(permission.to_account_info().as_ref())
        .group(group.to_account_info().as_ref())
        .delegated_account(user.to_account_info().as_ref()) 
        .payer(user.to_account_info().as_ref())
        .system_program(system_program.to_account_info().as_ref())
        .invoke()?;

    msg!("Permissions initialized automatically for Bet: {}", group_id);

    emit!(BetPlaced {
        bet_address: ctx.accounts.user_bet.key(),
        user: ctx.accounts.user.key(),
        market_identifier: market.name.clone(),
        market_type: MarketType::Fixed,
        amount: net_deposit,
        end_timestamp: market.end_time,
        payout_multiplier_bps: final_multiplier,
    });

    Ok(())
}