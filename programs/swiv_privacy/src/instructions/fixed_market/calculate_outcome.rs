use anchor_lang::prelude::*;
use crate::state::{FixedMarket, UserBet, BetStatus, MarketMode};
use crate::constants::{SEED_FIXED_MARKET};
use crate::errors::CustomError;
use crate::utils::fixed_math::{
    calculate_accuracy_score, 
    calculate_time_bonus, 
    calculate_conviction_bonus, 
    calculate_parimutuel_weight,
};

#[derive(Accounts)]
pub struct CalculateFixedOutcome<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    /// CHECK: We verify this matches user_bet.owner below
    pub bet_owner: UncheckedAccount<'info>,

    #[account(
        mut,
        seeds = [SEED_FIXED_MARKET, fixed_market.name.as_bytes()],
        bump = fixed_market.bump
    )]
    pub fixed_market: Account<'info, FixedMarket>,

    #[account(
        mut,
        constraint = user_bet.owner == bet_owner.key() @ CustomError::Unauthorized,
        constraint = user_bet.status == BetStatus::Active @ CustomError::AlreadySettled,
        constraint = user_bet.is_revealed @ CustomError::BetNotRevealed
    )]
    pub user_bet: Account<'info, UserBet>,
}

pub fn calculate_fixed_outcome(ctx: Context<CalculateFixedOutcome>) -> Result<()> {
    let market = &mut ctx.accounts.fixed_market;
    let bet = &mut ctx.accounts.user_bet;

    require!(market.is_resolved, CustomError::SettlementTooEarly);
    require!(!bet.is_weight_added, CustomError::AlreadySettled);

    let user_prediction = bet.prediction_target;
    let result = market.resolution_target;

    let accuracy_score = calculate_accuracy_score(
        user_prediction,
        result,
        market.max_accuracy_buffer
    )?;

    if market.mode == MarketMode::Parimutuel {
        let time_bonus = calculate_time_bonus(
            market.start_time,
            market.end_time,
            bet.creation_ts
        )?;
        let conviction_bonus = calculate_conviction_bonus(bet.update_count);

        let weight = calculate_parimutuel_weight(
            bet.deposit,
            accuracy_score,
            time_bonus,
            conviction_bonus
        )?;

        market.total_weight = market.total_weight.checked_add(weight).unwrap();
        
        bet.calculated_weight = weight;
        bet.is_weight_added = true;
        bet.status = BetStatus::Calculated;

        msg!("Calculated Parimutuel for User: {}", ctx.accounts.bet_owner.key());

    } else {
        bet.calculated_weight = accuracy_score as u128;
        bet.is_weight_added = true; 
        bet.status = BetStatus::Calculated;
        
        msg!("Calculated House for User: {}", ctx.accounts.bet_owner.key());
    }

    Ok(())
}