use anchor_lang::prelude::*;

#[account]
pub struct Pool {
    pub admin: Pubkey,
    pub name: String,

    pub start_time: i64,
    pub end_time: i64,

    pub is_resolved: bool,
    pub final_outcome: u64,
    pub resolution_ts: i64,

    pub total_weight: u128,
    pub weight_finalized: bool,

    pub vault_balance: u64,
    pub locked_for_payouts: u64,

    pub max_accuracy_buffer: u64,
    pub conviction_bonus_bps: u64,

    pub bump: u8,
}

impl Pool {
    pub const LEN: usize = 8 + 32 + 4 + 64 + 8 + 8 + 1 + 8 + 16 + 8 + 8 + 8 + 1 + 8;
}