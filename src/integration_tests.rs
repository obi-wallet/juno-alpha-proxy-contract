use crate::msg::{AdminResponse, ExecuteMsg, InstantiateMsg};
use anyhow::{anyhow, Result};
use cosmwasm_std::{
    to_binary, Addr, CosmosMsg, Empty, QueryRequest, StdError, Uint128, WasmMsg, WasmQuery,
};
use cw1::Cw1Contract;
use cw_multi_test::{App, AppResponse, Contract, ContractWrapper, Executor};
use derivative::Derivative;
use serde::{de::DeserializeOwned, Serialize};

#[allow(dead_code)]
fn mock_app() -> App {
    App::default()
}

#[allow(dead_code)]
fn contract_cw1() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        crate::contract::execute,
        crate::contract::instantiate,
        crate::contract::query,
    );
    Box::new(contract)
}

#[derive(Derivative)]
#[derivative(Debug)]
pub struct Suite {
    /// Application mock
    #[allow(dead_code)]
    #[derivative(Debug = "ignore")]
    app: App,
    /// Special account
    pub owner: String,
    /// ID of stored code for cw1 contract
    cw1_id: u64,
}

impl Suite {
    #[allow(dead_code)]
    pub fn init() -> Result<Suite> {
        let mut app = mock_app();
        let owner = "owner".to_owned();
        let cw1_id = app.store_code(contract_cw1());

        Ok(Suite { app, owner, cw1_id })
    }

    #[allow(dead_code)]
    pub fn instantiate_cw1_contract(
        &mut self,
        admin: String,
        hot_wallets: Vec<crate::state::HotWallet>,
    ) -> Cw1Contract {
        let contract = self
            .app
            .instantiate_contract(
                self.cw1_id,
                Addr::unchecked(self.owner.clone()),
                &InstantiateMsg {
                    admin,
                    hot_wallets,
                    uusd_fee_debt: Uint128::from(0u128),
                    fee_lend_repay_wallet: "test_repay_address".to_string(),
                    home_network: "local".to_string(),
                    signers: [
                        "testsigner1".to_string(),
                        "testsigner2".to_string(),
                        "testsigner3".to_string(),
                    ]
                    .to_vec(),
                },
                &[],
                "Whitelist",
                None,
            )
            .unwrap();
        Cw1Contract(contract)
    }

    #[allow(dead_code)]
    pub fn execute<M>(
        &mut self,
        sender_contract: Addr,
        target_contract: &Addr,
        msg: M,
    ) -> Result<AppResponse>
    where
        M: Serialize + DeserializeOwned,
    {
        let execute: ExecuteMsg = ExecuteMsg::Execute {
            msgs: vec![CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: target_contract.to_string(),
                msg: to_binary(&msg)?,
                funds: vec![],
            })],
        };
        self.app
            .execute_contract(
                Addr::unchecked(self.owner.clone()),
                sender_contract,
                &execute,
                &[],
            )
            .map_err(|err| anyhow!(err))
    }

    #[allow(dead_code)]
    pub fn query<M>(&self, target_contract: Addr, msg: M) -> Result<AdminResponse, StdError>
    where
        M: Serialize + DeserializeOwned,
    {
        self.app.wrap().query(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr: target_contract.to_string(),
            msg: to_binary(&msg).unwrap(),
        }))
    }
}
