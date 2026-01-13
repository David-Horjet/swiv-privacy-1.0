use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, TokenAccount, Transfer};
use crate::state::{FixedMarket, GlobalConfig, MarketMode, WinCriterion};
use crate::constants::{SEED_GLOBAL_CONFIG, SEED_FIXED_MARKET};
use crate::errors::CustomError;
use crate::events::FixedMarketCreated;

#[derive(Accounts)]
#[instruction(
    name: String, 
    metadata: Option<String>, 
    start_time: i64, 
    end_time: i64, 
    initial_liquidity: u64,
    mode: MarketMode,
    criterion: WinCriterion,
    max_accuracy_buffer: u64,
    conviction_bonus_bps: u64 
)]
pub struct CreateFixedMarket<'info> {
    #[account(
        mut,
        seeds = [SEED_GLOBAL_CONFIG],
        bump,
        constraint = global_config.admin == admin.key() @ CustomError::Unauthorized
    )]
    pub global_config: Account<'info, GlobalConfig>,

    #[account(
        init,
        payer = admin,
        space = 200 + (4 + name.len()) + (4 + metadata.as_ref().map(|s| s.len()).unwrap_or(0)),
        seeds = [SEED_FIXED_MARKET, name.as_bytes()],
        bump
    )]
    pub fixed_market: Account<'info, FixedMarket>,

    #[account(
        init,
        payer = admin,
        seeds = [b"fixed_vault", fixed_market.key().as_ref()],
        bump,
        token::mint = token_mint,
        token::authority = fixed_market,
    )]
    pub market_vault: Account<'info, TokenAccount>,

    pub token_mint: Account<'info, token::Mint>,

    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(mut)]
    pub admin_token_account: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

pub fn create_fixed_market(
    ctx: Context<CreateFixedMarket>,
    name: String,
    metadata: Option<String>,
    start_time: i64,
    end_time: i64,
    initial_liquidity: u64,
    mode: MarketMode,
    criterion: WinCriterion,
    max_accuracy_buffer: u64,
    conviction_bonus_bps: u64,
) -> Result<()> {
    require!(end_time > start_time, CustomError::DurationTooShort);
    
    let global_config = &ctx.accounts.global_config;
    let mint_key = ctx.accounts.token_mint.key();
    
    let is_whitelisted = global_config.allowed_assets.iter().any(|&asset| asset == mint_key);
    require!(is_whitelisted, CustomError::AssetNotWhitelisted); 

    if mode == MarketMode::Parimutuel {
        require!(criterion == WinCriterion::TargetOnly, CustomError::InvalidAsset);
    }

    let market = &mut ctx.accounts.fixed_market;
    market.admin = ctx.accounts.admin.key();
    market.name = name.clone();
    market.metadata = metadata; // Save Metadata
    market.token_mint = ctx.accounts.token_mint.key();
    market.start_time = start_time;
    market.end_time = end_time;
    market.vault_balance = 0; 
    market.locked_for_payouts = 0;
    
    market.mode = mode;
    market.criterion = criterion;
    market.max_accuracy_buffer = max_accuracy_buffer;
    market.conviction_bonus_bps = conviction_bonus_bps; 
    
    market.is_resolved = false;
    market.resolution_target = 0;
    
    market.total_weight = 0;
    market.weight_finalized = false;

    market.bump = ctx.bumps.fixed_market;

    if initial_liquidity > 0 {
        token::transfer(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.admin_token_account.to_account_info(),
                    to: ctx.accounts.market_vault.to_account_info(),
                    authority: ctx.accounts.admin.to_account_info(),
                },
            ),
            initial_liquidity,
        )?;
        market.vault_balance = initial_liquidity;
    }

    emit!(FixedMarketCreated {
        market_name: name,
        start_time,
        end_time,
        initial_liquidity,
    });

    Ok(())
}