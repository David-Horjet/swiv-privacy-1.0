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
pub struct BatchCalculateOutcome<'info> {
    #[account(mut)]
    pub admin: Signer<'info>, 

    #[account(
        mut,
        seeds = [SEED_FIXED_MARKET, fixed_market.name.as_bytes()],
        bump = fixed_market.bump
    )]
    pub fixed_market: Account<'info, FixedMarket>,
}

pub fn batch_calculate_outcome<'info>(
    ctx: Context<'_, '_, '_, 'info, BatchCalculateOutcome<'info>>
) -> Result<()> {
    let market = &mut ctx.accounts.fixed_market;
    let accounts_iter = &mut ctx.remaining_accounts.iter();
    let clock = Clock::get()?;

    require!(market.is_resolved, CustomError::SettlementTooEarly);
    if market.mode == MarketMode::Parimutuel {
        require!(!market.weight_finalized, CustomError::AlreadySettled);
    }

    let batch_wait_duration = 24 * 60 * 60; // 24 Hours in seconds
    
    require!(
        clock.unix_timestamp > market.resolution_ts + batch_wait_duration,
        CustomError::SettlementTooEarly
    );

    let result = market.resolution_target;
    let start_time = market.start_time;
    let end_time = market.end_time;
    let max_accuracy_buffer = market.max_accuracy_buffer;

    loop {
        let user_bet_acc_info = match accounts_iter.next() {
            Some(acc) => acc,
            None => break,
        };

        let mut user_bet_data = user_bet_acc_info.try_borrow_mut_data()?;
        let mut user_bet = UserBet::try_deserialize(&mut &user_bet_data[..])?;

        // --- Skip checks ---
        if user_bet.market_identifier != market.name { continue; }
        if user_bet.status != BetStatus::Active || !user_bet.is_revealed { continue; }

        // --- A. Calculate Base Accuracy ---
        let accuracy_score = calculate_accuracy_score(
            user_bet.prediction_target,
            result,
            max_accuracy_buffer
        )?;

        // --- B. Apply Mode Logic ---
        if market.mode == MarketMode::Parimutuel {
            let time_bonus = calculate_time_bonus(
                start_time,
                end_time,
                user_bet.creation_ts
            )?;
            let conviction_bonus = calculate_conviction_bonus(user_bet.update_count);

            let mut weight = calculate_parimutuel_weight(
                user_bet.deposit,
                accuracy_score,
                time_bonus,
                conviction_bonus
            )?;

            let penalty = weight.checked_div(20).unwrap(); 
            weight = weight.checked_sub(penalty).unwrap();

            // Update Global Weight
            market.total_weight = market.total_weight.checked_add(weight).unwrap();
            
            // Update User
            user_bet.calculated_weight = weight;
            user_bet.is_weight_added = true;
            user_bet.status = BetStatus::Calculated;

            msg!("Force Calc (Pari) for {}: Weight {} (Penalty Applied)", user_bet.owner, weight);

        } else {
            let mut final_score = accuracy_score as u128;

            let penalty = final_score.checked_div(20).unwrap();
            final_score = final_score.checked_sub(penalty).unwrap();

            user_bet.calculated_weight = final_score;
            user_bet.is_weight_added = true; 
            user_bet.status = BetStatus::Calculated;

            msg!("Force Calc (House) for {}: Score {}", user_bet.owner, final_score);
        }

        // --- C. Serialize Data Back ---
        let mut new_data: Vec<u8> = Vec::new();
        user_bet.try_serialize(&mut new_data)?;

        if new_data.len() <= user_bet_data.len() {
            user_bet_data[..new_data.len()].copy_from_slice(&new_data);
        } else {
            return Err(ProgramError::AccountDataTooSmall.into());
        }
    }

    Ok(())
}