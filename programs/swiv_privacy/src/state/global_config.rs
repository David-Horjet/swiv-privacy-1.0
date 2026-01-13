use anchor_lang::prelude::*;

#[account]
pub struct GlobalConfig {
    /// The Super Admin who can pause/unpause, change fees, and update whitelist
    pub admin: Pubkey,
    
    /// Wallet that collects the protocol fees (formerly fee_wallet)
    pub treasury_wallet: Pubkey,
    
    /// Fee for House/Infinite markets (charged on Entry). E.g., 10 = 0.1%
    pub house_fee_bps: u64,

    /// Fee for Parimutuel markets (charged on Resolution). E.g., 250 = 2.5%
    pub parimutuel_fee_bps: u64,

    /// List of allowed Token Mints for creating markets
    pub allowed_assets: Vec<Pubkey>,
    
    /// Circuit Breaker
    pub paused: bool,
    
    /// Stats
    pub total_users: u64,

    /// Time to wait before admin can force settle abandoned bets
    pub batch_settle_wait_duration: i64,
}

impl GlobalConfig {
    pub const BASE_LEN: usize = 8 + 32 + 32 + 8 + 8 + 4 + 1 + 8 + 8;
}