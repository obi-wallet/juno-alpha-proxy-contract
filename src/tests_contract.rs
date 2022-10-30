pub const OWNER: &str = "alice";
pub const PERMISSIONED_ADDRESS: &str = "hotcarl";

#[cfg(test)]
mod tests {
    use crate::permissioned_address::PeriodType;
    use crate::state::ObiProxyContract;
    /* use crate::defaults::get_local_pair_contracts; */
    use super::*;
    use crate::msg::{CanSpendResponse, Cw20ExecuteMsg, ExecuteMsg, OwnerResponse};
    use crate::tests_helpers::{
        add_test_permissioned_address, get_test_instantiate_message, test_spend_bank,
    };
    use crate::ContractError;
    use cosmwasm_std::StdError::GenericErr;

    use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
    use cosmwasm_std::{
        coin, coins, to_binary, Attribute, BankMsg, Coin, CosmosMsg, DistributionMsg, Response,
        StakingMsg, SubMsg, Uint128, WasmMsg,
    };

    const NEW_OWNER: &str = "bob";
    const ANYONE: &str = "anyone";
    const RECEIVER: &str = "diane";
    const PERMISSIONED_USDC_WALLET: &str = "hotearl";

    #[test]
    fn instantiate_and_modify_owner() {
        let mut deps = mock_dependencies();
        let current_env = mock_env();
        let obi = ObiProxyContract::default();
        let _res = obi
            .instantiate(
                deps.as_mut(),
                current_env.clone(),
                mock_info(OWNER, &[]),
                get_test_instantiate_message(
                    current_env,
                    Coin {
                        amount: Uint128::from(0u128),
                        denom: "ujunox".to_string(),
                    },
                    false,
                ),
            )
            .unwrap();

        // ensure expected config
        let expected = OwnerResponse {
            owner: OWNER.to_string(),
        };
        assert_eq!(obi.query_owner(deps.as_ref()).unwrap(), expected);

        // anyone cannot propose updating owner on the contract
        let msg = ExecuteMsg::ProposeUpdateOwner {
            new_owner: ANYONE.to_string(),
        };
        let info = mock_info(ANYONE, &[]);
        let err = obi
            .execute(deps.as_mut(), mock_env(), info, msg)
            .unwrap_err();
        assert_eq!(err, ContractError::Unauthorized {});

        // but alice can propose an update
        let msg = ExecuteMsg::ProposeUpdateOwner {
            new_owner: NEW_OWNER.to_string(),
        };
        let info = mock_info(OWNER, &[]);
        obi.execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        // now, the owner isn't updated yet
        let expected = OwnerResponse {
            owner: OWNER.to_string(),
        };
        assert_eq!(obi.query_owner(deps.as_ref()).unwrap(), expected);

        // but if bob accepts...
        let msg = ExecuteMsg::ConfirmUpdateOwner {
            signers: vec!["test_confirm_owner".to_string()],
            signer_types: vec!["new_owner_type".to_string()],
        };
        let info = mock_info(NEW_OWNER, &[]);
        let res = obi.execute(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(res.events.len(), 1);
        assert_eq!(
            res.events[0].attributes[0],
            Attribute::new("signer".to_string(), "test_confirm_owner".to_string())
        );

        // then owner is updated
        let expected = OwnerResponse {
            owner: NEW_OWNER.to_string(),
        };
        assert_eq!(obi.query_owner(deps.as_ref()).unwrap(), expected);
    }

    #[test]
    fn execute_messages_has_proper_permissions() {
        let mut deps = mock_dependencies();
        let current_env = mock_env();
        let obi = ObiProxyContract::default();
        let _res = obi
            .instantiate(
                deps.as_mut(),
                current_env.clone(),
                mock_info(OWNER, &[]),
                get_test_instantiate_message(
                    current_env,
                    Coin {
                        amount: Uint128::from(0u128),
                        denom: "ujunox".to_string(),
                    },
                    false,
                ),
            )
            .unwrap();

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

        // receiver or anyone else cannot execute them ... and gets PermissionedAddressDoesNotExist since
        // this is a spend, so contract assumes we're trying against spend limit
        // if not owner
        let info = mock_info(RECEIVER, &[]);
        let err = obi
            .execute(deps.as_mut(), mock_env(), info, execute_msg.clone())
            .unwrap_err();
        assert_eq!(
            err,
            ContractError::Std(GenericErr {
                msg: "Permissioned address does not exist or over spend limit".to_string()
            })
        );

        // but owner can
        let info = mock_info(OWNER, &[]);
        let res = obi
            .execute(deps.as_mut(), mock_env(), info, execute_msg)
            .unwrap();
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
        let obi = ObiProxyContract::default();
        let _res = obi
            .instantiate(
                deps.as_mut(),
                current_env.clone(),
                mock_info(OWNER, &[]),
                get_test_instantiate_message(
                    current_env,
                    Coin {
                        amount: Uint128::from(0u128),
                        denom: "ujunox".to_string(),
                    },
                    false,
                ),
            )
            .unwrap();

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
        let res = obi
            .query_can_execute(deps.as_ref(), OWNER.to_string(), send_msg.clone())
            .unwrap();
        assert!(res.can_execute);

        // owner can stake
        let res = obi
            .query_can_execute(deps.as_ref(), OWNER.to_string(), staking_msg.clone())
            .unwrap();
        assert!(res.can_execute);

        // anyone cannot send
        let res = obi
            .query_can_execute(deps.as_ref(), ANYONE.to_string(), send_msg)
            .unwrap();
        assert!(!res.can_execute);

        // anyone cannot stake
        let res = obi
            .query_can_execute(deps.as_ref(), ANYONE.to_string(), staking_msg)
            .unwrap();
        assert!(!res.can_execute);
    }

    #[test]
    fn add_spend_rm_permissioned_address() {
        let mut deps = mock_dependencies();
        let current_env = mock_env();
        let mut obi = ObiProxyContract::default();
        let _res = obi
            .instantiate(
                deps.as_mut(),
                current_env.clone(),
                mock_info(OWNER, &[]),
                get_test_instantiate_message(
                    current_env.clone(),
                    Coin {
                        amount: Uint128::from(0u128),
                        denom: "ujunox".to_string(),
                    },
                    false,
                ),
            )
            .unwrap();
        // this helper includes a PermissionedAddress

        // query to see we have "hotcarl" as permissioned address
        let res = obi.query_permissioned_addresses(deps.as_ref()).unwrap();
        assert!(res.permissioned_addresses.len() == 1);
        assert!(res.permissioned_addresses[0].address == PERMISSIONED_ADDRESS);

        // check that can_spend returns true
        let res = obi
            .can_spend(
                deps.as_ref(),
                current_env.clone(),
                PERMISSIONED_ADDRESS.to_string(),
                vec![],
                vec![CosmosMsg::Bank(BankMsg::Send {
                    to_address: RECEIVER.to_string(),
                    amount: coins(9_000u128, "testtokens"),
                })],
            )
            .unwrap();
        assert!(res.0.can_spend);

        // and returns false with some huge amount
        let res = obi
            .can_spend(
                deps.as_ref(),
                current_env.clone(),
                PERMISSIONED_ADDRESS.to_string(),
                vec![],
                vec![CosmosMsg::Bank(BankMsg::Send {
                    to_address: RECEIVER.to_string(),
                    amount: coins(999_999_999_000u128, "testtokens"),
                })],
            )
            .unwrap();
        assert!(!res.0.can_spend);

        // plus returns false with some unsupported kind of msg
        let expected_res = CanSpendResponse {
            can_spend: false,
            reason: "Distribution CosmosMsg not yet supported".to_string(),
        };
        let res = obi
            .can_spend(
                deps.as_ref(),
                current_env.clone(),
                PERMISSIONED_ADDRESS.to_string(),
                vec![],
                vec![CosmosMsg::Distribution(
                    DistributionMsg::SetWithdrawAddress {
                        address: RECEIVER.to_string(),
                    },
                )],
            )
            .unwrap();
        assert_eq!(res.0, expected_res);

        // and returns true with authorized contract
        let res = obi
            .can_spend(
                deps.as_ref(),
                current_env.clone(),
                PERMISSIONED_ADDRESS.to_string(),
                vec![],
                vec![CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr:
                        "juno1x5xz6wu8qlau8znmc60tmazzj3ta98quhk7qkamul3am2x8fsaqqcwy7n9"
                            .to_string(),
                    msg: to_binary(&Cw20ExecuteMsg::Transfer {
                        recipient: RECEIVER.to_string(),
                        amount: Uint128::from(1u128),
                    })
                    .unwrap(),
                    funds: vec![],
                })],
            )
            .unwrap();
        assert!(res.0.can_spend);

