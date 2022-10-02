use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{Coin, CosmosMsg, Uint128};

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
    ConfirmUpdateAdmin { signers: Vec<String> },
    /// Cancels a proposed admin - must be called by current admin.
    /// This can be used to cancel during a waiting period.
    CancelUpdateAdmin {},
    /// Adds a spend-limited wallet, which can call cw20 Transfer/Send and BankMsg
    /// transactions if within the known recurring spend limit.
    AddHotWallet { new_hot_wallet: HotWallet },
    /// Removes an active spend-limited wallet.
    RmHotWallet { doomed_hot_wallet: String },
    /// Updates spend limit for a wallet. Update of period not supported: rm and re-add
    UpdateHotWalletSpendLimit {
        hot_wallet: String,
        new_spend_limits: Vec<Coin>,
    },
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
    /// Returns true if address 1) is admin, 2) is hot wallet and msg is spendable
    /// by hot wallet, or 3) is one of approved cw20s (no funds attached tho)
    CanSpend {
        sender: String,
        msgs: Vec<CosmosMsg>,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct MigrateMsg {}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, JsonSchema, Debug)]
pub struct AdminResponse {
    pub admin: String,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, JsonSchema, Debug)]
pub struct CanSpendResponse {
    pub can_spend: bool,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, JsonSchema, Debug)]
pub enum Cw20ExecuteMsg {
    Transfer { recipient: String, amount: Uint128 },
}
