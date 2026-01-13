use anchor_lang::prelude::*;
use crate::state::GlobalConfig;
use crate::constants::SEED_GLOBAL_CONFIG;
use crate::errors::CustomError;
use crate::events::PauseChanged;

#[derive(Accounts)]
pub struct SetPause<'info> {
    #[account(
        mut,
        seeds = [SEED_GLOBAL_CONFIG],
        bump,
        constraint = global_config.admin == admin.key() @ CustomError::Unauthorized
    )]
    pub global_config: Account<'info, GlobalConfig>,

    pub admin: Signer<'info>,
}

pub fn set_pause(ctx: Context<SetPause>, paused: bool) -> Result<()> {
    ctx.accounts.global_config.paused = paused;

    emit!(PauseChanged {
        is_paused: paused,
    });

    Ok(())
}