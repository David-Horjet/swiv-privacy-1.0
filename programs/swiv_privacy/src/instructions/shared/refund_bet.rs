use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, TokenAccount, Transfer};
use crate::state::{UserBet, GlobalConfig, BetStatus};
use crate::constants::{SEED_FIXED_MARKET, SEED_GLOBAL_CONFIG};
use crate::errors::CustomError;

#[derive(Accounts)]
pub struct RefundBet<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        mut,
        constraint = user_bet.owner == user.key() @ CustomError::Unauthorized,
        constraint = user_bet.status != BetStatus::Settled @ CustomError::AlreadySettled
    )]
    pub user_bet: Box<Account<'info, UserBet>>,
    
    #[account(
        seeds = [SEED_GLOBAL_CONFIG],
        bump,
    )]
    pub global_config: Box<Account<'info, GlobalConfig>>,

    /// CHECK: Must match global config treasury
    #[account(mut, address = global_config.treasury_wallet)]
    pub treasury_wallet: UncheckedAccount<'info>,

    #[account(mut)]
    pub user_token_account: Account<'info, TokenAccount>,

    // --- INFINITE MARKET ACCOUNTS ---
    #[account(
        mut,
        seeds = [SEED_VAULT, user_bet.market_identifier.as_bytes()],
        bump, 
        constraint = user_bet.market_type == MarketType::Infinite
    )]
    pub infinite_vault: Option<Box<Account<'info, LiquidityVault>>>,

    #[account(
        mut,
        seeds = [b"pool_vault", infinite_vault.as_ref().unwrap().key().as_ref()],
        bump,
        token::mint = infinite_vault.as_ref().unwrap().token_mint,
        token::authority = infinite_vault.as_ref().unwrap(),
    )]
    pub infinite_vault_token_account: Option<Account<'info, TokenAccount>>,

    // --- FIXED MARKET ACCOUNTS ---
    #[account(
        mut,
        seeds = [SEED_FIXED_MARKET, user_bet.market_identifier.as_bytes()],
        bump, 
        constraint = user_bet.market_type == MarketType::Fixed
    )]
    pub fixed_market: Option<Box<Account<'info, FixedMarket>>>,

    #[account(
        mut,
        seeds = [b"fixed_vault", fixed_market.as_ref().unwrap().key().as_ref()],
        bump,
        token::authority = fixed_market.as_ref().unwrap(),
    )]
    pub fixed_market_vault: Option<Account<'info, TokenAccount>>,

    pub token_program: Program<'info, Token>,
}

pub fn refund_bet(ctx: Context<RefundBet>) -> Result<()> {
    let clock = Clock::get()?;
    let user_bet = &mut ctx.accounts.user_bet;
    
    // 1. Reveal Check: You cannot refund if you already successfully revealed!
    require!(!user_bet.is_revealed, CustomError::CannotRefundRevealed);
    
    require!(clock.unix_timestamp > user_bet.end_timestamp, CustomError::SettlementTooEarly);

    // 3. Calculate Penalty (1%)
    let penalty_bps = 100u64;
    let penalty_amount = user_bet.deposit
        .checked_mul(penalty_bps).unwrap()
        .checked_div(10_000).unwrap();

    let refund_amount = user_bet.deposit.checked_sub(penalty_amount).unwrap();

    // 4. Execute Refund Transfer
    if ctx.accounts.infinite_vault.is_some() {
        // ... Infinite Market Refund Logic ...
        let vault = ctx.accounts.infinite_vault.as_mut().unwrap();
        let vault_token = ctx.accounts.infinite_vault_token_account.as_ref().unwrap();

        let seeds = &[
            SEED_VAULT,
            vault.asset_symbol.as_bytes(),
            &[vault.bump],
        ];
        let signer = &[&seeds[..]];

        // Send Refund to User
        token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: vault_token.to_account_info(),
                    to: ctx.accounts.user_token_account.to_account_info(),
                    authority: vault.to_account_info(),
                },
                signer,
            ),
            refund_amount,
        )?;

        // Send Penalty to Treasury
        if penalty_amount > 0 {
            token::transfer(
                CpiContext::new_with_signer(
                    ctx.accounts.token_program.to_account_info(),
                    Transfer {
                        from: vault_token.to_account_info(),
                        to: ctx.accounts.treasury_wallet.to_account_info(),
                        authority: vault.to_account_info(),
                    },
                    signer,
                ),
                penalty_amount,
            )?;
        }
        vault.total_assets = vault.total_assets.checked_sub(user_bet.deposit).unwrap();

    } else if ctx.accounts.fixed_market.is_some() {
        let market = ctx.accounts.fixed_market.as_mut().unwrap();
        let market_vault = ctx.accounts.fixed_market_vault.as_ref().unwrap();

        let market_key = market.key();
        let bump = market.bump;
        let seeds = &[b"fixed_vault", market_key.as_ref(), &[bump]];
        let signer = &[&seeds[..]];

        token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: market_vault.to_account_info(),
                    to: ctx.accounts.user_token_account.to_account_info(),
                    authority: market.to_account_info(),
                },
                signer,
            ),
            refund_amount,
        )?;

        if penalty_amount > 0 {
            token::transfer(
                CpiContext::new_with_signer(
                    ctx.accounts.token_program.to_account_info(),
                    Transfer {
                        from: market_vault.to_account_info(),
                        to: ctx.accounts.treasury_wallet.to_account_info(),
                        authority: market.to_account_info(),
                    },
                    signer,
                ),
                penalty_amount,
            )?;
        }
        market.vault_balance = market.vault_balance.checked_sub(user_bet.deposit).unwrap();
    } else {
        return Err(CustomError::MarketMismatch.into());
    }

    user_bet.status = BetStatus::Settled;
    msg!("Refund Complete. Refund: {}, Penalty: {}", refund_amount, penalty_amount);

    Ok(())
}