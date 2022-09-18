use cosmwasm_std::{Deps, Uint128};

use crate::constants::{get_usdc_sourced_coin, MAINNET_AXLUSDC_IBC};
use crate::pair_contract::{PairContract, PairMessageType};
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
    let cfg = STATE.load(deps.storage)?;
    let pair_contract = cfg.get_pair_contract(denoms)?; // bool is whether reversed
    match pair_contract.0.query_format.clone() {
        PairMessageType::JunoType => simulate(deps, pair_contract, amount, true, true),
        PairMessageType::LoopType => simulate(deps, pair_contract, amount, true, true),
    }
}

pub fn simulate_swap(
    deps: Deps,
    denoms: (String, String),
    amount: Uint128,
) -> Result<SourcedCoin, ContractError> {
    let cfg = STATE.load(deps.storage)?;
    let pair_contract = cfg.get_pair_contract(denoms)?; // bool is whether reversed
    match pair_contract.0.query_format.clone() {
        PairMessageType::JunoType => simulate(deps, pair_contract, amount, true, false),
        PairMessageType::LoopType => simulate(deps, pair_contract, amount, true, false),
    }
}

#[allow(unreachable_code)]
#[allow(unused_variables)]
pub fn simulate(
    deps: Deps,
    pair_contract: (PairContract, bool),
    amount: Uint128,
    target_amount: bool,        // when you want to meet a target number
    reverse_message_type: bool, // type of simulation message
) -> Result<SourcedCoin, ContractError> {
    pair_contract.0.query_contract(
        deps,
        amount,
        pair_contract.1,
        target_amount,
        reverse_message_type,
    )
}
