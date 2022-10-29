#[cfg(not(feature = "library"))]
use cosmwasm_std::{
    to_binary, Addr, BankMsg, Binary, Coin, CosmosMsg, Deps, DepsMut, Env, MessageInfo, Response,
    StakingMsg, StdError, StdResult, Uint128, WasmMsg,
};
use cosmwasm_std::{Api, Order};

use crate::authorizations::Authorization;
use crate::constants::MAINNET_AXLUSDC_IBC;
use crate::error::ContractError;
use crate::hot_wallet::{CoinLimit, HotWallet, HotWalletParams, HotWalletsResponse};
use crate::msg::{
    AuthorizationsResponse, CanSpendResponse, ExecuteMsg, InstantiateMsg, MigrateMsg,
    OwnerResponse, QueryMsg, SignersResponse, UpdateDelayResponse,
};
use crate::pair_contract::{PairContract, PairContracts};
use crate::signers::Signers;
use crate::sourced_coin::SourcedCoin;
use crate::sources::Sources;
use crate::state::{ObiProxyContract, State};
use crate::submsgs::{PendingSubmsg, SubmsgType};
use cw1::CanExecuteResponse;
use cw2::{get_contract_version, set_contract_version};
use cw_storage_plus::{Bound, Item};
use schemars::JsonSchema;
use semver::Version;
use serde::{Deserialize, Serialize};

// version info for migration info
const CONTRACT_NAME: &str = "obi-proxy-contract";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");
const DEFAULT_LIMIT: u32 = 10;
const MAX_LIMIT: u32 = 30;

pub struct SourcedRepayMsg {
    pub repay_msg: Option<BankMsg>,
    pub wrapped_sources: Sources,
}

impl<'a> ObiProxyContract<'a> {
    pub fn instantiate(
        &self,
        deps: DepsMut,
        env: Env,
        _info: MessageInfo,
        msg: InstantiateMsg,
    ) -> StdResult<Response> {
        set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
        let valid_owner: Addr = deps.api.addr_validate(&msg.owner)?;
        let valid_repay_wallet: Addr = deps.api.addr_validate(&msg.fee_lend_repay_wallet)?;
        for wallet in msg.hot_wallets.clone() {
            wallet.assert_is_valid()?;
        }
        let owner_signers = Signers::new(deps.as_ref(), msg.signers, msg.signer_types)?;
        let (signers_event, activate_delay) = owner_signers.create_event();
        let mut cfg = State {
            owner: valid_owner.clone(),
            owner_signers,
            pending: valid_owner,
            hot_wallets: msg.hot_wallets.into_iter().map(HotWallet::new).collect(),
            uusd_fee_debt: msg.uusd_fee_debt,
            fee_lend_repay_wallet: valid_repay_wallet,
            home_network: msg.home_network,
            pair_contracts: PairContracts {
                pair_contracts: vec![],
            },
            update_delay_hours: if activate_delay { 24u16 } else { 0u16 },
            update_pending_time: env.block.time,
            auth_count: Uint128::from(0u128),
            frozen: false,
        };
        cfg.pair_contracts
            .set_pair_contracts(cfg.home_network.clone())?;
        self.cfg.save(deps.storage, &cfg)?;
        Ok(Response::new().add_event(signers_event))
    }

