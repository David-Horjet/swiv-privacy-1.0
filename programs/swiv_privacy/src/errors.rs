use anchor_lang::prelude::*;

#[error_code]
pub enum CustomError {
    #[msg("Global protocol is paused.")]
    Paused,
    #[msg("Unauthorized admin action.")]
    Unauthorized,
    #[msg("Math operation overflow.")]
    MathOverflow,
    #[msg("Slippage tolerance exceeded.")]
    SlippageExceeded,
    #[msg("Insufficient liquidity in vault.")]
    InsufficientLiquidity,
    #[msg("Bet rejected: Potential payout exceeds vault solvency limit.")]
    SolvencyRisk,
    #[msg("Bet is already settled.")]
    AlreadySettled,
    #[msg("Bet duration is too short.")]
    DurationTooShort,
    #[msg("Invalid asset symbol.")]
    InvalidAsset,
    #[msg("Asset is not whitelisted.")]
    AssetNotWhitelisted,
    #[msg("Bet does not match the current vault/asset config")]
    MarketMismatch,
    #[msg("Parimutuel MUST be TargetOnly")]
    InvalidMode,
    #[msg("Oracle price is non-positive.")]
    InvalidOraclePrice,
    #[msg("Admin force-settlement is not yet allowed for this bet.")]
    SettlementTooEarly,
    #[msg("Emergency refund timeout has not been met.")]
    TimeoutNotMet,
    #[msg("Bet has not been calculated by the TEE yet.")]
    NotCalculatedYet,
    #[msg("The provided prediction does not match the commitment hash.")]
    InvalidCommitment,
    #[msg("Bet is already revealed.")]
    AlreadyRevealed,
    #[msg("Bet is already revealed.")]
    BetNotRevealed,
    #[msg("You cannot refund a bet that has been revealed. Wait for settlement.")]
    CannotRefundRevealed,
    #[msg("Reveal window has expired. Please request a refund.")]
    RevealWindowExpired,
    #[msg("Instruction has been removed in the Pool refactor.")]
    InstructionDeprecated,
}