use crate::errors::CustomError;
use anchor_lang::prelude::*;

// Precision for internal calculations (1_000_000 = 1.0)
pub const MATH_PRECISION: u128 = 1_000_000; 

// --- 1. ACCURACY SCORE (Linear Normalization) ---
/// Formula: Accuracy = 1.0 - ( |Prediction - Result| / Buffer )
/// Returns a value between 0 and MATH_PRECISION (0.0 to 1.0)
pub fn calculate_accuracy_score(
    prediction: u64,
    result: u64,
    buffer: u64,
) -> Result<u64> {
    if buffer == 0 {
        return Ok(0); // Safety against division by zero
    }

    let diff = if prediction > result {
        prediction - result
    } else {
        result - prediction
    };

    // If outside the buffer, accuracy is 0
    if diff >= buffer {
        return Ok(0);
    }

    // Fraction of error = diff / buffer
    // Score = 1.0 - Fraction
    
    let diff_u128 = diff as u128;
    let buffer_u128 = buffer as u128;

    let error_fraction = diff_u128
        .checked_mul(MATH_PRECISION)
        .ok_or(CustomError::MathOverflow)?
        .checked_div(buffer_u128)
        .ok_or(CustomError::MathOverflow)?;

    let score = MATH_PRECISION.saturating_sub(error_fraction);

    Ok(score as u64)
}

// --- 2. TIME BONUS (Linear Decay) ---
/// Formula: Factor = 1.0 + ( (EndTime - EntryTime) / TotalDuration )
/// - Entry at Start: Bonus = 1.0 + 1.0 = 2.0x
/// - Entry at End: Bonus = 1.0 + 0.0 = 1.0x
pub fn calculate_time_bonus(
    start_time: i64,
    end_time: i64,
    entry_time: i64,
) -> Result<u64> {
    if entry_time >= end_time {
        return Ok(MATH_PRECISION as u64); // 1.0x (No bonus)
    }

    let total_duration = (end_time - start_time) as u128;
    let remaining_time = (end_time - entry_time) as u128;

    if total_duration == 0 {
        return Ok(MATH_PRECISION as u64);
    }

    // Bonus Portion = Remaining / Total
    let bonus_portion = remaining_time
        .checked_mul(MATH_PRECISION)
        .unwrap()
        .checked_div(total_duration)
        .unwrap();

    // Total Factor = 1.0 + Bonus Portion
    let factor = MATH_PRECISION.checked_add(bonus_portion).unwrap();

    Ok(factor as u64)
}

// --- 3. CONVICTION BONUS ---
/// If update_count == 0, returns 1.5x (adjustable). Else 1.0x.
pub fn calculate_conviction_bonus(update_count: u32) -> u64 {
    if update_count == 0 {
        // 1.5x Bonus
        1_500_000
    } else {
        // 1.0x (No Bonus)
        1_000_000
    }
}

// --- 4. MASTER WEIGHT CALCULATION ---
/// Weight = Stake * Accuracy * Time * Conviction
pub fn calculate_parimutuel_weight(
    stake: u64,
    accuracy_score_scaled: u64, // 0 to 1,000,000
    time_bonus_scaled: u64,     // 1,000,000 to 2,000,000
    conviction_scaled: u64,     // 1,000,000 or 1,500,000
) -> Result<u128> {
    
    // We do all multiplication in u128
    let stake_u128 = stake as u128;
    
    // Perform multiplication
    let raw_product = stake_u128
        .checked_mul(accuracy_score_scaled as u128).unwrap()
        .checked_mul(time_bonus_scaled as u128).unwrap()
        .checked_mul(conviction_scaled as u128).unwrap();

    // Divide by Precision^3 because we multiplied 3 scaled numbers
    // Result is the "Weight" in raw units relative to stake
    let final_weight = raw_product
        .checked_div(MATH_PRECISION).unwrap()
        .checked_div(MATH_PRECISION).unwrap()
        .checked_div(MATH_PRECISION).unwrap();

    Ok(final_weight)
}   