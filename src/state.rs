//use cw_multi_test::Contract;
use chrono::Datelike;
use chrono::{NaiveDate, NaiveDateTime};
use cosmwasm_std::{
    to_binary, Addr, Coin, Deps, QueryRequest, StdError, StdResult, Timestamp, Uint128, WasmQuery,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cw_storage_plus::Item;

use crate::constants::{MAINNET_AXLUSDC_IBC, MAINNET_ID, TESTNET_ID};
#[cfg(test)]
use crate::constants_tests::get_test_sourced_swap;
use crate::defaults::{
    get_local_pair_contracts, get_mainnet_pair_contracts, get_testnet_pair_contracts,
};
use crate::helpers::convert_coin_to_usdc;
#[allow(unused_imports)]
use crate::helpers::{simulate_reverse_swap, simulate_swap};
use crate::msg::{
    Asset, AssetInfo, DexQueryMsg, ReverseSimulationMsg, SimulationMsg, Tallyable,
    Token1ForToken2Msg, Token2ForToken1Msg,
};
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
    pub sources: Vec<SourcedSwap>,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
pub struct SourcedSwap {
    pub coin: Coin,
    pub contract_addr: String,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, JsonSchema, Debug)]
pub struct MultiSourcePrice {
    pub price: Uint128,
    pub sources: Vec<(String, String)>,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, JsonSchema, Debug)]
pub enum PairMessageType {
    LoopType,
    JunoType,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, JsonSchema, Debug)]
pub struct PairContract {
    pub contract_addr: String,
    pub denom1: String,
    pub denom2: String,
    pub query_format: PairMessageType,
}

impl PairContract {
    pub fn get_denoms(&self) -> Result<(String, String), ContractError> {
        Ok((self.denom1.clone(), self.denom2.clone()))
    }

    #[allow(unreachable_code)]
    #[allow(unused_variables)]
    pub fn query_contract<T>(
        self,
        deps: Deps,
        amount: Uint128,
        reverse: bool,
        amount_is_target: bool,
    ) -> Result<SourcedSwap, ContractError>
    where
        T: for<'de> Deserialize<'de>,
        T: Tallyable,
    {
        let query_msg: DexQueryMsg = match self.query_format {
            PairMessageType::LoopType => {
                let simulation_asset = Asset {
                    amount,
                    info: AssetInfo::NativeToken {
                        denom: self.denom1.clone(),
                    },
                };
                match amount_is_target {
                    false => DexQueryMsg::Simulation(SimulationMsg {
                        offer_asset: simulation_asset,
                    }),
                    true => {
                        DexQueryMsg::ReverseSimulation(ReverseSimulationMsg {
                            ask_asset: simulation_asset,
                        }) // no cw20 support yet (expect for the base asset)
                    }
                }
            }
            PairMessageType::JunoType => {
                let mut flip_assets: bool = reverse;
                if amount_is_target {
                    flip_assets = !flip_assets;
                }
                match flip_assets {
                    false => DexQueryMsg::Token1ForToken2Price(Token1ForToken2Msg {
                        token1_amount: amount,
                    }),
                    true => DexQueryMsg::Token2ForToken1Price(Token2ForToken1Msg {
                        token2_amount: amount,
                    }), // no cw20 support yet (expect for the base asset)
                }
            }
        };
        let response_asset = self.denom2;
        #[cfg(test)]
        println!(
            "Bypassing query message on contract {}: {:?}",
            self.contract_addr.clone(),
            query_msg.clone()
        );
        #[cfg(test)]
        return Ok(get_test_sourced_swap(
            (self.denom1, response_asset.clone()),
            amount,
            reverse,
        ));
        let query_response: Result<T, StdError> =
            deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
                contract_addr: self.contract_addr.clone(),
                msg: to_binary(&query_msg)?,
            }));
        match query_response {
            Ok(res) => Ok(SourcedSwap {
                coin: Coin {
                    denom: response_asset,
                    amount: (res.tally()),
                },
                contract_addr: self.contract_addr,
            }),
            Err(e) => Err(ContractError::PriceCheckFailed(
                format!("{:?}", to_binary(&query_msg)?),
                e.to_string(),
            )),
        }
    }
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
    pub fn reset_period(
        &mut self,
        current_time: Timestamp,
    ) -> Result<NaiveDateTime, ContractError> {
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
                        return Err(ContractError::DayUpdateError {});
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
        new_dt
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
    pub pair_contracts: Vec<PairContract>,
}

