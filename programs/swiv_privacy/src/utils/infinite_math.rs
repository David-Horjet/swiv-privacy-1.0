use crate::errors::CustomError;
use anchor_lang::prelude::*;

// --- CONSTANTS ---
pub const PRECISION: u128 = 1_000_000;
pub const BPS_SCALE: u128 = 10_000; 

// Safety Caps
pub const MAX_MULTIPLIER_CAP_BPS: u64 = 500_000; 
pub const MIN_MULTIPLIER_FLOOR_BPS: u64 = 1_000; 

/// -------------------------------------------------------------------
/// 1. LIQUIDITY PROVIDER MATH (Raydium Style)
/// -------------------------------------------------------------------

/// Calculates how many LP shares to mint when depositing USDT.
pub fn calculate_shares_to_mint(
    deposit_amount: u64,
    total_assets: u64,
    total_shares: u64,
) -> Result<u64> {
    // Initial Liquidity Provision
    if total_shares == 0 || total_assets == 0 {
        return Ok(deposit_amount);
    }

    let deposit_u128 = deposit_amount as u128;
    let shares_u128 = total_shares as u128;
    let assets_u128 = total_assets as u128;

    let shares_to_mint = deposit_u128
        .checked_mul(shares_u128)
        .ok_or(CustomError::MathOverflow)?
        .checked_div(assets_u128)
        .ok_or(CustomError::MathOverflow)?;

    Ok(shares_to_mint as u64)
}

/// Calculates how much USDT to send back when burning LP shares.
pub fn calculate_assets_to_withdraw(
    shares_to_burn: u64,
    total_assets: u64,
    total_shares: u64,
) -> Result<u64> {
    if total_shares == 0 {
        return Ok(0);
    }

    let burn_u128 = shares_to_burn as u128;
    let assets_u128 = total_assets as u128;
    let shares_u128 = total_shares as u128;

    let asset_amount = burn_u128
        .checked_mul(assets_u128)
        .ok_or(CustomError::MathOverflow)?
        .checked_div(shares_u128)
        .ok_or(CustomError::MathOverflow)?;

    Ok(asset_amount as u64)
}

/// -------------------------------------------------------------------
/// 2. TIME DECAY MATH (For Fixed Markets)
/// -------------------------------------------------------------------

/// Calculates the Time Decay factor.
/// - First 10% of time: 100% Factor (Golden Window).
/// - Last 10% of time: 0% Factor (Betting Locked).
/// - Middle 80%: Linearly decays from 100% to 0%.
pub fn calculate_time_decay_factor(
    start_time: i64,
    end_time: i64,
    current_time: i64,
) -> Result<u64> {
    if current_time < start_time {
        return Ok(10_000);
    } // Pre-start

    let total_duration = end_time
        .checked_sub(start_time)
        .ok_or(CustomError::MathOverflow)?;
    let elapsed = current_time
        .checked_sub(start_time)
        .ok_or(CustomError::MathOverflow)?;

    // 10% Thresholds
    let golden_window = total_duration / 10; // First 10%
    let lock_time = total_duration.checked_mul(9).unwrap() / 10; // 90% mark

    // Too late?
    if elapsed >= lock_time {
        return Ok(0); // Locked
    }

    // In Golden Window?
    if elapsed <= golden_window {
        return Ok(10_000); // 100% (1.0)
    }

    // Decay Window (Between 10% and 90%)
    let decay_duration = lock_time - golden_window;
    let time_into_decay = elapsed - golden_window;

    // Fraction Lost = Time_Into_Decay / Decay_Duration
    // Factor = 1.0 - Fraction_Lost
    let fraction_lost_bps = (time_into_decay as u128)
        .checked_mul(BPS_SCALE)
        .unwrap()
        .checked_div(decay_duration as u128)
        .unwrap() as u64;

    let factor = 10_000u64.saturating_sub(fraction_lost_bps);

    Ok(factor)
}

/// -------------------------------------------------------------------
/// 3. HYBRID SLOPE MATH & SOLVENCY
/// -------------------------------------------------------------------

