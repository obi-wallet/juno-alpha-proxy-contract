#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    from_binary, to_binary, Addr, BankMsg, Binary, Coin, CosmosMsg, Deps, DepsMut, Empty, Env,
    MessageInfo, Response, StdError, StdResult, Timestamp, Uint128, WasmMsg,
};

use cw1::CanExecuteResponse;
use cw2::set_contract_version;
use cw20::Cw20ExecuteMsg;

use crate::constants::MAINNET_AXLUSDC_IBC;
use crate::error::ContractError;
use crate::helpers::convert_coin_to_usdc;
use crate::msg::{
    AdminResponse, ExecuteMsg, HotWalletsResponse, InstantiateMsg, MigrateMsg, QueryMsg,
};
use crate::state::{HotWallet, SourcedCoin, SourcedSwap, State, STATE};

// version info for migration info
const CONTRACT_NAME: &str = "obi-proxy-contract";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

struct CorePayload {
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
    let valid_admin: Addr = deps.api.addr_validate(&msg.admin)?;
    let valid_repay_wallet: Addr = deps.api.addr_validate(&msg.fee_lend_repay_wallet)?;
    for wallet in msg.hot_wallets.clone() {
        wallet.check_is_valid()?;
    }
    let mut cfg = State {
        admin: valid_admin.clone(),
        pending: valid_admin,
        hot_wallets: msg.hot_wallets,
        uusd_fee_debt: msg.uusd_fee_debt,
        fee_lend_repay_wallet: valid_repay_wallet,
        home_network: msg.home_network,
        pair_contracts: vec![],
    };
    cfg.set_pair_contracts(cfg.home_network.clone())?;
    STATE.save(deps.storage, &cfg)?;
    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(_deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    let mut cfg = STATE.load(_deps.storage)?;
    cfg.set_pair_contracts(cfg.home_network.clone())?;
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
        ExecuteMsg::Execute { msgs } => execute_execute(&mut deps, env, info, msgs, false),
        ExecuteMsg::SimExecute { msgs } => execute_execute(&mut deps, env, info, msgs, true),
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

// Simulation gatekeeping is all in this block
pub fn execute_execute(
    deps: &mut DepsMut,
    env: Env,
    info: MessageInfo,
    msgs: Vec<CosmosMsg>,
    simulation: bool,
) -> Result<Response, ContractError> {
    let cfg = STATE.load(deps.storage)?;
    let mut res = Response::new();
    if cfg.uusd_fee_debt == Uint128::from(0u128) && cfg.is_admin(info.sender.to_string()) {
        // if there is no debt AND user is admin, process immediately
        res = res.add_attribute("action", "execute_execute");
        if !simulation {
            res = res.add_messages(msgs);
        }
        Ok(res)
    } else {
        // otherwise, we need to do some checking. Note that attaching
        // fee repayment is handled in the try_bank_send and (todo)
        // the try_wasm_send functions
        let mut core_payload = CorePayload {
            info: info.clone(),
            this_msg: CosmosMsg::Custom(Empty {}),
            current_time: env.block.time,
        };
        //make sure the message is a send of some kind
        for this_msg in msgs {
            core_payload.this_msg = this_msg.clone();
            match &this_msg {
                // if it's a Wasm message, it needs to be Cw20 Transfer OR Send
                CosmosMsg::Wasm(wasm) => {
                    let partial_res = try_wasm_send(deps, wasm, &mut core_payload)?;
                    res = res.add_attribute("action", "execute_spend_limit_or_debt");
                    if !simulation {
                        res = res.add_message(partial_res.messages[0].msg.clone());
                    }
                    res = res.add_attributes(partial_res.attributes);
                }
                // otherwise it must be a bank transfer
                CosmosMsg::Bank(bank) => {
                    res = res.add_attribute("action", "execute_spend_limit_or_debt");
                    let partial_res = try_bank_send(deps, bank, &mut core_payload)?;
                    if !simulation {
                        for submsg in partial_res.messages {
                            res = res.add_message(submsg.msg.clone());
                        }
                    }
                    res = res.add_attributes(partial_res.attributes);
                }
                _ => {
                    if cfg.is_admin(info.sender.to_string()) {
                        res = res.add_attribute("action", "execute_execute");
                        if !simulation {
                            res = res.add_message(this_msg);
                        }
                    } else {
                        return Err(ContractError::BadMessageType {});
                    }
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

pub struct SourcedRepayMsg {
    pub repay_msg: Option<BankMsg>,
    pub top_sourced_swap: SourcedSwap,
    pub bottom_sourced_swap: SourcedSwap,
}

fn check_and_repay_debt(deps: &mut DepsMut, asset: Coin) -> Result<SourcedRepayMsg, ContractError> {
    let state: State = STATE.load(deps.storage)?;
    if state.uusd_fee_debt.u128() > 0u128 {
        let swaps = match asset.denom.as_str() {
            val if val == MAINNET_AXLUSDC_IBC => SourcedCoin {
                coin: Coin {
                    denom: MAINNET_AXLUSDC_IBC.to_string(),
                    amount: state.uusd_fee_debt,
                },
                top: SourcedSwap {
                    coin: Coin {
                        amount: state.uusd_fee_debt,
                        denom: asset.denom.clone(),
                    },
                    contract_addr: "1 USDC is 1 USDC".to_string(),
                },
                bottom: SourcedSwap {
                    coin: Coin {
                        amount: state.uusd_fee_debt,
                        denom: asset.denom,
                    },
                    contract_addr: "Yup, still 1 USDC".to_string(),
                },
            },
            "ujuno" | "ujunox" | "testtokens" => convert_coin_to_usdc(
                deps.as_ref(),
                asset.denom.clone(),
                state.uusd_fee_debt,
                true,
            )?,
            _ => return Err(ContractError::RepayFeesFirst(state.uusd_fee_debt.u128())), // todo: more general handling
        };
        let mut new_state = state.clone();
        new_state.uusd_fee_debt = Uint128::from(0u128);
        STATE.save(deps.storage, &new_state)?;
        Ok(SourcedRepayMsg {
            repay_msg: Some(BankMsg::Send {
                to_address: state.fee_lend_repay_wallet.to_string(),
                amount: vec![swaps.coin.clone()],
            }),
            top_sourced_swap: swaps.top,
            bottom_sourced_swap: swaps.bottom,
        })
    } else {
        Ok(SourcedRepayMsg {
            repay_msg: None,
            top_sourced_swap: SourcedSwap {
                coin: Coin {
                    denom: "no debt".to_string(),
                    amount: Uint128::from(0u128),
                },
                contract_addr: "no debt".to_string(),
            },
            bottom_sourced_swap: SourcedSwap {
                coin: Coin {
                    denom: "no debt".to_string(),
                    amount: Uint128::from(0u128),
                },
                contract_addr: "no debt".to_string(),
            },
        })
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
            let attach_repay_msg = check_and_repay_debt(deps, amount[0].clone())?;
            let res = check_and_spend(deps, core_payload, amount.clone())?;
            match attach_repay_msg.repay_msg {
                None => Ok(res),
                Some(msg) => Ok(res
                    .add_attribute("note", "repaying one-time fee debt")
                    .add_message(msg)
                    .add_attribute(
                        "swap_1_contract",
                        attach_repay_msg.top_sourced_swap.contract_addr,
                    )
                    .add_attribute(
                        "swap_1_to_amount",
                        attach_repay_msg.top_sourced_swap.coin.amount,
                    )
                    .add_attribute(
                        "swap_2_contract",
                        attach_repay_msg.bottom_sourced_swap.contract_addr,
                    )
                    .add_attribute(
                        "swap_2_to_amount",
                        attach_repay_msg.bottom_sourced_swap.coin.amount,
                    )),
            }
        }
        _ => {
            //probably unreachable as check_spend_limits throws
            Err(ContractError::SpendNotAuthorized {})
        }
    }
}

fn check_and_spend(
    deps: &mut DepsMut,
    core_payload: &mut CorePayload,
    spend: Vec<Coin>,
) -> Result<Response, ContractError> {
    let mut cfg = STATE.load(deps.storage)?;
    let spend_reduction = cfg.check_spend_limits(
        deps.as_ref(),
        core_payload.current_time,
        core_payload.info.sender.to_string(),
        spend,
    )?;
    let res = Response::new()
        .add_messages(vec![core_payload.this_msg.clone()])
        .add_attribute("action", "execute")
        .add_attribute("spend_limit_reduction", spend_reduction.price)
        .add_attributes(spend_reduction.sources);
    STATE.save(deps.storage, &cfg)?;
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
        cfg.pending = deps.api.addr_validate(&new_admin)?;
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
        QueryMsg::Pending {} => to_binary(&query_pending(deps)?),
        QueryMsg::CanExecute { sender, msg } => to_binary(&query_can_execute(deps, sender, msg)?),
        QueryMsg::HotWallets {} => to_binary(&query_hot_wallets(deps)?),
    }
}

pub fn query_admin(deps: Deps) -> StdResult<AdminResponse> {
    let cfg = STATE.load(deps.storage)?;
    Ok(AdminResponse {
        admin: cfg.admin.to_string(),
    })
}

pub fn query_pending(deps: Deps) -> StdResult<AdminResponse> {
    let cfg = STATE.load(deps.storage)?;
    Ok(AdminResponse {
        admin: cfg.pending.to_string(),
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

pub fn query_hot_wallets(deps: Deps) -> StdResult<HotWalletsResponse> {
    let cfg = STATE.load(deps.storage)?;
    Ok(HotWalletsResponse {
        hot_wallets: cfg.hot_wallets,
    })
}
