pub mod authorizations;
pub mod constants;
pub mod contract;
pub mod error;
pub mod hot_wallet;
#[cfg(test)]
mod integration_tests;
pub mod msg;
pub mod pair_contract;
pub mod pair_contract_defaults;
pub mod signers;
pub mod simulation;
pub mod sourced_coin;
pub mod sources;
pub mod state;
pub mod submsgs;
#[cfg(test)]
pub mod tests_constants;
#[cfg(test)]
mod tests_contract;
#[cfg(test)]
pub mod tests_helpers;
#[cfg(test)]
mod tests_hot_wallet;
#[cfg(test)]
mod tests_pair_contract;
#[cfg(test)]
mod tests_signers;
#[cfg(test)]
mod tests_state;

pub use crate::error::ContractError;
pub use serde_json_value_wasm;

#[cfg(not(feature = "library"))]
pub mod entry {
    use crate::msg::ExecuteMsg;
    use crate::msg::InstantiateMsg;
    use crate::msg::MigrateMsg;
    use crate::msg::QueryMsg;
    use crate::state::ObiProxyContract;

    use super::*;
    use cosmwasm_std::entry_point;
    use cosmwasm_std::{Binary, Deps, DepsMut, Env, MessageInfo, Response, StdResult};

    // This makes a conscious choice on the various generics used by the contract
    #[entry_point]
    pub fn instantiate(
        deps: DepsMut,
        env: Env,
        info: MessageInfo,
        msg: InstantiateMsg,
    ) -> StdResult<Response> {
        let obi = ObiProxyContract::default();
        obi.instantiate(deps, env, info, msg)
    }

    #[entry_point]
    pub fn execute(
        deps: DepsMut,
        env: Env,
        info: MessageInfo,
        msg: ExecuteMsg,
    ) -> Result<Response, ContractError> {
        let obi = ObiProxyContract::default();
        obi.execute(deps, env, info, msg)
    }

    #[entry_point]
    pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
        let obi = ObiProxyContract::default();
        obi.query(deps, env, msg)
    }

    #[entry_point]
    pub fn migrate(_deps: DepsMut, _env: Env, _msg: MigrateMsg) -> StdResult<Response> {
        Ok(Response::default())
    }
}
