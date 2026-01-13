use anchor_lang::prelude::*;
use crate::state::GlobalConfig;
use crate::constants::SEED_GLOBAL_CONFIG;
use crate::errors::CustomError;

#[derive(Accounts)]
#[instruction(
    new_treasury: Option<Pubkey>, 
    new_parimutuel_fee_bps: Option<u64>, 
    new_house_fee_bps: Option<u64>, 
    new_allowed_assets: Option<Vec<Pubkey>>
)]
pub struct UpdateConfig<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        mut,
        seeds = [SEED_GLOBAL_CONFIG],
        bump,
        constraint = global_config.admin == admin.key() @ CustomError::Unauthorized,
        realloc = GlobalConfig::BASE_LEN + (32 * new_allowed_assets.as_ref().map_or(global_config.allowed_assets.len(), |v| v.len())),
        realloc::payer = admin,
        realloc::zero = false
    )]
    pub global_config: Account<'info, GlobalConfig>,

    pub system_program: Program<'info, System>,
}

pub fn update_config(
    ctx: Context<UpdateConfig>,
    new_treasury: Option<Pubkey>,
    new_parimutuel_fee_bps: Option<u64>,
    new_house_fee_bps: Option<u64>,
    new_allowed_assets: Option<Vec<Pubkey>>,
) -> Result<()> {
    let global_config = &mut ctx.accounts.global_config;

    if let Some(treasury) = new_treasury {
        global_config.treasury_wallet = treasury;
    }

    if let Some(pari_fee) = new_parimutuel_fee_bps {
        global_config.parimutuel_fee_bps = pari_fee;
    }

    if let Some(house_fee) = new_house_fee_bps {
        global_config.house_fee_bps = house_fee;
    }

    if let Some(assets) = new_allowed_assets {
        global_config.allowed_assets = assets;
    }

    msg!("Global Config Updated");

    Ok(())
}