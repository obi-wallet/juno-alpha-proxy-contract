//use cw_multi_test::Contract;
use chrono::Datelike;
use chrono::{NaiveDate, NaiveDateTime};
use cosmwasm_std::{Addr, Coin, Deps, Timestamp, Uint128};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cw_storage_plus::Item;

use crate::constants::MAINNET_AXLUSDC_IBC;
#[allow(unused_imports)]
use crate::helpers::get_current_price;
use crate::ContractError;

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

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, JsonSchema, Debug)]
pub struct SourcedPrice {
    pub price: Uint128,
    pub contract_addr: String,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
pub struct SourcedCoin {
    pub coin: Coin,
    pub top: SourcedPrice,
    pub bottom: SourcedPrice,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
pub struct MultiSourcePrice {
    pub price: Uint128,
    pub sources: Vec<(String, String)>,
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
    pub fn reset(&mut self) {
        let mut new_limits = self.spend_limits.clone();
        for n in 0..new_limits.len() {
            new_limits[n].limit_remaining = new_limits[n].amount;
        }
        self.spend_limits = new_limits;
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, JsonSchema, Debug)]
pub struct State {
    pub admin: Addr,
    pub pending: Addr,
    pub hot_wallets: Vec<HotWallet>,
    pub uusd_fee_debt: Uint128, // waiting to pay back fees
    pub fee_lend_repay_wallet: Addr,
    pub home_network: String,
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

