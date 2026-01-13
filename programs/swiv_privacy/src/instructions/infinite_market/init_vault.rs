use anchor_lang::prelude::*;
use anchor_spl::token::{Token, Mint, TokenAccount}; 
use crate::state::{LiquidityVault, AssetConfig, GlobalConfig};
use crate::constants::{SEED_VAULT, SEED_ASSET_CONFIG, SEED_GLOBAL_CONFIG};
use crate::errors::CustomError;

#[derive(Accounts)]
#[instruction(symbol: String)]
pub struct InitVault<'info> {
    #[account(
        init,
        payer = admin,
        space = LiquidityVault::LEN,
        seeds = [SEED_VAULT, symbol.as_bytes()],
        bump
    )]
    pub vault: Account<'info, LiquidityVault>,

    #[account(
        init,
        payer = admin,
        seeds = [b"pool_vault", vault.key().as_ref()],
        bump,
        token::mint = token_mint,
        token::authority = vault,
    )]
    pub vault_token_account: Account<'info, TokenAccount>,

    #[account(
        seeds = [SEED_ASSET_CONFIG, symbol.as_bytes()],
        bump = asset_config.bump
    )]
    pub asset_config: Account<'info, AssetConfig>,
    
    // NEW: Needed for Whitelist Check
    #[account(
        seeds = [SEED_GLOBAL_CONFIG],
        bump,
    )]
    pub global_config: Account<'info, GlobalConfig>,

    pub token_mint: Account<'info, Mint>,

    #[account(
        init,
        payer = admin,
        seeds = [b"lp_mint", vault.key().as_ref()],
        bump,
        mint::decimals = 6,
        mint::authority = vault,
    )]
    pub lp_mint: Account<'info, Mint>,

    #[account(mut)]
    pub admin: Signer<'info>,

    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    pub rent: Sysvar<'info, Rent>,
}

pub fn init_vault(ctx: Context<InitVault>, symbol: String) -> Result<()> {
    // --- WHITELIST CHECK ---
    let global_config = &ctx.accounts.global_config;
    let mint_key = ctx.accounts.token_mint.key();
    
    let is_whitelisted = global_config.allowed_assets.iter().any(|&asset| asset == mint_key);
    require!(is_whitelisted, CustomError::InvalidAsset);

    let vault = &mut ctx.accounts.vault;
    vault.token_mint = ctx.accounts.token_mint.key();
    vault.asset_symbol = symbol;
    vault.total_assets = 0;
    vault.total_shares = 0;
    vault.accumulated_fees = 0;
    vault.total_exposure = 0;
    vault.bump = ctx.bumps.vault;

    Ok(())
}