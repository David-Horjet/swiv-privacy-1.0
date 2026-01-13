use anchor_lang::prelude::*;

#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq)]
pub enum BetStatus {
    Active,
    Calculated, 
    Settled,    
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq)]
pub enum MarketType {
    Infinite,
    Fixed,
}

#[account]
pub struct UserBet {
    pub owner: Pubkey,
    pub market_identifier: String,
    pub market_type: MarketType,
    pub deposit: u64,
    pub end_timestamp: i64,
    
    // --- FIXED MARKET SPECIFIC ---
    pub payout_multiplier: u64, 
    pub creation_ts: i64,       
    pub update_count: u32,     
    
    // --- PARIMUTUEL CALCULATION ---
    pub calculated_weight: u128, 
    pub is_weight_added: bool,

    // --- SNAPSHOTS ---
    pub entry_price: u64,
    pub volatility_factor_at_entry: u64, 

    pub referrer: Option<Pubkey>,
    
    // --- PRIVACY / COMMIT-REVEAL ---
    pub commitment: [u8; 32], 
    pub is_revealed: bool,    
    
    // These start as 0. Filled via 'reveal_bet'
    pub prediction_low: u64,
    pub prediction_high: u64,
    pub prediction_target: u64, 
    
    pub status: BetStatus,
    
    pub bump: u8,
}

impl UserBet {
    pub const SPACE: usize = 450; 
}