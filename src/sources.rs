use cosmwasm_std::Attribute;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::sourced_coin::SourcedCoin;

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, JsonSchema, Debug)]
pub struct Source {
    pub contract_addr: String,
    pub query_msg: String,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, JsonSchema, Debug)]
pub struct Sources {
    pub sources: Vec<Source>,
}

impl Sources {
    pub fn append_sources(&mut self, sourced_coin: SourcedCoin) {
        for m in 0..sourced_coin.wrapped_sources.sources.len() {
            self.sources.push(Source {
                contract_addr: sourced_coin.wrapped_sources.sources[m]
                    .contract_addr
                    .clone(),
                query_msg: sourced_coin.wrapped_sources.sources[m].query_msg.clone(),
            });
        }
    }

    pub fn to_attributes(&self) -> Vec<Attribute> {
        let mut attributes: Vec<Attribute> = vec![];
        for source in self.sources.clone() {
            attributes.push(Attribute::new(source.contract_addr, source.query_msg));
        }
        attributes
    }
}
