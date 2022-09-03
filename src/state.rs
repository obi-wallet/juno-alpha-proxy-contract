//use cw_multi_test::Contract;
use chrono::Datelike;
use chrono::{NaiveDate, NaiveDateTime};
use cosmwasm_std::{Coin, Deps, Timestamp, Uint128};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cw_storage_plus::Item;

#[allow(unused_imports)]
use crate::helpers::get_current_price;
use crate::ContractError;

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, JsonSchema, Debug)]
pub enum PeriodType {
    DAYS,
    MONTHS,
}

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

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug, Default)]
pub struct State {
    pub admin: String,
    pub pending: String,
    pub hot_wallets: Vec<HotWallet>,
    pub debt: Coin, // waiting to pay back fees
}

impl State {
    pub fn add_hot_wallet(&mut self, new_hot_wallet: HotWallet) {
        self.hot_wallets.push(new_hot_wallet);
    }

    pub fn rm_hot_wallet(&mut self, doomed_hot_wallet: String) {
        self.hot_wallets
            .retain(|wallet| wallet.address != doomed_hot_wallet);
    }

    /// returns true if the address is a registered admin
    pub fn is_admin(&self, addr: String) -> bool {
        let addr: &str = &addr;
        self.admin == addr
    }

    /// returns true if the address is pending to become a registered admin
    pub fn is_pending(&self, addr: String) -> bool {
        let addr: &str = &addr;
        self.pending == addr
    }

    pub fn can_spend(
        &mut self,
        deps: Deps,
        current_time: Timestamp,
        addr: String,
        spend: Vec<Coin>,
    ) -> Result<bool, ContractError> {
        let addr = &addr;
        let this_wallet_index = self.hot_wallets.iter().position(|a| &a.address == addr);
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
        if current_time.seconds() > wallet_config.current_period_reset {
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
                            deps,
                            CheckType::TotalLimit,
                            &mut new_spend_limits,
                            n.clone(),
                            new_wallet_configs[index].usdc_denom.clone(),
                        )?;
                    }
                    new_wallet_configs[index].current_period_reset = dt.timestamp() as u64;
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
                    deps,
                    CheckType::RemainingLimit,
                    &mut new_spend_limits,
                    n.clone(),
                    new_wallet_configs[index].usdc_denom.clone(),
                )?;
            }
            new_wallet_configs[index].spend_limits = new_spend_limits;
            self.hot_wallets = new_wallet_configs;
            Ok(true)
        }
    }

    // get mainnet price of JUNO in LOOP
    // junod q wasm contract-state smart juno1qc8mrs3hmxm0genzrd92akja5r0v7mfm6uuwhktvzphhz9ygkp8ssl4q07 $MAINNODE $MAINID '{"simulation":{"offer_asset":{"amount":"1000000","info":{"token":{"contract_addr":"juno1utkr0ep06rkxgsesq6uryug93daklyd6wneesmtvxjkz0xjlte9qdj2"}}}}}'

    // get mainnet price of LOOP in USDC
    // junod q wasm contract-state smart juno1utkr0ep06rkxgsesq6uryug93daklyd6wneesmtvxjkz0xjlte9qdj2s8q $MAINNODE $MAINID '{"simulation":{"offer_asset":{"amount":"1000000","info":{"native_token":{"denom":"ibc/EAC38D55372F38F1AFD68DF7FE9EF762DCF69F26520643CF3F9D292A738D8034"}}}}}'

    // a "reverse_simulation" exists as well but may not be necessary
    #[allow(unused_variables)]
    fn convert_coin_to_usdc(&self, deps: Deps, spend: Coin) -> Result<Coin, ContractError> {
        let usdc = "ibc/EAC38D55372F38F1AFD68DF7FE9EF762DCF69F26520643CF3F9D292A738D8034";
        #[cfg(test)]
        return Ok(Coin {
            denom: usdc.to_string(),
            amount: spend.amount.saturating_mul(Uint128::from(100u128)),
        });
        #[cfg(not(test))]
        Ok(Coin {
            denom: usdc.to_string(),
            amount: get_current_price(deps, spend.denom, spend.amount)?
                / get_current_price(deps, usdc.to_string(), Uint128::from(1_000_000u128))?,
        })
    }

    fn check_spend_against_limit(
        &self,
        deps: Deps,
        check_type: CheckType,
        new_spend_limits: &mut [CoinLimit],
        spend: Coin,
        usdc_denom: Option<String>,
    ) -> Result<(), ContractError> {
        let i = match usdc_denom.clone() {
            Some(setting) if setting == *"true" => {
                new_spend_limits.iter().position(|limit| {
                    limit.denom
                        == *"ibc/EAC38D55372F38F1AFD68DF7FE9EF762DCF69F26520643CF3F9D292A738D8034"
                })
            }
            _ => new_spend_limits
                .iter()
                .position(|limit| limit.denom == spend.denom),
        };
        match i {
            None => {
                return Err(ContractError::CannotSpendThisAsset(spend.denom));
            }
            Some(i) => {
                let converted_spend_amt = match usdc_denom {
                    Some(setting) if setting == "true" => self.convert_coin_to_usdc(deps, spend)?,
                    _ => spend,
                };
                // spend can't be bigger than total spend limit
                let limit_remaining = match check_type {
                    CheckType::TotalLimit => new_spend_limits[i]
                        .amount
                        .checked_sub(converted_spend_amt.amount.u128() as u64),
                    CheckType::RemainingLimit => new_spend_limits[i]
                        .limit_remaining
                        .checked_sub(converted_spend_amt.amount.u128() as u64),
                };
                let limit_remaining = match limit_remaining {
                    Some(remaining) => remaining,
                    None => {
                        return Err(ContractError::CannotSpendMoreThanLimit {});
                    }
                };
                new_spend_limits[i] = CoinLimit {
                    denom: new_spend_limits[i].denom.clone(),
                    amount: new_spend_limits[i].amount,
                    limit_remaining,
                }
            }
        }
        Ok(())
    }
}