        // actually spend as the permissioned address
        let owner_info = mock_info(OWNER, &[]);
        let permissioned_address_info = mock_info(PERMISSIONED_ADDRESS, &[]);
        test_spend_bank(
            deps.as_mut(),
            &mut obi,
            current_env.clone(),
            RECEIVER.to_string(),
            coins(9_000u128, "testtokens"), //900_000 of usdc spend limit down
            permissioned_address_info,
        )
        .unwrap();

        // add a second permissioned address
        add_test_permissioned_address(
            deps.as_mut(),
            &mut obi,
            "hot_diane".to_string(),
            current_env.clone(),
            owner_info.clone(),
            1u16,
            PeriodType::DAYS,
            1_000_000u64,
        )
        .unwrap();

        // rm the permissioned address
        let bad_info = mock_info(ANYONE, &[]);
        let execute_msg = ExecuteMsg::RmPermissionedAddress {
            doomed_permissioned_address: PERMISSIONED_ADDRESS.to_string(),
        };
        let _res = obi
            .execute(
                deps.as_mut(),
                current_env.clone(),
                bad_info,
                execute_msg.clone(),
            )
            .unwrap_err();
        let _res = obi
            .execute(
                deps.as_mut(),
                current_env.clone(),
                owner_info.clone(),
                execute_msg,
            )
            .unwrap();

