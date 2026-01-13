use anchor_lang::prelude::*;

#[account]
pub struct AssetConfig {
    /// E.g., "SOL"
    pub symbol: String,
    
    /// The Pyth Price Feed Account address for this asset
    pub pyth_feed: Pubkey,
    
    /// Risk Multiplier. Higher = Higher Payouts but higher risk for House.
    /// Scaled by 1000 (e.g., 1500 = 1.5x volatility)
    pub volatility_factor: u64,
    
    /// How close a loser needs to be to get a "Mercy Refund".
    /// Measured in Basis Points (e.g., 500 = 5% price movement).
    pub mercy_buffer_bps: u64,
    
    /// If true, multiplier is derived from Pyth confidence. If false, uses volatility_factor.
    pub use_pyth_volatility: bool,

    pub bump: u8,
}

impl AssetConfig {
    // 8 + (4 + 10) String + 32 + 8 + 8 + 1 + 1
    pub const LEN: usize = 8 + 14 + 32 + 8 + 8 + 1 + 1;
}