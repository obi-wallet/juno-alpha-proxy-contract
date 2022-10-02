#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    to_binary, Addr, BankMsg, Binary, Coin, CosmosMsg, Deps, DepsMut, Empty, Env, Event,
    MessageInfo, Response, StakingMsg, StdError, StdResult, Timestamp, Uint128, WasmMsg,
};

use cw1::CanExecuteResponse;
use cw2::{get_contract_version, set_contract_version};
use semver::Version;

use crate::constants::MAINNET_AXLUSDC_IBC;
use crate::error::ContractError;
use crate::hot_wallet::{HotWallet, HotWalletsResponse, CoinLimit};
use crate::msg::{
    AdminResponse, CanSpendResponse, ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg,
};
use crate::sourced_coin::SourcedCoin;
use crate::sources::Sources;
use crate::state::{State, STATE};
use crate::submsgs::{PendingSubmsg, SubmsgType, WasmmsgType};

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
        wallet.assert_is_valid()?;
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

#[allow(unused_variables)]
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(deps: DepsMut, _env: Env, msg: MigrateMsg) -> Result<Response, ContractError> {
    match get_contract_version(deps.storage) {
        Ok(res) => {
            let version: Version = CONTRACT_VERSION.parse()?;
            let storage_version: Version = res.version.parse()?;
            if storage_version < version {
                set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
            }
            Ok(Response::default())
        }
        Err(_) => Ok(Response::new().add_attribute("warning", "no contract versioning")),
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::Execute { msgs } => execute_execute(deps, env, info, msgs, false),
        ExecuteMsg::SimExecute { msgs } => execute_execute(deps, env, info, msgs, true),
        ExecuteMsg::AddHotWallet { new_hot_wallet } => {
            add_hot_wallet(deps, env, info, new_hot_wallet)
        }
        ExecuteMsg::RmHotWallet { doomed_hot_wallet } => {
            rm_hot_wallet(deps, env, info, doomed_hot_wallet)
        }
        ExecuteMsg::ProposeUpdateAdmin { new_admin } => {
            propose_update_admin(deps, env, info, new_admin)
        }
        ExecuteMsg::ConfirmUpdateAdmin { signers } => {
            confirm_update_admin(deps, env, info, signers)
        }
        ExecuteMsg::CancelUpdateAdmin {} => cancel_update_admin(deps, env, info),
        ExecuteMsg::UpdateHotWalletSpendLimit { hot_wallet, new_spend_limits } => {
            update_hot_wallet(deps, env, info, hot_wallet, new_spend_limits)
        }
    }
}

// Simulation gatekeeping is all in this block
pub fn execute_execute(
    mut deps: DepsMut,
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
            info,
            this_msg: CosmosMsg::Custom(Empty {}),
            current_time: env.block.time,
        };
        for this_msg in msgs {
            core_payload.this_msg = this_msg.clone();
            let maybe_repay_msg =
                check_and_spend_total_coins(deps.branch(), this_msg.clone(), &mut core_payload)?;
            if !simulation {
                if let Some(msg) = maybe_repay_msg {
                    if let Some(repay_msg) = msg.repay_msg {
                        res = res.add_message(repay_msg);
                    }
                    res = res.add_attributes(msg.wrapped_sources.to_attributes())
                }
                res = res.add_message(this_msg);
            }
            res = res.add_attribute("action", "execute_spend_limit_or_debt");
        }
    }
    Ok(res)
}

fn check_and_spend_total_coins(
    deps: DepsMut,
    msg: CosmosMsg,
    core_payload: &mut CorePayload,
) -> Result<Option<SourcedRepayMsg>, ContractError> {
    let mut processed_msg = PendingSubmsg {
        msg,
        contract_addr: None,
        binarymsg: None,
        funds: vec![],
        ty: SubmsgType::Unknown,
    };
    processed_msg.add_funds(core_payload.info.funds.to_vec());
    match processed_msg.process_and_get_msg_type() {
        SubmsgType::BankSend
        | SubmsgType::BankBurn
        | SubmsgType::ExecuteWasm(WasmmsgType::Cw20Transfer)
        | SubmsgType::ExecuteWasm(WasmmsgType::Cw20Send)
        | SubmsgType::ExecuteWasm(WasmmsgType::Cw20Burn)
        | SubmsgType::ExecuteWasm(WasmmsgType::Cw20IncreaseAllowance) => {
            check_coins(deps, core_payload, processed_msg.funds)
        }
        SubmsgType::ExecuteWasm(_other_type) => {
            let cfg = STATE.load(deps.storage)?;
            cfg.assert_admin(core_payload.info.sender.to_string(), ContractError::OnlyTransferSendAllowed {})?;
            Ok(None)
        }
        SubmsgType::Unknown => Err(ContractError::BadMessageType("unknown".to_string())),
    }
}

pub struct SourcedRepayMsg {
    pub repay_msg: Option<BankMsg>,
    pub wrapped_sources: Sources,
}