    #[allow(unused_variables)]
    pub fn migrate(
        &self,
        deps: DepsMut,
        env: Env,
        msg: MigrateMsg,
    ) -> Result<Response, ContractError> {
        #[derive(Serialize, Deserialize, Clone, PartialEq, Eq, JsonSchema, Debug)]
        struct OldState {
            pub admin: Addr,
            pub pending: Addr,
            pub hot_wallets: Vec<HotWallet>,
            pub uusd_fee_debt: Uint128, // waiting to pay back fees
            pub fee_lend_repay_wallet: Addr,
            pub home_network: String,
            pub pair_contracts: Vec<PairContract>,
        }

        // No migrate allowed if owner update is pending
        // otherwise the new code owner might force a migration
        // to some malicious code id before the old user can cancel
        // during the safety delay. Notice that if code owner has
        // been updated to an attacker owner, this just lets the
        // user retain control long enough to save assets, not to
        // save the account. Control of code owner update should
        // be carefully guarded.
        let cfg: StdResult<State> = self.cfg.load(deps.storage);

        match cfg {
            Err(_) => {
                const OLDSTATE: Item<OldState> = Item::new("state");
                let old_cfg: OldState = OLDSTATE.load(deps.storage)?;
                let cfg = State {
                    update_delay_hours: 24u16,
                    update_pending_time: env.block.time,
                    owner: old_cfg.admin.clone(),
                    owner_signers: Signers::new(deps.as_ref(), vec![], vec![])?,
                    pending: old_cfg.pending.clone(),
                    hot_wallets: old_cfg.hot_wallets,
                    uusd_fee_debt: old_cfg.uusd_fee_debt,
                    fee_lend_repay_wallet: old_cfg.fee_lend_repay_wallet,
                    home_network: old_cfg.home_network,
                    pair_contracts: PairContracts {
                        pair_contracts: old_cfg.pair_contracts,
                    },
                    auth_count: Uint128::from(0u128),
                    frozen: false,
                };
                if old_cfg.admin != old_cfg.pending {
                    return Err(ContractError::CannotMigrateUpdatePending {});
                }
                self.cfg.save(deps.storage, &cfg)?;
            }
            Ok(mut current_cfg) => {
                if current_cfg.is_update_pending() {
                    return Err(ContractError::CannotMigrateUpdatePending {});
                }
                current_cfg.update_delay_hours = 24u16;
                current_cfg.update_pending_time = env.block.time;
                self.cfg.save(deps.storage, &current_cfg)?;
            }
        }

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

    pub fn execute(
        &self,
        deps: DepsMut,
        env: Env,
        info: MessageInfo,
        msg: ExecuteMsg,
    ) -> Result<Response, ContractError> {
        match msg {
            ExecuteMsg::Execute { msgs } => self.execute_execute(deps, env, info, msgs),
            ExecuteMsg::AddHotWallet { new_hot_wallet } => {
                self.add_hot_wallet(deps, env, info, new_hot_wallet)
            }
            ExecuteMsg::RmHotWallet { doomed_hot_wallet } => {
                self.rm_hot_wallet(deps, env, info, doomed_hot_wallet)
            }
            ExecuteMsg::ProposeUpdateOwner { new_owner } => {
                self.propose_update_owner(deps, env, info, new_owner)
            }
            ExecuteMsg::ConfirmUpdateOwner {
                signers,
                signer_types,
            } => self.confirm_update_owner(deps, env, info, signers, signer_types),
            ExecuteMsg::CancelUpdateOwner {} => self.cancel_update_owner(deps, env, info),
            ExecuteMsg::UpdateHotWalletSpendLimit {
                hot_wallet,
                new_spend_limits,
            } => self.update_hot_wallet_spend_limit(deps, env, info, hot_wallet, new_spend_limits),
            ExecuteMsg::UpdateUpdateDelay { hours } => {
                self.update_update_delay_hours(deps, env, info, hours)
            }
            ExecuteMsg::AddAuthorization { new_authorization } => {
                self.add_authorization(deps, info, new_authorization)
            }
            ExecuteMsg::RemoveAuthorization {
                authorization_to_remove,
            } => self.rm_authorization(deps, info, authorization_to_remove),
        }
    }

    // Simulation gatekeeping is all in this block
    pub fn execute_execute(
        &self,
        deps: DepsMut,
        env: Env,
        info: MessageInfo,
        msgs: Vec<CosmosMsg>,
    ) -> Result<Response, ContractError> {
        let mut cfg = self.cfg.load(deps.storage)?;
        let mut res = Response::new();
        let can_execute_result = self.can_spend(
            deps.as_ref(),
            env,
            info.sender.to_string(),
            info.funds,
            msgs.clone(),
        );

        match can_execute_result {
            Err(e) => {
                Err(ContractError::Std(e))
            }
            Ok((can_spend, spend_limit_reduction, repay_msg)) => {
                if !can_spend.can_spend {
                    Err(ContractError::Std(StdError::GenericErr {
                        msg: can_spend.reason,
                    }))
                } else {
                    res = res.add_attribute("action", "execute_execute");
                    res = res.add_messages(msgs);
                    if let Some(wrapped_repay_msg) = repay_msg {
                        if let Some(inner_repay_msg) = wrapped_repay_msg.repay_msg {
                            res = res.add_message(inner_repay_msg);
                            cfg.uusd_fee_debt = Uint128::from(0u128);
                        }
                    }
                    if let Some(spend_limit_reduction) = spend_limit_reduction {
                        let this_hot_wallet =
                            cfg.maybe_get_hot_wallet_mut(info.sender.to_string())?;
                        this_hot_wallet.reduce_limit_direct(spend_limit_reduction.coin)?;
                    }
                    self.cfg.save(deps.storage, &cfg)?;
                    Ok(res)
                }
            }
        }
    }

    fn convert_debt_to_asset_spent(
        &self,
        deps: Deps,
        pair_contracts: PairContracts,
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
                unconverted_fee.get_converted_to_usdc(deps, pair_contracts, true)
            }
            _ => Err(ContractError::RepayFeesFirst(usd_debt.u128())), // todo: more general handling
        }
    }

    /// Does not actually repay, due to Deps, but
    /// if it returns a message, repay can be added
    fn try_repay_debt(&self, deps: Deps, asset: Coin) -> Result<SourcedRepayMsg, ContractError> {
        let cfg: State = self.cfg.load(deps.storage)?;
        let swaps =
            self.convert_debt_to_asset_spent(deps, cfg.pair_contracts, cfg.uusd_fee_debt, asset)?;
        Ok(SourcedRepayMsg {
            repay_msg: Some(BankMsg::Send {
                to_address: cfg.fee_lend_repay_wallet.to_string(),
                amount: vec![swaps.coin.clone()],
            }),
            wrapped_sources: swaps.wrapped_sources,
        })
    }

    pub fn add_hot_wallet(
        &self,
        deps: DepsMut,
        _env: Env,
        info: MessageInfo,
        new_hot_wallet_params: HotWalletParams,
    ) -> Result<Response, ContractError> {
        let mut cfg = self.cfg.load(deps.storage)?;
        cfg.assert_owner(info.sender.to_string(), ContractError::Unauthorized {})?;
        if cfg
            .hot_wallets
            .iter()
            .any(|wallet| wallet.address() == new_hot_wallet_params.address)
        {
            Err(ContractError::HotWalletExists {})
        } else {
            let _addrcheck = deps.api.addr_validate(&new_hot_wallet_params.address)?;
            cfg.add_hot_wallet(new_hot_wallet_params);
            self.cfg.save(deps.storage, &cfg)?;
            Ok(Response::new().add_attribute("action", "add_hot_wallet"))
        }
    }

    pub fn rm_hot_wallet(
        &self,
        deps: DepsMut,
        _env: Env,
        info: MessageInfo,
        doomed_hot_wallet: String,
    ) -> Result<Response, ContractError> {
        let mut cfg = self.cfg.load(deps.storage)?;
        if !cfg.is_owner(info.sender.to_string()) {
            Err(ContractError::Unauthorized {})
        } else if !cfg
            .hot_wallets
            .iter()
            .any(|wallet| wallet.address() == doomed_hot_wallet)
        {
            Err(ContractError::HotWalletDoesNotExist {})
        } else {
            cfg.rm_hot_wallet(doomed_hot_wallet);
            self.cfg.save(deps.storage, &cfg)?;
            Ok(Response::new().add_attribute("action", "rm_hot_wallet"))
        }
    }

    pub fn update_update_delay_hours(
        &self,
        deps: DepsMut,
        _env: Env,
        info: MessageInfo,
        hours: u16,
    ) -> Result<Response, ContractError> {
        let mut cfg = self.cfg.load(deps.storage)?;
        cfg.assert_owner(info.sender.to_string(), ContractError::Unauthorized {})?;
        if cfg.is_update_pending() {
            return Err(ContractError::CannotUpdateUpdatePending {});
        }
        cfg.update_delay_hours = hours;
        self.cfg.save(deps.storage, &cfg)?;
        Ok(Response::default())
    }

    pub fn update_hot_wallet_spend_limit(
        &self,
        deps: DepsMut,
        _env: Env,
        info: MessageInfo,
        hot_wallet: String,
        new_spend_limits: CoinLimit,
    ) -> Result<Response, ContractError> {
        let mut cfg = self.cfg.load(deps.storage)?;
        cfg.assert_owner(info.sender.to_string(), ContractError::Unauthorized {})?;
        let wallet = cfg
            .hot_wallets
            .iter_mut()
            .find(|wallet| wallet.address() == hot_wallet)
            .ok_or(ContractError::HotWalletDoesNotExist {})?;
        wallet.update_spend_limit(new_spend_limits)?;
        Ok(Response::new())
    }

    pub fn propose_update_owner(
        &self,
        deps: DepsMut,
        env: Env,
        info: MessageInfo,
        new_owner: String,
    ) -> Result<Response, ContractError> {
        let mut cfg = self.cfg.load(deps.storage)?;
        if !cfg.is_owner(info.sender.to_string()) {
            Err(ContractError::Unauthorized {})
        } else {
            cfg.update_pending_time = env.block.time;
            cfg.pending = deps.api.addr_validate(&new_owner)?;
            self.cfg.save(deps.storage, &cfg)?;

            let res = Response::new().add_attribute("action", "propose_update_owner");
            Ok(res)
        }
    }

    pub fn confirm_update_owner(
        &self,
        deps: DepsMut,
        env: Env,
        info: MessageInfo,
        signers: Vec<String>,
        signer_types: Vec<String>,
    ) -> Result<Response, ContractError> {
        let mut cfg = self.cfg.load(deps.storage)?;
        if cfg.update_delay_hours > 0 {
            cfg.assert_update_allowed_now(env.block.time)?;
        }
        cfg.owner_signers = Signers::new(deps.as_ref(), signers, signer_types)?;
        let (signers_event, activate_delay) = cfg.owner_signers.create_event();
        cfg.update_pending_time = env.block.time;
        if activate_delay {
            cfg.update_delay_hours = 24u16;
        } else {
            cfg.update_delay_hours = 0u16;
        }
        self.cfg.save(deps.storage, &cfg)?;
        let mut res = self.execute_update_owner(deps, env, info, false)?;
        res = res.add_event(signers_event);
        Ok(res)
    }

    pub fn cancel_update_owner(
        &self,
        deps: DepsMut,
        _env: Env,
        info: MessageInfo,
    ) -> Result<Response, ContractError> {
        self.execute_update_owner(deps, _env, info, true)
    }

    fn execute_update_owner(
        &self,
        deps: DepsMut,
        _env: Env,
        info: MessageInfo,
        cancel: bool,
    ) -> Result<Response, ContractError> {
        let mut cfg = self.cfg.load(deps.storage)?;
        if !cfg.is_pending(info.sender.to_string()) {
            Err(ContractError::CallerIsNotPendingNewOwner {})
        } else {
            match cancel {
                true => cfg.pending = cfg.owner.clone(),
                false => cfg.owner = cfg.pending.clone(),
            };
            self.cfg.save(deps.storage, &cfg)?;

            let res = Response::new().add_attribute("action", "confirm_update_owner");
            Ok(res)
        }
    }

    fn can_execute(&self, deps: Deps, sender: &str) -> StdResult<bool> {
        let cfg = self.cfg.load(deps.storage)?;
        let can = cfg.is_owner(sender.to_string());
        Ok(can)
    }

    pub fn query(&self, deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
        match msg {
            QueryMsg::Owner {} => to_binary(&self.query_owner(deps)?),
            QueryMsg::Pending {} => to_binary(&self.query_pending(deps)?),
            QueryMsg::Signers {} => to_binary(&self.query_signers(deps)?),
            QueryMsg::CanExecute { sender, msg } => {
                to_binary(&self.query_can_execute(deps, sender, msg)?)
            }
            QueryMsg::HotWallets {} => to_binary(&self.query_hot_wallets(deps)?),
            QueryMsg::CanSpend {
                sender,
                funds,
                msgs,
            } => to_binary(&self.query_can_spend(deps, env, sender, funds, msgs)?),
            QueryMsg::UpdateDelay {} => to_binary(&self.query_update_delay(deps)?),
            QueryMsg::Authorizations {
                target_contract,
                limit,
                start_after,
            } => {
                to_binary(&self.query_authorizations(deps, target_contract, limit, start_after)?)
            }
        }
    }

    pub fn query_owner(&self, deps: Deps) -> StdResult<OwnerResponse> {
        let cfg = self.cfg.load(deps.storage)?;
        Ok(OwnerResponse {
            owner: cfg.owner.to_string(),
        })
    }

    pub fn query_pending(&self, deps: Deps) -> StdResult<OwnerResponse> {
        let cfg = self.cfg.load(deps.storage)?;
        Ok(OwnerResponse {
            owner: cfg.pending.to_string(),
        })
    }

    pub fn query_signers(&self, deps: Deps) -> StdResult<SignersResponse> {
        let cfg = self.cfg.load(deps.storage)?;
        Ok(SignersResponse {
            signers: cfg.owner_signers.signers(),
        })
    }
    pub fn query_can_execute(
        &self,
        deps: Deps,
        sender: String,
        _msg: CosmosMsg,
    ) -> StdResult<CanExecuteResponse> {
        Ok(CanExecuteResponse {
            can_execute: self.can_execute(deps, &sender)?,
        })
    }

    pub fn query_hot_wallets(&self, deps: Deps) -> StdResult<HotWalletsResponse> {
        let cfg = self.cfg.load(deps.storage)?;
        Ok(HotWalletsResponse {
            hot_wallets: cfg
                .hot_wallets
                .into_iter()
                .map(|wallet| wallet.get_params())
                .collect(),
        })
    }

    pub fn query_update_delay(&self, deps: Deps) -> StdResult<UpdateDelayResponse> {
        let cfg = self.cfg.load(deps.storage)?;
        Ok(UpdateDelayResponse {
            update_delay_hours: cfg.update_delay_hours,
        })
    }

    pub fn query_can_spend(
        &self,
        deps: Deps,
        env: Env,
        sender: String,
        funds: Vec<Coin>,
        msgs: Vec<CosmosMsg>,
    ) -> StdResult<CanSpendResponse> {
        Ok(self.can_spend(deps, env, sender, funds, msgs)?.0)
    }

    pub fn can_spend(
        &self,
        deps: Deps,
        env: Env,
        sender: String,
        funds: Vec<Coin>,
        msgs: Vec<CosmosMsg>,
    ) -> StdResult<(
        CanSpendResponse,
        Option<SourcedCoin>,
        Option<SourcedRepayMsg>,
    )> {
        let cfg = self.cfg.load(deps.storage)?;
        // if owner, always – though technically this might not be true
        // if first token send with nothing left to repay fees
        if cfg.is_owner(sender.clone()) {
            if cfg.uusd_fee_debt > Uint128::from(0u128) {
                for n in 0..msgs.len() {
                    let mut processed_msg = PendingSubmsg {
                        msg: msgs[n].clone(),
                        contract_addr: None,
                        binarymsg: None,
                        funds: vec![],
                        ty: SubmsgType::Unknown,
                    };
                    if n == 0 {
                        processed_msg.add_funds(funds.clone());
                    }
                    let _msg_type = processed_msg.process_and_get_msg_type();
                    if !processed_msg.funds.is_empty() {
                        // more robust handling needed with multiple fund types;
                        // currently if first is insufficient, fails
                        match self.try_repay_debt(deps, processed_msg.funds[0].clone()) {
                            Err(e) => {
                                return Ok((
                                    CanSpendResponse {
                                        can_spend: false,
                                        reason: e.to_string(),
                                    },
                                    None,
                                    None,
                                ))
                            }
                            Ok(message) => {
                                return Ok((
                                    CanSpendResponse {
                                        can_spend: true,
                                        reason: "Spender is owner, debt is repayable".to_string(),
                                    },
                                    None,
                                    Some(message),
                                ));
                            }
                        }
                    }
                }
                return Ok((
                    CanSpendResponse {
                        can_spend: true,
                        reason: "Spender is owner; funds not being sent".to_string(),
                    },
                    None,
                    None,
                ));
            } else {
                return Ok((
                    CanSpendResponse {
                        can_spend: true,
                        reason: "Spender is owner, with zero debt".to_string(),
                    },
                    None,
                    None,
                ));
            }
        }

        // if one of authorized token contracts and spender is hot wallet, yes
        if msgs.len() > 1 {
            return Ok((
                CanSpendResponse {
                    can_spend: false,
                    reason: "Multi-message txes with hot wallets not supported yet".to_string(),
                },
                None,
                None,
            ));
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
                return Ok((
                    CanSpendResponse {
                        can_spend: true,
                        reason: "Active hot wallet spending blanket-authorized token".to_string(),
                    },
                    None,
                    None,
                ));
            }
        };
        let funds: Vec<Coin> = match msgs[0].clone() {
            //strictly speaking cw20 spend limits not supported yet, unless blanket authorized.
            //As kludge, send/transfer is blocked if debt exists. Otherwise, depends on
            //authorization.
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: _,
                msg: _,
                funds,
            }) => {
                let mut processed_msg = PendingSubmsg {
                    msg: msgs[0].clone(),
                    contract_addr: None,
                    binarymsg: None,
                    funds: vec![],
                    ty: SubmsgType::Unknown,
                };
                processed_msg.add_funds(funds.to_vec());
                let _msg_type = processed_msg.process_and_get_msg_type();
                // if is an active authorization, we will check against authorizations
                // and can check funds later
                match self.assert_authorized_action(
                    deps,
                    deps.api.addr_validate(&sender)?,
                    processed_msg,
                ) {
                    Ok(()) => {
                        // can't immediately pass but can proceed to fund checking
                        match funds {
                            x if x == vec![] => {
                                return Ok((
                                    CanSpendResponse {
                                        can_spend: true,
                                        reason: "Authorized action with no funds".to_string(),
                                    },
                                    None,
                                    None,
                                ));
                            }
                            _ => funds,
                        }
                    }
                    Err(_) => {
                        return Ok((
                            CanSpendResponse {
                                can_spend: false,
                                reason: "Not an authorized action".to_string(),
                            },
                            None,
                            None,
                        ));
                    }
                }
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
                return Ok((
                    CanSpendResponse {
                        can_spend: false,
                        reason: "Custom CosmosMsg not yet supported".to_string(),
                    },
                    None,
                    None,
                ));
            }
            CosmosMsg::Distribution(_) => {
                return Ok((
                    CanSpendResponse {
                        can_spend: false,
                        reason: "Distribution CosmosMsg not yet supported".to_string(),
                    },
                    None,
                    None,
                ));
            }
            _ => {
                return Ok((
                    CanSpendResponse {
                        can_spend: false,
                        reason: "This CosmosMsg type not yet supported".to_string(),
                    },
                    None,
                    None,
                ));
            }
        };
        let res = cfg.check_spend_limits(
            deps,
            self.cfg.load(deps.storage)?.pair_contracts,
            env.block.time,
            sender,
            funds,
        );
        match res {
            Ok(coin) => Ok((
                CanSpendResponse {
                    can_spend: true,
                    reason: "Hot wallet, with spending within spend limits".to_string(),
                },
                Some(coin),
                None,
            )),
            Err(_) => Ok((
                CanSpendResponse {
                    can_spend: false,
                    reason: "Hot wallet does not exist or over spend limit".to_string(),
                },
                None,
                None,
            )),
        }
    }

    pub fn maybe_addr(&self, api: &dyn Api, human: Option<String>) -> StdResult<Option<Addr>> {
        human.map(|x| api.addr_validate(&x)).transpose()
    }

    pub fn query_authorizations(
        &self,
        deps: Deps,
        target_contract: Option<String>,
        limit: Option<u32>,
        start_after: Option<String>,
    ) -> StdResult<AuthorizationsResponse> {
        let limit = limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as usize;
        let start_raw = start_after
            .clone()
            .map(|s| Bound::ExclusiveRaw(s.into_bytes()));
        let start_addr = self.maybe_addr(deps.api, start_after)?;
        let start = start_addr.map(|addr| Bound::exclusive(addr.as_ref()));
        let authorizations = match target_contract {
            None => {
                self.authorizations
                    .range(deps.storage, start_raw, None, Order::Ascending)
                    .take(limit)
                    .map(|item| Ok(/*(String::from_utf8(item?.0)?,*/ item?.1))
                    .collect::<StdResult<Vec<Authorization>>>()?
            }
            Some(target) => {
                self.authorizations
                    .idx
                    .contract
                    .prefix(deps.api.addr_validate(&target)?)
                    .range(deps.storage, start, None, Order::Ascending)
                    .take(limit)
                    .map(|item| Ok(/*item?.0,*/ item?.1))
                    .collect::<StdResult<Vec<Authorization>>>()?
            }
        };
        Ok(AuthorizationsResponse { authorizations })
    }
}
