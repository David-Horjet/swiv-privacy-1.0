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
pub struct AssetConfigUpdated {
    pub symbol: String,
    pub pyth_feed: Pubkey,
    pub volatility_factor: u64,
}

#[event]
pub struct FixedMarketCreated {
    pub market_name: String,
    pub start_time: i64,
    pub end_time: i64,
    pub initial_liquidity: u64,
}

// --- LIQUIDITY ---
#[event]
pub struct LiquidityAdded {
    pub user: Pubkey,
    pub asset_symbol: String,
    pub amount_deposited: u64,
    pub shares_minted: u64,
}

#[event]
pub struct LiquidityRemoved {
    pub user: Pubkey,
    pub asset_symbol: String,
    pub shares_burned: u64,
    pub amount_withdrawn: u64,
}

// --- BETTING ---
#[event]
pub struct BetPlaced {
    pub bet_address: Pubkey,
    pub user: Pubkey,
    pub market_identifier: String, 
    pub market_type: MarketType,
    pub amount: u64,
    pub end_timestamp: i64,
    pub payout_multiplier_bps: u64,
}

#[event]
pub struct BetUpdated {
    pub bet_address: Pubkey,
    pub user: Pubkey,
    pub old_multiplier_bps: u64,
    pub new_multiplier_bps: u64,
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