#[cfg(test)]
mod tests {
    use crate::contract::{
        execute, execute_execute, instantiate, query_admin, query_can_execute, query_hot_wallets,
    };
    use crate::msg::{AdminResponse, ExecuteMsg, InstantiateMsg};
    use crate::state::{CoinLimit, HotWallet, PeriodType};
    use crate::ContractError;

    use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info, MockApi, MockQuerier};
    use cosmwasm_std::{
        coin, coins, BankMsg, CosmosMsg, Empty, Env, MemoryStorage, OwnedDeps, StakingMsg, SubMsg,
    };
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
                usdc_denom: Some(false),
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
                usdc_denom: Some(true),
            }],
        };
        let info = mock_info(ADMIN, &[]);
        instantiate(deps.as_mut(), mock_env(), info, instantiate_msg).unwrap();
    }
}
