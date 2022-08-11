use schemars::JsonSchema;
use std::fmt;

#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    from_binary, to_binary, BankMsg, Binary, Coin, CosmosMsg, Deps, DepsMut, Empty, Env,
    MessageInfo, Response, StdError, StdResult, WasmMsg,
};

use cw1::CanExecuteResponse;
use cw2::set_contract_version;
use cw20::Cw20ExecuteMsg;

use crate::error::ContractError;
use crate::msg::{AdminResponse, ExecuteMsg, HotWalletsResponse, InstantiateMsg, QueryMsg};
use crate::state::{Admins, HotWallet, ADMINS, PENDING};

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
    let cfg = Admins {
        admin: deps.api.addr_validate(&msg.admin)?.to_string(),
        hot_wallets: vec![],
    };
    ADMINS.save(deps.storage, &cfg)?;
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
        ExecuteMsg::AddHotWallet { new_hot_wallet } => add_hot_wallet(deps, env, info, new_hot_wallet),
        ExecuteMsg::RmHotWallet { doomed_hot_wallet } => rm_hot_wallet(deps, env, info, doomed_hot_wallet),
        ExecuteMsg::ProposeUpdateAdmin { new_admin } => {
            propose_update_admin(deps, env, info, new_admin)
        }
        ExecuteMsg::ConfirmUpdateAdmin {} => confirm_update_admin(deps, env, info),
    }
}

pub fn execute_execute<T>(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msgs: Vec<CosmosMsg<T>>,
) -> Result<Response<T>, ContractError>
where
    T: Clone + fmt::Debug + PartialEq + JsonSchema,
{
    let mut admins = ADMINS.load(deps.storage)?;
    if admins.is_admin(info.sender.to_string()) {
        let res = Response::new()
            .add_messages(msgs)
            .add_attribute("action", "execute");
        return Ok(res);
    } else {
        //make sure the message is doing nothing else but sending
        for n in 0..msgs.len() {
            match &msgs[n] {
                // if it's a Wasm message, it needs to be Cw20 Transfer OR Send
                CosmosMsg::Wasm(wasm) => {
                    match wasm {
                        WasmMsg::Execute {
                            contract_addr,
                            msg,
                            funds,
                        } => {
                            // prevent attaching funds to tx, as Transfer message's
                            // amount param is where funds are specified, instead
                            let empty_vec: Vec<Coin> = [].to_vec();
                            if funds != &empty_vec {
                                return Err(ContractError::AttachedFundsNotAllowed {});
                            }
                            let msg_de: Result<cw20::Cw20ExecuteMsg, StdError> = from_binary(msg);
                            match msg_de {
                                Ok(msg_contents) => {
                                    // must be Transfer or Send
                                    match msg_contents {
                                        Cw20ExecuteMsg::Transfer {
                                            recipient: _,
                                            amount,
                                        } => {
                                            if admins.can_spend(
                                                env.block.time,
                                                info.sender.as_ref(),
                                                vec![Coin {
                                                    denom: contract_addr.to_string(),
                                                    amount,
                                                }],
                                            )? {
                                                let res = Response::new()
                                                    .add_messages(vec![msgs[n].clone()])
                                                    .add_attribute("action", "execute");
                                                return Ok(res);
                                            }
                                        }
                                        Cw20ExecuteMsg::Send {
                                            contract: _,
                                            amount,
                                            msg: _,
                                        } => {
                                            if admins.can_spend(
                                                env.block.time,
                                                info.sender.as_ref(),
                                                vec![Coin {
                                                    denom: contract_addr.to_string(),
                                                    amount,
                                                }],
                                            )? {
                                                let res = Response::new()
                                                    .add_messages(vec![msgs[n].clone()])
                                                    .add_attribute("action", "execute");
                                                return Ok(res);
                                            }
                                        }
                                        _ => {
                                            return Err(ContractError::OnlyTransferSendAllowed {});
                                        }
                                    }
                                }
                                Err(_) => {
                                    return Err(ContractError::ErrorDeserializingCw20Message {});
                                }
                            }
                        }
                        _ => {
                            return Err(ContractError::WasmMsgMustBeExecute {});
                        }
                    }
                }
                // otherwise it must be a bank transfer
                CosmosMsg::Bank(bank) => match bank {
                    BankMsg::Send {
                        to_address: _,
                        amount,
                    } if admins.can_spend(
                        env.block.time,
                        info.sender.as_ref(),
                        amount.clone(),
                    )? =>
                    {
                        let res = Response::new()
                            .add_messages(vec![msgs[n].clone()])
                            .add_attribute("action", "execute");
                        return Ok(res);
                    }
                    _ => {
                        //probably unreachable as can_spend tends to throw
                        return Err(ContractError::SpendNotAuthorized {});
                    }
                },
                _ => {
                    return Err(ContractError::BadMessageType {});
                }
            };
        }
    }
    Ok(Response::default())
}

