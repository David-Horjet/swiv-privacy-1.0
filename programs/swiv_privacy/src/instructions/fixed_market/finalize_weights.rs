use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, TokenAccount, Transfer};
use crate::state::{FixedMarket, GlobalConfig, MarketMode};
use crate::constants::{SEED_GLOBAL_CONFIG, SEED_FIXED_MARKET};
use crate::errors::CustomError;

#[derive(Accounts)]
pub struct FinalizeWeights<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        seeds = [SEED_GLOBAL_CONFIG],
        bump,
        constraint = global_config.admin == admin.key() @ CustomError::Unauthorized
    )]
    pub global_config: Account<'info, GlobalConfig>,

    #[account(
        mut,
        seeds = [SEED_FIXED_MARKET, fixed_market.name.as_bytes()],
        bump = fixed_market.bump,
        constraint = fixed_market.mode == MarketMode::Parimutuel @ CustomError::InvalidAsset
    )]
    pub fixed_market: Account<'info, FixedMarket>,

    #[account(
        mut,
        seeds = [b"fixed_vault", fixed_market.key().as_ref()],
        bump,
        token::authority = fixed_market,
    )]
    pub market_vault: Account<'info, TokenAccount>,

    /// CHECK: Validated against GlobalConfig
    #[account(
        mut, 
        token::authority = global_config.treasury_wallet
    )]
    pub treasury_wallet: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
}

pub fn finalize_weights(ctx: Context<FinalizeWeights>) -> Result<()> {
    let market = &mut ctx.accounts.fixed_market;
    let global_config = &ctx.accounts.global_config;
    
    require!(market.is_resolved, CustomError::SettlementTooEarly);
    require!(!market.weight_finalized, CustomError::AlreadySettled);

    let total_pot = market.vault_balance;
    let fee_amount = total_pot
        .checked_mul(global_config.parimutuel_fee_bps).unwrap()
        .checked_div(10000).unwrap();

    if fee_amount > 0 {
        let name_bytes = market.name.as_bytes();
        let bump = market.bump;
        let seeds = &[SEED_FIXED_MARKET, name_bytes, &[bump]];
        let signer = &[&seeds[..]];

        token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.market_vault.to_account_info(),
                    to: ctx.accounts.treasury_wallet.to_account_info(),
                    authority: market.to_account_info(),
                },
                signer,
            ),
            fee_amount,
        )?;

        market.vault_balance = market.vault_balance.checked_sub(fee_amount).unwrap();
        msg!("Parimutuel Fee Deducted: {}", fee_amount);
    }

    market.locked_for_payouts = market.vault_balance;

    market.weight_finalized = true;
    
    msg!("Parimutuel Weights Finalized. Total Weight: {}", market.total_weight);

    Ok(())
}