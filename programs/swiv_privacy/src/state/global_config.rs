use anchor_lang::prelude::*;

#[account]
pub struct GlobalConfig {
    /// The Super Admin who can pause/unpause and change protocol params
    pub admin: Pubkey,
    
    /// Wallet that collects the protocol fees
    pub treasury_wallet: Pubkey,

    /// Fee for the protocol (charged on Resolution). E.g., 250 = 2.5%
    pub protocol_fee_bps: u64,

    /// Circuit Breaker
    pub paused: bool,
    
    /// Stats
    pub total_users: u64,
}

impl GlobalConfig {
    pub const BASE_LEN: usize = 8 + 32 + 32 + 8 + 1 + 8;
}