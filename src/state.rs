//use cw_multi_test::Contract;
use chrono::Datelike;
use chrono::{NaiveDate, NaiveDateTime};
use cosmwasm_std::{Addr, Coin, Timestamp, Uint128};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cw_storage_plus::Item;

use crate::ContractError;

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
pub enum PeriodType {
    DAYS,
    MONTHS,
}

enum CheckType {
    TotalLimit,
    RemainingLimit,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
pub struct CoinLimit {
    coin_limit: Coin,
    limit_remaining: Uint128,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
pub struct HotWallet {
    pub address: Addr,
    pub current_period_reset: Timestamp,
    pub period_type: PeriodType,
    pub period_multiple: u16,
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
    pub fn add_hot_wallet(&mut self, new_hot_wallet: HotWallet) {
        self.hot_wallets.push(new_hot_wallet);
    }

    pub fn rm_hot_wallet(&mut self, doomed_hot_wallet: String) {
        self.hot_wallets
            .retain(|wallet| wallet.address == doomed_hot_wallet);
    }

    /// returns true if the address is a registered admin
    pub fn is_admin(&self, addr: String) -> bool {
        let addr: &str = &addr;
        self.admin == addr
    }

    pub fn can_spend(
        &mut self,
        current_time: Timestamp,
        addr: impl AsRef<str>,
        spend: Vec<Coin>,
    ) -> Result<bool, ContractError> {
        let addr = addr.as_ref();
        let this_wallet_index = self
            .hot_wallets
            .iter()
            .position(|a| a.address.as_ref() == addr);
        let index = match this_wallet_index {
            Some(index) => index,
            None => {
                return Err(ContractError::HotWalletDoesNotExist {});
            }
        };
        let wallet_config = self.hot_wallets[index].clone();
        let mut new_wallet_configs = self.hot_wallets.clone();
        // check if we should reset to full spend limit again
        // (i.e. reset time has passed)
        if current_time.seconds() > wallet_config.current_period_reset.seconds() {
            // get a current NaiveDateTime so we can easily find the next
            // reset threshold
            let new_dt = NaiveDateTime::from_timestamp(current_time.seconds() as i64, 0u32);
            // how far ahead we set new current_period_reset to
            // depends on the spend limit period (type and multiple)
            let new_dt: Result<NaiveDateTime, ContractError> = match wallet_config.period_type {
                PeriodType::DAYS => {
                    let working_dt = new_dt.checked_add_signed(chrono::Duration::days(
                        wallet_config.period_multiple as i64,
                    ));
                    match working_dt {
                        Some(dt) => Ok(dt),
                        None => {
                            return Err(ContractError::DayUpdateError {});
                        }
                    }
                }
                PeriodType::MONTHS => {
                    let working_month = new_dt.month() as u16 + wallet_config.period_multiple;
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
            match new_dt {
                Ok(dt) => {
                    let mut new_spend_limits = new_wallet_configs[index].spend_limits.clone();
                    for n in spend {
                        self.check_spend_against_limit(
                            CheckType::TotalLimit,
                            &mut new_spend_limits,
                            n.clone(),
                        )?;
                    }
                    new_wallet_configs[index].current_period_reset =
                        Timestamp::from_seconds(dt.timestamp() as u64);
                    new_wallet_configs[index].spend_limits = new_spend_limits;
                    self.hot_wallets = new_wallet_configs;
                    Ok(true)
                }
                Err(e) => Err(e),
            }
        } else {
            let mut new_spend_limits = new_wallet_configs[index].spend_limits.clone();
            for n in spend {
                self.check_spend_against_limit(
                    CheckType::RemainingLimit,
                    &mut new_spend_limits,
                    n.clone(),
                )?;
            }
            new_wallet_configs[index].spend_limits = new_spend_limits;
            self.hot_wallets = new_wallet_configs;
            Ok(true)
        }
    }

    fn check_spend_against_limit(
        &self,
        check_type: CheckType,
        new_spend_limits: &mut [CoinLimit],
        spend: Coin,
    ) -> Result<(), ContractError> {
        let i = new_spend_limits
            .iter()
            .position(|limit| limit.coin_limit.denom == spend.denom);
        match i {
            None => {
                return Err(ContractError::CannotSpendThisAsset(spend.denom));
            }
            Some(i) => {
                // spend can't be bigger than total spend limit
                let limit_remaining = match check_type {
                    CheckType::TotalLimit => new_spend_limits[i]
                        .coin_limit
                        .amount
                        .checked_sub(spend.amount),
                    CheckType::RemainingLimit => new_spend_limits[i]
                        .limit_remaining
                        .checked_sub(spend.amount),
                };
                let limit_remaining = match limit_remaining {
                    Ok(remaining) => remaining,
                    Err(_) => {
                        return Err(ContractError::CannotSpendMoreThanLimit {});
                    }
                };
                new_spend_limits[i] = CoinLimit {
                    coin_limit: Coin {
                        denom: new_spend_limits[i].coin_limit.denom.clone(),
                        amount: new_spend_limits[i].coin_limit.amount,
                    },
                    limit_remaining,
                }
            }
        }
        Ok(())
    }
}

pub const ADMINS: Item<Admins> = Item::new("admins");
pub const PENDING: Item<Admins> = Item::new("pending");

#[cfg(test)]
mod tests {
    use chrono::NaiveTime;
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

    #[test]
    fn daily_spend_limit() {
        let admin: &str = "bob";
        let spender = Addr::unchecked("owner");
        let bad_spender: &str = "medusa";
        let dt = NaiveDateTime::new(
            NaiveDate::from_ymd(2022, 6, 3),
            NaiveTime::from_hms_milli(12, 00, 00, 000),
        );
        let mut now_env = mock_env();
        now_env.block.time = Timestamp::from_seconds(dt.timestamp() as u64);
        // 3 day spend limit period
        let mut config = Admins {
            admin: admin.to_string(),
            hot_wallets: vec![HotWallet {
                address: spender.clone(),
                current_period_reset: Timestamp::from_seconds(dt.timestamp() as u64),
                period_type: PeriodType::DAYS,
                period_multiple: 3,
                spend_limits: vec![
                    CoinLimit {
                        coin_limit: Coin {
                            amount: Uint128::from(100_000_000u128),
                            denom: "ujuno".to_string(),
                        },
                        limit_remaining: Uint128::from(100_000_000u128),
                    },
                    CoinLimit {
                        coin_limit: Coin {
                            amount: Uint128::from(100_000_000u128),
                            denom: "uaxlusdc".to_string(),
                        },
                        limit_remaining: Uint128::from(100_000_000u128),
                    },
                    CoinLimit {
                        coin_limit: Coin {
                            amount: Uint128::from(9_000_000_000u128),
                            denom: "uloop".to_string(),
                        },
                        limit_remaining: Uint128::from(9_000_000_000u128),
                    },
                ], // 100 JUNO, 100 axlUSDC, 9000 LOOP
            }],
        };

        assert!(config
            .can_spend(
                now_env.block.time,
                spender.clone(),
                vec![Coin {
                    denom: "ujuno".to_string(),
                    amount: Uint128::from(1_000_000u128)
                }]
            )
            .unwrap());
        config
            .can_spend(
                now_env.block.time,
                bad_spender,
                vec![Coin {
                    denom: "ujuno".to_string(),
                    amount: Uint128::from(1_000_000u128),
                }],
            )
            .unwrap_err();
        config
            .can_spend(
                now_env.block.time,
                spender.clone(),
                vec![Coin {
                    denom: "ujuno".to_string(),
                    amount: Uint128::from(99_500_000u128),
                }],
            )
            .unwrap_err();

        // now we shouldn't be able to total just over our spend limit
        config
            .can_spend(
                now_env.block.time,
                spender.clone(),
                vec![Coin {
                    denom: "ujuno".to_string(),
                    amount: Uint128::from(99_000_001u128),
                }],
            )
            .unwrap_err();

        // but go 3 days + 1 second into the future and we should
        let mut env_future = now_env;
        env_future.block.time =
            Timestamp::from_seconds(env_future.block.time.seconds() as u64 + 259206u64);
        config
            .can_spend(
                env_future.block.time,
                spender,
                vec![Coin {
                    denom: "ujuno".to_string(),
                    amount: Uint128::from(100_000_000u128),
                }],
            )
            .unwrap();
    }

    #[test]
    fn monthly_spend_limit() {
        let admin: &str = "bob";
        let spender = Addr::unchecked("owner");
        let bad_spender: &str = "medusa";
        let dt = NaiveDateTime::new(
            NaiveDate::from_ymd(2022, 6, 3),
            NaiveTime::from_hms_milli(12, 00, 00, 000),
        );
        let mut now_env = mock_env();
        now_env.block.time = Timestamp::from_seconds(dt.timestamp() as u64);

        // Let's do a 38 month spend limit period
        // and for kicks use a contract address for LOOP
        let mut config = Admins {
            admin: admin.to_string(),
            hot_wallets: vec![HotWallet {
                address: spender.clone(),
                current_period_reset: Timestamp::from_seconds(dt.timestamp() as u64),
                period_type: PeriodType::MONTHS,
                period_multiple: 38,
                spend_limits: vec![
                    CoinLimit {
                        coin_limit: Coin {
                            amount: Uint128::from(7_000_000_000u128),
                            denom: "ujuno".to_string(),
                        },
                        limit_remaining: Uint128::from(100_000_000u128),
                    },
                    CoinLimit {
                        coin_limit: Coin {
                            amount: Uint128::from(100_000_000u128),
                            denom: "uaxlusdc".to_string(),
                        },
                        limit_remaining: Uint128::from(100_000_000u128),
                    },
                    CoinLimit {
                        coin_limit: Coin {
                            amount: Uint128::from(999_000_000_000u128),
                            denom:
                                "juno1mrshruqvgctq5wah5plpe5wd97pq32f6ysc97tzxyd89gj8uxa7qcdwmnm"
                                    .to_string(),
                        },
                        limit_remaining: Uint128::from(999_000_000_000u128),
                    },
                ], // 100 JUNO, 100 axlUSDC, 9000 LOOP
            }],
        };

        assert!(config
            .can_spend(
                now_env.block.time,
                spender.clone(),
                vec![Coin {
                    denom: "juno1mrshruqvgctq5wah5plpe5wd97pq32f6ysc97tzxyd89gj8uxa7qcdwmnm"
                        .to_string(),
                    amount: Uint128::from(9_000_000_000u128)
                }]
            )
            .unwrap());
        config
            .can_spend(
                now_env.block.time,
                bad_spender,
                vec![Coin {
                    denom: "ujuno".to_string(),
                    amount: Uint128::from(1_000_000u128),
                }],
            )
            .unwrap_err();
        config
            .can_spend(
                now_env.block.time,
                spender.clone(),
                vec![Coin {
                    denom: "juno1mrshruqvgctq5wah5plpe5wd97pq32f6ysc97tzxyd89gj8uxa7qcdwmnm"
                        .to_string(),
                    amount: Uint128::from(999_000_000_000u128),
                }],
            )
            .unwrap_err();

        // now we shouldn't be able to total just over our spend limit
        config
            .can_spend(
                now_env.block.time,
                spender.clone(),
                vec![Coin {
                    denom: "juno1mrshruqvgctq5wah5plpe5wd97pq32f6ysc97tzxyd89gj8uxa7qcdwmnm"
                        .to_string(),
                    amount: Uint128::from(990_000_000_001u128),
                }],
            )
            .unwrap_err();

        // but go 38 months (minus a couple of days - reset is the 1st, not the 3rd)
        // into the future and we should be able to spend
        let dt = NaiveDateTime::new(
            NaiveDate::from_ymd(2025, 8, 1),
            NaiveTime::from_hms_milli(12, 00, 00, 000),
        );
        let mut env_future = mock_env();
        env_future.block.time = Timestamp::from_seconds(dt.timestamp() as u64);
        config
            .can_spend(
                env_future.block.time,
                spender,
                vec![Coin {
                    denom: "juno1mrshruqvgctq5wah5plpe5wd97pq32f6ysc97tzxyd89gj8uxa7qcdwmnm"
                        .to_string(),
                    amount: Uint128::from(990_000_000_001u128),
                }],
            )
            .unwrap();
    }
}
