#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    from_binary, to_binary, Addr, BankMsg, Binary, Coin, CosmosMsg, Deps, DepsMut, Empty, Env,
    Event, MessageInfo, Response, StakingMsg, StdError, StdResult, Timestamp, Uint128, WasmMsg,
};

use cw1::CanExecuteResponse;
use cw2::{get_contract_version, set_contract_version};
use cw20::Cw20ExecuteMsg;
use semver::Version;

use crate::constants::MAINNET_AXLUSDC_IBC;
use crate::error::ContractError;
use crate::helpers::convert_coin_to_usdc;
use crate::hot_wallet::{HotWallet, HotWalletsResponse};
use crate::msg::{
    AdminResponse, CanSpendResponse, ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg,
};
use crate::state::{Source, SourcedCoin, State, STATE};

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
    let mut signers_event = Event::new("obisign");
    for signer in msg.signers {
        signers_event =
            signers_event.add_attribute("signer", deps.api.addr_validate(&signer)?.to_string());
    }
    Ok(Response::new().add_event(signers_event))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(deps: DepsMut, _env: Env, msg: MigrateMsg) -> Result<Response, ContractError> {
    let mut cfg = STATE.load(deps.storage)?;
    match cfg.set_pair_contracts(cfg.home_network.clone()) {
        Ok(_) => {}
        Err(_) => {
            return Ok(Response::new().add_attribute("warning", "failed to inject pair contracts"));
        }
    }
    match get_contract_version(deps.storage) {
        Ok(res) => {
            let version: Version = CONTRACT_VERSION.parse()?;
            let storage_version: Version = res.version.parse()?;
            if storage_version < version {
                set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

                // If state structure changed in any contract version in the way migration is needed, it
                // should occur here
            }
            Ok(Response::default())
        }
        Err(_) => return Ok(Response::new().add_attribute("warning", "no contract versioning")),
    }
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
        // certain authorized token contracts process immediately if hot wallet (or admin)
        if let CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr,
            msg: _,
            funds,
        }) = msgs[0].clone()
        {
            if funds.is_empty()
                && cfg.is_authorized_hotwallet_contract(contract_addr)
                && cfg.is_active_hot_wallet(info.sender.clone())?
            {
                let res = Response::new()
                    .add_attribute("action", "execute_authorized_contract")
                    .add_message(msgs[0].clone());
                return Ok(res);
            }
        }
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
                    let partial_res = try_wasm_send(deps, wasm, &mut core_payload, info.clone())?;
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
    info: MessageInfo,
) -> Result<Response, ContractError> {
    let cfg = STATE.load(deps.storage)?;
    match wasm {
        WasmMsg::Execute {
            contract_addr,
            msg,
            funds: _,
        } => {
            // prevent attaching funds to tx, as Transfer message's
            // amount param is where funds are specified, instead
            // disabled: this was causing some issues with users whose
            // fees were not yet repaid
            /*
            let empty_vec: Vec<Coin> = [].to_vec();
            if funds != &empty_vec {
                return Err(ContractError::AttachedFundsNotAllowed {});
            }
            */
            let msg_de: Result<cw20::Cw20ExecuteMsg, StdError> = from_binary(msg);
            match msg_de {
                Ok(msg_contents) => {
                    // must be Transfer or Send if hot wallet
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
                        _ => {
                            if cfg.is_admin(info.sender.to_string()) {
                                Ok(Response::new()
                                    .add_attribute("note", "admin_bypass_wasmmsg_restrictions")
                                    .add_message(CosmosMsg::Wasm(wasm.clone())))
                            } else {
                                Err(ContractError::OnlyTransferSendAllowed {})
                            }
                        }
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
    pub sources: Vec<Source>,
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
                sources: vec![Source {
                    contract_addr: "1 USDC is 1 USDC".to_string(),
                    query_msg: format!(
                        "converted {} to {}",
                        state.uusd_fee_debt, state.uusd_fee_debt
                    ),
                }],
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
            sources: swaps.sources,
        })
    } else {
        Ok(SourcedRepayMsg {
            repay_msg: None,
            sources: vec![Source {
                contract_addr: "no debt".to_string(),
                query_msg: "no conversion necessary".to_string(),
            }],
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
            let mut res = check_and_spend(deps, core_payload, amount.clone())?;
            match attach_repay_msg.repay_msg {
                None => Ok(res),
                Some(msg) => {
                    for n in 0..attach_repay_msg.sources.len() {
                        res = res
                            .add_attribute(
                                format!("swap {} contract", n + 1),
                                attach_repay_msg.sources[n].contract_addr.clone(),
                            )
                            .add_attribute(
                                format!("swap {} query info", n + 1),
                                attach_repay_msg.sources[n].query_msg.clone(),
                            )
                    }
                    Ok(res
                        .add_attribute("note", "repaying one-time fee debt")
                        .add_message(msg))
                }
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
        .add_attribute("spend_limit_reduction", spend_reduction.coin.amount)
        .add_attributes(spend_reduction.sources_as_attributes());
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
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Admin {} => to_binary(&query_admin(deps)?),
        QueryMsg::Pending {} => to_binary(&query_pending(deps)?),
        QueryMsg::CanExecute { sender, msg } => to_binary(&query_can_execute(deps, sender, msg)?),
        QueryMsg::HotWallets {} => to_binary(&query_hot_wallets(deps)?),
        QueryMsg::CanSpend { sender, msg } => to_binary(&query_can_spend(deps, env, sender, msg)?),
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

pub fn query_can_spend(
    deps: Deps,
    env: Env,
    sender: String,
    msg: CosmosMsg,
) -> StdResult<CanSpendResponse> {
    let cfg = STATE.load(deps.storage)?;
    // if admin, always – though technically this might not be true
    // if first token send with nothing left to repay fees
    if cfg.is_admin(sender.clone()) {
        return Ok(CanSpendResponse { can_spend: true });
    }
    // if one of authorized token contracts and spender is hot wallet, yes
    if let CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr,
        msg: _,
        funds,
    }) = msg.clone()
    {
        if cfg.is_active_hot_wallet(deps.api.addr_validate(&sender)?)?
            && cfg.is_authorized_hotwallet_contract(contract_addr)
            && funds == vec![]
        {
            return Ok(CanSpendResponse { can_spend: true });
        }
    };
    let funds: Vec<Coin> = match msg {
        //strictly speaking cw20 spend limits not supported yet, unless blanket authorized
        CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: _,
            msg: _,
            funds: _,
        }) => {
            return Err(StdError::GenericErr {
                msg: "Spend-limit-based cw20 transfers not yet supported".to_string(),
            })
        }
        CosmosMsg::Bank(BankMsg::Send {
            to_address: _,
            amount,
        }) => amount,
        CosmosMsg::Staking(StakingMsg::Delegate {
            validator: _,
            amount,
        }) => {
            vec![amount]
        }
        CosmosMsg::Custom(_) => {
            return Err(StdError::GenericErr {
                msg: "Custom CosmosMsg not yet supported".to_string(),
            })
        }
        CosmosMsg::Distribution(_) => {
            return Err(StdError::GenericErr {
                msg: "Distribution CosmosMsg not yet supported".to_string(),
            })
        }
        _ => {
            return Err(StdError::GenericErr {
                msg: "This CosmosMsg type not yet supported".to_string(),
            })
        }
    };
    let res = cfg.check_spend_limits_nonmut(deps, env.block.time, sender, funds);
    match res {
        Ok(_) => Ok(CanSpendResponse { can_spend: true }),
        Err(_) => Ok(CanSpendResponse { can_spend: false }),
    }
}
