use anchor_lang::prelude::*;
use crate::state::GlobalConfig;
use crate::constants::SEED_GLOBAL_CONFIG;
use crate::errors::CustomError;

#[derive(Accounts)]
pub struct TransferAdmin<'info> {
    #[account(mut)]
    pub current_admin: Signer<'info>,

    #[account(
        mut,
        seeds = [SEED_GLOBAL_CONFIG],
        bump,
        constraint = global_config.admin == current_admin.key() @ CustomError::Unauthorized
    )]
    pub global_config: Account<'info, GlobalConfig>,
}

pub fn transfer_admin(ctx: Context<TransferAdmin>, new_admin: Pubkey) -> Result<()> {
    let global_config = &mut ctx.accounts.global_config;
    
    global_config.admin = new_admin;
    
    msg!("Admin transferred from {} to {}", ctx.accounts.current_admin.key(), new_admin);

    Ok(())
}