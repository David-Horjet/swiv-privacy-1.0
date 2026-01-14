use anchor_lang::prelude::*;
use crate::state::{MarketType};

// --- GLOBAL & ADMIN ---
#[event]
pub struct ProtocolInitialized {
    pub admin: Pubkey,
    pub fee_wallet: Pubkey,
}

#[event]
pub struct PauseChanged {
    pub is_paused: bool,
}

#[event]
pub struct PoolCreated {
    pub pool_name: String,
    pub start_time: i64,
    pub end_time: i64,
}


// --- BETTING ---
#[event]
pub struct BetPlaced {
    pub bet_address: Pubkey,
    pub user: Pubkey,
    pub pool: Pubkey,
    pub amount: u64,
    pub end_timestamp: i64,
}

#[event]
pub struct BetUpdated {
    pub bet_address: Pubkey,
    pub user: Pubkey,
    pub old_low: u64,
    pub old_high: u64,
    pub old_target: u64,
    pub new_low: u64,
    pub new_high: u64,
    pub new_target: u64,
}

// --- SETTLEMENT ---
#[event]
pub struct RevealRequested {
    pub bet_address: Pubkey,
    pub request_id: String,
    pub timestamp: i64,
}

#[event]
pub struct BetRevealed {
    pub bet_address: Pubkey,
    pub decrypted_low: u64,
    pub decrypted_high: u64,
    pub decrypted_target: u64, 
}

#[event]
pub struct BetSettled {
    pub bet_address: Pubkey,
    pub user: Pubkey,
    pub outcome_price: u64,
    pub is_win: bool,
    pub payout: u64,
    pub refund_amount: u64,
    pub referral_fee: u64,
    pub forced_by_admin: bool, // True if batch settled (forfeit applied)
}