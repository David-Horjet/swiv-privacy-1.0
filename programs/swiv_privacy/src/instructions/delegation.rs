use anchor_lang::prelude::*;
use crate::state::{UserBet, FixedMarket};
use crate::constants::{SEED_BET, SEED_FIXED_MARKET};
use crate::errors::CustomError;
use ephemeral_rollups_sdk::anchor::{delegate, commit};
use ephemeral_rollups_sdk::cpi::DelegateConfig;
use ephemeral_rollups_sdk::ephem::commit_and_undelegate_accounts;

// ------------------------------------------------------------------
// DELEGATE INSTRUCTION
// ------------------------------------------------------------------

#[delegate]
#[derive(Accounts)]
#[instruction(request_id: String)]
pub struct DelegateBet<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        seeds = [SEED_FIXED_MARKET, user_bet.market_identifier.as_bytes()],
        bump
    )]
    pub fixed_market: Account<'info, FixedMarket>,

    #[account(
        mut,
        del,
        seeds = [
            SEED_BET, 
            fixed_market.key().as_ref(), 
            user.key().as_ref(), 
            request_id.as_bytes()
        ],
        bump = user_bet.bump,
        constraint = user_bet.owner == user.key() @ CustomError::Unauthorized,
    )]
    pub user_bet: Account<'info, UserBet>,
    
}

pub fn delegate_bet(ctx: Context<DelegateBet>, request_id: String) -> Result<()> {
    // 1. Prepare Seeds for Signing (Since UserBet is a PDA)
    let market_key = ctx.accounts.fixed_market.key();
    let user_key = ctx.accounts.user.key();
    let bump = ctx.accounts.user_bet.bump;
    
    let seeds = &[
        SEED_BET,
        market_key.as_ref(),
        user_key.as_ref(),
        request_id.as_bytes(),
        &[bump],
    ];

    let config = DelegateConfig::default();
    
    // Let's follow the Logic:
    ctx.accounts.delegate_user_bet(
        &ctx.accounts.user, // Payer
        seeds,              // PDA Seeds for signing
        config,             // Config
    )?;

    msg!("Bet Delegated successfully");
    Ok(())
}

// ------------------------------------------------------------------
// UNDELEGATE INSTRUCTION
// ------------------------------------------------------------------

/// Add the #[commit] macro for undelegation/commit.
#[commit]
#[derive(Accounts)]
#[instruction(request_id: String)]
pub struct UndelegateBet<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        seeds = [SEED_FIXED_MARKET, user_bet.market_identifier.as_bytes()],
        bump
    )]
    pub fixed_market: Account<'info, FixedMarket>,

    #[account(
        mut,
        seeds = [
            SEED_BET, 
            fixed_market.key().as_ref(), 
            user.key().as_ref(), 
            request_id.as_bytes()
        ],
        bump = user_bet.bump,
        constraint = user_bet.owner == user.key() @ CustomError::Unauthorized,
    )]
    pub user_bet: Account<'info, UserBet>,
}

pub fn undelegate_bet(ctx: Context<UndelegateBet>, _request_id: String) -> Result<()> {
    
    commit_and_undelegate_accounts(
        &ctx.accounts.user,
        vec![&ctx.accounts.user_bet.to_account_info()],
        &ctx.accounts.magic_context,
        &ctx.accounts.magic_program,
    )?;

    msg!("Bet Undelegated (Committed)");
    Ok(())
}