pub struct SettlementResult {
    pub payout: u64, // Total to send to user
    pub is_win: bool,
    pub referrer_fee: u64,         // Portion of residue for referrer
    pub vault_profit: u64,         // Amount the vault keeps
    pub peak_multiplier_used: u64, // For logging
}

/// Used during 'Place Bet' to lock funds based on what the UI declared.
pub fn calculate_max_potential_profit(deposit: u64, declared_multiplier_bps: u64) -> Result<u64> {
    let deposit_u128 = deposit as u128;
    let mult_u128 = declared_multiplier_bps as u128;

    // Profit = Deposit * (Multiplier / 10000)
    let profit = deposit_u128
        .checked_mul(mult_u128)
        .ok_or(CustomError::MathOverflow)?
        .checked_div(BPS_SCALE)
        .ok_or(CustomError::MathOverflow)?;

    Ok(profit as u64)
}

/// Helper: Calculates what the multiplier SHOULD be based on Range vs Entry Volatility.
pub fn calculate_implied_multiplier(
    prediction_low: u64,
    prediction_high: u64,
    entry_price: u64,
    asset_volatility_bps: u64, // Must be the Snapshot Value from PlaceBet
) -> Result<u64> {
    // Safety check: High must be > Low
    if prediction_high <= prediction_low {
        return Ok(MIN_MULTIPLIER_FLOOR_BPS);
    }

    let range_width = (prediction_high - prediction_low) as u128;

    // (Range * 10,000) / Entry Price
    let user_range_bps = range_width
        .checked_mul(BPS_SCALE)
        .ok_or(CustomError::MathOverflow)?
        .checked_div(entry_price as u128)
        .ok_or(CustomError::MathOverflow)?;

    let safe_range_bps = if user_range_bps == 0 {
        1
    } else {
        user_range_bps
    };

    // Difficulty = Asset Volatility / User Range
    // Example: Vol 500bps / Range 100bps = 5.0x Multiplier
    let difficulty_ratio_bps = (asset_volatility_bps as u128)
        .checked_mul(BPS_SCALE)
        .ok_or(CustomError::MathOverflow)?
        .checked_div(safe_range_bps)
        .ok_or(CustomError::MathOverflow)?;

    // Caps
    let mut final_bps = difficulty_ratio_bps as u64;

    if final_bps > MAX_MULTIPLIER_CAP_BPS {
        final_bps = MAX_MULTIPLIER_CAP_BPS;
    }
    if final_bps < MIN_MULTIPLIER_FLOOR_BPS {
        final_bps = MIN_MULTIPLIER_FLOOR_BPS;
    }

    Ok(final_bps)
}

