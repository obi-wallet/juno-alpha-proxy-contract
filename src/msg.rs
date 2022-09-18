use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{CosmosMsg, Uint128};

use crate::hot_wallet::HotWallet;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct InstantiateMsg {
    pub admin: String,
    pub hot_wallets: Vec<HotWallet>,
    pub uusd_fee_debt: Uint128,
    pub fee_lend_repay_wallet: String,
    pub home_network: String,
    pub signers: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    /// Execute requests the contract to re-dispatch all these messages with the
    /// contract's address as sender. Every implementation has it's own logic to
    /// determine in
    Execute { msgs: Vec<CosmosMsg> },
    /// Might still have spend limit etc affects, but avoids attaching
    /// any messages. Attaches attributes as normal. For debugging purposes –
    /// others should use query
    SimExecute { msgs: Vec<CosmosMsg> },
    /// Proposes a new admin for the proxy contract – must be called by the existing admin
    ProposeUpdateAdmin { new_admin: String },
    /// Confirms a proposed admin - must be called by the new admin.
    /// This is to prevent accidentally transitioning to an uncontrolled address.
    ConfirmUpdateAdmin {},
    /// Cancels a proposed admin - must be called by current admin.
    /// This can be used to cancel during a waiting period.
    CancelUpdateAdmin {},
    /// Adds a spend-limited wallet, which can call cw20 Transfer/Send and BankMsg
    /// transactions if within the known recurring spend limit.
    AddHotWallet { new_hot_wallet: HotWallet },
    /// Removes an active spend-limited wallet.
    RmHotWallet { doomed_hot_wallet: String },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    /// Shows admin; always mutable
    Admin {},
    /// Shows pending admin (subject to becoming new admin when
    /// ConfirmUpdateAdmin is called successfully)
    Pending {},
    /// Checks permissions of the caller on this proxy.
    /// If CanExecute returns true then a call to `Execute` with the same message,
    /// before any further state changes, should also succeed.
    /// TODO: support can_spend for hot wallets in this check.
    CanExecute { sender: String, msg: CosmosMsg },
    /// Gets an array of all the active HotWallets for this proxy.
    HotWallets {},
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DexQueryMsg {
    ReverseSimulation(ReverseSimulationMsg),
    Simulation(SimulationMsg),
    Token1ForToken2Price(Token1ForToken2Msg),
    Token2ForToken1Price(Token2ForToken1Msg),
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

impl Tallyable for Token1ForToken2PriceResponse {
    fn tally(self) -> Uint128 {
        self.token2_amount
    }
}

impl Tallyable for Token2ForToken1PriceResponse {
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

pub trait Tallyable {
    fn tally(self) -> Uint128;
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct SimulationResponse {
    pub commission_amount: Uint128,
    pub return_amount: Uint128,
    pub spread_amount: Uint128,
}

impl Tallyable for SimulationResponse {
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

impl Tallyable for ReverseSimulationResponse {
    fn tally(self) -> Uint128 {
        self.commission_amount + self.offer_amount
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MigrateMsg {}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, JsonSchema, Debug)]
pub struct AdminResponse {
    pub admin: String,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, JsonSchema, Debug)]
pub struct HotWalletsResponse {
    pub hot_wallets: Vec<HotWallet>,
}
