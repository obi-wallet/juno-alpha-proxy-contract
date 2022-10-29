use cosmwasm_std::{Addr, Deps, DepsMut, MessageInfo, Order, Response, StdResult, Uint128};
use cw_storage_plus::{Index, IndexList, IndexedMap, Item, MultiIndex};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json_value_wasm::Value;

use crate::{state::ObiProxyContract, submsgs::PendingSubmsg, ContractError};

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

    fn find_authorization(
        &self,
        deps: Deps,
        authorization: &Authorization,
    ) -> Result<Vec<u8>, ContractError> {
        let message_name = authorization.message_name.clone();
        let authorizations: Vec<(Vec<u8>, Authorization)> = self
            .authorizations
            .idx
            .contract
            .prefix(authorization.clone().contract)
            .range(deps.storage, None, None, Order::Ascending)
            .filter(|item| match item {
                Err(_) => false,
                Ok(val) => val.1.message_name == message_name,
            })
            .collect::<StdResult<Vec<(Vec<u8>, Authorization)>>>()?;
        for this_auth in authorizations.iter().take(authorizations.len() as usize) {
            if this_auth.clone().1.fields == authorization.clone().fields {
                return Ok(this_auth.0.clone());
            }
        }
        Err(ContractError::NoSuchAuthorization {})
    }

    pub fn add_authorization(
        &self,
        deps: DepsMut,
        info: MessageInfo,
        authorization: Authorization,
    ) -> Result<Response, ContractError> {
        let cfg = self.cfg.load(deps.storage)?;
        if info.sender != cfg.owner {
            return Err(ContractError::Unauthorized {});
        }

        match self.find_authorization(deps.as_ref(), &authorization) {
            Err(_) => {
                self.authorizations
                    .save(deps.storage, cfg.owner.as_ref(), &authorization)?;
            }
            Ok(_key) => {
                // may add expiration here instead in future version
                return Err(ContractError::CustomError {
                    val: "temporary error: auth exists".to_string(),
                });
            }
        }

        Ok(Response::default())
    }

    pub fn rm_authorization(
        &self,
        deps: DepsMut,
        info: MessageInfo,
        authorization: Authorization,
    ) -> Result<Response, ContractError> {
        if info.sender != self.cfg.load(deps.storage)?.owner {
            return Err(ContractError::Unauthorized {});
        }
        let found_auth_key = match self.find_authorization(deps.as_ref(), &authorization) {
            Err(_) => return Err(ContractError::NoSuchAuthorization {}),
            Ok(key) => key,
        };
        self.authorizations
            .remove(deps.storage, std::str::from_utf8(&found_auth_key)?)?;
        Ok(Response::default())
    }

    pub fn assert_authorized_action(
        &self,
        deps: Deps,
        sender: Addr,
        msg: PendingSubmsg,
    ) -> Result<(), ContractError> {
        let contract_address = match msg.contract_addr {
            None => return Err(ContractError::MissingContractAddress {}),
            Some(addr) => addr,
        };
        // check there is an authorization for this contract
        let authorizations: Vec<Authorization> = self
            .authorizations
            .idx
            .contract
            .prefix(deps.api.addr_validate(&contract_address)?)
            .range(deps.storage, None, None, Order::Ascending)
            .filter(|item| match item {
                Err(_) => false,
                Ok(val) => val.1.actor == sender,
            })
            .map(|item| Ok(item?.1))
            .collect::<StdResult<Vec<Authorization>>>()?;
        if authorizations.is_empty() {
            return Err(ContractError::NoSuchAuthorization {});
        }

        let msg_value: Value = match &msg.binarymsg {
            None => return Err(ContractError::MissingBinaryMessage {}),
            Some(bin) => serde_json_wasm::from_slice(&bin)?,
        };

        let msg_obj = match msg_value.as_object() {
            Some(obj) => obj,
            None => return Err(ContractError::Unauthorized {}),
        };

        // allow the msg if a matching authorization has no field reqs,
        // or if the message matches the field reqs for one authorization
        'outer: for auth in &authorizations {
            let this_auth: Authorization = auth.clone();
            match this_auth.fields {
                Some(vals) => {
                    for kv in 0..vals.len() {
                        let this_key: String = vals[kv].clone().0;
                        let this_value: String = vals[kv].clone().1;
                        if msg_obj.contains_key(&this_key) {
                            if msg_obj[&this_key] != this_value && kv == vals.len() - 1 {
                                return Err(ContractError::FieldMismatch {
                                    key: this_key,
                                    value: this_value,
                                });
                            }
                        } else {
                            return Err(ContractError::MissingRequiredField {
                                key: this_key,
                                value: this_value,
                            });
                        }
                    }
                }
                None => break 'outer,
            }
        }
        Ok(())
    }
}
