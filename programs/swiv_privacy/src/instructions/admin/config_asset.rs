use anchor_lang::prelude::*;
use crate::state::{GlobalConfig, AssetConfig};
use crate::constants::{SEED_GLOBAL_CONFIG, SEED_ASSET_CONFIG, MERCY_BUFFER_DEFAULT};
use crate::errors::CustomError;
use crate::events::AssetConfigUpdated;

#[derive(Accounts)]
#[instruction(symbol: String)]
pub struct ConfigAsset<'info> {
    #[account(
        mut,
        seeds = [SEED_GLOBAL_CONFIG],
        bump,
        constraint = global_config.admin == admin.key() @ CustomError::Unauthorized
    )]
    pub global_config: Account<'info, GlobalConfig>,

    #[account(
        init_if_needed,
        payer = admin,
        space = AssetConfig::LEN,
        seeds = [SEED_ASSET_CONFIG, symbol.as_bytes()],
        bump
    )]
    pub asset_config: Account<'info, AssetConfig>,

    #[account(mut)]
    pub admin: Signer<'info>,

    /// CHECK: We trust the admin to provide the correct Pyth Feed ID
    pub pyth_feed: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

pub fn config_asset(
    ctx: Context<ConfigAsset>,
    symbol: String,
    _pyth_feed: Pubkey, // Passed in struct, used in body
    volatility: u64,
    use_pyth_volatility: bool,
) -> Result<()> {
    let asset_config = &mut ctx.accounts.asset_config;
    
    asset_config.symbol = symbol.clone();
    asset_config.pyth_feed = ctx.accounts.pyth_feed.key();
    asset_config.volatility_factor = volatility;
    asset_config.use_pyth_volatility = use_pyth_volatility;
    
    // Set default mercy buffer if not already set (or we could make this an argument)
    if asset_config.mercy_buffer_bps == 0 {
        asset_config.mercy_buffer_bps = MERCY_BUFFER_DEFAULT;
    }
    
    asset_config.bump = ctx.bumps.asset_config;

    emit!(AssetConfigUpdated {
        symbol,
        pyth_feed: ctx.accounts.pyth_feed.key(),
        volatility_factor: volatility,
    });

    Ok(())
}