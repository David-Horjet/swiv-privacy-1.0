use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, TokenAccount, Transfer, Burn};
use crate::state::{LiquidityVault, GlobalConfig};
use crate::constants::{SEED_VAULT, SEED_GLOBAL_CONFIG};
use crate::errors::CustomError;
use crate::utils::infinite_math::calculate_assets_to_withdraw;
use crate::events::LiquidityRemoved;

#[derive(Accounts)]
pub struct RemoveLiquidity<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        seeds = [SEED_GLOBAL_CONFIG],
        bump,
        constraint = !global_config.paused @ CustomError::Paused
    )]
    pub global_config: Account<'info, GlobalConfig>,

    #[account(
        mut,
        seeds = [SEED_VAULT, vault.asset_symbol.as_bytes()],
        bump = vault.bump
    )]
    pub vault: Account<'info, LiquidityVault>,

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

    // FIX: Changed token::authority to mint::authority for Mint account
    #[account(
        mut,
        seeds = [b"lp_mint", vault.key().as_ref()],
        bump,
        mint::authority = vault,
    )]
    pub lp_mint: Account<'info, token::Mint>,

    #[account(mut)]
    pub user_token_account: Account<'info, TokenAccount>,

    #[account(mut)]
    pub user_lp_account: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
}

pub fn remove_liquidity(ctx: Context<RemoveLiquidity>, shares_to_burn: u64) -> Result<()> {
    let vault = &mut ctx.accounts.vault;

    require!(shares_to_burn > 0, CustomError::MathOverflow);
    require!(shares_to_burn <= vault.total_shares, CustomError::InsufficientLiquidity);

    let assets_out = calculate_assets_to_withdraw(
        shares_to_burn,
        vault.total_assets,
        vault.total_shares,
    )?;

    token::burn(
        CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            Burn {
                mint: ctx.accounts.lp_mint.to_account_info(),
                from: ctx.accounts.user_lp_account.to_account_info(),
                authority: ctx.accounts.user.to_account_info(),
            },
        ),
        shares_to_burn,
    )?;

    let seeds = &[
        SEED_VAULT,
        vault.asset_symbol.as_bytes(),
        &[vault.bump],
    ];
    let signer = &[&seeds[..]];

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
        assets_out,
    )?;

    vault.total_assets = vault.total_assets.checked_sub(assets_out).unwrap();
    vault.total_shares = vault.total_shares.checked_sub(shares_to_burn).unwrap();

    emit!(LiquidityRemoved {
        user: ctx.accounts.user.key(),
        asset_symbol: vault.asset_symbol.clone(),
        shares_burned: shares_to_burn,
        amount_withdrawn: assets_out,
    });

    Ok(())
}