impl State {
    pub fn get_pair_contract(
        &self,
        denoms: (String, String),
    ) -> Result<(PairContract, bool), ContractError> {
        for n in 0..self.pair_contracts.len() {
            if self.pair_contracts[n].denom1 == denoms.0
                && self.pair_contracts[n].denom2 == denoms.1
            {
                return Ok((self.pair_contracts[n].clone(), false));
            } else if self.pair_contracts[n].denom2 == denoms.0
                && self.pair_contracts[n].denom1 == denoms.1
            {
                return Ok((self.pair_contracts[n].clone(), true));
            }
        }
        Err(ContractError::PairContractNotFound(
            denoms.0,
            denoms.1,
            self.pair_contracts.clone(),
        ))
    }

    pub fn set_pair_contracts(&mut self, network: String) -> Result<(), StdError> {
        match network {
            val if val == MAINNET_ID => {
                self.pair_contracts = get_mainnet_pair_contracts().to_vec();
                Ok(())
            }
            val if val == TESTNET_ID => {
                self.pair_contracts = get_testnet_pair_contracts().to_vec();
                Ok(())
            }
            val if val == "local".to_string() => {
                self.pair_contracts = get_local_pair_contracts().to_vec();
                Ok(())
            }
            _ => Err(StdError::GenericErr {
                msg: "Failed to init pair contracts; unsupported chain id".to_string(),
            }),
        }
    }

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
            let this_wallet_opt: Option<&mut HotWallet> =
                self.hot_wallets.iter_mut().find(|a| &a.address == addr);
            if this_wallet_opt == None {
                return Err(ContractError::HotWalletDoesNotExist {});
            }
            let this_wallet = this_wallet_opt.unwrap();

            // check if we should reset to full spend limit again
            // (i.e. reset time has passed)
            if current_time.seconds() > this_wallet.current_period_reset {
                println!("LIMIT RESET TRIGGERED");
                // get a current NaiveDateTime so we can easily find the next
                // reset threshold
                let new_dt = this_wallet.reset_period(current_time);
                match new_dt {
                    Ok(dt) => {
                        println!(
                            "Old reset date is {:?}",
                            this_wallet.current_period_reset.clone()
                        );
                        println!("Resetting to {:?}", dt.timestamp());
                        let mut spend_tally: Uint128 = Uint128::from(0u128);
                        let mut spend_tally_sources: Vec<(String, String)> = vec![];
                        for n in spend {
                            let spend_check_with_sources =
                                this_wallet.reduce_limit(deps, n.clone())?;
                            for m in 0..spend_check_with_sources.sources.len() {
                                spend_tally_sources.push((
                                    format!("Price for {}", n.denom),
                                    format!("{}", spend_check_with_sources.sources[m].coin.amount),
                                ));
                            }
                            spend_tally =
                                spend_tally.saturating_add(spend_check_with_sources.coin.amount);
                        }
                        Ok(MultiSourcePrice {
                            price: spend_tally,
                            sources: spend_tally_sources,
                        })
                    }
                    Err(e) => Err(e),
                }
            } else {
                let mut spend_tally: Uint128 = Uint128::from(0u128);
                let mut spend_tally_sources: Vec<(String, String)> = vec![];
                for n in spend {
                    let spend_check_with_sources = this_wallet.reduce_limit(deps, n.clone())?;
                    for m in 0..spend_check_with_sources.sources.len() {
                        spend_tally_sources.push((
                            format!("Price for {}", n.denom),
                            format!("{}", spend_check_with_sources.sources[m].coin.amount),
                        ));
                    }
                    spend_tally = spend_tally.saturating_add(spend_check_with_sources.coin.amount);
                }
                Ok(MultiSourcePrice {
                    price: spend_tally,
                    sources: spend_tally_sources,
                })
            }
        }
    }
}

pub const STATE: Item<State> = Item::new("state");
