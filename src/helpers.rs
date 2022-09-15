use cosmwasm_std::{Coin, Deps, Uint128};
use serde::Deserialize;

use crate::constants::{get_usdc_sourced_coin, MAINNET_AXLUSDC_IBC};
use crate::msg::{ReverseSimulationResponse, SimulationResponse, Tallyable};
use crate::state::{SourcedCoin, SourcedSwap};
use crate::{state::STATE, ContractError};

/// reverse is true if we have a target USDC amount (for fees)
/// false if we're converting without a target (for spend limits)
pub fn convert_coin_to_usdc(
    deps: Deps,
    denom: String,
    amount: Uint128,
    reverse: bool,
) -> Result<SourcedCoin, ContractError> {
    if denom == MAINNET_AXLUSDC_IBC {
        return Ok(get_usdc_sourced_coin(amount));
    }
    match reverse {
        false => {
            // top will be the price in DEX base
            let price = simulate_swap(deps, (denom, MAINNET_AXLUSDC_IBC.to_string()), amount)?;
            Ok(SourcedCoin {
                coin: Coin {
                    denom: MAINNET_AXLUSDC_IBC.to_string(),
                    amount: price.coin.amount,
                },
                sources: vec![price],
            })
        }
        true => {
            // top will be the price in DEX base
            let price =
                simulate_reverse_swap(deps, (MAINNET_AXLUSDC_IBC.to_string(), denom), amount)?;
            Ok(SourcedCoin {
                coin: Coin {
                    denom: MAINNET_AXLUSDC_IBC.to_string(),
                    amount: price.coin.amount,
                },
                sources: vec![price],
            })
        }
    }
}

// convenience functions
pub fn simulate_reverse_swap(
    deps: Deps,
    denoms: (String, String),
    amount: Uint128,
) -> Result<SourcedSwap, ContractError> {
    simulate::<ReverseSimulationResponse>(deps, denoms, amount, true)
}

pub fn simulate_swap(
    deps: Deps,
    denoms: (String, String),
    amount: Uint128,
) -> Result<SourcedSwap, ContractError> {
    simulate::<SimulationResponse>(deps, denoms, amount, false)
}

#[allow(unreachable_code)]
#[allow(unused_variables)]
pub fn simulate<T>(
    deps: Deps,
    denoms: (String, String),
    amount: Uint128,
    target_amount: bool, // when you want to meet a target number
) -> Result<SourcedSwap, ContractError>
where
    T: for<'de> Deserialize<'de>,
    T: Tallyable,
{
    let cfg = STATE.load(deps.storage)?;
    let pair_contract = cfg.get_pair_contract(denoms)?; // bool is whether reversed
    pair_contract
        .0
        .query_contract::<T>(deps, amount, pair_contract.1, target_amount)
}
