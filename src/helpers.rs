use cosmwasm_std::{to_binary, Coin, Deps, QueryRequest, StdError, Uint128, WasmQuery};
use serde::Deserialize;

use crate::constants::{
    get_usdc_sourced_coin, MAINNET_AXLUSDC_IBC, MAINNET_DENOM, MAINNET_DEX_DENOM, MAINNET_ID,
    MAINNET_JUNO_TO_LOOP_PAIR_CONTRACT, MAINNET_LOOP_TO_JUNO_PAIR_CONTRACT,
    MAINNET_LOOP_TO_USDC_PAIR_CONTRACT, MAINNET_USDC_TO_LOOP_PAIR_CONTRACT, TESTNET_ID,
    TESTNET_LOOP_PAIR_DUMMY_CONTRACT,
};
#[cfg(test)]
use crate::constants_tests::get_test_sourced_swap;
use crate::msg::{ReverseSimulationMsg, ReverseSimulationResponse, SimulationResponse, Tallyable};
use crate::state::{SourcedCoin, SourcedSwap};
use crate::{
    msg::{Asset, AssetInfo, DexQueryMsg, SimulationMsg},
    state::STATE,
    ContractError,
};

//TODO: make this correct in both environments
fn get_pair_contract(network: String, assets: (String, String)) -> Result<String, ContractError> {
    // dummy contract on testnet
    match network.as_str() {
        val if val == TESTNET_ID => Ok(TESTNET_LOOP_PAIR_DUMMY_CONTRACT.to_owned()),
        val if val == MAINNET_ID => match assets {
            denoms
                if denoms
                    == (
                        MAINNET_AXLUSDC_IBC.to_string(),
                        MAINNET_DEX_DENOM.to_string(),
                    ) =>
            {
                Ok(MAINNET_USDC_TO_LOOP_PAIR_CONTRACT.to_owned())
            }
            denoms
                if denoms
                    == (
                        MAINNET_DEX_DENOM.to_string(),
                        MAINNET_AXLUSDC_IBC.to_string(),
                    ) =>
            {
                Ok(MAINNET_LOOP_TO_USDC_PAIR_CONTRACT.to_owned())
            }
            denoms if denoms == (MAINNET_DENOM.to_string(), MAINNET_DEX_DENOM.to_string()) => {
                Ok(MAINNET_JUNO_TO_LOOP_PAIR_CONTRACT.to_owned())
            }
            denoms if denoms == (MAINNET_DEX_DENOM.to_string(), MAINNET_DENOM.to_string()) => {
                Ok(MAINNET_LOOP_TO_JUNO_PAIR_CONTRACT.to_owned())
            }
            _ => Ok("".to_owned()),
        },
        _ => Err(ContractError::UnknownHomeNetwork(network)),
    }
}

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
    // Dex base asset is never identified, always the other
    let simulation_asset_denom = match denoms.0.clone() {
        val if val == "uloop".to_string() => denoms.1.clone(),
        _ => denoms.0.clone(),
    };
    let simulation_asset = Asset {
        amount,
        info: AssetInfo::NativeToken {
            denom: simulation_asset_denom,
        },
    };
    let response_asset = denoms.1.clone();
    let query_msg: DexQueryMsg = match reverse {
        false => DexQueryMsg::Simulation(SimulationMsg {
            offer_asset: simulation_asset,
        }),
        true => {
            DexQueryMsg::ReverseSimulation(ReverseSimulationMsg {
                ask_asset: simulation_asset,
            }) // no cw20 support yet (expect for the base asset)
        }
    };
    let contract_addr = get_pair_contract(cfg.home_network, denoms)?;
    let query_response: Result<T, StdError> =
        deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr: contract_addr.clone(),
            msg: to_binary(&query_msg)?,
        }));
    match query_response {
        Ok(res) => Ok(SourcedSwap {
            coin: Coin {
                denom: response_asset,
                amount: (res.tally()),
            },
            contract_addr,
        }),
        Err(e) => Err(ContractError::PriceCheckFailed(
            format!("{:?}", to_binary(&query_msg)?),
            e.to_string(),
        )),
    }
}
