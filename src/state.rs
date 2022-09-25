//use cw_multi_test::Contract;
use cosmwasm_std::{Addr, Coin, Deps, StdError, StdResult, Timestamp, Uint128};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cw_storage_plus::Item;

use crate::constants::{MAINNET_AXLUSDC_IBC, MAINNET_ID, TESTNET_ID};
use crate::hot_wallet::HotWallet;
use crate::pair_contract::PairContract;
use crate::pair_contract_defaults::{
    get_local_pair_contracts, get_mainnet_pair_contracts, get_testnet_pair_contracts,
};
use crate::sourced_coin::SourcedCoin;
use crate::ContractError;

pub fn get_admin_sourced_coin() -> SourcedCoin {
    SourcedCoin {
        coin: Coin {
            denom: String::from("unlimited"),
            amount: Uint128::from(0u128),
        },
        sources: [Source {
            contract_addr: String::from("no spend limit check"),
            query_msg: String::from("caller is admin"),
        }].to_vec()
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, JsonSchema, Debug)]
pub struct Source {
    pub contract_addr: String,
    pub query_msg: String,
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
}

impl State {
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

    pub fn check_spend_limits(
        &mut self,
        deps: Deps,
        current_time: Timestamp,
        addr: String,
        spend: Vec<Coin>,
    ) -> Result<SourcedCoin, ContractError> {
        if self.is_admin(addr.clone()) {
            return Ok(get_admin_sourced_coin());
        }
        let this_wallet_opt: Option<&mut HotWallet> =
            self.hot_wallets.iter_mut().find(|a| &a.address == &addr);
        let this_wallet: &mut HotWallet = match this_wallet_opt {
            None => { return Err(ContractError::HotWalletDoesNotExist {}); },
            Some(wal) => wal,
        };

        // check if we should reset to full spend limit again
        // (i.e. reset time has passed)
        if current_time.seconds() > this_wallet.current_period_reset {
            println!("LIMIT RESET TRIGGERED");
            let new_dt = this_wallet.reset_period(current_time);
            match new_dt {
                Ok(()) => {
                    let mut spend_tally: Uint128 = Uint128::from(0u128);
                    let mut spend_tally_sources: Vec<Source> = vec![];
                    for n in spend {
                        let spend_check_with_sources = this_wallet.reduce_limit(deps, n.clone())?;
                        for m in 0..spend_check_with_sources.sources.len() {
                            spend_tally_sources.push(Source {
                                contract_addr: spend_check_with_sources.sources[m]
                                    .contract_addr
                                    .clone(),
                                query_msg: spend_check_with_sources.sources[m].query_msg.clone(),
                            });
                        }
                        spend_tally =
                            spend_tally.saturating_add(spend_check_with_sources.coin.amount);
                    }
                    Ok(SourcedCoin {
                        coin: Coin {
                            amount: spend_tally,
                            denom: MAINNET_AXLUSDC_IBC.to_string(),
                        },
                        sources: spend_tally_sources,
                    })
                }
                Err(e) => Err(e),
            }
        } else {
            let mut spend_tally: Uint128 = Uint128::from(0u128);
            let mut spend_tally_sources: Vec<Source> = vec![];
            for n in spend {
                let spend_check_with_sources = this_wallet.reduce_limit(deps, n.clone())?;
                for m in 0..spend_check_with_sources.sources.len() {
                    spend_tally_sources.push(Source {
                        contract_addr: spend_check_with_sources.sources[m].contract_addr.clone(),
                        query_msg: spend_check_with_sources.sources[m].query_msg.clone(),
                    });
                }
                spend_tally = spend_tally.saturating_add(spend_check_with_sources.coin.amount);
            }
            Ok(SourcedCoin {
                coin: Coin {
                    amount: spend_tally,
                    denom: MAINNET_AXLUSDC_IBC.to_string(),
                },
                sources: spend_tally_sources,
            })
        }
    }

    // very soon to refactor (nearly copies above)
    pub fn check_spend_limits_nonmut(
        &self,
        deps: Deps,
        current_time: Timestamp,
        addr: String,
        spend: Vec<Coin>,
    ) -> Result<SourcedCoin, ContractError> {
        if self.is_admin(addr.clone()) {
            return Ok(get_admin_sourced_coin());
        } 
        let this_wallet_opt: Option<&HotWallet> =
            self.hot_wallets.iter().find(|a| &a.address == &addr);
        let this_wallet: &HotWallet = match this_wallet_opt {
            None => { return Err(ContractError::HotWalletDoesNotExist {}); },
            Some(wal) => wal,
        };

        // check if we should reset to full spend limit again
        // (i.e. reset time has passed)
        if current_time.seconds() > this_wallet.current_period_reset {
            let mut spend_tally: Uint128 = Uint128::from(0u128);
            let mut spend_tally_sources: Vec<Source> = vec![];
            for n in spend {
                let spend_check_with_sources =
                    this_wallet.simulate_reduce_limit(deps, n.clone(), true)?.1;
                for m in 0..spend_check_with_sources.sources.len() {
                    spend_tally_sources.push(Source {
                        contract_addr: spend_check_with_sources.sources[m]
                            .contract_addr
                            .clone(),
                        query_msg: spend_check_with_sources.sources[m].query_msg.clone(),
                    });
                }
                spend_tally = spend_tally.saturating_add(spend_check_with_sources.coin.amount);
            }
            Ok(SourcedCoin {
                coin: Coin {
                    amount: spend_tally,
                    denom: MAINNET_AXLUSDC_IBC.to_string(),
                },
                sources: spend_tally_sources,
            })
        } else {
            let mut spend_tally: Uint128 = Uint128::from(0u128);
            let mut spend_tally_sources: Vec<Source> = vec![];
            for n in spend {
                let spend_check_with_sources =
                    this_wallet.simulate_reduce_limit(deps, n.clone(), false)?.1;
                for m in 0..spend_check_with_sources.sources.len() {
                    spend_tally_sources.push(Source {
                        contract_addr: spend_check_with_sources.sources[m]
                            .contract_addr
                            .clone(),
                        query_msg: spend_check_with_sources.sources[m].query_msg.clone(),
                    });
                }
                spend_tally = spend_tally.saturating_add(spend_check_with_sources.coin.amount);
            }
            Ok(SourcedCoin {
                coin: Coin {
                    amount: spend_tally,
                    denom: MAINNET_AXLUSDC_IBC.to_string(),
                },
                sources: spend_tally_sources,
            })
        }
    }
}

pub const STATE: Item<State> = Item::new("state");
