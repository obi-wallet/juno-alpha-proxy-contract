use cosmwasm_std::{Coin, Deps, Uint128};
use serde::Deserialize;

use crate::constants::{get_usdc_sourced_coin, MAINNET_AXLUSDC_IBC, MAINNET_DEX_DENOM};
#[cfg(test)]
use crate::constants_tests::get_test_sourced_swap;
use crate::msg::{ReverseSimulationResponse, SimulationResponse, Tallyable};
use crate::state::{SourcedCoin, SourcedSwap};
use crate::{state::STATE, ContractError};

/// reverse is true if we have a target USDC amount (for fees)
/// false if we're converting without a target (for spend limits)
#[allow(unused_variables)]
pub fn convert_coin_to_usdc(
    deps: Deps,
    denom: String,
    amount: Uint128,
    reverse: bool,
) -> Result<SourcedCoin, ContractError> {
    if denom == *MAINNET_AXLUSDC_IBC {
        return Ok(get_usdc_sourced_coin(amount));
    }
    match reverse {
        false => {
            // top will be the price in DEX base
            let top = simulate_swap(deps, (denom, MAINNET_DEX_DENOM.to_string()), amount)?;
            // now bottom will be the price of that in target
            let bottom = simulate_swap(
                deps,
                (
                    MAINNET_DEX_DENOM.to_string(),
                    MAINNET_AXLUSDC_IBC.to_string(),
                ),
                top.coin.amount,
            )?;
            Ok(SourcedCoin {
                coin: Coin {
                    denom: MAINNET_AXLUSDC_IBC.to_string(),
                    amount: bottom.coin.amount,
                },
                top,
                bottom,
            })
        }
        true => {
            // top will be the price in DEX base
            let top = simulate_reverse_swap(
                deps,
                (
                    MAINNET_AXLUSDC_IBC.to_string(),
                    MAINNET_DEX_DENOM.to_string(),
                ),
                amount,
            )?;
            // now bottom will be the price of that in target
            let bottom = simulate_reverse_swap(
                deps,
                (MAINNET_DEX_DENOM.to_string(), denom),
                top.coin.amount,
            )?;
            Ok(SourcedCoin {
                coin: Coin {
                    denom: MAINNET_AXLUSDC_IBC.to_string(),
                    amount: bottom.coin.amount,
                },
                top,
                bottom,
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
    reverse: bool, // when you want to meet a target number
) -> Result<SourcedSwap, ContractError>
where
    T: for<'de> Deserialize<'de>,
    T: Tallyable,
{
    #[cfg(test)]
    return Ok(get_test_sourced_swap(denoms, amount, reverse));
    // TODO: if asset is source base token, return 1
    let cfg = STATE.load(deps.storage)?;
    let pair_contract = cfg.get_pair_contract(denoms.clone())?; // bool is whether reversed
    pair_contract
        .0
        .query_contract::<T>(deps, amount, pair_contract.1)
}
