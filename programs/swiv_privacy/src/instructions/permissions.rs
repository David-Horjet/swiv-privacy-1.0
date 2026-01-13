use anchor_lang::prelude::*;
use ephemeral_rollups_sdk::access_control::{
    CreateGroupCpiBuilder, CreatePermissionCpiBuilder
};
use crate::state::UserBet;
use crate::errors::CustomError;

#[derive(Accounts)]
pub struct SetupBetPermissions<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    /// The bet account we want to protect inside the TEE
    #[account(
        mut, 
        constraint = user_bet.owner == user.key() @ CustomError::Unauthorized
    )]
    pub user_bet: Account<'info, UserBet>,

    /// The Permission Group PDA (derived from seeds)
    /// CHECK: Checked via CPI to MagicBlock program
    #[account(mut)]
    pub group: UncheckedAccount<'info>,

    /// The Permission PDA
    /// CHECK: Checked via CPI to MagicBlock program
    #[account(mut)]
    pub permission: UncheckedAccount<'info>,

    /// The MagicBlock Access Control Program
    /// CHECK: Checked via CPI
    pub permission_program: UncheckedAccount<'info>,
    
    pub system_program: Program<'info, System>,
}

pub fn setup_bet_permissions(ctx: Context<SetupBetPermissions>) -> Result<()> {
    // 1. Define references for clarity
    let permission_program = &ctx.accounts.permission_program;
    let group = &ctx.accounts.group;
    let user = &ctx.accounts.user;
    let system_program = &ctx.accounts.system_program;
    let user_bet = &ctx.accounts.user_bet;
    let permission = &ctx.accounts.permission;

    // 2. Create the Group
    // We use the UserBet key as the ID for the group
    let group_id = user_bet.key();
    
    // FIX: Use CpiBuilder pattern
    CreateGroupCpiBuilder::new(permission_program.to_account_info().as_ref())
        .group(group.to_account_info().as_ref())
        .id(group_id)
        .payer(user.to_account_info().as_ref())
        .system_program(system_program.to_account_info().as_ref())
        .invoke()?;

    CreatePermissionCpiBuilder::new(permission_program.to_account_info().as_ref())
        .permission(permission.to_account_info().as_ref())
        .group(group.to_account_info().as_ref())
        .delegated_account(user_bet.to_account_info().as_ref()) // SDK uses 'delegated_account'
        .payer(user.to_account_info().as_ref())
        .system_program(system_program.to_account_info().as_ref())
        .invoke()?;

    msg!("Permissions initialized for Bet: {}", ctx.accounts.user_bet.key());

    Ok(())
}