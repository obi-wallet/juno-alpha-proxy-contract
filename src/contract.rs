use schemars::JsonSchema;
use std::fmt;

#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    to_binary, Binary, CosmosMsg, Deps, DepsMut, Empty, Env, MessageInfo, Response,
    StdResult,
};

use cw1::CanExecuteResponse;
use cw2::set_contract_version;

use crate::error::ContractError;
use crate::msg::{AdminResponse, ExecuteMsg, InstantiateMsg, QueryMsg};
use crate::state::{Admin, ADMIN};

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:cw1-whitelist";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<Response> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    let cfg = Admin {
        admin: deps.api.addr_validate(&msg.admin)?.to_string(),
    };
    ADMIN.save(deps.storage, &cfg)?;
    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    // Note: implement this function with different type to add support for custom messages
    // and then import the rest of this contract code.
    msg: ExecuteMsg<Empty>,
) -> Result<Response<Empty>, ContractError> {
    match msg {
        ExecuteMsg::Execute { msgs } => execute_execute(deps, env, info, msgs),
        ExecuteMsg::UpdateAdmin { new_admin } => execute_update_admin(deps, env, info, new_admin),
        //ExecuteMsg::ProposeUpdateAdmin { admin } => propose_update_admin(deps, env, info, admin),
        //ExecuteMsg::ConfirmUpdateAdmin { admin } => confirm_update_admin(deps, env, info),
    }
}

pub fn execute_execute<T>(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msgs: Vec<CosmosMsg<T>>,
) -> Result<Response<T>, ContractError>
where
    T: Clone + fmt::Debug + PartialEq + JsonSchema,
{
    if !can_execute(deps.as_ref(), info.sender.as_ref())? {
        Err(ContractError::Unauthorized {})
    } else {
        let res = Response::new()
            .add_messages(msgs)
            .add_attribute("action", "execute");
        Ok(res)
    }
}

pub fn execute_update_admin(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    admin: String,
) -> Result<Response, ContractError> {
    let mut cfg = ADMIN.load(deps.storage)?;
    if !cfg.is_admin(info.sender.to_string()) {
        Err(ContractError::Unauthorized {})
    } else {
        cfg.admin = deps.api.addr_validate(&admin)?.to_string();
        ADMIN.save(deps.storage, &cfg)?;

        let res = Response::new().add_attribute("action", "update_admins");
        Ok(res)
    }
}

fn can_execute(deps: Deps, sender: &str) -> StdResult<bool> {
    let cfg = ADMIN.load(deps.storage)?;
    let can = cfg.is_admin(sender.to_string());
    Ok(can)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Admin {} => to_binary(&query_admin(deps)?),
        QueryMsg::CanExecute { sender, msg } => to_binary(&query_can_execute(deps, sender, msg)?),
    }
}

pub fn query_admin(deps: Deps) -> StdResult<AdminResponse> {
    let cfg = ADMIN.load(deps.storage)?;
    Ok(AdminResponse {
        admin: cfg.admin,
    })
}

pub fn query_can_execute(
    deps: Deps,
    sender: String,
    _msg: CosmosMsg,
) -> StdResult<CanExecuteResponse> {
    Ok(CanExecuteResponse {
        can_execute: can_execute(deps, &sender)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
    use cosmwasm_std::{coin, coins, BankMsg, StakingMsg, SubMsg};
    //use cosmwasm_std::WasmMsg;

    #[test]
    fn instantiate_and_modify_config() {
        let mut deps = mock_dependencies();

        let alice = "alice";
        let bob = "bob";

        let anyone = "anyone";

        // instantiate the contract
        let instantiate_msg = InstantiateMsg {
            admin: alice.to_string(),
        };
        let info = mock_info(anyone, &[]);
        instantiate(deps.as_mut(), mock_env(), info, instantiate_msg).unwrap();

        // ensure expected config
        let expected = AdminResponse {
            admin: alice.to_string(),
        };
        assert_eq!(query_admin(deps.as_ref()).unwrap(), expected);

        // anyone cannot modify the contract
        let msg = ExecuteMsg::UpdateAdmin {
            new_admin: anyone.to_string(),
        };
        let info = mock_info(anyone, &[]);
        let err = execute(deps.as_mut(), mock_env(), info, msg).unwrap_err();
        assert_eq!(err, ContractError::Unauthorized {});

        // but alice can update
        let msg = ExecuteMsg::UpdateAdmin {
            new_admin: bob.to_string(),
        };
        let info = mock_info(alice, &[]);
        execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        // ensure expected config
        let expected = AdminResponse {
            admin: bob.to_string(),
        };
        assert_eq!(query_admin(deps.as_ref()).unwrap(), expected);
    }

    #[test]
    fn execute_messages_has_proper_permissions() {
        let mut deps = mock_dependencies();

        let alice = "alice";
        let bob = "bob";

        // instantiate the contract
        let instantiate_msg = InstantiateMsg {
            admin: alice.to_string(),
        };
        let info = mock_info(bob, &[]);
        instantiate(deps.as_mut(), mock_env(), info, instantiate_msg).unwrap();

        let msgs = vec![
            BankMsg::Send {
                to_address: bob.to_string(),
                amount: coins(10000, "DAI"),
            }
            .into(),
            /*WasmMsg::Execute {
                contract_addr: "some contract".into(),
                msg: to_binary(&freeze).unwrap(),
                funds: vec![],
            }
            .into(),*/
        ];

        // make some nice message
        let execute_msg = ExecuteMsg::Execute { msgs: msgs.clone() };

        // bob cannot execute them
        let info = mock_info(bob, &[]);
        let err = execute(deps.as_mut(), mock_env(), info, execute_msg.clone()).unwrap_err();
        assert_eq!(err, ContractError::Unauthorized {});

        // but alice can
        let info = mock_info(alice, &[]);
        let res = execute(deps.as_mut(), mock_env(), info, execute_msg).unwrap();
        assert_eq!(
            res.messages,
            msgs.into_iter().map(SubMsg::new).collect::<Vec<_>>()
        );
        assert_eq!(res.attributes, [("action", "execute")]);
    }

    #[test]
    fn can_execute_query_works() {
        let mut deps = mock_dependencies();

        let alice = "alice";

        let anyone = "anyone";

        // instantiate the contract
        let instantiate_msg = InstantiateMsg {
            admin: alice.to_string(),
        };
        let info = mock_info(anyone, &[]);
        instantiate(deps.as_mut(), mock_env(), info, instantiate_msg).unwrap();

        // let us make some queries... different msg types by owner and by other
        let send_msg = CosmosMsg::Bank(BankMsg::Send {
            to_address: anyone.to_string(),
            amount: coins(12345, "ushell"),
        });
        let staking_msg = CosmosMsg::Staking(StakingMsg::Delegate {
            validator: anyone.to_string(),
            amount: coin(70000, "ureef"),
        });

        // owner can send
        let res = query_can_execute(deps.as_ref(), alice.to_string(), send_msg.clone()).unwrap();
        assert!(res.can_execute);

        // owner can stake
        let res = query_can_execute(deps.as_ref(), alice.to_string(), staking_msg.clone()).unwrap();
        assert!(res.can_execute);

        // anyone cannot send
        let res = query_can_execute(deps.as_ref(), anyone.to_string(), send_msg).unwrap();
        assert!(!res.can_execute);

        // anyone cannot stake
        let res = query_can_execute(deps.as_ref(), anyone.to_string(), staking_msg).unwrap();
        assert!(!res.can_execute);
    }
}
