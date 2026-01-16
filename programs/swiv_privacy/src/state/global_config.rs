use anchor_lang::prelude::*;

#[account]
pub struct GlobalConfig {
    pub admin: Pubkey,
    pub treasury_wallet: Pubkey,
    pub protocol_fee_bps: u64,
    pub paused: bool,
    pub total_users: u64,
}

impl GlobalConfig {
    pub const BASE_LEN: usize = 8 + 32 + 32 + 8 + 1 + 8;
}