use anchor_lang::prelude::*;
use crate::state::{Pool, GlobalConfig};
use crate::constants::{SEED_GLOBAL_CONFIG, SEED_POOL};
use crate::errors::CustomError;

#[derive(Accounts)]
pub struct ResolvePool<'info> {
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
        seeds = [SEED_POOL, pool.name.as_bytes()],
        bump = pool.bump
    )]
    pub pool: Account<'info, Pool>,
}

pub fn resolve_pool(ctx: Context<ResolvePool>, final_outcome: u64) -> Result<()> {
    let pool = &mut ctx.accounts.pool;
    
    require!(!pool.is_resolved, CustomError::AlreadySettled);
    
    let clock = Clock::get()?;
    require!(clock.unix_timestamp >= pool.end_time, CustomError::DurationTooShort);

    pool.final_outcome = final_outcome;
    pool.is_resolved = true;
    
    pool.resolution_ts = clock.unix_timestamp; 
    
    pool.weight_finalized = false; 
    
    msg!("Pool Resolved. Outcome: {}", final_outcome);
    
    Ok(())
}