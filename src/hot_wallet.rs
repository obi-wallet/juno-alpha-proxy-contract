use chrono::{Datelike, NaiveDate, NaiveDateTime};
use cosmwasm_std::{Coin, Deps, StdError, StdResult, Timestamp};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    constants::MAINNET_AXLUSDC_IBC, helpers::convert_coin_to_usdc, state::SourcedCoin,
    ContractError,
};

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, JsonSchema, Debug)]
pub enum PeriodType {
    DAYS,
    MONTHS,
}

#[allow(dead_code)]
enum CheckType {
    TotalLimit,
    RemainingLimit,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, JsonSchema, Debug)]
pub struct CoinLimit {
    pub denom: String,
    pub amount: u64,
    pub limit_remaining: u64,
}

// could do hot wallets as Map or even IndexedMap, but this contract
// for more than 2-3 hot wallets at this time
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, JsonSchema, Debug)]
pub struct HotWallet {
    pub address: String,
    pub current_period_reset: u64, //seconds
    pub period_type: PeriodType,
    pub period_multiple: u16,
    pub spend_limits: Vec<CoinLimit>,
    pub usdc_denom: Option<String>,
}

impl HotWallet {
    pub fn check_is_valid(self) -> StdResult<()> {
        if self.usdc_denom != Some("true".to_string())
            || self.spend_limits.len() > 1
            || (self.spend_limits[0].denom != MAINNET_AXLUSDC_IBC
                && self.spend_limits[0].denom != *"testtokens")
        {
            Err(StdError::GenericErr { msg: "Multiple spend limits are no longer supported. Remove this wallet and re-add with a USD spend limit.".to_string() })
        } else {
            Ok(())
        }
    }

    pub fn update_spend_limit(&mut self, new_limit: CoinLimit) -> StdResult<()> {
        self.spend_limits = vec![new_limit];
        Ok(())
    }

    pub fn reset_limits(&mut self) {
        self.spend_limits[0].limit_remaining = self.spend_limits[0].amount;
    }

    pub fn reduce_limit(&mut self, deps: Deps, spend: Coin) -> Result<SourcedCoin, ContractError> {
        let converted_spend_amt = convert_coin_to_usdc(deps, spend.denom, spend.amount, false)?;
        // spend can't be bigger than total spend limit
        println!(
            "Current limit is {:?}",
            self.spend_limits[0].limit_remaining
        );
        println!("Reducing by {:?}", converted_spend_amt.coin);
        let limit_remaining = self.spend_limits[0]
            .limit_remaining
            .checked_sub(converted_spend_amt.coin.amount.u128() as u64);
        println!("new limit is {:?}", limit_remaining);
        let limit_remaining = match limit_remaining {
            Some(remaining) => remaining,
            None => {
                return Err(ContractError::CannotSpendMoreThanLimit {});
            }
        };
        self.spend_limits[0].limit_remaining = limit_remaining;
        Ok(converted_spend_amt)
    }

    // it would be great for hot wallet to also handle its own
    // period update, spend limit check, etc.
    pub fn reset_period(&mut self, current_time: Timestamp) -> Result<(), ContractError> {
        let new_dt = NaiveDateTime::from_timestamp(current_time.seconds() as i64, 0u32);
        // how far ahead we set new current_period_reset to
        // depends on the spend limit period (type and multiple)
        let new_dt: Result<NaiveDateTime, ContractError> = match self.period_type {
            PeriodType::DAYS => {
                let working_dt =
                    new_dt.checked_add_signed(chrono::Duration::days(self.period_multiple as i64));
                match working_dt {
                    Some(dt) => Ok(dt),
                    None => {
                        return Err(ContractError::DayUpdateError("unknown error".to_string()));
                    }
                }
            }
            PeriodType::MONTHS => {
                let working_month = new_dt.month() as u16 + self.period_multiple;
                match working_month {
                    2..=12 => Ok(NaiveDate::from_ymd(new_dt.year(), working_month as u32, 1)
                        .and_hms(0, 0, 0)),
                    13..=268 => {
                        let year_increment: i32 = (working_month / 12u16) as i32;
                        Ok(NaiveDate::from_ymd(
                            new_dt.year() + year_increment,
                            working_month as u32 % 12,
                            1,
                        )
                        .and_hms(0, 0, 0))
                    }
                    _ => Err(ContractError::MonthUpdateError {}),
                }
            }
        };
        self.reset_limits();
        let dt = match new_dt {
            Ok(dt) => dt,
            Err(e) => return Err(ContractError::DayUpdateError(e.to_string())),
        };
        println!("Old reset date is {:?}", self.current_period_reset.clone());
        println!("Resetting to {:?}", dt.clone().timestamp());
        self.current_period_reset = dt.timestamp() as u64;
        Ok(())
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, JsonSchema, Debug)]
pub struct HotWalletsResponse {
    pub hot_wallets: Vec<HotWallet>,
}