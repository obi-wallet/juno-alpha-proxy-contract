use cosmwasm_std::{Coin, Attribute, Deps, Uint128};
use schemars::JsonSchema;
use serde::{Serialize, Deserialize};

use crate::{state::{Source, STATE}, ContractError, constants::{MAINNET_AXLUSDC_IBC, get_usdc_sourced_coin}};

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
pub struct SourcedCoin {
    pub coin: Coin,
    pub sources: Vec<Source>,
}

impl SourcedCoin {
    pub fn sources_as_attributes(&self) -> Vec<Attribute> {
        let mut attributes: Vec<Attribute> = vec![];
        for n in 0..self.sources.len() {
            attributes.push(Attribute {
                key: format!(
                    "query to contract {}",
                    self.sources[n].contract_addr.clone()
                ),
                value: self.sources[n].query_msg.clone(),
            })
        }
        attributes
    }

    /// reverse is true if we have a target USDC amount (for fees)
    /// false if we're converting without a target (for spend limits)
    pub fn get_converted_to_usdc(
        &self,
        deps: Deps,
        reverse: bool,
    ) -> Result<SourcedCoin, ContractError> {
        if self.coin.denom.clone() == MAINNET_AXLUSDC_IBC {
            return Ok(get_usdc_sourced_coin(self.coin.amount));
        }
        match reverse {
            false => {
                self.simulate_swap(deps, (self.coin.denom.clone(), MAINNET_AXLUSDC_IBC.to_string()), self.coin.amount)
            }
            true => {
                self.simulate_reverse_swap(deps, (MAINNET_AXLUSDC_IBC.to_string(), self.coin.denom.clone()), self.coin.amount)
            }
        }
    }

    // convenience functions
    pub fn simulate_reverse_swap(
        &self,
        deps: Deps,
        denoms: (String, String),
        amount: Uint128,
    ) -> Result<SourcedCoin, ContractError> {
        self.simulate(deps, denoms, amount, true, true)
    }

    pub fn simulate_swap(
        &self,
        deps: Deps,
        denoms: (String, String),
        amount: Uint128,
    ) -> Result<SourcedCoin, ContractError> {
        self.simulate(deps, denoms, amount, true, false)
    }

    #[allow(unreachable_code)]
    #[allow(unused_variables)]
    pub fn simulate(
        &self,
        deps: Deps,
        denoms: (String, String),
        amount: Uint128,
        target_amount: bool,        // when you want to meet a target number
        reverse_message_type: bool, // type of simulation message
    ) -> Result<SourcedCoin, ContractError> {
        let cfg = STATE.load(deps.storage)?;
        let pair_contract = cfg.get_pair_contract(denoms)?; // bool is whether reversed
        pair_contract.0.query_contract(
            deps,
            amount,
            pair_contract.1,
            target_amount,
            reverse_message_type,
        )
    }
}