    pub fn check_spend_limits(
        &mut self,
        deps: Deps,
        current_time: Timestamp,
        addr: String,
        spend: Vec<Coin>,
    ) -> Result<MultiSourcePrice, ContractError> {
        if self.is_admin(addr.clone()) {
            Ok(MultiSourcePrice {
                price: Uint128::from(0u128),
                sources: vec![(
                    "no spend limit check".to_string(),
                    "caller is admin".to_string(),
                )],
            })
        } else {
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
                println!("LIMIT RESET TRIGGERED");
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
                            2..=12 => {
                                Ok(NaiveDate::from_ymd(new_dt.year(), working_month as u32, 1)
                                    .and_hms(0, 0, 0))
                            }
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
                        println!(
                            "Old reset date is {:?}",
                            new_wallet_configs[index].current_period_reset
                        );
                        println!("Resetting to {:?}", dt.timestamp());
                        new_wallet_configs[index].current_period_reset = dt.timestamp() as u64;
                        new_wallet_configs[index].reset();
                        let mut new_spend_limits = new_wallet_configs[index].spend_limits.clone();
                        self.hot_wallets = new_wallet_configs.clone();
                        let mut spend_tally: Uint128 = Uint128::from(0u128);
                        let mut spend_tally_sources: Vec<(String, String)> = vec![];
                        for n in spend {
                            let spend_check_with_sources = self.check_spend_against_limit(
                                deps,
                                CheckType::RemainingLimit,
                                &mut new_spend_limits,
                                n.clone(),
                                new_wallet_configs[index].usdc_denom.clone(),
                            )?;
                            spend_tally_sources.push((
                                format!("Price for {}", n.denom),
                                format!("{}", spend_check_with_sources.top.price),
                            ));
                            spend_tally_sources.push((
                                format!("Price for {}", n.denom),
                                format!("{}", spend_check_with_sources.bottom.price),
                            ));
                            spend_tally =
                                spend_tally.saturating_add(spend_check_with_sources.coin.amount);
                        }
                        new_wallet_configs[index].spend_limits = new_spend_limits;
                        Ok(MultiSourcePrice {
                            price: spend_tally,
                            sources: spend_tally_sources,
                        })
                    }
                    Err(e) => Err(e),
                }
            } else {
                let mut new_spend_limits = new_wallet_configs[index].spend_limits.clone();
                let mut spend_tally: Uint128 = Uint128::from(0u128);
                let mut spend_tally_sources: Vec<(String, String)> = vec![];
                for n in spend {
                    let spend_check_with_sources = self.check_spend_against_limit(
                        deps,
                        CheckType::RemainingLimit,
                        &mut new_spend_limits,
                        n.clone(),
                        new_wallet_configs[index].usdc_denom.clone(),
                    )?;
                    spend_tally_sources.push((
                        format!("Price for {}", n.denom),
                        format!("{}", spend_check_with_sources.top.price),
                    ));
                    spend_tally_sources.push((
                        format!("Price for {}", n.denom),
                        format!("{}", spend_check_with_sources.bottom.price),
                    ));
                    spend_tally = spend_tally.saturating_add(spend_check_with_sources.coin.amount);
                }
                new_wallet_configs[index].spend_limits = new_spend_limits;
                self.hot_wallets = new_wallet_configs;
                Ok(MultiSourcePrice {
                    price: spend_tally,
                    sources: spend_tally_sources,
                })
            }
        }
    }

    #[allow(unused_variables)]
    fn convert_coin_to_usdc(&self, deps: Deps, spend: Coin) -> Result<SourcedCoin, ContractError> {
        #[cfg(test)]
        return Ok(SourcedCoin {
            coin: Coin {
                denom: MAINNET_AXLUSDC_IBC.to_string(),
                amount: spend.amount.saturating_mul(Uint128::from(100u128)),
            },
            top: SourcedPrice {
                price: Uint128::from(0u128),
                contract_addr: "test".to_string(),
            },
            bottom: SourcedPrice {
                price: Uint128::from(0u128),
                contract_addr: "test".to_string(),
            },
        });
        #[cfg(not(test))]
        {
            let top = get_current_price(deps, spend.denom, spend.amount)?;
            let bottom = get_current_price(
                deps,
                MAINNET_AXLUSDC_IBC.to_string(),
                Uint128::from(1_000_000u128),
            )?;
            let top_mul = top.price.checked_mul(Uint128::from(1_000u128))?;
            let bottom_mul = bottom.price.checked_mul(Uint128::from(1_000u128))?;
            Ok(SourcedCoin {
                coin: Coin {
                    denom: MAINNET_AXLUSDC_IBC.to_string(),
                    amount: top_mul / bottom_mul,
                },
                top,
                bottom,
            })
        }
    }

    fn check_spend_against_limit(
        &self,
        deps: Deps,
        check_type: CheckType,
        new_spend_limits: &mut [CoinLimit],
        spend: Coin,
        usdc_denom: Option<String>,
    ) -> Result<SourcedCoin, ContractError> {
        let i = match usdc_denom.clone() {
            Some(setting) if setting == *"true" => new_spend_limits
                .iter()
                .position(|limit| limit.denom == MAINNET_AXLUSDC_IBC),
            _ => new_spend_limits
                .iter()
                .position(|limit| limit.denom == spend.denom),
        };
        match i {
            None => Err(ContractError::CannotSpendThisAsset(spend.denom)),
            Some(i) => {
                let converted_spend_amt = match usdc_denom {
                    Some(setting) if setting == "true" => self.convert_coin_to_usdc(deps, spend)?,
                    _ => SourcedCoin {
                        coin: spend,
                        top: SourcedPrice {
                            price: Uint128::from(0u128),
                            contract_addr: "usdc denom is disabled".to_string(),
                        },
                        bottom: SourcedPrice {
                            price: Uint128::from(0u128),
                            contract_addr: "usdc denom is disabled".to_string(),
                        },
                    },
                };
                // debug print
                println!("Converted Spend Amount is {:?}", converted_spend_amt);
                println!("against limit of {}", new_spend_limits[i].amount);
                // spend can't be bigger than total spend limit
                let limit_remaining = match check_type {
                    CheckType::TotalLimit => new_spend_limits[i]
                        .amount
                        .checked_sub(converted_spend_amt.coin.amount.u128() as u64),
                    CheckType::RemainingLimit => new_spend_limits[i]
                        .limit_remaining
                        .checked_sub(converted_spend_amt.coin.amount.u128() as u64),
                };
                println!("new limit is {:?}", limit_remaining);
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
                };
                Ok(converted_spend_amt)
            }
        }
    }
}

pub const STATE: Item<State> = Item::new("state");
