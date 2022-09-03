use cosmwasm_std::{to_binary, Deps, QueryRequest, Uint128, WasmQuery};

use crate::{
    msg::{Asset, AssetInfo, DexQueryMsg, SimulationMsg, SimulationResponse},
    ContractError,
};

//TODO: make this correct in both environments
fn get_pair_contract(asset: String) -> Result<String, ContractError> {
    // dummy contract on testnet
    match &asset[..] {
        "ujunox" => {
            Ok("juno1dmwfwqvke4hew5s93ut8h4tgu6sxv67zjw0y3hskgkfpy3utnpvseqyjs7".to_owned())
        }
        "ibc/EAC38D55372F38F1AFD68DF7FE9EF762DCF69F26520643CF3F9D292A738D8034" => {
            Ok("juno1dmwfwqvke4hew5s93ut8h4tgu6sxv67zjw0y3hskgkfpy3utnpvseqyjs7".to_owned())
        }
        _ => {
            // this should probably fail quietly – if we're dealing with an entirely unknown asset,
            // transactions should go through by default if admin and fail if hot wallet
            // TODO here
            Ok("".to_owned())
        }
    }
    // here are mainnet addresses
    /*
    match &asset[..] {
        "ujunox" => {
            Ok("juno1qc8mrs3hmxm0genzrd92akja5r0v7mfm6uuwhktvzphhz9ygkp8ssl4q07".to_owned())
        },
        "ibc/EAC38D55372F38F1AFD68DF7FE9EF762DCF69F26520643CF3F9D292A738D8034" => {
            Ok("juno1utkr0ep06rkxgsesq6uryug93daklyd6wneesmtvxjkz0xjlte9qdj2s8q4".to_owned())
        },
        _ => {
            // this should probably fail quietly – if we're dealing with an entirely unknown asset,
            // transactions should go through by default if admin and fail if hot wallet
            // TODO here
            Ok("".to_owned())
        },
    }
    */
}

pub fn get_current_price(deps: Deps, asset: String) -> Result<Uint128, ContractError> {
    // TODO: if asset is source base token, return 1
    let query_msg: DexQueryMsg = DexQueryMsg::Simulation(SimulationMsg {
        offer_asset: Asset {
            info: AssetInfo::NativeToken {
                denom: asset.clone(),
            },
            amount: Uint128::from(1_000_000u128),
        },
    }); // no cw20 support yet (expect for the base asset)
    let query_response: SimulationResponse =
        deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr: get_pair_contract(asset)?,
            msg: to_binary(&query_msg)?,
        }))?;
    Ok(query_response.return_amount + query_response.commission_amount)
}
