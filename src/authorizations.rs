use cosmwasm_std::{Addr, Uint128};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct Authorization {
    pub count: Uint128,
    pub contract: Addr,
    pub message_name: String,
    pub fields: Option<Vec<(String, String)>>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct Authorizations {
    authorizations: Vec<Authorization>,
}
