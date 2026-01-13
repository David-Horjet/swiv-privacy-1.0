use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, TokenAccount, Transfer};
use pyth_solana_receiver_sdk::price_update::PriceUpdateV2;
use crate::state::{GlobalConfig, AssetConfig, LiquidityVault, UserBet, MarketType, BetStatus};
use crate::constants::{SEED_GLOBAL_CONFIG, SEED_ASSET_CONFIG, SEED_VAULT, SEED_BET};
use crate::errors::CustomError;
use crate::utils::infinite_math::{calculate_max_potential_profit};
use crate::events::BetPlaced;
use crate::utils::pyth::get_pyth_price_and_conf;
use ephemeral_rollups_sdk::access_control::{
    CreateGroupCpiBuilder, CreatePermissionCpiBuilder
};

#[derive(Accounts)]
#[instruction(
    amount: u64,
    duration: i64,
    declared_multiplier_bps: u64, 
    commitment: [u8; 32], 
    request_id: String
)]
pub struct PlaceBet<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        seeds = [SEED_GLOBAL_CONFIG],
        bump,
        constraint = !global_config.paused @ CustomError::Paused
    )]
    pub global_config: Box<Account<'info, GlobalConfig>>,

    #[account(
        seeds = [SEED_ASSET_CONFIG, asset_config.symbol.as_bytes()],
        bump = asset_config.bump
    )]
    pub asset_config: Box<Account<'info, AssetConfig>>,

    #[account(
        mut,
        seeds = [SEED_VAULT, asset_config.symbol.as_bytes()],
        bump = vault.bump
    )]
    pub vault: Box<Account<'info, LiquidityVault>>,

    #[account(
        mut,
        seeds = [b"pool_vault", vault.key().as_ref()],
        bump,
        token::mint = token_mint,
        token::authority = vault,
    )]
    pub vault_token_account: Account<'info, TokenAccount>,

    #[account(address = vault.token_mint)]
    pub token_mint: Account<'info, token::Mint>,

    #[account(mut)]
    pub user_token_account: Account<'info, TokenAccount>,

    /// CHECK: Validated against GlobalConfig
    #[account(mut, address = global_config.treasury_wallet)]
    pub treasury_wallet: Account<'info, TokenAccount>,

    #[account(
        constraint = referrer.key() != user.key() @ CustomError::Unauthorized
    )]
    pub referrer: Option<UncheckedAccount<'info>>,

    #[account(
        init,
        payer = user,
        space = UserBet::SPACE,
        seeds = [SEED_BET, vault.key().as_ref(), user.key().as_ref(), request_id.as_bytes()], 
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
    
    pub price_update: Account<'info, PriceUpdateV2>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

pub fn place_bet(
    ctx: Context<PlaceBet>,
    amount: u64,
    duration: i64,
    declared_multiplier_bps: u64, 
    commitment: [u8; 32],  
    _request_id: String, 
) -> Result<()> {
    let vault = &mut ctx.accounts.vault;
    let asset_config = &ctx.accounts.asset_config;
    let global_config = &ctx.accounts.global_config;
    let clock = Clock::get()?;

    // 1. Snapshot Entry Price
    let price_info = get_pyth_price_and_conf(&ctx.accounts.price_update, &asset_config.pyth_feed)?;

    // 2. Fees calculation
    let fee_amount = amount
        .checked_mul(global_config.house_fee_bps).unwrap()
        .checked_div(10000).unwrap();
    
    let net_deposit = amount.checked_sub(fee_amount).unwrap();

    // 3. Solvency Check
    let potential_profit = calculate_max_potential_profit(net_deposit, declared_multiplier_bps)?;
    
    let new_exposure = vault.total_exposure.checked_add(potential_profit).ok_or(CustomError::MathOverflow)?;
    require!(new_exposure <= vault.total_assets, CustomError::SolvencyRisk);

    vault.total_exposure = new_exposure;
    
    // 4. Transfers
    token::transfer(
        CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            Transfer {
                from: ctx.accounts.user_token_account.to_account_info(),
                to: ctx.accounts.vault_token_account.to_account_info(),
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

    vault.total_assets = vault.total_assets.checked_add(net_deposit).unwrap();
    
    // 5. Initialize UserBet Account
    let bet_end_timestamp = clock.unix_timestamp + duration;

    {
        let user_bet = &mut ctx.accounts.user_bet;
        user_bet.owner = ctx.accounts.user.key();
        user_bet.market_identifier = asset_config.symbol.clone();
        user_bet.market_type = MarketType::Infinite;
        user_bet.deposit = net_deposit;
        user_bet.end_timestamp = bet_end_timestamp;
        
        user_bet.payout_multiplier = declared_multiplier_bps; 
        user_bet.entry_price = price_info.price;
        user_bet.volatility_factor_at_entry = asset_config.volatility_factor; 

        user_bet.creation_ts = clock.unix_timestamp; 
        user_bet.update_count = 0;
        user_bet.calculated_weight = 0;
        user_bet.is_weight_added = false;

        user_bet.status = BetStatus::Active;
        
        // --- COMMIT-REVEAL SETUP ---
        user_bet.commitment = commitment;
        user_bet.is_revealed = false;
        user_bet.prediction_low = 0;     
        user_bet.prediction_high = 0;    
        user_bet.prediction_target = 0;  

        if let Some(referrer_account) = &ctx.accounts.referrer {
            user_bet.referrer = Some(referrer_account.key());
        } else {
            user_bet.referrer = None;
        }
        
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

    // Create Group
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
        market_identifier: asset_config.symbol.clone(),
        market_type: MarketType::Infinite,
        amount: net_deposit,
        end_timestamp: bet_end_timestamp,
        payout_multiplier_bps: declared_multiplier_bps,
    });

    Ok(())
}