use anchor_lang::prelude::*;

pub mod constants;
pub mod errors;
pub mod events;
pub mod instructions;
pub mod state;
pub mod utils;

use instructions::*;

declare_id!("3RpaT6ZyhUPzbARNFMvPycvdWBv2ixBe6MiggSAGuBx7");

#[program]
pub mod swiv_privacy {
    use super::*;

    // --- ADMIN & CONFIG ---
    
    pub fn initialize_protocol(
        ctx: Context<InitializeProtocol>, 
        protocol_fee_bps: u64
    ) -> Result<()> {
        admin::initialize_protocol(ctx, protocol_fee_bps)
    }

    pub fn update_config(
        ctx: Context<UpdateConfig>,
        new_treasury: Option<Pubkey>,
        new_protocol_fee_bps: Option<u64>,
    ) -> Result<()> {
        admin::update_config(
            ctx, 
            new_treasury, 
            new_protocol_fee_bps
        )
    }

    pub fn transfer_admin(ctx: Context<TransferAdmin>, new_admin: Pubkey) -> Result<()> {
        admin::transfer_admin(ctx, new_admin)
    }

    pub fn set_pause(ctx: Context<SetPause>, paused: bool) -> Result<()> {
        admin::set_pause(ctx, paused)
    }

   pub fn delegate_bet(ctx: Context<DelegateBet>, request_id: String) -> Result<()> {
        instructions::delegation::delegate_bet(ctx, request_id)
    }

    pub fn undelegate_bet(ctx: Context<UndelegateBet>, request_id: String) -> Result<()> {
        instructions::delegation::undelegate_bet(ctx, request_id)
    }
    // --- POOL (Parimutuel) ---
    pub fn create_pool(
        ctx: Context<CreatePool>,
        name: String,
        start_time: i64,
        end_time: i64,
        max_accuracy_buffer: u64,
        conviction_bonus_bps: u64,
    ) -> Result<()> {
        pool::create_pool(ctx, name, start_time, end_time, max_accuracy_buffer, conviction_bonus_bps)
    }

    pub fn place_bet(
        ctx: Context<PlaceBet>,
        amount: u64,
        commitment: [u8; 32], 
        request_id: String,
    ) -> Result<()> {
        pool::place_bet(ctx, amount, commitment, request_id)
    }

    pub fn resolve_pool(
        ctx: Context<ResolvePool>,
        final_outcome: u64,
    ) -> Result<()> {
        pool::resolve_pool(ctx, final_outcome)
    }

    pub fn calculate_pool_outcome(ctx: Context<CalculatePoolOutcome>) -> Result<()> {
        pool::calculate_pool_outcome(ctx)
    }

    pub fn finalize_weights(ctx: Context<FinalizeWeights>) -> Result<()> {
        pool::finalize_weights(ctx)
    }

    pub fn claim_pool_reward(ctx: Context<ClaimPoolReward>) -> Result<()> {
        pool::claim_pool_reward(ctx)
    }

    // --- SHARED ---
    pub fn update_bet(
        ctx: Context<UpdateBet>,
        new_prediction_low: u64,
        new_prediction_high: u64,
        new_prediction_target: u64,
    ) -> Result<()> {
        shared::update_bet(
            ctx,
            new_prediction_low,
            new_prediction_high,
            new_prediction_target,
        )
    }
    
    // NEW: Reveal Bet Instruction
    pub fn reveal_bet(
        ctx: Context<RevealBet>,
        prediction_low: u64,
        prediction_high: u64,
        prediction_target: u64,
        salt: [u8; 32],
    ) -> Result<()> {
        shared::reveal_bet(
            ctx,
            prediction_low,
            prediction_high,
            prediction_target,
            salt
        )
    }

}