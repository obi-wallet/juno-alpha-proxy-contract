#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    from_binary, to_binary, BankMsg, Binary, Coin, CosmosMsg, Deps, DepsMut, Empty, Env,
    MessageInfo, Response, StdError, StdResult, Timestamp, WasmMsg,
};

use cw1::CanExecuteResponse;
use cw2::set_contract_version;
use cw20::Cw20ExecuteMsg;

use crate::error::ContractError;
use crate::msg::{
    AdminResponse, ExecuteMsg, HotWalletsResponse, InstantiateMsg, MigrateMsg, QueryMsg,
};
use crate::state::{Admins, HotWallet, ADMINS, PENDING};

// version info for migration info
const CONTRACT_NAME: &str = "obi-proxy-contract";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

struct CorePayload {
    cfg: Admins,
    info: MessageInfo,
    this_msg: CosmosMsg,
    current_time: Timestamp,
}

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
        hot_wallets: msg.hot_wallets,
    };
    ADMINS.save(deps.storage, &cfg)?;
    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(_deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    // No state migrations performed right now, just return a Response
    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::Execute { msgs } => execute_execute(&mut deps, env, info, msgs),
        ExecuteMsg::AddHotWallet { new_hot_wallet } => {
            add_hot_wallet(deps, env, info, new_hot_wallet)
        }
        ExecuteMsg::RmHotWallet { doomed_hot_wallet } => {
            rm_hot_wallet(deps, env, info, doomed_hot_wallet)
        }
        ExecuteMsg::ProposeUpdateAdmin { new_admin } => {
            propose_update_admin(deps, env, info, new_admin)
        }
        ExecuteMsg::ConfirmUpdateAdmin {} => confirm_update_admin(deps, env, info),
    }
}

pub fn execute_execute(
    deps: &mut DepsMut,
    env: Env,
    info: MessageInfo,
    msgs: Vec<CosmosMsg>,
) -> Result<Response, ContractError> {
    let cfg = ADMINS.load(deps.storage)?;
    if cfg.is_admin(info.sender.to_string()) {
        let res = Response::new()
            .add_messages(msgs)
            .add_attribute("action", "execute_execute");
        Ok(res)
    } else {
        let mut core_payload = CorePayload {
            cfg,
            info,
            this_msg: CosmosMsg::Custom(Empty {}),
            current_time: env.block.time,
        };
        //make sure the message is a send of some kind
        let mut res = Response::new().add_attribute("action", "execute_spend_limit");
        for this_msg in msgs {
            core_payload.this_msg = this_msg.clone();
            match &this_msg {
                // if it's a Wasm message, it needs to be Cw20 Transfer OR Send
                CosmosMsg::Wasm(wasm) => {
                    let partial_res = try_wasm_send(deps, wasm, &mut core_payload)?;
                    res = res.add_message(partial_res.messages[0].msg.clone());
                }
                // otherwise it must be a bank transfer
                CosmosMsg::Bank(bank) => {
                    let partial_res = try_bank_send(deps, bank, &mut core_payload)?;
                    res = res.add_message(partial_res.messages[0].msg.clone());
                }
                _ => {
                    return Err(ContractError::BadMessageType {});
                }
            };
        }
        Ok(res)
    }
}

fn try_wasm_send(
    deps: &mut DepsMut,
    wasm: &WasmMsg,
    core_payload: &mut CorePayload,
) -> Result<Response, ContractError> {
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
                        } => check_and_spend(
                            deps,
                            core_payload,
                            vec![Coin {
                                denom: contract_addr.to_string(),
                                amount,
                            }],
                        ),
                        Cw20ExecuteMsg::Send {
                            contract: _,
                            amount,
                            msg: _,
                        } => check_and_spend(
                            deps,
                            core_payload,
                            vec![Coin {
                                denom: contract_addr.to_string(),
                                amount,
                            }],
                        ),
                        _ => Err(ContractError::OnlyTransferSendAllowed {}),
                    }
                }
                Err(_) => Err(ContractError::ErrorDeserializingCw20Message {}),
            }
        }
        _ => Err(ContractError::WasmMsgMustBeExecute {}),
    }
}

fn try_bank_send(
    deps: &mut DepsMut,
    bank: &BankMsg,
    core_payload: &mut CorePayload,
) -> Result<Response, ContractError> {
    match bank {
        BankMsg::Send {
            to_address: _,
            amount,
        } => check_and_spend(deps, core_payload, amount.clone()),
        _ => {
            //probably unreachable as can_spend throws
            Err(ContractError::SpendNotAuthorized {})
        }
    }
}

