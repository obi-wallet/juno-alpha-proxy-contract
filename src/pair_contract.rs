use cosmwasm_std::{to_binary, Coin, Deps, QueryRequest, StdError, Uint128, WasmQuery};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[cfg(test)]
use crate::constants_tests::get_test_sourced_coin;
use crate::{
    msg::{
        Asset, AssetInfo, DexQueryMsg, ReverseSimulationMsg, SimulationMsg, Tallyable,
        Token1ForToken2Msg, Token1ForToken2PriceResponse, Token2ForToken1Msg,
        Token2ForToken1PriceResponse,
    },
    state::{Source, SourcedCoin},
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
        let response_asset: String;
        let mut flip_assets: bool = amount_is_target;
        if reverse_message_type {
            flip_assets = !flip_assets;
        }
        let query_msg: DexQueryMsg = match self.query_format {
            PairMessageType::LoopType => {
                response_asset = self.denom2.clone();
                let simulation_asset = Asset {
                    amount,
                    info: AssetInfo::NativeToken {
                        denom: self.denom1.clone(),
                    },
                };
                match amount_is_target {
                    false => DexQueryMsg::Simulation(SimulationMsg {
                        offer_asset: simulation_asset,
                    }),
                    true => {
                        DexQueryMsg::ReverseSimulation(ReverseSimulationMsg {
                            ask_asset: simulation_asset,
                        }) // no cw20 support yet (expect for the base asset)
                    }
                }
            }
            PairMessageType::JunoType => {
                if reverse {
                    flip_assets = !flip_assets;
                }
                match flip_assets {
                    false => {
                        response_asset = self.denom2.clone();
                        DexQueryMsg::Token1ForToken2Price(Token1ForToken2Msg {
                            token1_amount: amount,
                        })
                    }
                    true => {
                        response_asset = self.denom1.clone();
                        DexQueryMsg::Token2ForToken1Price(Token2ForToken1Msg {
                            token2_amount: amount,
                        })
                    } // no cw20 support yet (except for the base asset)
                }
            }
        };
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
            return get_test_sourced_coin((test_denom1, response_asset), amount, reverse);
        }
        let query_result: Result<SourcedCoin, ContractError>;
        match flip_assets {
            false => {
                self.process_query::<Token1ForToken2PriceResponse>(deps, &query_msg, response_asset)
            }
            true => {
                self.process_query::<Token2ForToken1PriceResponse>(deps, &query_msg, response_asset)
            }
        }
    }

    fn process_query<T>(
        &self,
        deps: Deps,
        query_msg: &DexQueryMsg,
        response_asset: String,
    ) -> Result<SourcedCoin, ContractError>
    where
        T: for<'de> Deserialize<'de>,
        T: Tallyable,
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
