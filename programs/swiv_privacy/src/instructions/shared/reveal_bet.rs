use anchor_lang::prelude::*;
use solana_program::keccak; 
use crate::state::UserBet; 
use crate::errors::CustomError;
use crate::events::BetRevealed;
use crate::constants::PERMISSION_PROGRAM_ID;

#[derive(Accounts)]
pub struct RevealBet<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        mut,
        constraint = user_bet.owner == user.key() @ CustomError::Unauthorized,
        constraint = !user_bet.is_revealed @ CustomError::AlreadyRevealed
    )]
    pub user_bet: Account<'info, UserBet>,

    /// CHECK: Seeds verification
    #[account(
        seeds = [b"group", user_bet.key().as_ref()],
        seeds::program = permission_program.key(),
        bump
    )]
    pub group: UncheckedAccount<'info>,

    /// CHECK: Seeds verification
    #[account(
        seeds = [b"permission", group.key().as_ref(), user.key().as_ref()],
        seeds::program = permission_program.key(),
        bump
    )]
    pub permission: UncheckedAccount<'info>,

    /// CHECK: Seeds verification
    #[account(address = PERMISSION_PROGRAM_ID)]
    pub permission_program: UncheckedAccount<'info>,
}

pub fn reveal_bet(
    ctx: Context<RevealBet>,
    prediction_low: u64,
    prediction_high: u64,
    prediction_target: u64,
    salt: [u8; 32], 
) -> Result<()> {
    let user_bet = &mut ctx.accounts.user_bet;
    let clock = Clock::get()?;

    // 1. Verify Permissions
    require!(!ctx.accounts.permission.data_is_empty(), CustomError::Unauthorized);

    // 2. Timing Check (5 min window)
    let max_delay_seconds = 300; 
    if clock.unix_timestamp > user_bet.creation_ts + max_delay_seconds {
        return Err(CustomError::RevealWindowExpired.into());
    }

    // 3. Verify Hash
    let mut data = Vec::new();
    data.extend_from_slice(&prediction_low.to_le_bytes());
    data.extend_from_slice(&prediction_high.to_le_bytes());
    data.extend_from_slice(&prediction_target.to_le_bytes());
    data.extend_from_slice(&salt);

    let calculated_hash = keccak::hash(&data);

    require!(
        calculated_hash.to_bytes() == user_bet.commitment,
        CustomError::InvalidCommitment
    );

    // 4. Update State
    user_bet.prediction_low = prediction_low;
    user_bet.prediction_high = prediction_high;
    user_bet.prediction_target = prediction_target;
    user_bet.is_revealed = true;

    emit!(BetRevealed {
        bet_address: user_bet.key(),
        decrypted_low: prediction_low,
        decrypted_high: prediction_high,
        decrypted_target: prediction_target
    });

    msg!("Bet Revealed Successfully");
    Ok(())
}