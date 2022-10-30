use std::convert::TryInto;

use chrono::{Datelike, NaiveDate, NaiveDateTime};
use cosmwasm_std::{Coin, Deps, StdError, StdResult, Timestamp, Uint128};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    authorizations::Authorization, constants::MAINNET_AXLUSDC_IBC, pair_contract::PairContracts,
    sourced_coin::SourcedCoin, sources::Sources, ContractError,
};

/// The `PeriodType` type is used for recurring components, including spend limits.
/// Multiples of `DAYS` and `MONTHS` allow for weekly and yearly recurrence.
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

/// The `CoinLimit` type is a practically extended `Coin` type
/// that includes a remaining limit.
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, JsonSchema, Debug)]
pub struct CoinLimit {
    pub denom: String,
    pub amount: u64,
    pub limit_remaining: u64,
}

/// The `PermissionedAddress` type allows addresses to trigger actions by this contract
/// under certain conditions. The addresses may or may not be signers: some
/// possible other use cases include dependents, employees or contractors,
/// wealth managers, single-purpose addresses used by a service somewhere,
/// subscriptions or recurring payments, etc.
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, JsonSchema, Debug)]
pub struct PermissionedAddress {
    params: PermissionedAddressParams,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, JsonSchema, Debug)]
pub struct PermissionedAddressParams {
    pub address: String,
    pub current_period_reset: u64, //seconds
    pub period_type: PeriodType,
    pub period_multiple: u16,
    pub spend_limits: Vec<CoinLimit>,
    pub usdc_denom: Option<String>,
    pub default: Option<bool>,
    pub authorizations: Option<Vec<Authorization>>,
}

impl PermissionedAddressParams {
    /// Checks that `self` is valid, meaning that there is only one spend limit and
    /// the limit is in USDC. Previous versions had multiple spend limits; those permissioned
    /// addresses still work but cannot be created this way. This will be expanded to be
    /// more customizable later.
    pub fn assert_is_valid(&self) -> StdResult<()> {
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
}

impl PermissionedAddress {
    pub fn new(params: PermissionedAddressParams) -> Self {
        Self { params }
    }
}

// simple getters
impl PermissionedAddress {
    pub fn address(&self) -> String {
        self.params.address.clone()
    }

    pub fn get_params(&self) -> PermissionedAddressParams {
        self.params.clone()
    }
}

// spending limit time period reset handlers
impl PermissionedAddress {
    /// Checks whether the `current_time` is past the `current_period_reset` for
    /// this `PermissionedAddress`, which means that the remaining limit CAN be reset to full.
    /// This function does not actually process the reset; use reset_period()
    ///
    /// # Arguments
    ///
    /// * `current_time` - a Timestamp of the current time (or simulated reset time).
    /// Usually `env.block.time`
    pub fn should_reset(&self, current_time: Timestamp) -> bool {
        current_time.seconds() > self.params.current_period_reset
    }

