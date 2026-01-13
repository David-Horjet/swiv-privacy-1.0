use anchor_lang::prelude::*;
use crate::state::{FixedMarket, GlobalConfig};
use crate::constants::{SEED_GLOBAL_CONFIG, SEED_FIXED_MARKET};
use crate::errors::CustomError;

#[derive(Accounts)]
pub struct ResolveFixedMarket<'info> {
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
        seeds = [SEED_FIXED_MARKET, fixed_market.name.as_bytes()],
        bump = fixed_market.bump
    )]
    pub fixed_market: Account<'info, FixedMarket>,
}

pub fn resolve_fixed_market(ctx: Context<ResolveFixedMarket>, final_outcome: u64) -> Result<()> {
    let market = &mut ctx.accounts.fixed_market;
    
    require!(!market.is_resolved, CustomError::AlreadySettled);
    
    let clock = Clock::get()?;
    require!(clock.unix_timestamp >= market.end_time, CustomError::DurationTooShort);

    market.resolution_target = final_outcome;
    market.is_resolved = true;
    
    market.resolution_ts = clock.unix_timestamp; 
    
    market.weight_finalized = false; 
    
    msg!("Fixed Market Resolved. Outcome: {}", final_outcome);
    
    Ok(())
}