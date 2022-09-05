use cosmwasm_std::{to_binary, Deps, QueryRequest, StdError, Uint128, WasmQuery};

use crate::{
    msg::{Asset, AssetInfo, DexQueryMsg, SimulationMsg, SimulationResponse},
    state::STATE,
    ContractError,
};

//TODO: make this correct in both environments
fn get_pair_contract(network: String, asset: String) -> Result<String, ContractError> {
    // dummy contract on testnet
    match &*network {
        "uni-3" => {
            match &asset[..] {
                "ujunox" => Ok(
                    "juno1dmwfwqvke4hew5s93ut8h4tgu6sxv67zjw0y3hskgkfpy3utnpvseqyjs7".to_owned(),
                ),
                "ibc/EAC38D55372F38F1AFD68DF7FE9EF762DCF69F26520643CF3F9D292A738D8034" => Ok(
                    "juno1dmwfwqvke4hew5s93ut8h4tgu6sxv67zjw0y3hskgkfpy3utnpvseqyjs7".to_owned(),
                ),
                _ => {
                    // this should probably fail quietly – if we're dealing with an entirely unknown asset,
                    // transactions should go through by default if admin and fail if hot wallet
                    // TODO here
                    Ok("".to_owned())
                }
            }
        }
        "juno-1" => {
            match &asset[..] {
                "ujunox" => Ok(
                    "juno1qc8mrs3hmxm0genzrd92akja5r0v7mfm6uuwhktvzphhz9ygkp8ssl4q07".to_owned(),
                ),
                "ibc/EAC38D55372F38F1AFD68DF7FE9EF762DCF69F26520643CF3F9D292A738D8034" => Ok(
                    "juno1utkr0ep06rkxgsesq6uryug93daklyd6wneesmtvxjkz0xjlte9qdj2s8q4".to_owned(),
                ),
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

#[allow(unreachable_code)]
#[allow(unused_variables)]
pub fn get_current_price(
    deps: Deps,
    asset: String,
    amount: Uint128,
) -> Result<Uint128, ContractError> {
    #[cfg(test)]
    match &*asset {
        "testtokens" => {
            return Ok(Uint128::from(100u128));
        }
        _ => {
            return Ok(Uint128::from(1u128));
        }
    }
    // TODO: if asset is source base token, return 1
    let cfg = STATE.load(deps.storage)?;
    let query_msg: DexQueryMsg = DexQueryMsg::Simulation(SimulationMsg {
        offer_asset: Asset {
            info: AssetInfo::NativeToken {
                denom: asset.clone(),
            },
            amount,
        },
    }); // no cw20 support yet (expect for the base asset)
    let query_response: Result<SimulationResponse, StdError> =
        deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr: get_pair_contract(cfg.home_network, asset.clone())?,
            msg: to_binary(&query_msg)?,
        }));
    match query_response {
        Ok(res) => Ok(res.return_amount + res.commission_amount),
        Err(_) => Err(ContractError::PriceCheckFailed(asset)),
    }
}