fn convert_debt_to_asset_spent(
    deps: Deps,
    usd_debt: Uint128,
    asset: Coin,
) -> Result<SourcedCoin, ContractError> {
    match asset.denom.as_str() {
        val if val == MAINNET_AXLUSDC_IBC => Ok(SourcedCoin {
            coin: Coin {
                denom: MAINNET_AXLUSDC_IBC.to_string(),
                amount: usd_debt,
            },
            wrapped_sources: Sources { sources: vec![] },
        }),
        "ujuno" | "ujunox" | "testtokens" => {
            let unconverted_fee = SourcedCoin {
                coin: Coin {
                    denom: asset.denom.clone(),
                    amount: usd_debt,
                },
                wrapped_sources: Sources { sources: vec![] },
            };
            unconverted_fee.get_converted_to_usdc(deps, true)
        }
        _ => Err(ContractError::RepayFeesFirst(usd_debt.u128())), // todo: more general handling
    }
}

/// Does not actually repay, due to Deps, but
/// if it returns a message, repay can be added
fn try_repay_debt(deps: Deps, asset: Coin) -> Result<SourcedRepayMsg, ContractError> {
    let cfg: State = STATE.load(deps.storage)?;
    let swaps = convert_debt_to_asset_spent(deps, cfg.uusd_fee_debt, asset)?;
    Ok(SourcedRepayMsg {
        repay_msg: Some(BankMsg::Send {
            to_address: cfg.fee_lend_repay_wallet.to_string(),
            amount: vec![swaps.coin.clone()],
        }),
        wrapped_sources: swaps.wrapped_sources,
    })
}

fn check_coins(
    deps: DepsMut,
    core_payload: &mut CorePayload,
    spend: Vec<Coin>,
) -> Result<Option<SourcedRepayMsg>, ContractError> {
    let mut cfg = STATE.load(deps.storage)?;
    let mut sourced_repay: Option<SourcedRepayMsg> = None;
    if cfg.uusd_fee_debt > Uint128::from(0u128) {
        'debt_cycle: for coin in spend.clone() {
            if let Ok(msg) = try_repay_debt(deps.as_ref(), coin) {
                sourced_repay = Some(msg);
                cfg.uusd_fee_debt = Uint128::from(0u128);
                break 'debt_cycle;
            }
        }
        if cfg.uusd_fee_debt > Uint128::from(0u128) {
            return Err(ContractError::UnableToRepayDebt(
                cfg.uusd_fee_debt.to_string(),
            ));
        }
    }
    cfg.check_and_update_spend_limits(
        deps.as_ref(),
        core_payload.current_time,
        core_payload.info.sender.to_string(),
        spend,
    )?;
    STATE.save(deps.storage, &cfg)?;
    Ok(sourced_repay)
}

pub fn add_hot_wallet(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    new_hot_wallet: HotWallet,
) -> Result<Response, ContractError> {
    let mut cfg = STATE.load(deps.storage)?;
    cfg.assert_admin(info.sender.to_string(), ContractError::Unauthorized {})?;
    if cfg
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

pub fn update_hot_wallet(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    hot_wallet: String,
    new_spend_limits: Vec<Coin>,
) -> Result<Response, ContractError> {
    let mut cfg = STATE.load(deps.storage)?;
    cfg.assert_admin(info.sender.to_string(), ContractError::Unauthorized {})?;
    let mut wallet = cfg.hot_wallets
        .iter_mut()
        .find(|wallet| wallet.address == hot_wallet)
        .ok_or_else(|| ContractError::HotWalletDoesNotExist { })?;
    wallet.spend_limits = new_spend_limits.into_iter().map(|coin| 
        CoinLimit {
            amount: coin.amount.u128() as u64,
            denom: coin.denom,
            limit_remaining: coin.amount.u128() as u64,
        }
    ).collect();
    Ok(Response::new())
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
    signers: Vec<String>,
) -> Result<Response, ContractError> {
    let mut signers_event = Event::new("obisign");
    for signer in signers {
        signers_event =
            signers_event.add_attribute("signer", deps.api.addr_validate(&signer)?.to_string());
    }
    let mut res = execute_update_admin(deps, _env, info, false)?;
    res = res.add_event(signers_event);
    Ok(res)
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
        QueryMsg::CanSpend { sender, msgs } => {
            to_binary(&query_can_spend(deps, env, sender, msgs)?)
        }
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
    msgs: Vec<CosmosMsg>,
) -> StdResult<CanSpendResponse> {
    let cfg = STATE.load(deps.storage)?;
    // if admin, always – though technically this might not be true
    // if first token send with nothing left to repay fees
    if cfg.is_admin(sender.clone()) {
        return Ok(CanSpendResponse { can_spend: true });
    }
    // if one of authorized token contracts and spender is hot wallet, yes
    if msgs.len() > 1 {
        return Err(StdError::GenericErr {
            msg: "Multi-message txes with hot wallets not supported yet".to_string(),
        });
    }
    if let CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr,
        msg: _,
        funds,
    }) = msgs[0].clone()
    {
        if cfg.is_active_hot_wallet(deps.api.addr_validate(&sender)?)?
            && cfg.is_authorized_hotwallet_contract(contract_addr)
            && funds == vec![]
        {
            return Ok(CanSpendResponse { can_spend: true });
        }
    };
    let funds: Vec<Coin> = match msgs[0].clone() {
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
    let res = cfg.check_spend_limits(deps, env.block.time, sender, funds);
    match res {
        Ok(_) => Ok(CanSpendResponse { can_spend: true }),
        Err(_) => Ok(CanSpendResponse { can_spend: false }),
    }
}