pub const STATE: Item<State> = Item::new("state");

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveTime;
    use cosmwasm_std::testing::{mock_dependencies, mock_env};
    use cosmwasm_std::Uint128;

    #[test]
    fn is_admin() {
        let admin: &str = "bob";
        let config = State {
            admin: admin.to_string(),
            pending: admin.to_string(),
            hot_wallets: vec![],
            debt: Coin {
                amount: Uint128::from(0u128),
                denom: "ibc/EAC38D55372F38F1AFD68DF7FE9EF762DCF69F26520643CF3F9D292A738D8034"
                    .to_string(),
            },
        };

        assert!(config.is_admin(admin.to_string()));
        assert!(!config.is_admin("other".to_string()));
    }

    #[test]
    fn daily_spend_limit() {
        let deps = mock_dependencies();
        let admin: &str = "bob";
        let spender = "owner";
        let bad_spender: &str = "medusa";
        let dt = NaiveDateTime::new(
            NaiveDate::from_ymd(2022, 6, 3),
            NaiveTime::from_hms_milli(12, 00, 00, 000),
        );
        let mut now_env = mock_env();
        now_env.block.time = Timestamp::from_seconds(dt.timestamp() as u64);
        // 3 day spend limit period
        let mut config = State {
            admin: admin.to_string(),
            pending: admin.to_string(),
            hot_wallets: vec![HotWallet {
                address: spender.to_string(),
                current_period_reset: dt.timestamp() as u64,
                period_type: PeriodType::DAYS,
                period_multiple: 3,
                spend_limits: vec![
                    CoinLimit {
                        amount: 100_000_000u64,
                        denom: "ujuno".to_string(),
                        limit_remaining: 100_000_000u64,
                    },
                    CoinLimit {
                        amount: 100_000_000u64,
                        denom: "uaxlusdc".to_string(),
                        limit_remaining: 100_000_000u64,
                    },
                    CoinLimit {
                        amount: 9_000_000_000u64,
                        denom: "uloop".to_string(),
                        limit_remaining: 9_000_000_000u64,
                    },
                ],
                usdc_denom: Some("false".to_string()), // to avoid breaking local tests for now
                                                       // 100 JUNO, 100 axlUSDC, 9000 LOOP – but really only the USDC matters
                                                       // since usdc_denom is true
            }],
            debt: Coin {
                amount: Uint128::from(0u128),
                denom: "ibc/EAC38D55372F38F1AFD68DF7FE9EF762DCF69F26520643CF3F9D292A738D8034"
                    .to_string(),
            },
        };

        assert!(config
            .can_spend(
                deps.as_ref(),
                now_env.block.time,
                spender.to_string(),
                vec![Coin {
                    denom: "ujuno".to_string(),
                    amount: Uint128::from(1_000_000u128)
                }]
            )
            .unwrap());
        config
            .can_spend(
                deps.as_ref(),
                now_env.block.time,
                bad_spender.to_string(),
                vec![Coin {
                    denom: "ujuno".to_string(),
                    amount: Uint128::from(1_000_000u128),
                }],
            )
            .unwrap_err();
        config
            .can_spend(
                deps.as_ref(),
                now_env.block.time,
                spender.to_string(),
                vec![Coin {
                    denom: "ujuno".to_string(),
                    amount: Uint128::from(99_500_000u128),
                }],
            )
            .unwrap_err();

        // now we shouldn't be able to total just over our spend limit
        config
            .can_spend(
                deps.as_ref(),
                now_env.block.time,
                spender.to_string(),
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
                deps.as_ref(),
                env_future.block.time,
                spender.to_string(),
                vec![Coin {
                    denom: "ujuno".to_string(),
                    amount: Uint128::from(100_000_000u128),
                }],
            )
            .unwrap();
    }

    #[test]
    fn monthly_spend_limit() {
        let deps = mock_dependencies();
        let admin: &str = "bob";
        let spender = "owner";
        let bad_spender: &str = "medusa";
        let dt = NaiveDateTime::new(
            NaiveDate::from_ymd(2022, 6, 3),
            NaiveTime::from_hms_milli(12, 00, 00, 000),
        );
        let mut now_env = mock_env();
        now_env.block.time = Timestamp::from_seconds(dt.timestamp() as u64);

        // Let's do a 38 month spend limit period
        // and for kicks use a contract address for LOOP
        let mut config = State {
            admin: admin.to_string(),
            pending: admin.to_string(),
            hot_wallets: vec![HotWallet {
                address: spender.to_string(),
                current_period_reset: dt.timestamp() as u64,
                period_type: PeriodType::MONTHS,
                period_multiple: 38,
                spend_limits: vec![
                    CoinLimit {
                        amount: 7_000_000_000u64,
                        denom: "ujuno".to_string(),
                        limit_remaining: 100_000_000u64,
                    },
                    CoinLimit {
                        amount: 100_000_000u64,
                        denom: "uaxlusdc".to_string(),
                        limit_remaining: 100_000_000u64,
                    },
                    CoinLimit {
                        amount: 999_000_000_000u64,
                        denom: "juno1mrshruqvgctq5wah5plpe5wd97pq32f6ysc97tzxyd89gj8uxa7qcdwmnm"
                            .to_string(),
                        limit_remaining: 999_000_000_000u64,
                    },
                ],
                usdc_denom: None, // 100 JUNO, 100 axlUSDC, 9000 LOOP
            }],
            debt: Coin {
                amount: Uint128::from(0u128),
                denom: "ibc/EAC38D55372F38F1AFD68DF7FE9EF762DCF69F26520643CF3F9D292A738D8034"
                    .to_string(),
            },
        };

        assert!(config
            .can_spend(
                deps.as_ref(),
                now_env.block.time,
                spender.to_string(),
                vec![Coin {
                    denom: "juno1mrshruqvgctq5wah5plpe5wd97pq32f6ysc97tzxyd89gj8uxa7qcdwmnm"
                        .to_string(),
                    amount: Uint128::from(9_000_000_000u128)
                }]
            )
            .unwrap());
        config
            .can_spend(
                deps.as_ref(),
                now_env.block.time,
                bad_spender.to_string(),
                vec![Coin {
                    denom: "ujuno".to_string(),
                    amount: Uint128::from(1_000_000u128),
                }],
            )
            .unwrap_err();
        config
            .can_spend(
                deps.as_ref(),
                now_env.block.time,
                spender.to_string(),
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
                deps.as_ref(),
                now_env.block.time,
                spender.to_string(),
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
                deps.as_ref(),
                env_future.block.time,
                spender.to_string(),
                vec![Coin {
                    denom: "juno1mrshruqvgctq5wah5plpe5wd97pq32f6ysc97tzxyd89gj8uxa7qcdwmnm"
                        .to_string(),
                    amount: Uint128::from(990_000_000_001u128),
                }],
            )
            .unwrap();
    }
}
