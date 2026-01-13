use anchor_lang::prelude::*;

#[account]
pub struct LiquidityVault {
    /// The Mint address of the token being pooled (e.g., USDT)
    pub token_mint: Pubkey,
    
    /// Links this vault to a specific AssetConfig (e.g., "SOL")
    pub asset_symbol: String,
    
    /// Total raw assets (USDT) currently held in the vault
    pub total_assets: u64,
    
    /// Total LP tokens (shares) minted to providers
    pub total_shares: u64,
    
    /// The total potential profit the vault is liable for across all active bets.
    /// MUST be less than total_assets.
    pub total_exposure: u64,

    /// Accumulator for admin fees that haven't been claimed yet
    pub accumulated_fees: u64,

    pub bump: u8,
}

impl LiquidityVault {
    // 8 + 32 + (4 + 10) + 8 + 8 + 8 + 8 + 1
    pub const LEN: usize = 8 + 32 + 14 + 8 + 8 + 8 + 8 + 1;
}