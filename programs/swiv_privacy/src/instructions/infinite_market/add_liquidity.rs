use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, TokenAccount, Transfer, MintTo};
use crate::state::{LiquidityVault, GlobalConfig};
use crate::constants::{SEED_VAULT, SEED_GLOBAL_CONFIG};
use crate::errors::CustomError;
use crate::utils::infinite_math::calculate_shares_to_mint;
use crate::events::LiquidityAdded;

#[derive(Accounts)]
pub struct AddLiquidity<'info> {
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
    pub system_program: Program<'info, System>,
}

pub fn add_liquidity(ctx: Context<AddLiquidity>, amount_usdt: u64) -> Result<()> {
    let vault = &mut ctx.accounts.vault;

    let shares_to_mint = calculate_shares_to_mint(
        amount_usdt,
        vault.total_assets,
        vault.total_shares,
    )?;

    token::transfer(
        CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            Transfer {
                from: ctx.accounts.user_token_account.to_account_info(),
                to: ctx.accounts.vault_token_account.to_account_info(),
                authority: ctx.accounts.user.to_account_info(),
            },
        ),
        amount_usdt,
    )?;

    let seeds = &[
        SEED_VAULT,
        vault.asset_symbol.as_bytes(),
        &[vault.bump],
    ];
    let signer = &[&seeds[..]];

    token::mint_to(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            MintTo {
                mint: ctx.accounts.lp_mint.to_account_info(),
                to: ctx.accounts.user_lp_account.to_account_info(),
                authority: vault.to_account_info(),
            },
            signer,
        ),
        shares_to_mint,
    )?;

    vault.total_assets = vault.total_assets.checked_add(amount_usdt).unwrap();
    vault.total_shares = vault.total_shares.checked_add(shares_to_mint).unwrap();

    emit!(LiquidityAdded {
        user: ctx.accounts.user.key(),
        asset_symbol: vault.asset_symbol.clone(),
        amount_deposited: amount_usdt,
        shares_minted: shares_to_mint,
    });

    Ok(())
}