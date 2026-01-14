use anchor_lang::prelude::*;
use crate::state::GlobalConfig;
use crate::constants::SEED_GLOBAL_CONFIG;
use crate::errors::CustomError;

#[derive(Accounts)]
#[instruction(
    new_treasury: Option<Pubkey>, 
    new_protocol_fee_bps: Option<u64>
)]
pub struct UpdateConfig<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        mut,
        seeds = [SEED_GLOBAL_CONFIG],
        bump,
        constraint = global_config.admin == admin.key() @ CustomError::Unauthorized
    )]
    pub global_config: Account<'info, GlobalConfig>,

    pub system_program: Program<'info, System>,
}

pub fn update_config(
    ctx: Context<UpdateConfig>,
    new_treasury: Option<Pubkey>,
    new_protocol_fee_bps: Option<u64>,
) -> Result<()> {
    let global_config = &mut ctx.accounts.global_config;

    if let Some(treasury) = new_treasury {
        global_config.treasury_wallet = treasury;
    }

    if let Some(p_fee) = new_protocol_fee_bps {
        global_config.protocol_fee_bps = p_fee;
    }

    msg!("Global Config Updated");

    Ok(())
}