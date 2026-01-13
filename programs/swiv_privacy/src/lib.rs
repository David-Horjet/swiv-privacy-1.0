use anchor_lang::prelude::*;

pub mod constants;
pub mod errors;
pub mod events;
pub mod instructions;
pub mod state;
pub mod utils;

use instructions::*;

declare_id!("8T1csV1bwt73D3wbUANo6F2gduKa8ugVv5oqrNoM2H7b");

#[program]
pub mod swiv_privacy {
    use super::*;

    // --- ADMIN & CONFIG ---
    
    pub fn initialize_protocol(
        ctx: Context<InitializeProtocol>, 
        house_fee_bps: u64, 
        parimutuel_fee_bps: u64, 
        allowed_assets: Vec<Pubkey>
    ) -> Result<()> {
        admin::initialize_protocol(ctx, house_fee_bps, parimutuel_fee_bps, allowed_assets)
    }

    pub fn update_config(
        ctx: Context<UpdateConfig>,
        new_treasury: Option<Pubkey>,
        new_parimutuel_fee_bps: Option<u64>,
        new_house_fee_bps: Option<u64>,
        new_allowed_assets: Option<Vec<Pubkey>>,
    ) -> Result<()> {
        admin::update_config(
            ctx, 
            new_treasury, 
            new_parimutuel_fee_bps, 
            new_house_fee_bps, 
            new_allowed_assets
        )
    }

    pub fn transfer_admin(ctx: Context<TransferAdmin>, new_admin: Pubkey) -> Result<()> {
        admin::transfer_admin(ctx, new_admin)
    }

    pub fn config_asset(
        ctx: Context<ConfigAsset>,
        symbol: String,
        pyth_feed: Pubkey,
        volatility: u64,
        use_pyth_volatility: bool,
    ) -> Result<()> {
        admin::config_asset(ctx, symbol, pyth_feed, volatility, use_pyth_volatility)
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
    // --- INFINITE MARKET ---
    pub fn init_vault(ctx: Context<InitVault>, symbol: String) -> Result<()> {
        infinite_market::init_vault(ctx, symbol)
    }

    pub fn add_liquidity(ctx: Context<AddLiquidity>, amount: u64) -> Result<()> {
        infinite_market::add_liquidity(ctx, amount)
    }

    pub fn remove_liquidity(ctx: Context<RemoveLiquidity>, shares: u64) -> Result<()> {
        infinite_market::remove_liquidity(ctx, shares)
    }

    pub fn place_bet(
        ctx: Context<PlaceBet>,
        amount: u64,
        duration: i64,
        declared_multiplier_bps: u64,
        commitment: [u8; 32], 
        request_id: String,
    ) -> Result<()> {
        infinite_market::place_bet(
            ctx,
            amount,
            duration,
            declared_multiplier_bps,
            commitment, 
            request_id,
        )
    }

    // --- FIXED MARKET ---
    pub fn create_fixed_market(
        ctx: Context<CreateFixedMarket>,
        name: String,
        metadata: Option<String>,
        start_time: i64,
        end_time: i64,
        initial_liquidity: u64,
        mode: state::MarketMode,
        criterion: state::WinCriterion,
        max_accuracy_buffer: u64,
        conviction_bonus_bps: u64,
    ) -> Result<()> {
        fixed_market::create_fixed_market(
            ctx,
            name,
            metadata,
            start_time,
            end_time,
            initial_liquidity,
            mode,
            criterion,
            max_accuracy_buffer,
            conviction_bonus_bps,
        )
    }

    pub fn place_bet_fixed(
        ctx: Context<PlaceBetFixed>,
        amount: u64,
        declared_multiplier_bps: u64,
        commitment: [u8; 32], 
        request_id: String,
    ) -> Result<()> {
        fixed_market::place_bet_fixed(
            ctx,
            amount,
            declared_multiplier_bps,
            commitment,
            request_id,
        )
    }

    pub fn resolve_fixed_market(
        ctx: Context<ResolveFixedMarket>,
        final_outcome: u64,
    ) -> Result<()> {
        fixed_market::resolve_fixed_market(ctx, final_outcome)
    }

    pub fn calculate_fixed_outcome(ctx: Context<CalculateFixedOutcome>) -> Result<()> {
        fixed_market::calculate_fixed_outcome(ctx)
    }

    pub fn batch_calculate_outcome<'info>(
        ctx: Context<'_, '_, '_, 'info, BatchCalculateOutcome<'info>>
    ) -> Result<()> {
        admin::batch_calculate_outcome(ctx)
    }

    pub fn finalize_weights(ctx: Context<FinalizeWeights>) -> Result<()> {
        fixed_market::finalize_weights(ctx)
    }

    pub fn claim_fixed_reward(ctx: Context<ClaimFixedReward>) -> Result<()> {
        fixed_market::claim_fixed_reward(ctx)
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

    // NEW: Refund Bet Instruction
    pub fn refund_bet(ctx: Context<RefundBet>) -> Result<()> {
        shared::refund_bet(ctx)
    }

    // --- INFINITE MARKET SETTLEMENT ---
    pub fn settle_bet(ctx: Context<SettleBet>) -> Result<()> {
        shared::settle_bet(ctx)
    }

    pub fn batch_settle<'info>(
        ctx: Context<'_, '_, '_, 'info, BatchSettle<'info>>,
        pyth_price_proof: Vec<u8>,
    ) -> Result<()> {
        admin::batch_settle(ctx, pyth_price_proof)
    }

    pub fn emergency_refund(ctx: Context<EmergencyRefund>) -> Result<()> {
        shared::emergency_refund(ctx)
    }

}