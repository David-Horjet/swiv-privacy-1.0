use anchor_lang::prelude::*;
use crate::state::GlobalConfig;
use crate::constants::{SEED_GLOBAL_CONFIG};
use crate::events::ProtocolInitialized;

#[derive(Accounts)]
#[instruction(
    protocol_fee_bps: u64
)]
pub struct InitializeProtocol<'info> {
    #[account(
        init,
        payer = admin,
        space = GlobalConfig::BASE_LEN,
        seeds = [SEED_GLOBAL_CONFIG],
        bump
    )]
    pub global_config: Account<'info, GlobalConfig>,

    #[account(mut)]
    pub admin: Signer<'info>,

    /// CHECK: Treasury wallet for protocol fees
    pub treasury_wallet: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

pub fn initialize_protocol(
    ctx: Context<InitializeProtocol>,
    protocol_fee_bps: u64,
) -> Result<()> {
    let global_config = &mut ctx.accounts.global_config;
    
    global_config.admin = ctx.accounts.admin.key();
    global_config.treasury_wallet = ctx.accounts.treasury_wallet.key();
    
    global_config.protocol_fee_bps = protocol_fee_bps;

    global_config.paused = false;
    global_config.total_users = 0;

    emit!(ProtocolInitialized {
        admin: ctx.accounts.admin.key(),
        fee_wallet: ctx.accounts.treasury_wallet.key(),
    });

    Ok(())
}