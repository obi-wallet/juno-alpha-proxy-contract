use cosmwasm_std::{to_binary, Coin, Deps, QueryRequest, StdError, Uint128, WasmQuery};

use crate::constants::{
    MAINNET_AXLUSDC_IBC, MAINNET_ID, MAINNET_JUNO_LOOP_PAIR_CONTRACT,
    MAINNET_USDC_LOOP_PAIR_CONTRACT, TESTNET_ID, TESTNET_LOOP_PAIR_DUMMY_CONTRACT,
};
use crate::msg::ReverseSimulationMsg;
use crate::state::SourcedSwap;
use crate::{
    msg::{Asset, AssetInfo, DexQueryMsg, SimulationMsg, SimulationResponse},
    state::STATE,
    ContractError,
};

//TODO: make this correct in both environments
fn get_pair_contract(network: String, asset: String) -> Result<String, ContractError> {
    // dummy contract on testnet
    match network.as_str() {
        val if val == TESTNET_ID => {
            match &asset[..] {
                "ujunox" => Ok(TESTNET_LOOP_PAIR_DUMMY_CONTRACT.to_owned()),
                val if val == MAINNET_AXLUSDC_IBC => {
                    Ok(TESTNET_LOOP_PAIR_DUMMY_CONTRACT.to_owned())
                }
                _ => {
                    // this should probably fail quietly – if we're dealing with an entirely unknown asset,
                    // transactions should go through by default if admin and fail if hot wallet
                    // TODO here
                    Ok("".to_owned())
                }
            }
        }
        val if val == MAINNET_ID => {
            match &asset[..] {
                "ujuno" => Ok(MAINNET_JUNO_LOOP_PAIR_CONTRACT.to_owned()),
                val if val == MAINNET_AXLUSDC_IBC => Ok(MAINNET_USDC_LOOP_PAIR_CONTRACT.to_owned()),
                _ => {
                    // this should probably fail quietly – if we're dealing with an entirely unknown asset,
                    // transactions should go through by default if admin and fail if hot wallet
                    // TODO here
                    Ok("".to_owned())
                }
            }
        }
        _ => Err(ContractError::UnknownHomeNetwork(network)),
    }
}

// convenience
pub fn simulate_reverse_swap(
    deps: Deps,
    asset: String,
    amount: Uint128,
) -> Result<SourcedSwap, ContractError> {
    simulate_swap(deps, asset, amount, true)
}

#[allow(unreachable_code)]
#[allow(unused_variables)]
pub fn simulate_swap(
    deps: Deps,
    asset: String,
    amount: Uint128,
    reverse: bool,
) -> Result<SourcedSwap, ContractError> {
    #[cfg(test)]
    match &*asset {
        "testtokens" => {
            return Ok(SourcedSwap {
                coin: Coin {
                    amount: Uint128::from(100u128),
                    denom: "testtokens".to_string(),
                },
                contract_addr: "local test path 1".to_string(),
            });
        }
        _ => {
            return Ok(SourcedSwap {
                coin: Coin {
                    amount: Uint128::from(100u128),
                    denom: "testDexAsset".to_string(),
                },
                contract_addr: "local test path 2".to_string(),
            });
        }
    }
    // TODO: if asset is source base token, return 1
    let cfg = STATE.load(deps.storage)?;
    let simulation_asset = Asset {
        amount,
        info: AssetInfo::NativeToken {
            denom: asset.clone(),
        },
    };
    let response_asset;
    let query_msg: DexQueryMsg = match reverse {
        false => {
            response_asset = "uloop".to_string();
            DexQueryMsg::Simulation(SimulationMsg {
                offer_asset: simulation_asset,
            })
        }
        true => {
            response_asset = asset.clone();
            DexQueryMsg::ReverseSimulation(ReverseSimulationMsg {
                ask_asset: simulation_asset,
            }) // no cw20 support yet (expect for the base asset)
        }
    };
    let contract_addr = get_pair_contract(cfg.home_network, asset)?;
    let query_response: Result<SimulationResponse, StdError> =
        deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr: contract_addr.clone(),
            msg: to_binary(&query_msg)?,
        }));
    match query_response {
        Ok(res) => Ok(SourcedSwap {
            coin: Coin {
                denom: response_asset,
                amount: (res.return_amount + res.commission_amount),
            },
            contract_addr,
        }),
        Err(e) => Err(ContractError::PriceCheckFailed(
            format!("{:?}", to_binary(&query_msg)?),
            e.to_string(),
        )),
    }
}
