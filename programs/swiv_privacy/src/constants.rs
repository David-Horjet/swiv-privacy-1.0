use anchor_lang::prelude::*;
use anchor_lang::pubkey;

pub const SEED_GLOBAL_CONFIG: &[u8] = b"global_config_v1";
pub const SEED_ASSET_CONFIG: &[u8] = b"asset_config";
pub const SEED_VAULT: &[u8] = b"liquidity_vault";
pub const SEED_BET: &[u8] = b"user_bet";
pub const SEED_FIXED_MARKET: &[u8] = b"fixed_market";
pub const MAX_STRATEGY_LENGTH: usize = 32;
pub const MERCY_BUFFER_DEFAULT: u64 = 500; 
pub const DISCRIMINATOR_SIZE: usize = 8;
pub const PERMISSION_PROGRAM_ID: Pubkey = pubkey!("BTWAqWNBmF2TboMh3fxMJfgR16xGHYD7Kgr2dPwbRPBi");