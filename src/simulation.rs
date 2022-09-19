use cosmwasm_std::Uint128;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub trait Tally {
    fn tally(self) -> Uint128;
}

pub trait FormatQueryMsg {
    fn format_query_msg(self, reverse: bool) -> DexQueryMsgFormatted;
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DexQueryMsgType {
    ReverseSimulation,
    Simulation,
    Token1ForToken2Price,
    Token2ForToken1Price,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DexQueryMsgFormatted {
    ReverseSimulation(ReverseSimulationMsg),
    Simulation(SimulationMsg),
    Token1ForToken2Price(Token1ForToken2Msg),
    Token2ForToken1Price(Token2ForToken1Msg),
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct DexQueryMsg {
    pub ty: DexQueryMsgType,
    pub denom: String,
    pub amount: Uint128,
}

impl FormatQueryMsg for DexQueryMsg {
    fn format_query_msg(self, reverse: bool) -> DexQueryMsgFormatted {
        let mut ty = self.ty;
        if reverse {
            ty = match ty {
                DexQueryMsgType::ReverseSimulation => DexQueryMsgType::Simulation,
                DexQueryMsgType::Simulation => DexQueryMsgType::ReverseSimulation,
                DexQueryMsgType::Token1ForToken2Price => DexQueryMsgType::Token2ForToken1Price,
                DexQueryMsgType::Token2ForToken1Price => DexQueryMsgType::Token1ForToken2Price,
            }
        }
        match ty {
            DexQueryMsgType::ReverseSimulation => {
                DexQueryMsgFormatted::ReverseSimulation(ReverseSimulationMsg {
                    ask_asset: Asset {
                        info: AssetInfo::NativeToken { denom: self.denom },
                        amount: self.amount,
                    },
                })
            }
            DexQueryMsgType::Simulation => DexQueryMsgFormatted::Simulation(SimulationMsg {
                offer_asset: Asset {
                    info: AssetInfo::NativeToken { denom: self.denom },
                    amount: self.amount,
                },
            }),
            DexQueryMsgType::Token1ForToken2Price => {
                DexQueryMsgFormatted::Token1ForToken2Price(Token1ForToken2Msg {
                    token1_amount: self.amount,
                })
            }
            DexQueryMsgType::Token2ForToken1Price => {
                DexQueryMsgFormatted::Token2ForToken1Price(Token2ForToken1Msg {
                    token2_amount: self.amount,
                })
            }
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct Token1ForToken2Msg {
    pub token1_amount: Uint128,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct Token2ForToken1Msg {
    pub token2_amount: Uint128,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct Token1ForToken2PriceResponse {
    pub token2_amount: Uint128,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct Token2ForToken1PriceResponse {
    pub token1_amount: Uint128,
}

impl Tally for Token1ForToken2PriceResponse {
    fn tally(self) -> Uint128 {
        self.token2_amount
    }
}

impl Tally for Token2ForToken1PriceResponse {
    fn tally(self) -> Uint128 {
        self.token1_amount
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct SimulationMsg {
    pub offer_asset: Asset,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct Asset {
    pub amount: Uint128,
    pub info: AssetInfo,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AssetInfo {
    NativeToken { denom: String },
    Token { contract_addr: String },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct ReverseSimulationMsg {
    pub ask_asset: Asset,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct SimulationResponse {
    pub commission_amount: Uint128,
    pub return_amount: Uint128,
    pub spread_amount: Uint128,
}

impl Tally for SimulationResponse {
    fn tally(self) -> Uint128 {
        self.commission_amount + self.return_amount
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct ReverseSimulationResponse {
    pub commission_amount: Uint128,
    pub offer_amount: Uint128,
    pub spread_amount: Uint128,
}

impl Tally for ReverseSimulationResponse {
    fn tally(self) -> Uint128 {
        self.commission_amount + self.offer_amount
    }
}
