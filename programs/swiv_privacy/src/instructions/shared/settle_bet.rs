use crate::constants::SEED_VAULT;
use crate::errors::CustomError;
use crate::events::BetSettled;
use crate::state::{AssetConfig, BetStatus, LiquidityVault, UserBet};
use crate::utils::infinite_math::{calculate_max_potential_profit, calculate_settlement};
use crate::utils::pyth::get_pyth_price;
use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, TokenAccount, Transfer};
use pyth_solana_receiver_sdk::price_update::PriceUpdateV2;

#[derive(Accounts)]
pub struct SettleBet<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(mut)]
    pub user_bet: Box<Account<'info, UserBet>>,

    #[account(
        mut,
        seeds = [SEED_VAULT, user_bet.market_identifier.as_bytes()],
        bump = vault.bump
    )]
    pub vault: Box<Account<'info, LiquidityVault>>,

    #[account(
        mut,
        seeds = [b"pool_vault", vault.key().as_ref()],
        bump,
        token::mint = token_mint,
        token::authority = vault,
    )]
    pub vault_token_account: Box<Account<'info, TokenAccount>>,

    pub token_mint: Account<'info, token::Mint>,

    #[account(mut)]
    /// CHECK: Where we send winnings or refunds
    pub user_token_account: UncheckedAccount<'info>,

    #[account(mut)]
    /// CHECK: Referrer wallet (optional)
    pub referrer_token_account: Option<UncheckedAccount<'info>>,

    #[account(mut)]
    /// CHECK: Admin fee wallet (only used for force-settle penalty)
    pub fee_wallet: UncheckedAccount<'info>,

    pub asset_config: Box<Account<'info, AssetConfig>>,

    pub price_update: Account<'info, PriceUpdateV2>,

    pub token_program: Program<'info, Token>,
}

pub fn settle_bet(ctx: Context<SettleBet>) -> Result<()> {
    let user_bet_struct = &mut *ctx.accounts.user_bet;

    // Prepare AccountInfos
    let vault_token_account_info = ctx.accounts.vault_token_account.to_account_info();
    let user_token_info = ctx.accounts.user_token_account.to_account_info();
    let fee_wallet_info = ctx.accounts.fee_wallet.to_account_info();
    let referrer_info = ctx
        .accounts
        .referrer_token_account
        .as_ref()
        .map(|x| x.to_account_info());

    execute_settlement_logic(
        user_bet_struct,
        &mut ctx.accounts.vault, // Mut Borrow
        &ctx.accounts.asset_config,
        &ctx.accounts.price_update,
        &vault_token_account_info,
        &user_token_info,
        referrer_info,
        &fee_wallet_info,
        &ctx.accounts.token_program,
        false,
    )
}

pub fn execute_settlement_logic<'info>(
    bet: &mut UserBet,
    vault: &mut Account<'info, LiquidityVault>,
    asset_config: &Account<'info, AssetConfig>,
    price_update: &Account<'info, PriceUpdateV2>,
    vault_token_account_info: &AccountInfo<'info>,
    user_token_info: &AccountInfo<'info>,
    referrer_info: Option<AccountInfo<'info>>,
    fee_wallet_info: &AccountInfo<'info>,
    token_program: &Program<'info, Token>,
    is_admin_forced: bool,
) -> Result<()> {
    // MAGIC BLOCK CHANGE: We no longer check for BetStatus::Revealed.
    // If the bet exists and time has passed, we settle it.
    require!(
        bet.status == BetStatus::Active,
        CustomError::AlreadySettled
    );

    // 1. Release Vault Exposure
    let max_potential_profit = calculate_max_potential_profit(bet.deposit, bet.payout_multiplier)?;
    vault.total_exposure = vault.total_exposure.saturating_sub(max_potential_profit);

    // 2. Get Final Settlement Price
    let actual_price = get_pyth_price(price_update, &asset_config.pyth_feed, bet.end_timestamp)?;

    // 3. Calculate Payout
    // MAGIC BLOCK CHANGE: We use the fields directly!
    // Inside the TEE, `bet.prediction_low` is accessible as a decrypted u64.
    let settlement = calculate_settlement(
        bet.deposit,
        bet.payout_multiplier, 
        bet.prediction_low,     // NEW: Direct access
        bet.prediction_high,    // NEW: Direct access
        bet.prediction_target,  // NEW: Direct access
        actual_price,
        bet.entry_price,            
        bet.volatility_factor_at_entry, 
        asset_config.mercy_buffer_bps,
        referrer_info.is_some(),
    )?;

    // ... (The rest of the function remains identical regarding transfers) ...
    // Copy the rest of the existing function here for clarity
    
    // 4. Update Vault Assets
    let total_outflow = settlement.payout + settlement.referrer_fee;
    if total_outflow > 0 {
        vault.total_assets = vault.total_assets.checked_sub(total_outflow).unwrap_or(0);
    }

    let mut final_user_payout = settlement.payout;

    // 5. Admin Force Settle Logic
    if is_admin_forced {
        if !settlement.is_win && final_user_payout > 0 {
            vault.total_assets = vault.total_assets.checked_add(final_user_payout).unwrap();
            final_user_payout = 0;
        }

        if final_user_payout > 1_000_000 {
            final_user_payout -= 1_000_000;
            // Transfer fee logic...
            let seeds: &[&[u8]] = &[
                SEED_VAULT, 
                vault.asset_symbol.as_bytes(), 
                &[vault.bump]
            ];
            let signer = &[&seeds[..]];

            token::transfer(
                CpiContext::new_with_signer(
                    token_program.to_account_info(),
                    Transfer {
                        from: vault_token_account_info.clone(),
                        to: fee_wallet_info.clone(),
                        authority: vault.to_account_info(),
                    },
                    signer,
                ),
                1_000_000,
            )?;
        }
    }

    // 6. Perform Transfers
    let seeds: &[&[u8]] = &[
        SEED_VAULT, 
        vault.asset_symbol.as_bytes(), 
        &[vault.bump]
    ];
    let signer = &[&seeds[..]];

    if final_user_payout > 0 {
        token::transfer(
            CpiContext::new_with_signer(
                token_program.to_account_info(),
                Transfer {
                    from: vault_token_account_info.clone(),
                    to: user_token_info.clone(),
                    authority: vault.to_account_info(),
                },
                signer,
            ),
            final_user_payout,
        )?;
    }

    if settlement.referrer_fee > 0 {
        if let Some(ref_acct) = referrer_info {
            token::transfer(
                CpiContext::new_with_signer(
                    token_program.to_account_info(),
                    Transfer {
                        from: vault_token_account_info.clone(),
                        to: ref_acct.clone(),
                        authority: vault.to_account_info(),
                    },
                    signer,
                ),
                settlement.referrer_fee,
            )?;
        }
    }

    bet.status = BetStatus::Settled;

    emit!(BetSettled {
        bet_address: bet.owner.key(), 
        user: bet.owner,
        outcome_price: actual_price,
        is_win: settlement.is_win,
        payout: final_user_payout,
        refund_amount: if settlement.is_win { 0 } else { final_user_payout },
        referral_fee: settlement.referrer_fee,
        forced_by_admin: is_admin_forced,
    });

    Ok(())
}