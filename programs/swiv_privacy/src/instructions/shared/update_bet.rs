use anchor_lang::prelude::*;
use crate::state::{UserBet, Pool, BetStatus};
use crate::errors::CustomError;
use crate::events::BetUpdated;
use crate::constants::PERMISSION_PROGRAM_ID;

#[derive(Accounts)]
pub struct UpdateBet<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        mut,
        constraint = user_bet.owner == user.key() @ CustomError::Unauthorized,
        constraint = user_bet.status == BetStatus::Active @ CustomError::AlreadySettled
    )]
    pub user_bet: Box<Account<'info, UserBet>>,

    #[account(
        mut,
        constraint = pool.key() == user_bet.pool @ CustomError::MarketMismatch
    )]
    pub pool: Box<Account<'info, Pool>>,

    /// CHECK: Seeds verification for MagicBlock Group
    #[account(
        seeds = [b"group", user_bet.key().as_ref()],
        seeds::program = permission_program.key(),
        bump
    )]
    pub group: UncheckedAccount<'info>,

    /// CHECK: Seeds verification for MagicBlock Permission
    #[account(
        seeds = [b"permission", group.key().as_ref(), user.key().as_ref()],
        seeds::program = permission_program.key(),
        bump
    )]
    pub permission: UncheckedAccount<'info>,

    /// CHECK: Seeds verification
    #[account(address = PERMISSION_PROGRAM_ID)]
    pub permission_program: UncheckedAccount<'info>,
}

pub fn update_bet(
    ctx: Context<UpdateBet>,
    new_prediction_low: u64,    
    new_prediction_high: u64,   
    new_prediction_target: u64, 
) -> Result<()> {
    let user_bet = &mut ctx.accounts.user_bet;
    let clock = Clock::get()?;

    // 1. Verify Permission Data exists (User didn't bypass setup)
    require!(!ctx.accounts.permission.data_is_empty(), CustomError::Unauthorized);

    // Timing Check - ensure pool still open
    require!(clock.unix_timestamp < ctx.accounts.pool.end_time, CustomError::DurationTooShort);

    user_bet.creation_ts = clock.unix_timestamp;
    user_bet.update_count = user_bet.update_count.checked_add(1).unwrap();

    // Update Predictions
    let old_low = user_bet.prediction_low;
    let old_high = user_bet.prediction_high;
    let old_target = user_bet.prediction_target;

    user_bet.prediction_low = new_prediction_low;
    user_bet.prediction_high = new_prediction_high;
    user_bet.prediction_target = new_prediction_target;
    
    msg!("Bet Updated securely via TEE.");

    emit!(BetUpdated {
        bet_address: user_bet.key(),
        user: ctx.accounts.user.key(),
        old_low,
        old_high,
        old_target,
        new_low: new_prediction_low,
        new_high: new_prediction_high,
        new_target: new_prediction_target,
    });

    Ok(())
}