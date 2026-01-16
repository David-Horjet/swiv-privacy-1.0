use anchor_lang::prelude::*;

#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq)]
pub enum BetStatus {
    Active,
    Calculated,
    Settled,
}

#[account]
pub struct UserBet {
    pub owner: Pubkey,
    /// Pubkey of the Pool this bet belongs to
    pub pool: Pubkey,

    pub deposit: u64,
    pub end_timestamp: i64,

    pub creation_ts: i64,
    pub update_count: u32,

    // --- PARIMUTUEL CALCULATION ---
    pub calculated_weight: u128,
    pub is_weight_added: bool,

    // --- PRIVACY / COMMIT-REVEAL ---
    pub commitment: [u8; 32],
    pub is_revealed: bool,

    // These are filled via 'reveal_bet'
    pub prediction_low: u64,
    pub prediction_high: u64,
    pub prediction_target: u64,

    pub status: BetStatus,

    pub bump: u8,
}

impl UserBet {
    pub const SPACE: usize = 400;
}