use cosmwasm_std::{Deps, Uint128};

use crate::constants::{get_usdc_sourced_coin, MAINNET_AXLUSDC_IBC};
use crate::state::SourcedCoin;
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
            Ok(price)
        }
        true => {
            // top will be the price in DEX base
            let price =
                simulate_reverse_swap(deps, (MAINNET_AXLUSDC_IBC.to_string(), denom), amount)?;
            Ok(price)
        }
    }
}

// convenience functions
pub fn simulate_reverse_swap(
    deps: Deps,
    denoms: (String, String),
    amount: Uint128,
) -> Result<SourcedCoin, ContractError> {
    simulate(deps, denoms, amount, true, true)
}

pub fn simulate_swap(
    deps: Deps,
    denoms: (String, String),
    amount: Uint128,
) -> Result<SourcedCoin, ContractError> {
    simulate(deps, denoms, amount, true, false)
}

#[allow(unreachable_code)]
#[allow(unused_variables)]
pub fn simulate(
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
