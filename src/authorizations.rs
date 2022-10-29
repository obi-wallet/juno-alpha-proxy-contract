use cosmwasm_std::{Addr, Uint128};
use cw_storage_plus::{Index, IndexList, IndexedMap, Item, MultiIndex};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::state::ObiProxyContract;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct Authorization {
    pub count: Uint128,
    pub actor: Addr,
    pub contract: Addr,
    pub message_name: String,
    pub fields: Option<Vec<(String, String)>>,
}

pub struct AuthorizationIndexes<'a> {
    // pk goes to second tuple element
    pub auth_count: MultiIndex<'a, String, Authorization, Vec<u8>>,
    pub actor: MultiIndex<'a, Addr, Authorization, Vec<u8>>,
    pub contract: MultiIndex<'a, Addr, Authorization, Vec<u8>>,
    pub message_name: MultiIndex<'a, String, Authorization, Vec<u8>>,
}

impl<'a> IndexList<Authorization> for AuthorizationIndexes<'a> {
    fn get_indexes(&'_ self) -> Box<dyn Iterator<Item = &'_ dyn Index<Authorization>> + '_> {
        let v: Vec<&dyn Index<Authorization>> = vec![
            &self.auth_count,
            &self.actor,
            &self.contract,
            &self.message_name,
        ];
        Box::new(v.into_iter())
    }
}

impl Default for ObiProxyContract<'static> {
    fn default() -> Self {
        Self::new(
            "auths",
            "auths__count",
            "auths__actor",
            "auths__contract",
            "auths__message_name",
            "cfg",
        )
    }
}

impl<'a> ObiProxyContract<'a> {
    fn new(
        authorizations_key: &'a str,
        authorizations_count_key: &'a str,
        authorizations_actor_key: &'a str,
        authorizations_contract_key: &'a str,
        authorizations_message_name_key: &'a str,
        config_key: &'a str,
    ) -> Self {
        let indexes = AuthorizationIndexes {
            auth_count: MultiIndex::new(
                |d| (d.count.to_string()),
                authorizations_key,
                authorizations_count_key,
            ),
            actor: MultiIndex::new(
                |d| (d.actor.clone()),
                authorizations_key,
                authorizations_actor_key,
            ),
            contract: MultiIndex::new(
                |d| (d.contract.clone()),
                authorizations_key,
                authorizations_contract_key,
            ),
            message_name: MultiIndex::new(
                |d| (d.message_name.clone()),
                authorizations_key,
                authorizations_message_name_key,
            ),
        };

        Self {
            authorizations: IndexedMap::new(authorizations_key, indexes),
            cfg: Item::new(config_key),
        }
    }
}