    /// Sets a new reset time for spending limit for this wallet. This also
    /// resets the limit directly by calling self.reset_limits().
    pub fn reset_period(&mut self, current_time: Timestamp) -> Result<(), ContractError> {
        let new_dt = NaiveDateTime::from_timestamp(current_time.seconds() as i64, 0u32);
        // how far ahead we set new current_period_reset to
        // depends on the spend limit period (type and multiple)
        let new_dt: Result<NaiveDateTime, ContractError> = match self.params.period_type {
            PeriodType::DAYS => {
                let working_dt = new_dt
                    .checked_add_signed(chrono::Duration::days(self.params.period_multiple as i64));
                match working_dt {
                    Some(dt) => Ok(dt),
                    None => {
                        return Err(ContractError::DayUpdateError("unknown error".to_string()));
                    }
                }
            }
            PeriodType::MONTHS => {
                let working_month = new_dt.month() as u16 + self.params.period_multiple;
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

        self.params.current_period_reset = dt.timestamp() as u64;
        Ok(())
    }
}

// handlers for modifying spend limits (not reset times)
impl PermissionedAddress {
    /// Replaces this wallet's current spending limit. Since only single USDC
    /// limit is currently supported, all limits are replaced.
    pub fn update_spend_limit(&mut self, new_limit: CoinLimit) -> StdResult<()> {
        self.params.spend_limits = vec![new_limit];
        Ok(())
    }

    pub fn reset_limits(&mut self) {
        self.params.spend_limits[0].limit_remaining = self.params.spend_limits[0].amount;
    }

    pub fn simulate_reduce_limit(
        &self,
        deps: Deps,
        pair_contracts: PairContracts,
        spend: Coin,
        reset: bool,
    ) -> Result<(u64, SourcedCoin), ContractError> {
        let unconverted_coin = SourcedCoin {
            coin: spend,
            wrapped_sources: Sources { sources: vec![] },
        };
        let converted_spend_amt =
            unconverted_coin.get_converted_to_usdc(deps, pair_contracts, false)?;
        // spend can't be bigger than total spend limit
        let limit_to_check = match reset {
            false => self.params.spend_limits[0].limit_remaining,
            true => self.params.spend_limits[0].amount,
        };
        let limit_remaining = limit_to_check
            .checked_sub(converted_spend_amt.coin.amount.u128() as u64)
            .ok_or_else(|| {
                ContractError::CannotSpendMoreThanLimit(
                    converted_spend_amt.coin.amount.to_string(),
                    converted_spend_amt.coin.denom.clone(),
                )
            })?;
        Ok((limit_remaining, converted_spend_amt))
    }

    pub fn make_usdc_sourced_coin(&self, amount: Uint128, wrapped_sources: Sources) -> SourcedCoin {
        SourcedCoin {
            coin: Coin {
                amount,
                denom: MAINNET_AXLUSDC_IBC.to_string(),
            },
            wrapped_sources,
        }
    }

    pub fn check_spend_vec(
        &self,
        deps: Deps,
        pair_contracts: PairContracts,
        spend_vec: Vec<Coin>,
        should_reset: bool,
    ) -> Result<SourcedCoin, ContractError> {
        let mut spend_tally = Uint128::from(0u128);
        let mut spend_tally_sources: Sources = Sources { sources: vec![] };

        for n in spend_vec {
            let spend_check_with_sources = self
                .simulate_reduce_limit(deps, pair_contracts.clone(), n.clone(), should_reset)?
                .1;
            spend_tally_sources.append_sources(spend_check_with_sources.clone());
            spend_tally = spend_tally.saturating_add(spend_check_with_sources.coin.amount);
        }
        Ok(self.make_usdc_sourced_coin(spend_tally, spend_tally_sources))
    }

    pub fn process_spend_vec(
        &mut self,
        deps: Deps,
        pair_contracts: PairContracts,
        spend_vec: Vec<Coin>,
    ) -> Result<SourcedCoin, ContractError> {
        let mut spend_tally = Uint128::from(0u128);
        let mut spend_tally_sources: Sources = Sources { sources: vec![] };

        for n in spend_vec {
            let spend_check_with_sources =
                self.reduce_limit(deps, pair_contracts.clone(), n.clone())?;
            spend_tally_sources.append_sources(spend_check_with_sources.clone());
            spend_tally = spend_tally.saturating_add(spend_check_with_sources.coin.amount);
        }
        Ok(self.make_usdc_sourced_coin(spend_tally, spend_tally_sources))
    }

    pub fn reduce_limit(
        &mut self,
        deps: Deps,
        pair_contracts: PairContracts,
        spend: Coin,
    ) -> Result<SourcedCoin, ContractError> {
        let spend_limit_reduction: (u64, SourcedCoin) =
            self.simulate_reduce_limit(deps, pair_contracts, spend, false)?;
        self.params.spend_limits[0].limit_remaining = spend_limit_reduction.0;
        Ok(spend_limit_reduction.1)
    }

    pub fn reduce_limit_direct(&mut self, limit_reduction: Coin) -> Result<(), ContractError> {
        match self.params.spend_limits[0]
            .limit_remaining
            .checked_sub(limit_reduction.amount.u128().try_into()?)
        {
            Some(val) => {
                self.params.spend_limits[0].limit_remaining = val;
                Ok(())
            }
            None => Err(ContractError::CannotSpendMoreThanLimit(
                limit_reduction.denom,
                limit_reduction.amount.to_string(),
            )),
        }
    }
}

// functions for tests only
#[cfg(test)]
impl PermissionedAddress {
    /// Deprecated, will be axed when better spend limit asset/multiasset
    /// handling is implemented.
    pub fn usdc_denom(&self) -> Option<String> {
        self.params.usdc_denom.clone()
    }

    pub fn set_usdc_denom(&mut self, new_setting: Option<String>) -> StdResult<()> {
        self.params.usdc_denom = new_setting;
        Ok(())
    }

    pub fn spend_limits(&self) -> Vec<CoinLimit> {
        self.params.spend_limits.clone()
    }

    pub fn current_period_reset(&self) -> u64 {
        self.params.current_period_reset
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, JsonSchema, Debug)]
pub struct PermissionedAddresssResponse {
    pub permissioned_addresses: Vec<PermissionedAddressParams>,
}
