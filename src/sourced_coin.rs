use cosmwasm_std::{Attribute, Coin, Deps, Uint128};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::pair_contract::PairContracts;
use crate::sources::Sources;
use crate::{
    constants::{get_usdc_sourced_coin, MAINNET_AXLUSDC_IBC},
    ContractError,
};

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
pub struct SourcedCoin {
    pub coin: Coin,
    pub wrapped_sources: Sources,
}

impl SourcedCoin {
    pub fn sources_as_attributes(&self) -> Vec<Attribute> {
        let mut attributes: Vec<Attribute> = vec![];
        for n in 0..self.wrapped_sources.sources.len() {
            attributes.push(Attribute {
                key: format!(
                    "query to contract {}",
                    self.wrapped_sources.sources[n].contract_addr.clone()
                ),
                value: self.wrapped_sources.sources[n].query_msg.clone(),
            })
        }
        attributes
    }

    /// reverse is true if we have a target USDC amount (for fees)
    /// false if we're converting without a target (for spend limits)
    pub fn get_converted_to_usdc(
        &self,
        deps: Deps,
        pair_contracts: PairContracts,
        reverse: bool,
    ) -> Result<SourcedCoin, ContractError> {
        if self.coin.denom.clone() == MAINNET_AXLUSDC_IBC {
            return Ok(get_usdc_sourced_coin(self.coin.amount));
        }
        match reverse {
            false => self.simulate_swap(
                deps,
                pair_contracts,
                (self.coin.denom.clone(), MAINNET_AXLUSDC_IBC.to_string()),
                self.coin.amount,
            ),
            true => self.simulate_reverse_swap(
                deps,
                pair_contracts,
                (MAINNET_AXLUSDC_IBC.to_string(), self.coin.denom.clone()),
                self.coin.amount,
            ),
        }
    }

    // convenience functions
    pub fn simulate_reverse_swap(
        &self,
        deps: Deps,
        pair_contracts: PairContracts,
        denoms: (String, String),
        amount: Uint128,
    ) -> Result<SourcedCoin, ContractError> {
        self.get_price_from_simulation(deps, pair_contracts, denoms, amount, true, true)
    }

    pub fn simulate_swap(
        &self,
        deps: Deps,
        pair_contracts: PairContracts,
        denoms: (String, String),
        amount: Uint128,
    ) -> Result<SourcedCoin, ContractError> {
        self.get_price_from_simulation(deps, pair_contracts, denoms, amount, false, false)
    }

    #[allow(unreachable_code)]
    #[allow(unused_variables)]
    pub fn get_price_from_simulation(
        &self,
        deps: Deps,
        pair_contracts: PairContracts,
        denoms: (String, String),
        amount: Uint128,
        target_amount: bool,        // when you want to meet a target number
        reverse_message_type: bool, // type of simulation message
    ) -> Result<SourcedCoin, ContractError> {
        let (pair_contract, reverse) = pair_contracts.get_pair_contract(denoms)?; // bool is whether reversed
        pair_contract.query_contract(deps, amount, reverse, target_amount, reverse_message_type)
    }
}
