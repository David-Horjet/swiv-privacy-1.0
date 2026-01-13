use anchor_lang::prelude::*;
use pyth_solana_receiver_sdk::price_update::{PriceUpdateV2};
use crate::errors::CustomError;

pub struct PythPriceInfo {
    pub price: u64,
    pub conf: u64,
}

pub fn get_pyth_price_and_conf(
    price_update: &Account<PriceUpdateV2>,
    feed_id_pubkey: &Pubkey,
) -> Result<PythPriceInfo> {
    let feed_id = feed_id_pubkey.to_bytes();
    let maximum_age: u64 = 60;

    let price = price_update.get_price_no_older_than(
        &Clock::get()?,
        maximum_age,
        &feed_id,
    )?;

    require!(price.price > 0, CustomError::InvalidOraclePrice);
    
    let target_decimals: i32 = 6;
    let exponent_diff = target_decimals - price.exponent;

    let final_price = if exponent_diff >= 0 {
        (price.price as u64) * 10u64.pow(exponent_diff as u32)
    } else {
        (price.price as u64) / 10u64.pow((-exponent_diff) as u32)
    };
    
    let final_conf = if exponent_diff >= 0 {
        price.conf * 10u64.pow(exponent_diff as u32)
    } else {
        price.conf / 10u64.pow((-exponent_diff) as u32)
    };

    Ok(PythPriceInfo { price: final_price, conf: final_conf })
}


pub fn get_pyth_price(
    price_update: &Account<PriceUpdateV2>,
    feed_id_pubkey: &Pubkey, 
    _target_time: i64, 
) -> Result<u64> {
    let price_info = get_pyth_price_and_conf(price_update, feed_id_pubkey)?;
    Ok(price_info.price)
}