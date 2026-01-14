use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, TokenAccount, Transfer};
use crate::state::{BetStatus, GlobalConfig, Pool, UserBet};
use crate::constants::{SEED_GLOBAL_CONFIG, SEED_POOL, SEED_BET};
use crate::errors::CustomError;
use crate::events::BetPlaced;
use ephemeral_rollups_sdk::access_control::{
    CreateGroupCpiBuilder, CreatePermissionCpiBuilder
};

#[derive(Accounts)]
#[instruction(
    amount: u64,
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
        mut,
        seeds = [SEED_POOL, pool.name.as_bytes()],
        bump = pool.bump
    )]
    pub pool: Box<Account<'info, Pool>>,

    #[account(
        mut,
        seeds = [b"pool_vault", pool.key().as_ref()],
        bump,
        token::authority = pool,
    )]
    pub pool_vault: Account<'info, TokenAccount>,

    #[account(mut)]
    pub user_token_account: Box<Account<'info, TokenAccount>>,

    #[account(
        mut, 
        token::authority = global_config.treasury_wallet
    )]
    pub treasury_wallet: Box<Account<'info, TokenAccount>>,

    #[account(
        init,
        payer = user,
        space = UserBet::SPACE,
        seeds = [SEED_BET, pool.key().as_ref(), user.key().as_ref(), request_id.as_bytes()], 
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

pub fn place_bet(
    ctx: Context<PlaceBet>,
    amount: u64,
    commitment: [u8; 32], 
    _request_id: String, 
) -> Result<()> {
    let pool = &mut ctx.accounts.pool;
    let global_config = &ctx.accounts.global_config;
    let clock = Clock::get()?;

    require!(clock.unix_timestamp >= pool.start_time, CustomError::DurationTooShort);
    require!(clock.unix_timestamp < pool.end_time, CustomError::DurationTooShort); 

    let fee_amount = amount.checked_mul(global_config.protocol_fee_bps).unwrap().checked_div(10000).unwrap();
    let net_deposit = amount.checked_sub(fee_amount).unwrap();

    // Transfer net_deposit into pool_vault
    token::transfer(
        CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            Transfer {
                from: ctx.accounts.user_token_account.to_account_info(),
                to: ctx.accounts.pool_vault.to_account_info(),
                authority: ctx.accounts.user.to_account_info(),
            },
        ),
        net_deposit,
    )?;

    // Transfer fee to treasury
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

    pool.vault_balance = pool.vault_balance.checked_add(net_deposit).unwrap();

    // 5. Initialize UserBet
    {
        let user_bet = &mut ctx.accounts.user_bet;
        user_bet.owner = ctx.accounts.user.key();
        user_bet.pool = pool.key();
        user_bet.deposit = net_deposit; 
        user_bet.end_timestamp = pool.end_time;
        
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
        pool: pool.key(),
        amount: net_deposit,
        end_timestamp: pool.end_time,
    });

    Ok(())
}