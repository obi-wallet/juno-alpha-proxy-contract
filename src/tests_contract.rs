pub const ADMIN: &str = "alice";
pub const HOT_WALLET: &str = "hotcarl";

#[cfg(test)]
mod tests {
    use crate::contract::{
        execute, query_admin, query_can_execute, query_can_spend, query_hot_wallets,
    };
    use crate::hot_wallet::PeriodType;
    /* use crate::defaults::get_local_pair_contracts; */
    use super::*;
    use crate::msg::{AdminResponse, Cw20ExecuteMsg, ExecuteMsg};
    use crate::tests_helpers::{add_test_hotwallet, instantiate_contract, test_spend_bank};
    use crate::ContractError;

    use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
    use cosmwasm_std::{
        coin, coins, to_binary, Attribute, BankMsg, Coin, CosmosMsg, DistributionMsg, Response, StakingMsg,
        SubMsg, Uint128, WasmMsg,
    };

    const NEW_ADMIN: &str = "bob";
    const ANYONE: &str = "anyone";
    const RECEIVER: &str = "diane";
    const HOT_USDC_WALLET: &str = "hotearl";

    #[test]
    fn instantiate_and_modify_admin() {
        let mut deps = mock_dependencies();
        let current_env = mock_env();
        instantiate_contract(
            &mut deps,
            current_env,
            Coin {
                amount: Uint128::from(0u128),
                denom: "ujunox".to_string(),
            },
        );

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
        let msg = ExecuteMsg::ConfirmUpdateAdmin { signers: vec!["test_confirm_admin".to_string()]};
        let info = mock_info(NEW_ADMIN, &[]);
        let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(res.events.len(), 1);
        assert_eq!(
            res.events[0].attributes[0],
            Attribute::new("signer".to_string(), "test_confirm_admin".to_string())
        );

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
        instantiate_contract(
            &mut deps,
            current_env,
            Coin {
                amount: Uint128::from(0u128),
                denom: "ujunox".to_string(),
            },
        );

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
        instantiate_contract(
            &mut deps,
            current_env,
            Coin {
                amount: Uint128::from(0u128),
                denom: "ujunox".to_string(),
            },
        );

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

        instantiate_contract(
            &mut deps,
            current_env.clone(),
            Coin {
                amount: Uint128::from(0u128),
                denom: "ujunox".to_string(),
            },
        );
        // this helper includes a hotwallet

        // query to see we have "hotcarl" as hot wallet
        let res = query_hot_wallets(deps.as_ref()).unwrap();
        assert!(res.hot_wallets.len() == 1);
        assert!(res.hot_wallets[0].address == HOT_WALLET);

        // check that can_spend returns true
        let res = query_can_spend(
            deps.as_ref(),
            current_env.clone(),
            HOT_WALLET.to_string(),
            CosmosMsg::Bank(BankMsg::Send {
                to_address: RECEIVER.to_string(),
                amount: coins(9_000u128, "testtokens"),
            }),
        )
        .unwrap();
        assert!(res.can_spend);

        // and returns false with some huge amount
        let res = query_can_spend(
            deps.as_ref(),
            current_env.clone(),
            HOT_WALLET.to_string(),
            CosmosMsg::Bank(BankMsg::Send {
                to_address: RECEIVER.to_string(),
                amount: coins(999_999_999_000u128, "testtokens"),
            }),
        )
        .unwrap();
        assert!(!res.can_spend);

        // plus returns error with some unsupported kind of msg
        let _res = query_can_spend(
            deps.as_ref(),
            current_env.clone(),
            HOT_WALLET.to_string(),
            CosmosMsg::Distribution(DistributionMsg::SetWithdrawAddress {
                address: RECEIVER.to_string(),
            }),
        )
        .unwrap_err();

        // and returns true with authorized contract
        let _res = query_can_spend(
            deps.as_ref(),
            current_env.clone(),
            HOT_WALLET.to_string(),
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: "juno1x5xz6wu8qlau8znmc60tmazzj3ta98quhk7qkamul3am2x8fsaqqcwy7n9"
                    .to_string(),
                msg: to_binary(&Cw20ExecuteMsg::Transfer {
                    recipient: RECEIVER.to_string(),
                    amount: Uint128::from(1u128),
                })
                .unwrap(),
                funds: vec![],
            }),
        )
        .unwrap();
        assert!(_res.can_spend);

        // actually spend as the hot wallet
        let admin_info = mock_info(ADMIN, &[]);
        let hot_wallet_info = mock_info(HOT_WALLET, &[]);
        test_spend_bank(
            deps.as_mut(),
            current_env.clone(),
            RECEIVER.to_string(),
            coins(9_000u128, "testtokens"), //900_000 of usdc spend limit down
            hot_wallet_info,
        )
        .unwrap();

        // add a second hot wallet
        add_test_hotwallet(
            deps.as_mut(),
            "hot_diane".to_string(),
            current_env.clone(),
            admin_info.clone(),
            1u16,
            PeriodType::DAYS,
            1_000_000u64,
        )
        .unwrap();

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
        let _res = execute(
            deps.as_mut(),
            current_env.clone(),
            admin_info.clone(),
            execute_msg,
        )
        .unwrap();

        // query hot wallets again, should be 1
        let res = query_hot_wallets(deps.as_ref()).unwrap();
        println!("hot wallets are: {:?}", res.hot_wallets);
        assert!(res.hot_wallets.len() == 1);

        // add another hot wallet, this time with high USDC spend limit
        add_test_hotwallet(
            deps.as_mut(),
            HOT_USDC_WALLET.to_string(),
            current_env.clone(),
            admin_info,
            1u16,
            PeriodType::DAYS,
            100_000_000u64,
        )
        .unwrap();
        let res = query_hot_wallets(deps.as_ref()).unwrap();
        assert!(res.hot_wallets.len() == 2);

        // now spend ... local tests will force price to be 1 = 100 USDC
        // so our spend limit of 100_000_000 will equal 1_000_000 testtokens

        let mocked_info = mock_info(HOT_USDC_WALLET, &[]);
        let mut quick_spend_test = |amount: u128| -> Result<Response, ContractError> {
            test_spend_bank(
                deps.as_mut(),
                current_env.clone(),
                RECEIVER.to_string(),
                coins(amount, "testtokens"),
                mocked_info.clone(),
            )
        };

        // three tests here: 1. we can spend a small amount
        quick_spend_test(1_000u128).unwrap();
        // 999_000 left

        // 2. we can spend up to limit
        quick_spend_test(999_000u128).unwrap();
        // 0 left

        // 3. now our limit is spent and we cannot spend anything
        quick_spend_test(1u128).unwrap_err();
        // -1 left
    }

    #[test]
    fn repay_fee_debt() {
        let mut deps = mock_dependencies();
        let current_env = mock_env();
        instantiate_contract(
            &mut deps,
            current_env,
            Coin {
                amount: Uint128::from(1_000_000u128),
                denom: "ibc/EAC38D55372F38F1AFD68DF7FE9EF762DCF69F26520643CF3F9D292A738D8034"
                    .to_string(),
            },
        );

        // under test conditions, "testtokens" are worth 100 USDC each
        // so this $1 debt is covered with 0.01 testtokens appended to first send out
        let msgs: Vec<CosmosMsg> = vec![CosmosMsg::Bank(BankMsg::Send {
            to_address: RECEIVER.to_string(),
            amount: coins(10000, "testtokens"),
        })];
        let test_msgs: Vec<CosmosMsg> = vec![
            CosmosMsg::Bank(BankMsg::Send {
                to_address: RECEIVER.to_string(),
                amount: coins(10000, "testtokens"),
            }),
            CosmosMsg::Bank(BankMsg::Send {
                to_address: "test_repay_address".to_string(),
                amount: coins(100, "testtokens"),
            }),
        ];
        let execute_msg = ExecuteMsg::Execute { msgs: msgs.clone() };

        let info = mock_info(ADMIN, &[]);
        let res = execute(deps.as_mut(), mock_env(), info.clone(), execute_msg.clone()).unwrap();
        assert_eq!(
            res.messages,
            test_msgs.into_iter().map(SubMsg::new).collect::<Vec<_>>()
        );

        // now next identical send should not add the same fee repay message
        let res = execute(deps.as_mut(), mock_env(), info, execute_msg).unwrap();
        assert_eq!(
            res.messages,
            msgs.into_iter().map(SubMsg::new).collect::<Vec<_>>()
        );
    }

    /* #[test]
    fn migrate() {
        let mut deps = mock_dependencies();
        let current_env = mock_env();
        instantiate_contract(
            &mut deps,
            current_env,
            Coin {
                amount: Uint128::from(1_000_000u128),
                denom: "ibc/EAC38D55372F38F1AFD68DF7FE9EF762DCF69F26520643CF3F9D292A738D8034"
                    .to_string(),
            },
        );
        let mut cfg = STATE.load(&deps.storage).unwrap();
        cfg.set_pair_contracts("EMPTY".to_string()).unwrap();
        STATE.save(&mut deps.storage, &cfg).unwrap();
        let cfg = STATE.load(&deps.storage).unwrap();
        assert_eq!(cfg.pair_contracts, vec![]);
        migrate();
        let local_contracts = get_local_pair_contracts().to_vec();
        assert_eq!(cfg.pair_contracts, local_contracts);
    } */
}