/// The Core Settlement Engine
pub fn calculate_settlement(
    deposit: u64,
    declared_multiplier_bps: u64, // The "Cap" (from UserBet state)
    prediction_low: u64,
    prediction_high: u64,
    prediction_target: u64,
    actual_price: u64,
    entry_price: u64,             // Snapshot from UserBet
    volatility_at_entry_bps: u64, // Snapshot from UserBet
    mercy_limit_bps: u64,
    referrer_exists: bool,
) -> Result<SettlementResult> {
    // --- 1. FAIRNESS CHECK (Invalid Inputs) ---
    // If the user made a mistake (Target outside range OR Low > High).
    // We treat this as a VOID bet and Refund 100% of the deposit.
    if prediction_target < prediction_low
        || prediction_target > prediction_high
        || prediction_high <= prediction_low
    {
        return Ok(SettlementResult {
            payout: deposit, // Full Refund
            is_win: false,
            referrer_fee: 0,
            vault_profit: 0, // Vault takes nothing
            peak_multiplier_used: 0,
        });
    }

    // --- 2. RANGE CHECK (Win/Loss) ---
    let is_in_range = actual_price >= prediction_low && actual_price <= prediction_high;

    if is_in_range {
        // --- WINNER LOGIC ---

        // A. Calculate what the multiplier SHOULD be based on ENTRY data
        let calculated_peak_bps = calculate_implied_multiplier(
            prediction_low,
            prediction_high,
            entry_price,
            volatility_at_entry_bps,
        )?;

        // B. Anti-Cheat: Use the LOWER of (Declared vs Calculated)
        let effective_peak_bps = std::cmp::min(declared_multiplier_bps, calculated_peak_bps);

        // C. Calculate Slope Precision (The Mountain)
        let actual = actual_price as u128;
        let target = prediction_target as u128;
        let low = prediction_low as u128;
        let high = prediction_high as u128;

        let max_distance;
        let actual_distance;

        if actual > target {
            // Right Slope
            max_distance = high - target;
            actual_distance = actual - target;
        } else {
            // Left Slope
            max_distance = target - low;
            actual_distance = target - actual;
        }

        // Precision = (Max - Actual) / Max
        let precision_fraction_bps = if max_distance == 0 {
            BPS_SCALE // Target == Edge (Perfect edge hit?)
        } else {
            max_distance
                .checked_sub(actual_distance)
                .unwrap_or(0)
                .checked_mul(BPS_SCALE)
                .ok_or(CustomError::MathOverflow)?
                .checked_div(max_distance)
                .ok_or(CustomError::MathOverflow)?
        };

        // D. Final Multiplier = Peak * Precision
        let final_multiplier_bps = (effective_peak_bps as u128)
            .checked_mul(precision_fraction_bps)
            .ok_or(CustomError::MathOverflow)?
            .checked_div(BPS_SCALE)
            .ok_or(CustomError::MathOverflow)?;

        // E. Calculate Profit
        let profit = (deposit as u128)
            .checked_mul(final_multiplier_bps)
            .ok_or(CustomError::MathOverflow)?
            .checked_div(BPS_SCALE)
            .ok_or(CustomError::MathOverflow)? as u64;

        let total_payout = deposit
            .checked_add(profit)
            .ok_or(CustomError::MathOverflow)?;

        return Ok(SettlementResult {
            payout: total_payout,
            is_win: true,
            referrer_fee: 0,
            vault_profit: 0, // Vault loses money in a win scenario (profit handled by payouts)
            peak_multiplier_used: effective_peak_bps,
        });
    }

    // --- 3. LOSER LOGIC (Mercy Refund) ---

    // Calculate Distance from the CLOSEST edge
    let diff_low = if actual_price > prediction_low {
        actual_price - prediction_low
    } else {
        prediction_low - actual_price
    };
    let diff_high = if actual_price > prediction_high {
        actual_price - prediction_high
    } else {
        prediction_high - actual_price
    };
    let diff = std::cmp::min(diff_low, diff_high);

    // Calculate Max Mercy Distance allowed
    let limit_distance = (actual_price as u128)
        .checked_mul(mercy_limit_bps as u128)
        .unwrap()
        .checked_div(BPS_SCALE)
        .unwrap() as u64;

    // Base Logic: 50% Hard Loss
    let hard_loss = deposit / 2;
    let mercy_pot = deposit - hard_loss;

    let mut user_refund = 0;

    // Calculate Partial Refund if within Mercy Limit
    if diff < limit_distance && limit_distance > 0 {
        let diff_u128 = diff as u128;
        let limit_u128 = limit_distance as u128;
        let pot_u128 = mercy_pot as u128;

        // Refund = Pot * (Limit - Diff) / Limit
        user_refund = pot_u128
            .checked_mul(limit_u128 - diff_u128)
            .unwrap()
            .checked_div(limit_u128)
            .unwrap() as u64;
    }

    let residue = mercy_pot.saturating_sub(user_refund);
    let mut referrer_fee = 0;
    let mut vault_keep = hard_loss + residue;

    // Distribute Residue to Referrer if exists
    if referrer_exists && residue > 0 {
        referrer_fee = residue / 2;
        vault_keep = vault_keep.saturating_sub(referrer_fee);
    }

    Ok(SettlementResult {
        payout: user_refund,
        is_win: false,
        referrer_fee,
        vault_profit: vault_keep,
        peak_multiplier_used: 0,
    })
}
