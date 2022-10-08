//use cw_multi_test::Contract;
use cosmwasm_std::{Addr, Coin, Deps, StdError, StdResult, Timestamp, Uint128};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cw_storage_plus::Item;

use crate::constants::{MAINNET_ID, TESTNET_ID};
use crate::hot_wallet::HotWallet;
use crate::pair_contract::PairContract;
use crate::pair_contract_defaults::{
    get_local_pair_contracts, get_mainnet_pair_contracts, get_testnet_pair_contracts,
};
use crate::sourced_coin::SourcedCoin;
use crate::ContractError;

use crate::sources::{Source, Sources};

pub fn get_admin_sourced_coin() -> SourcedCoin {
    SourcedCoin {
        coin: Coin {
            denom: String::from("unlimited"),
            amount: Uint128::from(0u128),
        },
        wrapped_sources: Sources {
            sources: [Source {
                contract_addr: String::from("no spend limit check"),
                query_msg: String::from("caller is admin"),
            }]
            .to_vec(),
        },
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, JsonSchema, Debug)]
pub struct State {
    pub admin: Addr,
    pub pending: Addr,
    pub hot_wallets: Vec<HotWallet>,
    pub uusd_fee_debt: Uint128, // waiting to pay back fees
    pub fee_lend_repay_wallet: Addr,
    pub home_network: String,
    pub pair_contracts: Vec<PairContract>,
    pub update_delay_hours: u16,
    pub update_pending_time: Timestamp,
}

impl State {
    pub fn assert_update_allowed_now(&self, current_time: Timestamp) -> Result<(), ContractError> {
        let allowed_time = self
            .update_pending_time
            .plus_seconds((self.update_delay_hours as u64).saturating_mul(3600));
        if allowed_time > current_time {
            Err(ContractError::UpdateDelayActive {})
        } else {
            Ok(())
        }
    }

    pub fn assert_admin(&self, a: String, e: ContractError) -> Result<(), ContractError> {
        if !self.is_admin(a) {
            return Err(e);
        }
        Ok(())
    }

    pub fn is_update_pending(&self) -> bool {
        self.admin != self.pending
    }

    pub fn is_active_hot_wallet(&self, addr: Addr) -> StdResult<bool> {
        let this_wallet_opt: Option<&HotWallet> =
            self.hot_wallets.iter().find(|a| a.address == addr);
        match this_wallet_opt {
            None => Ok(false),
            Some(_) => Ok(true),
        }
    }

    //hardcode for now since these kinds of authorizations
    //will eventually be handled by calls to gatekeeper, not here
    pub fn is_authorized_hotwallet_contract(&self, addr: String) -> bool {
        match addr {
            val if val == *"juno18c5uecrztn4rqakm23fskusasud7s8afujnl8yu54ule2kak5q4sdnvcz4" => {
                true //DRINK
            }
            val if val == *"juno1x5xz6wu8qlau8znmc60tmazzj3ta98quhk7qkamul3am2x8fsaqqcwy7n9" => {
                true //BOTTLE
            }
            _ => false,
        }
    }

    pub fn get_pair_contract(
        &self,
        denoms: (String, String),
    ) -> Result<(PairContract, bool), ContractError> {
        for n in 0..self.pair_contracts.len() {
            if self.pair_contracts[n].denom1 == denoms.0
                && self.pair_contracts[n].denom2 == denoms.1
            {
                return Ok((self.pair_contracts[n].clone(), false));
            } else if self.pair_contracts[n].denom2 == denoms.0
                && self.pair_contracts[n].denom1 == denoms.1
            {
                return Ok((self.pair_contracts[n].clone(), true));
            }
        }
        Err(ContractError::PairContractNotFound(
            denoms.0,
            denoms.1,
            self.pair_contracts.clone(),
        ))
    }

    pub fn set_pair_contracts(&mut self, network: String) -> Result<(), StdError> {
        match network {
            val if val == MAINNET_ID => {
                self.pair_contracts = get_mainnet_pair_contracts().to_vec();
                Ok(())
            }
            val if val == TESTNET_ID => {
                self.pair_contracts = get_testnet_pair_contracts().to_vec();
                Ok(())
            }
            val if val == *"local" => {
                self.pair_contracts = get_local_pair_contracts().to_vec();
                Ok(())
            }
            val if val == *"EMPTY" => {
                self.pair_contracts = [].to_vec();
                Ok(())
            }
            _ => Err(StdError::GenericErr {
                msg: "Failed to init pair contracts; unsupported chain id".to_string(),
            }),
        }
    }

    pub fn add_hot_wallet(&mut self, new_hot_wallet: HotWallet) {
        self.hot_wallets.push(new_hot_wallet);
    }

    pub fn rm_hot_wallet(&mut self, doomed_hot_wallet: String) {
        self.hot_wallets
            .retain(|wallet| wallet.address != doomed_hot_wallet);
    }

    /// returns true if the address is a registered admin
    pub fn is_admin(&self, addr: String) -> bool {
        let addr: &str = &addr;
        self.admin == addr
    }

    /// returns true if the address is pending to become a registered admin
    pub fn is_pending(&self, addr: String) -> bool {
        let addr: &str = &addr;
        self.pending == addr
    }

    pub fn maybe_get_hot_wallet(&self, addr: String) -> Result<&HotWallet, ContractError> {
        let this_wallet_opt: Option<&HotWallet> =
            self.hot_wallets.iter().find(|a| a.address == addr);
        match this_wallet_opt {
            None => Err(ContractError::HotWalletDoesNotExist {}),
            Some(wal) => Ok(wal),
        }
    }

    pub fn maybe_get_hot_wallet_mut(
        &mut self,
        addr: String,
    ) -> Result<&mut HotWallet, ContractError> {
        let this_wallet_opt: Option<&mut HotWallet> =
            self.hot_wallets.iter_mut().find(|a| a.address == addr);
        match this_wallet_opt {
            None => Err(ContractError::HotWalletDoesNotExist {}),
            Some(wal) => Ok(wal),
        }
    }

    pub fn check_and_update_spend_limits(
        &mut self,
        deps: Deps,
        current_time: Timestamp,
        addr: String,
        spend: Vec<Coin>,
    ) -> Result<SourcedCoin, ContractError> {
        if self.is_admin(addr.clone()) {
            return Ok(get_admin_sourced_coin());
        }
        let this_wallet = self.maybe_get_hot_wallet_mut(addr)?;

        // check if we should reset to full spend limit again
        // (i.e. reset time has passed)
        if current_time.seconds() > this_wallet.current_period_reset {
            let new_dt = this_wallet.reset_period(current_time);
            match new_dt {
                Ok(()) => this_wallet.process_spend_vec(deps, spend),
                Err(e) => Err(e),
            }
        } else {
            this_wallet.process_spend_vec(deps, spend)
        }
    }

    pub fn check_spend_limits(
        &self,
        deps: Deps,
        current_time: Timestamp,
        addr: String,
        spend: Vec<Coin>,
    ) -> Result<SourcedCoin, ContractError> {
        if self.is_admin(addr.clone()) {
            return Ok(get_admin_sourced_coin());
        }
        let this_wallet = self.maybe_get_hot_wallet(addr)?;

        // check if we should reset to full spend limit again
        // (i.e. reset time has passed)
        if this_wallet.should_reset(current_time) {
            this_wallet.check_spend_vec(deps, spend, true)
        } else {
            this_wallet.check_spend_vec(deps, spend, false)
        }
    }
}

pub const STATE: Item<State> = Item::new("state");
