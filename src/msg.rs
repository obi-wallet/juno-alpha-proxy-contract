use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::CosmosMsg;

use crate::state::HotWallet;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    pub admin: String,
    pub hot_wallets: Vec<HotWallet>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    /// Execute requests the contract to re-dispatch all these messages with the
    /// contract's address as sender. Every implementation has it's own logic to
    /// determine in
    Execute {
        msgs: Vec<CosmosMsg>,
    },
    /// Proposes a new admin for the proxy contract – must be called by the existing admin
    ProposeUpdateAdmin {
        new_admin: String,
    },
    /// Confirms a proposed admin - must be called by the new admin.
    /// This is to prevent accidentally transitioning to an uncontrolled address.
    ConfirmUpdateAdmin {},
    /// Adds a spend-limited wallet, which can call cw20 Transfer/Send and BankMsg
    /// transactions if within the known recurring spend limit.
    AddHotWallet {
        new_hot_wallet: HotWallet,
    },
    /// Removes an active spend-limited wallet.
    RmHotWallet {
        doomed_hot_wallet: String,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    /// Shows all admins; always mutable
    Admin {},
    /// Checks permissions of the caller on this proxy.
    /// If CanExecute returns true then a call to `Execute` with the same message,
    /// before any further state changes, should also succeed.
    /// TODO: support can_spend for hot wallets in this check.
    CanExecute {
        sender: String,
        msg: CosmosMsg,
    },
    /// Gets an array of all the active HotWallets for this proxy.
    HotWallets {},
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MigrateMsg {}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
pub struct AdminResponse {
    pub admin: String,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
pub struct HotWalletsResponse {
    pub hot_wallets: Vec<HotWallet>,
}
