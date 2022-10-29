use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{Binary, Coin, CosmosMsg, Uint128};

use crate::{
    authorizations::Authorization,
    hot_wallet::{CoinLimit, HotWalletParams},
    signers::Signer,
};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct InstantiateMsg {
    pub owner: String,
    pub hot_wallets: Vec<HotWalletParams>,
    pub uusd_fee_debt: Uint128,
    pub fee_lend_repay_wallet: String,
    pub home_network: String,
    pub signers: Vec<String>,
    pub signer_types: Vec<String>,
    pub update_delay_hours: Option<u16>,
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
    /// Proposes a new owner for the proxy contract – must be called by the existing owner
    ProposeUpdateOwner {
        new_owner: String,
    },
    /// Confirms a proposed owner - must be called by the new owner.
    /// This is to prevent accidentally transitioning to an uncontrolled address.
    ConfirmUpdateOwner {
        signers: Vec<String>,
        signer_types: Vec<String>,
    },
    /// Cancels a proposed owner - must be called by current owner.
    /// This can be used to cancel during a waiting period.
    CancelUpdateOwner {},
    /// Adds a spend-limited wallet, which can call cw20 Transfer/Send and BankMsg
    /// transactions if within the known recurring spend limit.
    AddHotWallet {
        new_hot_wallet: HotWalletParams,
    },
    /// Removes an active spend-limited wallet.
    RmHotWallet {
        doomed_hot_wallet: String,
    },
    /// Updates spend limit for a wallet. Update of period not supported: rm and re-add
    UpdateHotWalletSpendLimit {
        hot_wallet: String,
        new_spend_limits: CoinLimit,
    },
    /// Updates the update delay (when changing to new admin)
    UpdateUpdateDelay {
        hours: u16,
    },
    AddAuthorization {
        new_authorization: Authorization,
    },
    RemoveAuthorization {
        authorization_to_remove: Authorization,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    // GetCount returns the current count as a json-encoded number
    Authorizations {
        target_contract: Option<String>,
        limit: Option<u32>,
        start_after: Option<String>,
    },
    /// Shows owner; always mutable
    Owner {},
    /// Shows pending owner (subject to becoming new owner when
    /// ConfirmUpdateOwner is called successfully)
    Pending {},
    /// Returns the array of owner_signers stored in state.
    Signers {},
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
        funds: Vec<Coin>,
        msgs: Vec<CosmosMsg>,
    },
    /// Returns the update delay (when updating to a new admin) in hours
    UpdateDelay {},
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct MigrateMsg {}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct WasmExecuteMsg {
    contract_addr: String,
    /// msg is the json-encoded ExecuteMsg struct (as raw Binary)
    pub msg: Binary,
    funds: Vec<Coin>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct TestExecuteMsg {
    pub foo: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct TestFieldsExecuteMsg {
    pub recipient: String,
    pub strategy: String,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, JsonSchema, Debug)]
pub struct OwnerResponse {
    pub owner: String,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, JsonSchema, Debug)]
pub struct SignersResponse {
    pub signers: Vec<Signer>,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, JsonSchema, Debug)]
pub struct CanSpendResponse {
    pub can_spend: bool,
    pub reason: String,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, JsonSchema, Debug)]
pub struct UpdateDelayResponse {
    pub update_delay_hours: u16,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct AuthorizationsResponse {
    pub authorizations: Vec<Authorization>,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, JsonSchema, Debug)]
pub enum Cw20ExecuteMsg {
    Transfer { recipient: String, amount: Uint128 },
}
