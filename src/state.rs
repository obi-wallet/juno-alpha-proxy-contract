//use cw_multi_test::Contract;
use cosmwasm_std::{Addr, Coin, Deps, StdResult, Timestamp, Uint128};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cw_storage_plus::{IndexedMap, Item};

use crate::authorizations::{Authorization, AuthorizationIndexes};
use crate::pair_contract::PairContracts;
use crate::permissioned_address::{PermissionedAddress, PermissionedAddressParams};
use crate::signers::Signers;
use crate::sourced_coin::SourcedCoin;
use crate::ContractError;

use crate::sources::{Source, Sources};

pub struct ObiProxyContract<'a> {
    pub cfg: Item<'a, State>,
    pub authorizations: IndexedMap<'a, &'a str, Authorization, AuthorizationIndexes<'a>>,
}

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
    pub owner: Addr,
    pub owner_signers: Signers,
    pub pending: Addr,
    pub permissioned_addresses: Vec<PermissionedAddress>,
    pub uusd_fee_debt: Uint128, // waiting to pay back fees
    pub fee_lend_repay_wallet: Addr,
    pub home_network: String,
    pub pair_contracts: PairContracts,
    pub update_delay_hours: u16,
    pub update_pending_time: Timestamp,
    pub frozen: bool,
    pub auth_count: Uint128,
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

    pub fn assert_owner(&self, a: String, e: ContractError) -> Result<(), ContractError> {
        if !self.is_owner(a) {
            return Err(e);
        }
        Ok(())
    }

    pub fn is_update_pending(&self) -> bool {
        self.owner != self.pending
    }

    pub fn is_active_permissioned_address(&self, addr: Addr) -> StdResult<bool> {
        let this_wallet_opt: Option<&PermissionedAddress> = self
            .permissioned_addresses
            .iter()
            .find(|a| a.address() == addr);
        match this_wallet_opt {
            None => Ok(false),
            Some(_) => Ok(true),
        }
    }

    //hardcode for now since these kinds of authorizations
    //will eventually be handled by calls to gatekeeper, not here
    pub fn is_authorized_permissioned_address_contract(&self, addr: String) -> bool {
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

    pub fn add_permissioned_address(
        &mut self,
        new_permissioned_address: PermissionedAddressParams,
    ) {
        self.permissioned_addresses
            .push(PermissionedAddress::new(new_permissioned_address));
    }

    pub fn rm_permissioned_address(&mut self, doomed_permissioned_address: String) {
        self.permissioned_addresses
            .retain(|wallet| wallet.address() != doomed_permissioned_address);
    }

    /// returns true if the address is a registered admin
    pub fn is_owner(&self, addr: String) -> bool {
        let addr: &str = &addr;
        self.owner == addr
    }

    /// returns true if the address is pending to become a registered admin
    pub fn is_pending(&self, addr: String) -> bool {
        let addr: &str = &addr;
        self.pending == addr
    }

    pub fn maybe_get_permissioned_address(
        &self,
        addr: String,
    ) -> Result<&PermissionedAddress, ContractError> {
        let this_wallet_opt: Option<&PermissionedAddress> = self
            .permissioned_addresses
            .iter()
            .find(|a| a.address() == addr);
        match this_wallet_opt {
            None => Err(ContractError::PermissionedAddressDoesNotExist {}),
            Some(wal) => Ok(wal),
        }
    }

    pub fn maybe_get_permissioned_address_mut(
        &mut self,
        addr: String,
    ) -> Result<&mut PermissionedAddress, ContractError> {
        let this_wallet_opt: Option<&mut PermissionedAddress> = self
            .permissioned_addresses
            .iter_mut()
            .find(|a| a.address() == addr);
        match this_wallet_opt {
            None => Err(ContractError::PermissionedAddressDoesNotExist {}),
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
        if self.is_owner(addr.clone()) {
            return Ok(get_admin_sourced_coin());
        }
        let pair_contracts = self.pair_contracts.clone();
        let this_wallet = self.maybe_get_permissioned_address_mut(addr)?;

        // check if we should reset to full spend limit again
        // (i.e. reset time has passed)
        if this_wallet.should_reset(current_time) {
            let new_dt = this_wallet.reset_period(current_time);
            match new_dt {
                Ok(()) => this_wallet.process_spend_vec(deps, pair_contracts, spend),
                Err(e) => Err(e),
            }
        } else {
            this_wallet.process_spend_vec(deps, pair_contracts, spend)
        }
    }

    pub fn check_spend_limits(
        &self,
        deps: Deps,
        pair_contracts: PairContracts,
        current_time: Timestamp,
        addr: String,
        spend: Vec<Coin>,
    ) -> Result<SourcedCoin, ContractError> {
        if self.is_owner(addr.clone()) {
            return Ok(get_admin_sourced_coin());
        }
        let this_wallet = self.maybe_get_permissioned_address(addr)?;

        // check if we should reset to full spend limit again
        // (i.e. reset time has passed)
        if this_wallet.should_reset(current_time) {
            this_wallet.check_spend_vec(deps, pair_contracts, spend, true)
        } else {
            this_wallet.check_spend_vec(deps, pair_contracts, spend, false)
        }
    }
}
