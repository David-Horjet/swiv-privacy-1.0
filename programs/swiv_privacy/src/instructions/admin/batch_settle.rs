use anchor_lang::prelude::*;
use anchor_spl::token::{Token, TokenAccount};
use pyth_solana_receiver_sdk::price_update::PriceUpdateV2;

use crate::constants::SEED_GLOBAL_CONFIG;
use crate::errors::CustomError;
use crate::instructions::shared::execute_settlement_logic;
use crate::state::{AssetConfig, BetStatus, GlobalConfig, LiquidityVault, UserBet};

#[derive(Accounts)]
pub struct BatchSettle<'info> {
    #[account(
        mut,
        seeds = [SEED_GLOBAL_CONFIG],
        bump,
        constraint = global_config.admin == admin.key() @ CustomError::Unauthorized
    )]
    pub global_config: Account<'info, GlobalConfig>,

    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(mut)]
    pub vault: Box<Account<'info, LiquidityVault>>,

    #[account(
        mut,
        seeds = [b"pool_vault", vault.key().as_ref()],
        bump,
        token::authority = vault,
    )]
    pub vault_token_account: Box<Account<'info, TokenAccount>>,

    #[account(mut)]
    /// CHECK: Admin fee wallet
    pub fee_wallet: UncheckedAccount<'info>,

    pub asset_config: Box<Account<'info, AssetConfig>>,

    pub price_update: Account<'info, PriceUpdateV2>,

    pub token_program: Program<'info, Token>,
}

// Added explicit lifetime 'info to handle Context and remaining_accounts correctly
pub fn batch_settle<'info>(
    ctx: Context<'_, '_, '_, 'info, BatchSettle<'info>>,
    _pyth_price_proof: Vec<u8>,
) -> Result<()> {
    let accounts_iter = &mut ctx.remaining_accounts.iter();
    let clock = Clock::get()?;

    let vault_token_info = ctx.accounts.vault_token_account.to_account_info();
    let fee_wallet_info = ctx.accounts.fee_wallet.to_account_info();

    loop {
        let user_bet_acc_info = match accounts_iter.next() {
            Some(acc) => acc,
            None => break,
        };

        let user_token_acc_info = match accounts_iter.next() {
            Some(acc) => acc,
            None => return Err(ProgramError::NotEnoughAccountKeys.into()),
        };

        let referrer_acc_info = match accounts_iter.next() {
            Some(acc) => acc,
            None => return Err(ProgramError::NotEnoughAccountKeys.into()),
        };

        let mut user_bet_data = user_bet_acc_info.try_borrow_mut_data()?;
        let mut user_bet = UserBet::try_deserialize(&mut &user_bet_data[..])?;

        if user_bet.status == BetStatus::Settled {
            continue;
        }

        require!(
            user_bet.market_identifier == ctx.accounts.asset_config.symbol,
            CustomError::MarketMismatch 
        );

        require!(
            clock.unix_timestamp
                > user_bet.end_timestamp + ctx.accounts.global_config.batch_settle_wait_duration,
            CustomError::SettlementTooEarly
        );

        let referrer_option = if let Some(ref_key) = user_bet.referrer {
            if ref_key == *referrer_acc_info.key {
                Some(referrer_acc_info.clone())
            } else {
                None
            }
        } else {
            None
        };

        // This logic will now fail if the status isn't Revealed, which is intended.
        // For abandoned bets, the flow should be:
        // 1. User fails to call request_reveal.
        // 2. After 24h, anyone can call request_reveal for them.
        // 3. Once revealed, admin can batch_settle.
        execute_settlement_logic(
            &mut user_bet,
            &mut ctx.accounts.vault,
            &ctx.accounts.asset_config,
            &ctx.accounts.price_update,
            &vault_token_info,
            user_token_acc_info,
            referrer_option,
            &fee_wallet_info,
            &ctx.accounts.token_program,
            true,
        )?;

        let mut new_data: Vec<u8> = Vec::new();
        user_bet.try_serialize(&mut new_data)?;

        if new_data.len() <= user_bet_data.len() {
            user_bet_data[..new_data.len()].copy_from_slice(&new_data);
        } else {
            return Err(ProgramError::AccountDataTooSmall.into());
        }
    }

    Ok(())
}
