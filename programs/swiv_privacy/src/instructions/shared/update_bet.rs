use anchor_lang::prelude::*;
use crate::state::{UserBet, LiquidityVault, FixedMarket, MarketType, BetStatus, MarketMode};
use crate::constants::{SEED_VAULT, SEED_FIXED_MARKET};
use crate::errors::CustomError;
use crate::utils::infinite_math::{calculate_time_decay_factor, calculate_max_potential_profit};
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
        seeds = [SEED_VAULT, user_bet.market_identifier.as_bytes()],
        bump = vault.bump,
        constraint = user_bet.market_type == MarketType::Infinite
    )]
    pub vault: Option<Box<Account<'info, LiquidityVault>>>,

    #[account(
        mut,
        seeds = [SEED_FIXED_MARKET, user_bet.market_identifier.as_bytes()],
        bump = fixed_market.bump,
        constraint = user_bet.market_type == MarketType::Fixed
    )]
    pub fixed_market: Option<Box<Account<'info, FixedMarket>>>,

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

    // ====================================================
    // 2. FIXED MARKET LOGIC
    // ====================================================
    if let Some(market) = &mut ctx.accounts.fixed_market {
        require!(clock.unix_timestamp < market.end_time, CustomError::DurationTooShort);

        user_bet.creation_ts = clock.unix_timestamp;
        user_bet.update_count = user_bet.update_count.checked_add(1).unwrap();

        if market.mode == MarketMode::House {
            // Recalculate Multiplier based on Time Decay
            let old_max_profit = calculate_max_potential_profit(user_bet.deposit, user_bet.payout_multiplier)?;

            let decay_factor = calculate_time_decay_factor(
                market.start_time,
                market.end_time,
                clock.unix_timestamp
            )?;

            let mut new_multiplier = user_bet.payout_multiplier
                .checked_mul(decay_factor).unwrap()
                .checked_div(10_000).unwrap();

            // Apply Conviction Penalty
            let penalty_factor = 10_000u64.saturating_sub(market.conviction_bonus_bps);
            new_multiplier = new_multiplier
                .checked_mul(penalty_factor).unwrap()
                .checked_div(10_000).unwrap();

            user_bet.payout_multiplier = new_multiplier;
            
            // Adjust locked exposure in vault
            let new_max_profit = calculate_max_potential_profit(user_bet.deposit, new_multiplier)?;
            let exposure_reduction = old_max_profit.saturating_sub(new_max_profit);
            market.locked_for_payouts = market.locked_for_payouts.saturating_sub(exposure_reduction);
        }
    }
    // ====================================================
    // 3. INFINITE MARKET LOGIC
    // ====================================================
    else if let Some(vault) = &mut ctx.accounts.vault {
        // Simple halving logic for Infinite Market updates
        let old_max_profit = calculate_max_potential_profit(user_bet.deposit, user_bet.payout_multiplier)?;

        let new_multiplier = user_bet.payout_multiplier / 2;
        user_bet.payout_multiplier = new_multiplier;

        let new_max_profit = calculate_max_potential_profit(user_bet.deposit, new_multiplier)?;
        let exposure_reduction = old_max_profit.saturating_sub(new_max_profit);
        vault.total_exposure = vault.total_exposure.saturating_sub(exposure_reduction);

        user_bet.creation_ts = clock.unix_timestamp;
        user_bet.update_count += 1;
    }

    // ====================================================
    // 4. UPDATE PREDICTIONS (Securely inside TEE)
    // ====================================================
    user_bet.prediction_low = new_prediction_low;
    user_bet.prediction_high = new_prediction_high;
    user_bet.prediction_target = new_prediction_target;
    
    msg!("Bet Updated securely via TEE.");

    emit!(BetUpdated {
        bet_address: user_bet.key(),
        user: ctx.accounts.user.key(),
        old_multiplier_bps: 0, 
        new_multiplier_bps: user_bet.payout_multiplier,
    });

    Ok(())
}