use anchor_lang::prelude::*;
use crate::state::GlobalConfig;
use crate::constants::{SEED_GLOBAL_CONFIG};
use crate::events::ProtocolInitialized;

#[derive(Accounts)]
#[instruction(
    house_fee_bps: u64,
    parimutuel_fee_bps: u64,
    allowed_assets: Vec<Pubkey>
)]
pub struct InitializeProtocol<'info> {
    #[account(
        init,
        payer = admin,
        space = GlobalConfig::BASE_LEN + (32 * allowed_assets.len()),
        seeds = [SEED_GLOBAL_CONFIG],
        bump
    )]
    pub global_config: Account<'info, GlobalConfig>,

    #[account(mut)]
    pub admin: Signer<'info>,

    /// CHECK: This is the wallet that receives fees. Safe to be any address.
    pub treasury_wallet: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

pub fn initialize_protocol(
    ctx: Context<InitializeProtocol>,
    house_fee_bps: u64,
    parimutuel_fee_bps: u64,
    allowed_assets: Vec<Pubkey> 
) -> Result<()> {
    let global_config = &mut ctx.accounts.global_config;
    
    global_config.admin = ctx.accounts.admin.key();
    global_config.treasury_wallet = ctx.accounts.treasury_wallet.key();
    
    // Dynamic Fees
    global_config.house_fee_bps = house_fee_bps;
    global_config.parimutuel_fee_bps = parimutuel_fee_bps;
    
    // The Whitelist
    global_config.allowed_assets = allowed_assets;

    global_config.paused = false;
    global_config.total_users = 0;
    global_config.batch_settle_wait_duration = 60; 

    emit!(ProtocolInitialized {
        admin: ctx.accounts.admin.key(),
        fee_wallet: ctx.accounts.treasury_wallet.key(),
    });

    Ok(())
}