use anchor_lang::prelude::*;

#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq)]
pub enum MarketMode {
    House,      
    Parimutuel, 
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq)]
pub enum WinCriterion {
    TargetOnly, 
    Range,      
}

#[account]
pub struct FixedMarket {
    pub admin: Pubkey,
    pub name: String,
    pub token_mint: Pubkey,
    
    pub start_time: i64,
    pub end_time: i64,
    pub vault_balance: u64,
    
    // --- CONFIGURATION ---
    pub mode: MarketMode,
    pub criterion: WinCriterion,
    pub max_accuracy_buffer: u64,
    pub conviction_bonus_bps: u64, 
    
    // --- DYNAMIC METADATA (New) ---
    // Optional URL or IPFS hash. 
    pub metadata: Option<String>,

    // --- RESOLUTION ---
    pub resolution_target: u64,
    pub is_resolved: bool,
    
    // ADD THIS FIELD vvv
    pub resolution_ts: i64,
    
    // --- PARIMUTUEL STATE ---
    pub total_weight: u128,     
    pub weight_finalized: bool, 
    
    // --- HOUSE STATE ---
    pub locked_for_payouts: u64,

    pub bump: u8,
}