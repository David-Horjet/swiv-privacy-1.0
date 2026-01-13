use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, TokenAccount, Transfer};
use crate::state::{UserBet, LiquidityVault, BetStatus};
use crate::constants::{SEED_VAULT};
use crate::errors::CustomError;
use crate::utils::infinite_math::calculate_max_potential_profit;

const REFUND_TIMEOUT_SECONDS: i64 = 60; // 3 Days equivalent (test value)

#[derive(Accounts)]
pub struct EmergencyRefund<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        mut,
        constraint = user_bet.owner == user.key() @ CustomError::Unauthorized,
        constraint = user_bet.status != BetStatus::Settled @ CustomError::AlreadySettled
    )]
    pub user_bet: Box<Account<'info, UserBet>>,

    #[account(
        mut,
        seeds = [SEED_VAULT, user_bet.market_identifier.as_bytes()],
        bump = vault.bump
    )]
    pub vault: Box<Account<'info, LiquidityVault>>,

    #[account(
        mut,
        seeds = [b"pool_vault", vault.key().as_ref()],
        bump,
        token::mint = vault.token_mint,
        token::authority = vault,
    )]
    pub vault_token_account: Box<Account<'info, TokenAccount>>,

    #[account(
        mut,
        token::mint = vault.token_mint
    )]
    pub user_token_account: Box<Account<'info, TokenAccount>>,

    pub token_program: Program<'info, Token>,
}

pub fn emergency_refund(ctx: Context<EmergencyRefund>) -> Result<()> {
    let user_bet = &mut ctx.accounts.user_bet;
    let vault = &mut ctx.accounts.vault;
    let clock = Clock::get()?;

    require!(
        clock.unix_timestamp > user_bet.end_timestamp + REFUND_TIMEOUT_SECONDS,
        CustomError::TimeoutNotMet
    );

    // ACTION 1: Decrement vault exposure since the risk is now gone
    let max_potential_profit = calculate_max_potential_profit(
        user_bet.deposit,
        user_bet.payout_multiplier,
    )?;
    vault.total_exposure = vault.total_exposure.saturating_sub(max_potential_profit);

    // ACTION 2: Refund Amount
    let refund_amount = user_bet.deposit;

    // ACTION 3: Transfer the net refund from the vault back to the user
    let seeds = &[
        SEED_VAULT,
        vault.asset_symbol.as_bytes(),
        &[vault.bump],
    ];
    let signer = &[&seeds[..]];

    if refund_amount > 0 {
        token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.vault_token_account.to_account_info(),
                    to: ctx.accounts.user_token_account.to_account_info(),
                    authority: vault.to_account_info(),
                },
                signer,
            ),
            refund_amount,
        )?;
        
        vault.total_assets = vault.total_assets.saturating_sub(refund_amount);
    }

    user_bet.status = BetStatus::Settled;

    Ok(())
}