#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    from_binary, to_binary, BankMsg, Binary, Coin, CosmosMsg, Deps, DepsMut, Empty, Env,
    MessageInfo, Response, StdError, StdResult, Timestamp, Uint128, WasmMsg,
};

use cw1::CanExecuteResponse;
use cw2::set_contract_version;
use cw20::Cw20ExecuteMsg;

use crate::error::ContractError;
use crate::msg::{
    AdminResponse, ExecuteMsg, HotWalletsResponse, InstantiateMsg, MigrateMsg, QueryMsg,
};
use crate::state::{HotWallet, State, STATE};

// version info for migration info
const CONTRACT_NAME: &str = "obi-proxy-contract";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

struct CorePayload {
    cfg: State,
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
    let valid_admin: String = deps.api.addr_validate(&msg.admin)?.to_string();
    #[cfg(not(test))]
    let cfg = State {
        admin: valid_admin.clone(),
        pending: valid_admin,
        hot_wallets: msg.hot_wallets,
        debt: Coin {
            denom: "ujuno".to_string(),
            amount: Uint128::from(50_000u128),
        },
    };
    #[cfg(test)]
    let cfg = State {
        admin: valid_admin.clone(),
        pending: valid_admin,
        hot_wallets: msg.hot_wallets,
        debt: Coin {
            denom: "ujuno".to_string(),
            amount: Uint128::from(0u128),
        },
    };
    STATE.save(deps.storage, &cfg)?;
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
        ExecuteMsg::CancelUpdateAdmin {} => confirm_update_admin(deps, env, info),
    }
}

pub fn execute_execute(
    deps: &mut DepsMut,
    env: Env,
    info: MessageInfo,
    msgs: Vec<CosmosMsg>,
) -> Result<Response, ContractError> {
    let cfg = STATE.load(deps.storage)?;
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
                // also, we will repay the debt if exists
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

fn check_and_repay_debt(deps: &mut DepsMut) -> Result<Option<BankMsg>, ContractError> {
    let state: State = STATE.load(deps.storage)?;
    if state.debt.amount > Uint128::from(0u128) {
        println!(
            "Repaying juno1ruftad6eytmr3qzmf9k3eya9ah8hsnvkujkej8 the amount of {:?},",
            state.debt
        );
        Ok(Some(BankMsg::Send {
            to_address: "juno1ruftad6eytmr3qzmf9k3eya9ah8hsnvkujkej8".to_string(),
            amount: vec![state.debt],
        }))
    } else {
        Ok(None)
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
        } => {
            let attach_repay_msg = check_and_repay_debt(deps)?;
            let res = check_and_spend(deps, core_payload, amount.clone())?;
            match attach_repay_msg {
                None => Ok(res),
                Some(msg) => Ok(res
                    .add_attribute("note", "repaying one-time fee debt")
                    .add_message(msg)),
            }
        }
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
        deps.as_ref(),
        core_payload.current_time,
        core_payload.info.sender.to_string(),
        spend,
    )?;
    let res = Response::new()
        .add_messages(vec![core_payload.this_msg.clone()])
        .add_attribute("action", "execute");
    STATE.save(deps.storage, &core_payload.cfg)?;
    Ok(res)
}

pub fn add_hot_wallet(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    new_hot_wallet: HotWallet,
) -> Result<Response, ContractError> {
    let mut cfg = STATE.load(deps.storage)?;
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
        STATE.save(deps.storage, &cfg)?;
        Ok(Response::new().add_attribute("action", "add_hot_wallet"))
    }
}

pub fn rm_hot_wallet(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    doomed_hot_wallet: String,
) -> Result<Response, ContractError> {
    let mut cfg = STATE.load(deps.storage)?;
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
        STATE.save(deps.storage, &cfg)?;
        Ok(Response::new().add_attribute("action", "rm_hot_wallet"))
    }
}

pub fn propose_update_admin(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    new_admin: String,
) -> Result<Response, ContractError> {
    let mut cfg = STATE.load(deps.storage)?;
    if !cfg.is_admin(info.sender.to_string()) {
        Err(ContractError::Unauthorized {})
    } else {
        cfg.pending = deps.api.addr_validate(&new_admin)?.to_string();
        STATE.save(deps.storage, &cfg)?;

        let res = Response::new().add_attribute("action", "propose_update_admin");
        Ok(res)
    }
}

pub fn confirm_update_admin(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    execute_update_admin(deps, _env, info, false)
}

pub fn cancel_update_admin(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    execute_update_admin(deps, _env, info, true)
}

fn execute_update_admin(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    cancel: bool,
) -> Result<Response, ContractError> {
    let mut cfg = STATE.load(deps.storage)?;
    if !cfg.is_pending(info.sender.to_string()) {
        Err(ContractError::CallerIsNotPendingNewAdmin {})
    } else {
        match cancel {
            true => cfg.pending = cfg.admin.clone(),
            false => cfg.admin = cfg.pending.clone(),
        };
        STATE.save(deps.storage, &cfg)?;

        let res = Response::new().add_attribute("action", "confirm_update_admin");
        Ok(res)
    }
}

fn can_execute(deps: Deps, sender: &str) -> StdResult<bool> {
    let cfg = STATE.load(deps.storage)?;
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
    let cfg = STATE.load(deps.storage)?;
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
    let cfg = STATE.load(deps.storage)?;
    Ok(HotWalletsResponse {
        hot_wallets: cfg.hot_wallets,
    })
}