        // query permissioned addresss again, should be 1
        let res = obi.query_permissioned_addresses(deps.as_ref()).unwrap();
        assert!(res.permissioned_addresses.len() == 1);

        // add another permissioned address, this time with high USDC spend limit
        add_test_permissioned_address(
            deps.as_mut(),
            &mut obi,
            PERMISSIONED_USDC_WALLET.to_string(),
            current_env.clone(),
            owner_info,
            1u16,
            PeriodType::DAYS,
            100_000_000u64,
        )
        .unwrap();
        let res = obi.query_permissioned_addresses(deps.as_ref()).unwrap();
        assert!(res.permissioned_addresses.len() == 2);

        // now spend ... local tests will force price to be 1 = 100 USDC
        // so our spend limit of 100_000_000 will equal 1_000_000 testtokens

        let mocked_info = mock_info(PERMISSIONED_USDC_WALLET, &[]);
        let mut quick_spend_test = |amount: u128| -> Result<Response, ContractError> {
            test_spend_bank(
                deps.as_mut(),
                &mut obi,
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
        let obi = ObiProxyContract::default();
        let _res = obi
            .instantiate(
                deps.as_mut(),
                current_env.clone(),
                mock_info(OWNER, &[]),
                get_test_instantiate_message(
                    current_env,
                    Coin {
                        amount: Uint128::from(1_000_000u128),
                        denom:
                            "ibc/EAC38D55372F38F1AFD68DF7FE9EF762DCF69F26520643CF3F9D292A738D8034"
                                .to_string(),
                    },
                    false,
                ),
            )
            .unwrap();

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

        let info = mock_info(OWNER, &[]);
        let res = obi
            .execute(deps.as_mut(), mock_env(), info.clone(), execute_msg.clone())
            .unwrap();
        assert_eq!(
            res.messages,
            test_msgs.into_iter().map(SubMsg::new).collect::<Vec<_>>()
        );

        // now next identical send should not add the same fee repay message
        let res = obi
            .execute(deps.as_mut(), mock_env(), info, execute_msg)
            .unwrap();
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