pub fn add_hot_wallet(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    new_hot_wallet: HotWallet,
) -> Result<Response, ContractError> {
    let mut cfg = ADMINS.load(deps.storage)?;
    if !cfg.is_admin(info.sender.to_string()) {
        Err(ContractError::Unauthorized {})
    } else if cfg
        .hot_wallets
        .iter()
        .any(|wallet| wallet.address == new_hot_wallet.address)
    {
        Err(ContractError::HotWalletExists {})
    } else {
        cfg.add_hot_wallet(new_hot_wallet);
        Ok(Response::new().add_attribute("action", "add_hot_wallet"))
    }
}

pub fn rm_hot_wallet(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    doomed_hot_wallet: String,
) -> Result<Response, ContractError> {
    let mut cfg = ADMINS.load(deps.storage)?;
    if !cfg.is_admin(info.sender.to_string()) {
        Err(ContractError::Unauthorized {})
    } else if !cfg
        .hot_wallets
        .iter()
        .any(|wallet| wallet.address == doomed_hot_wallet)
    {
        Err(ContractError::HotWalletDoesNotExist {})
    } else {
        cfg.rm_hot_wallet(doomed_hot_wallet);
        Ok(Response::new().add_attribute("action", "rm_hot_wallet"))
    }
}

pub fn propose_update_admin(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    new_admin: String,
) -> Result<Response, ContractError> {
    let mut cfg = ADMINS.load(deps.storage)?;
    if !cfg.is_admin(info.sender.to_string()) {
        Err(ContractError::Unauthorized {})
    } else {
        cfg.admin = deps.api.addr_validate(&new_admin)?.to_string();
        PENDING.save(deps.storage, &cfg)?;

        let res = Response::new().add_attribute("action", "propose_update_admin");
        Ok(res)
    }
}

pub fn confirm_update_admin(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    let cfg = PENDING.load(deps.storage)?;
    if !cfg.is_admin(info.sender.to_string()) {
        Err(ContractError::CallerIsNotPendingNewAdmin {})
    } else {
        ADMINS.save(deps.storage, &cfg)?;

        let res = Response::new().add_attribute("action", "confirm_update_admin");
        Ok(res)
    }
}

fn can_execute(deps: Deps, sender: &str) -> StdResult<bool> {
    let cfg = ADMINS.load(deps.storage)?;
    let can = cfg.is_admin(sender.to_string());
    Ok(can)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Admin {} => to_binary(&query_admin(deps)?),
        QueryMsg::CanExecute { sender, msg } => to_binary(&query_can_execute(deps, sender, msg)?),
        QueryMsg::HotWallets { } => to_binary(&query_hot_wallets(deps)?),
    }
}

pub fn query_admin(deps: Deps) -> StdResult<AdminResponse> {
    let cfg = ADMINS.load(deps.storage)?;
    Ok(AdminResponse { admin: cfg.admin })
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

pub fn query_hot_wallets(deps: Deps) -> StdResult<HotWalletsResponse> {
    let cfg = ADMINS.load(deps.storage)?;
    Ok(HotWalletsResponse {
        hot_wallets: cfg.hot_wallets,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
    use cosmwasm_std::{coin, coins, BankMsg, StakingMsg, SubMsg};
    //use cosmwasm_std::WasmMsg;

    #[test]
    fn instantiate_and_modify_admin() {
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

        // anyone cannot propose updating admin on the contract
        let msg = ExecuteMsg::ProposeUpdateAdmin {
            new_admin: anyone.to_string(),
        };
        let info = mock_info(anyone, &[]);
        let err = execute(deps.as_mut(), mock_env(), info, msg).unwrap_err();
        assert_eq!(err, ContractError::Unauthorized {});

        // but alice can propose an update
        let msg = ExecuteMsg::ProposeUpdateAdmin {
            new_admin: bob.to_string(),
        };
        let info = mock_info(alice, &[]);
        execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        // now, the admin isn't updated yet
        let expected = AdminResponse {
            admin: alice.to_string(),
        };
        assert_eq!(query_admin(deps.as_ref()).unwrap(), expected);

        // but if bob accepts...
        let msg = ExecuteMsg::ConfirmUpdateAdmin {};
        let info = mock_info(bob, &[]);
        execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        // then admin is updated
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
