use anchor_lang::prelude::*;
use anchor_lang::pubkey;

pub const SEED_GLOBAL_CONFIG: &[u8] = b"global_config_v1";
pub const SEED_POOL: &[u8] = b"pool";
pub const SEED_BET: &[u8] = b"user_bet";
pub const SEED_FIXED_MARKET: &[u8] = b"fixed_market"; // legacy
pub const MAX_STRATEGY_LENGTH: usize = 32;
pub const MERCY_BUFFER_DEFAULT: u64 = 500; 
pub const DISCRIMINATOR_SIZE: usize = 8;
pub const PERMISSION_PROGRAM_ID: Pubkey = pubkey!("BTWAqWNBmF2TboMh3fxMJfgR16xGHYD7Kgr2dPwbRPBi");