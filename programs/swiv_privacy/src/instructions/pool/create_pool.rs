use anchor_lang::prelude::*;
use crate::state::{Pool, GlobalConfig};
use crate::constants::{SEED_GLOBAL_CONFIG, SEED_POOL};
use crate::errors::CustomError;
use crate::events::PoolCreated;

#[derive(Accounts)]
#[instruction(name: String, start_time: i64, end_time: i64, max_accuracy_buffer: u64, conviction_bonus_bps: u64)]
pub struct CreatePool<'info> {
    #[account(
        mut,
        seeds = [SEED_GLOBAL_CONFIG],
        bump,
        constraint = global_config.admin == admin.key() @ CustomError::Unauthorized
    )]
    pub global_config: Account<'info, GlobalConfig>,

    #[account(
        init,
        payer = admin,
        space = Pool::LEN + (4 + name.len()),
        seeds = [SEED_POOL, name.as_bytes()],
        bump
    )]
    pub pool: Account<'info, Pool>,

    #[account(mut)]
    pub admin: Signer<'info>,

    pub system_program: Program<'info, System>,
}

pub fn create_pool(
    ctx: Context<CreatePool>,
    name: String,
    start_time: i64,
    end_time: i64,
    max_accuracy_buffer: u64,
    conviction_bonus_bps: u64,
) -> Result<()> {
    require!(end_time > start_time, CustomError::DurationTooShort);

    let pool = &mut ctx.accounts.pool;
    pool.admin = ctx.accounts.admin.key();
    pool.name = name.clone();
    pool.start_time = start_time;
    pool.end_time = end_time;
    pool.is_resolved = false;
    pool.final_outcome = 0;
    pool.resolution_ts = 0;
    pool.total_weight = 0;
    pool.weight_finalized = false;
    pool.vault_balance = 0;
    pool.locked_for_payouts = 0;
    pool.max_accuracy_buffer = max_accuracy_buffer;
    pool.conviction_bonus_bps = conviction_bonus_bps;
    pool.bump = ctx.bumps.pool;

    emit!(PoolCreated {
        pool_name: name,
        start_time,
        end_time,
    });

    Ok(())
}