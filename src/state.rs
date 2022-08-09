use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use cosmwasm_std::{Addr, Coin, Timestamp, Env, Uint128};
use chrono::{NaiveDate, NaiveTime, NaiveDateTime};
use chrono::{Datelike, Timelike, Weekday};
use std::str::FromStr;
use std::time::Duration;

use cw_storage_plus::Item;

use crate::ContractError;

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
pub enum PeriodType {
    DAILY,
    WEEKLY,
    MONTHLY,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
pub struct CoinLimit{
    coin_limit: Coin,
    limit_remaining: Uint128,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
pub struct HotWallet{
    pub address: Addr,
    pub current_period_reset: Timestamp,
    pub period_type: PeriodType,
    pub spend_limits: Vec<CoinLimit>,
}

/* let ts = Timestamp::from_nanos(1_000_000_202);
assert_eq!(ts.nanos(), 1_000_000_202);
assert_eq!(ts.seconds(), 1);
assert_eq!(ts.subsec_nanos(), 202);

let ts = ts.plus_seconds(2);
assert_eq!(ts.nanos(), 3_000_000_202);
assert_eq!(ts.seconds(), 3); */

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug, Default)]
pub struct Admins {
    pub admin: String,
    pub hot_wallets: Vec<HotWallet>,
}

impl Admins {
    /// returns true if the address is a registered admin
    pub fn is_admin(&self, addr: String) -> bool {
        let addr: &str = &addr;
        self.admin == addr
    }

    pub fn can_spend(&self, env: Env, addr: impl AsRef<str>, spend: Coin) -> Result<bool, ContractError> {
        let addr = addr.as_ref();
        let this_wallet =
            self.hot_wallets.iter().find(|a| a.address.as_ref() == addr);
        match this_wallet {
            Some(wallet) => {
                // current block time
                let current_time: Timestamp = env.block.time;
                // check if we should reset to full spend limit again
                // (i.e. reset time has passed)
                if current_time.seconds() > wallet.current_period_reset.seconds() {
                    // get a current NaiveDateTime so we can easily find the next
                    // reset threshold
                    let new_dt = NaiveDateTime::from_timestamp(current_time.seconds() as i64, 0u32);
                    // how far ahead we set new current_period_reset to
                    // depends on the spend limit duration. For now,
                    // this is limited to daily monthly weekly
                    let new_dt: Result<NaiveDateTime, ContractError> = match wallet.period_type {
                        PeriodType::DAILY => {
                            let working_dt = new_dt.checked_add_signed(chrono::Duration::days(1));
                            match working_dt {
                                Some(dt) => Ok(dt),
                                None => { Err(ContractError::Unauthorized {  }) }
                            }
                        },
                        PeriodType::WEEKLY => {
                            let working_dt = new_dt.checked_add_signed(chrono::Duration::weeks(1));
                            match working_dt {
                                Some(dt) => Ok(dt),
                                None => { Err(ContractError::Unauthorized {  }) }
                            }
                        },
                        PeriodType::MONTHLY => {
                            let working_month = new_dt.month() + 1;
                            match working_month {
                                2..=12 => { Ok(NaiveDate::from_ymd(new_dt.year(), working_month, 1).and_hms(0, 0, 0)) },
                                13 => { Ok(NaiveDate::from_ymd(new_dt.year() + 1, 1, 1).and_hms(0, 0, 0)) },
                                _ => { Err(ContractError::Unauthorized {  }) }
                            }
                        },
                    };
                    Ok(true)
                } else {
                    Ok(true)
                }
            },
            None => Ok(false),
        }
    }

}

pub const ADMINS: Item<Admins> = Item::new("admins");
pub const PENDING: Item<Admins> = Item::new("pending");

#[cfg(test)]
mod tests {
    use cosmwasm_std::testing::mock_env;

    use super::*;

    #[test]
    fn is_admin() {
        let admin: &str = "bob";
        let config = Admins {
            admin: admin.to_string(),
            hot_wallets: vec![],
        };

        assert!(config.is_admin(admin.to_string()));
        assert!(!config.is_admin("other".to_string()));
    }

    fn can_spend() {
        let admin: &str = "bob";
        let spender = Addr::unchecked("owner");
        let bad_spender: &str = "medusa";
        let dt = NaiveDateTime::new(
            NaiveDate::from_ymd(2022, 6, 3),
            NaiveTime::from_hms_milli(12,00,00,000)
        );
        let config = Admins {
            admin: admin.to_string(),
            hot_wallets: vec![HotWallet {
                address: spender.clone(),
                current_period_reset: Timestamp::from_seconds(
                    dt.timestamp() as u64
                ),
                period_type: PeriodType::DAILY,
                spend_limits: vec![
                    CoinLimit{
                        coin_limit: Coin{
                            amount: Uint128::from(100_000_000u128),
                            denom: "ujuno".to_string(),
                        },
                        limit_remaining: Uint128::from(100_000_000u128),
                    },
                    CoinLimit{
                        coin_limit: Coin{
                            amount: Uint128::from(100_000_000u128),
                            denom: "uaxlusdc".to_string(),
                        },
                        limit_remaining: Uint128::from(100_000_000u128),
                    },
                    CoinLimit{
                        coin_limit: Coin{
                            amount: Uint128::from(9_000_000_000u128),
                            denom: "uloop".to_string(),
                        },
                        limit_remaining: Uint128::from(9_000_000_000u128),
                    },
                ], // 100 JUNO, 100 axlUSDC, 9000 LOOP
            }],
        };

        assert!(config.can_spend(mock_env(), spender, Coin { denom: "ujuno".to_string(), amount: Uint128::from(1_000_000u128) }).unwrap());
    }
}
