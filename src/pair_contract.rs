use cosmwasm_std::{to_binary, Coin, Deps, QueryRequest, StdError, Uint128, WasmQuery};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[cfg(test)]
use crate::tests_constants::get_test_sourced_coin;
use crate::{
    sourced_coin::SourcedCoin,
    simulation::{DexQueryMsg, Token1ForToken2PriceResponse, Token2ForToken1PriceResponse},
    simulation::{DexQueryMsgFormatted, DexQueryMsgType, FormatQueryMsg, Tally},
    state::Source,
    ContractError,
};

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, JsonSchema, Debug)]
pub enum PairMessageType {
    LoopType,
    JunoType,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, JsonSchema, Debug)]
pub struct PairContract {
    pub contract_addr: String,
    pub denom1: String,
    pub denom2: String,
    pub query_format: PairMessageType,
}

impl PairContract {
    pub fn get_denoms(&self) -> Result<(String, String), ContractError> {
        Ok((self.denom1.clone(), self.denom2.clone()))
    }

    #[allow(unreachable_code)]
    #[allow(unused_variables)]
    pub fn query_contract(
        self,
        deps: Deps,
        amount: Uint128,
        reverse: bool,
        amount_is_target: bool,
        reverse_message_type: bool,
    ) -> Result<SourcedCoin, ContractError> {
        let mut flip_assets: bool = amount_is_target;
        if reverse_message_type {
            flip_assets = !flip_assets;
        }
        if reverse {
            flip_assets = !flip_assets;
        }
        let query_msg = self.clone().create_query_msg(amount, flip_assets)?;
        #[cfg(test)]
        println!(
            "Bypassing query message on contract {}: {:?}, and bools are reverse: {}, reverse_message_type: {}, amount_is_target: {}",
            self.contract_addr, query_msg, reverse, reverse_message_type, amount_is_target
        );
        println!("flip_assets is {}", flip_assets);
        #[cfg(test)]
        {
            let test_denom1 = match flip_assets {
                true => self.denom2,
                false => self.denom1,
            };
            return get_test_sourced_coin((test_denom1, query_msg.1), amount, reverse);
        }
        let query_result: Result<SourcedCoin, ContractError>;
        match flip_assets {
            false => {
                self.process_query::<Token1ForToken2PriceResponse>(deps, &query_msg.0, query_msg.1)
            }
            true => {
                self.process_query::<Token2ForToken1PriceResponse>(deps, &query_msg.0, query_msg.1)
            }
        }
    }

    pub fn create_query_msg(
        self,
        amount: Uint128,
        flip_assets: bool,
    ) -> Result<(DexQueryMsgFormatted, String), ContractError> {
        let response_asset: String;
        Ok(match self.query_format {
            PairMessageType::LoopType => {
                let dex_query_msg = DexQueryMsg {
                    ty: DexQueryMsgType::Simulation,
                    denom: self.denom1.clone(),
                    amount,
                };
                response_asset = self.denom2;
                (dex_query_msg.format_query_msg(flip_assets), response_asset)
            }
            PairMessageType::JunoType => {
                let dex_query_msg = DexQueryMsg {
                    ty: DexQueryMsgType::Token1ForToken2Price,
                    denom: self.denom1.clone(), // unused by juno type
                    amount,
                };
                let response_asset = match flip_assets {
                    false => self.denom2,
                    true => self.denom1,
                    // no cw20 support yet (except for the base asset)
                };
                (dex_query_msg.format_query_msg(flip_assets), response_asset)
            }
        })
    }

    fn process_query<T>(
        &self,
        deps: Deps,
        query_msg: &DexQueryMsgFormatted,
        response_asset: String,
    ) -> Result<SourcedCoin, ContractError>
    where
        T: for<'de> Deserialize<'de>,
        T: Tally,
    {
        let query_response: Result<T, StdError> =
            deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
                contract_addr: self.contract_addr.clone(),
                msg: to_binary(query_msg)?,
            }));
        match query_response {
            Ok(res) => Ok(SourcedCoin {
                coin: Coin {
                    denom: response_asset,
                    amount: (res.tally()),
                },
                sources: vec![Source {
                    contract_addr: self.contract_addr.clone(),
                    query_msg: format!("{:?}", to_binary(&query_msg)?),
                }],
            }),
            Err(e) => Err(ContractError::PriceCheckFailed(
                format!("{:?}", to_binary(&query_msg)?),
                self.contract_addr.clone(),
                e.to_string(),
            )),
        }
    }
}
