use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, TokenAccount, Transfer};
use crate::state::{FixedMarket, UserBet, BetStatus, MarketMode};
use crate::constants::{SEED_FIXED_MARKET};
use crate::errors::CustomError;
use crate::utils::fixed_math::MATH_PRECISION;

#[derive(Accounts)]
pub struct ClaimFixedReward<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        mut,
        seeds = [SEED_FIXED_MARKET, fixed_market.name.as_bytes()],
        bump = fixed_market.bump
    )]
    pub fixed_market: Box<Account<'info, FixedMarket>>,

    #[account(
        mut,
        seeds = [b"fixed_vault", fixed_market.key().as_ref()],
        bump,
        token::authority = fixed_market,
    )]
    pub market_vault: Account<'info, TokenAccount>,

    #[account(
        mut,
        constraint = user_bet.owner == user.key() @ CustomError::Unauthorized,
        constraint = user_bet.status == BetStatus::Calculated @ CustomError::SettlementTooEarly
    )]
    pub user_bet: Box<Account<'info, UserBet>>,

    #[account(mut)]
    pub user_token_account: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
}

pub fn claim_fixed_reward(ctx: Context<ClaimFixedReward>) -> Result<()> {
    let market = &mut ctx.accounts.fixed_market;
    let bet = &mut ctx.accounts.user_bet;
    let mut payout_amount: u64 = 0;

    if market.mode == MarketMode::Parimutuel {
        require!(market.weight_finalized, CustomError::SettlementTooEarly);
        
        if bet.calculated_weight > 0 && market.total_weight > 0 {
            let total_distributable_pot = market.locked_for_payouts as u128;
            
            payout_amount = bet.calculated_weight
                .checked_mul(total_distributable_pot).unwrap()
                .checked_div(market.total_weight).unwrap() as u64;
        }

    } else {
        // --- HOUSE MODE LOGIC (Unchanged) ---
        let accuracy_score = bet.calculated_weight as u64; 
        
        if accuracy_score > 0 {
            let raw_profit = (bet.deposit as u128)
                .checked_mul(bet.payout_multiplier as u128).unwrap()
                .checked_div(10_000).unwrap();

            // Scale by Accuracy
            let adjusted_profit = raw_profit
                .checked_mul(accuracy_score as u128).unwrap()
                .checked_div(MATH_PRECISION).unwrap();
            
            // Total Payout = Deposit + Adjusted Profit
            payout_amount = bet.deposit
                .checked_add(adjusted_profit as u64).unwrap();
            
            // Reduce locked exposure
            market.locked_for_payouts = market.locked_for_payouts.saturating_sub(payout_amount);
        } else {
             // User lost. We release the exposure that was locked for them.
             let reserved = (bet.deposit as u128)
                .checked_mul(bet.payout_multiplier as u128).unwrap()
                .checked_div(10_000).unwrap() as u64;
             let total_reserved = bet.deposit + reserved;
             
             market.locked_for_payouts = market.locked_for_payouts.saturating_sub(total_reserved);
        }
    }

    // Perform Transfer if Payout > 0
    if payout_amount > 0 {
        require!(payout_amount <= market.vault_balance, CustomError::InsufficientLiquidity);

        let name_bytes = market.name.as_bytes();
        let bump = market.bump;
        let seeds = &[SEED_FIXED_MARKET, name_bytes, &[bump]];
        let signer = &[&seeds[..]];

        token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.market_vault.to_account_info(),
                    to: ctx.accounts.user_token_account.to_account_info(),
                    authority: market.to_account_info(),
                },
                signer,
            ),
            payout_amount,
        )?;

        // Decrease the ACTUAL vault balance (Tokens left in wallet)
        market.vault_balance = market.vault_balance.checked_sub(payout_amount).unwrap();
    }

    bet.status = BetStatus::Settled;
    
    Ok(())
}