fn check_and_spend(
    deps: &mut DepsMut,
    core_payload: &mut CorePayload,
    spend: Vec<Coin>,
) -> Result<Response, ContractError> {
    core_payload.cfg.can_spend(
        core_payload.current_time,
        core_payload.info.sender.to_string(),
        spend,
    )?;
    let res = Response::new()
        .add_messages(vec![core_payload.this_msg.clone()])
        .add_attribute("action", "execute");
    ADMINS.save(deps.storage, &core_payload.cfg)?;
    Ok(res)
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
        let _addrcheck = deps.api.addr_validate(&new_hot_wallet.address)?;
        cfg.add_hot_wallet(new_hot_wallet);
        ADMINS.save(deps.storage, &cfg)?;
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
        ADMINS.save(deps.storage, &cfg)?;
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
        QueryMsg::HotWallets {} => to_binary(&query_hot_wallets(deps)?),
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
    use crate::state::{CoinLimit, PeriodType};

    use super::*;
    use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info, MockApi, MockQuerier};
    use cosmwasm_std::{coin, coins, BankMsg, Empty, MemoryStorage, OwnedDeps, StakingMsg, SubMsg};
    //use cosmwasm_std::WasmMsg;

    const ADMIN: &str = "alice";
    const NEW_ADMIN: &str = "bob";
    const HOT_WALLET: &str = "hotcarl";
    const ANYONE: &str = "anyone";
    const RECEIVER: &str = "diane";

    #[test]
    fn instantiate_and_modify_admin() {
        let mut deps = mock_dependencies();
        let current_env = mock_env();
        instantiate_contract(&mut deps, current_env);

        // ensure expected config
        let expected = AdminResponse {
            admin: ADMIN.to_string(),
        };
        assert_eq!(query_admin(deps.as_ref()).unwrap(), expected);

        // anyone cannot propose updating admin on the contract
        let msg = ExecuteMsg::ProposeUpdateAdmin {
            new_admin: ANYONE.to_string(),
        };
        let info = mock_info(ANYONE, &[]);
        let err = execute(deps.as_mut(), mock_env(), info, msg).unwrap_err();
        assert_eq!(err, ContractError::Unauthorized {});

        // but alice can propose an update
        let msg = ExecuteMsg::ProposeUpdateAdmin {
            new_admin: NEW_ADMIN.to_string(),
        };
        let info = mock_info(ADMIN, &[]);
        execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        // now, the admin isn't updated yet
        let expected = AdminResponse {
            admin: ADMIN.to_string(),
        };
        assert_eq!(query_admin(deps.as_ref()).unwrap(), expected);

        // but if bob accepts...
        let msg = ExecuteMsg::ConfirmUpdateAdmin {};
        let info = mock_info(NEW_ADMIN, &[]);
        execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        // then admin is updated
        let expected = AdminResponse {
            admin: NEW_ADMIN.to_string(),
        };
        assert_eq!(query_admin(deps.as_ref()).unwrap(), expected);
    }

    #[test]
    fn execute_messages_has_proper_permissions() {
        let mut deps = mock_dependencies();
        let current_env = mock_env();
        instantiate_contract(&mut deps, current_env);

        let msgs = vec![
            BankMsg::Send {
                to_address: RECEIVER.to_string(),
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

        // receiver or anyone else cannot execute them ... and gets HotWalletDoesNotExist since
        // this is a spend, so contract assumes we're trying against spend limit
        // if not admin
        let info = mock_info(RECEIVER, &[]);
        let err = execute(deps.as_mut(), mock_env(), info, execute_msg.clone()).unwrap_err();
        assert_eq!(err, ContractError::HotWalletDoesNotExist {});

        // but admin can
        let info = mock_info(ADMIN, &[]);
        let res = execute(deps.as_mut(), mock_env(), info, execute_msg).unwrap();
        assert_eq!(
            res.messages,
            msgs.into_iter().map(SubMsg::new).collect::<Vec<_>>()
        );
        assert_eq!(res.attributes, [("action", "execute_execute")]);
    }

    #[test]
    fn can_execute_query_works() {
        let mut deps = mock_dependencies();
        let current_env = mock_env();
        instantiate_contract(&mut deps, current_env);

        // let us make some queries... different msg types by owner and by other
        let send_msg = CosmosMsg::Bank(BankMsg::Send {
            to_address: ANYONE.to_string(),
            amount: coins(12345, "ushell"),
        });
        let staking_msg = CosmosMsg::Staking(StakingMsg::Delegate {
            validator: ANYONE.to_string(),
            amount: coin(70000, "ureef"),
        });

        // owner can send
        let res = query_can_execute(deps.as_ref(), ADMIN.to_string(), send_msg.clone()).unwrap();
        assert!(res.can_execute);

        // owner can stake
        let res = query_can_execute(deps.as_ref(), ADMIN.to_string(), staking_msg.clone()).unwrap();
        assert!(res.can_execute);

        // anyone cannot send
        let res = query_can_execute(deps.as_ref(), ANYONE.to_string(), send_msg).unwrap();
        assert!(!res.can_execute);

        // anyone cannot stake
        let res = query_can_execute(deps.as_ref(), ANYONE.to_string(), staking_msg).unwrap();
        assert!(!res.can_execute);
    }

    #[test]
    fn add_spend_rm_hot_wallet() {
        let mut deps = mock_dependencies();
        let current_env = mock_env();

        instantiate_contract(&mut deps, current_env.clone());
        // this helper includes a hotwallet

        // query to see we have "hotcarl" as hot wallet
        let res = query_hot_wallets(deps.as_ref()).unwrap();
        assert!(res.hot_wallets.len() == 1);
        assert!(res.hot_wallets[0].address == HOT_WALLET);

        // spend as the hot wallet
        let send_msg = CosmosMsg::Bank(BankMsg::Send {
            to_address: RECEIVER.to_string(),
            amount: coins(999_000u128, "testtokens"), // only 1_000 left!
        });
        let info = mock_info(ADMIN, &[]);
        let res = execute_execute(
            &mut deps.as_mut(),
            current_env.clone(),
            info.clone(),
            vec![send_msg],
        )
        .unwrap();
        assert!(res.messages.len() == 1);
        let submsg = res.messages[0].clone();
        match submsg.msg {
            CosmosMsg::Bank(BankMsg::Send {
                to_address: _,
                amount: _,
            }) => (),
            _ => {
                panic!(
                    "We sent a send bankmsg but that's not the first submessage for some reason"
                );
            }
        }

        // add a second hot wallet
        let execute_msg = ExecuteMsg::AddHotWallet {
            new_hot_wallet: HotWallet {
                address: "hot_diane".to_string(),
                current_period_reset: current_env.block.time.seconds() as u64,
                period_type: PeriodType::DAYS,
                period_multiple: 1,
                spend_limits: vec![CoinLimit {
                    denom: "testtokens".to_string(),
                    amount: 1_000_000u64,
                    limit_remaining: 1_000_000u64,
                }],
            },
        };
        let _res = execute(
            deps.as_mut(),
            current_env.clone(),
            info.clone(),
            execute_msg,
        )
        .unwrap();
        let res = query_hot_wallets(deps.as_ref()).unwrap();
        assert!(res.hot_wallets.len() == 2);

        // rm the hot wallet
        let bad_info = mock_info(ANYONE, &[]);
        let execute_msg = ExecuteMsg::RmHotWallet {
            doomed_hot_wallet: HOT_WALLET.to_string(),
        };
        let _res = execute(
            deps.as_mut(),
            current_env.clone(),
            bad_info,
            execute_msg.clone(),
        )
        .unwrap_err();
        let _res = execute(deps.as_mut(), current_env, info, execute_msg).unwrap();

        // query hot wallets again, should be 0
        let res = query_hot_wallets(deps.as_ref()).unwrap();
        println!("hot wallets are: {:?}", res.hot_wallets);
        assert!(res.hot_wallets.len() == 1);
    }

    fn instantiate_contract(
        deps: &mut OwnedDeps<MemoryStorage, MockApi, MockQuerier<Empty>, Empty>,
        env: Env,
    ) {
        // instantiate the contract
        let instantiate_msg = InstantiateMsg {
            admin: ADMIN.to_string(),
            hot_wallets: vec![HotWallet {
                address: HOT_WALLET.to_string(),
                current_period_reset: env.block.time.seconds() as u64, // this is fine since it will calc on first spend
                period_type: PeriodType::DAYS,
                period_multiple: 1,
                spend_limits: vec![CoinLimit {
                    denom: "testtokens".to_string(),
                    amount: 1_000_000u64,
                    limit_remaining: 1_000_000u64,
                }],
            }],
        };
        let info = mock_info(ADMIN, &[]);
        instantiate(deps.as_mut(), mock_env(), info, instantiate_msg).unwrap();
    }
}
