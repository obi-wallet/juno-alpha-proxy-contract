#[cfg(test)]
mod tests {
    use crate::authorizations::Authorization;
    use crate::msg::{
        AuthorizationsResponse, CanSpendResponse, ExecuteMsg, QueryMsg, TestExecuteMsg,
        TestFieldsExecuteMsg,
    };
    use crate::state::ObiProxyContract;
    use crate::tests_contract::OWNER;
    use crate::tests_helpers::get_test_instantiate_message;
    use cosmwasm_std::testing::{mock_dependencies_with_balance, mock_env, mock_info};
    use cosmwasm_std::{coins, from_binary, to_binary, Api, Coin, CosmosMsg, Uint128, WasmMsg};

    #[test]
    fn add_authorization() {
        let mut deps = mock_dependencies_with_balance(&coins(2, "token"));

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

        let query_msg = QueryMsg::Authorizations {
            target_contract: Some("targetcontract".to_string()),
            limit: None,
            start_after: None,
        };

        // non-owner cannot add authorization
        let info = mock_info("anyone", &coins(2, "token"));
        let msg = ExecuteMsg::AddAuthorization {
            new_authorization: Authorization {
                count: Uint128::from(0u128),
                actor: deps.api.addr_validate("anyone").unwrap(),
                contract: deps.api.addr_validate("targetcontract").unwrap(),
                message_name: "test_execute_msg".to_string(),
                fields: None,
            },
        };
        let _res = obi
            .execute(deps.as_mut(), mock_env(), info, msg)
            .unwrap_err();

        // zero authorizations
        let res: AuthorizationsResponse = from_binary(
            &obi.query(deps.as_ref(), mock_env(), query_msg.clone())
                .unwrap(),
        )
        .unwrap();
        assert_eq!(res.authorizations.len(), 0);

        // owner can add authorization
        let info = mock_info(OWNER, &coins(2, "token"));
        let msg = ExecuteMsg::AddAuthorization {
            new_authorization: Authorization {
                count: Uint128::from(0u128),
                actor: deps.api.addr_validate("actor").unwrap(),
                contract: deps.api.addr_validate("targetcontract").unwrap(),
                message_name: "test_execute_msg".to_string(),
                fields: None,
            },
        };
        let _res = obi.execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        // now one authorization
        let res: AuthorizationsResponse = from_binary(
            &obi.query(deps.as_ref(), mock_env(), query_msg.clone())
                .unwrap(),
        )
        .unwrap();
        assert_eq!(res.authorizations.len(), 1);

        // given action should fail if NOT BY ACTOR
        let msg = QueryMsg::CanSpend {
            sender: "anyone".to_string(),
            funds: vec![],
            msgs: vec![CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: "targetcontract".to_string(),
                msg: to_binary(&TestExecuteMsg {
                    foo: "bar".to_string(),
                })
                .unwrap(),
                funds: vec![],
            })],
        };
        let expected_res = CanSpendResponse {
            can_spend: false,
            reason: "Not an authorized action".to_string(),
        };
        let res: CanSpendResponse =
            from_binary(&obi.query(deps.as_ref(), mock_env(), msg).unwrap()).unwrap();
        assert_eq!(res, expected_res);

        // given action should fail if WRONG TARGET CONTRACT
        let msg = QueryMsg::CanSpend {
            sender: "actor".to_string(),
            funds: vec![],
            msgs: vec![CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: "badtargetcontract".to_string(),
                msg: to_binary(&TestExecuteMsg {
                    foo: "bar".to_string(),
                })
                .unwrap(),
                funds: vec![],
            })],
        };
        let expected_res = CanSpendResponse {
            can_spend: false,
            reason: "Not an authorized action".to_string(),
        };
        let res: CanSpendResponse =
            from_binary(&obi.query(deps.as_ref(), mock_env(), msg).unwrap()).unwrap();
        assert_eq!(res, expected_res);

        // given action should succeed if contract correct (no field checking yet)
        let msg = QueryMsg::CanSpend {
            sender: "actor".to_string(),
            funds: vec![],
            msgs: vec![CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: "targetcontract".to_string(),
                msg: to_binary(&TestExecuteMsg {
                    foo: "bar".to_string(),
                })
                .unwrap(),
                funds: vec![],
            })],
        };
        let _res = obi.query(deps.as_ref(), mock_env(), msg).unwrap();

        // unauthorized user cannot remove an authorization
        let info = mock_info("baduser", &coins(2, "token"));
        let msg = ExecuteMsg::RemoveAuthorization {
            authorization_to_remove: Authorization {
                count: Uint128::from(0u128),
                actor: deps.api.addr_validate("actor").unwrap(),
                contract: deps.api.addr_validate("targetcontract").unwrap(),
                message_name: "test_execute_msg".to_string(),
                fields: None,
            },
        };
        let _res = obi
            .execute(deps.as_mut(), mock_env(), info, msg)
            .unwrap_err();

        // let's remove an authorization
        let info = mock_info(OWNER, &coins(2, "token"));
        let msg = ExecuteMsg::RemoveAuthorization {
            authorization_to_remove: Authorization {
                count: Uint128::from(0u128),
                actor: deps.api.addr_validate("actor").unwrap(),
                contract: deps.api.addr_validate("targetcontract").unwrap(),
                message_name: "test_execute_msg".to_string(),
                fields: None,
            },
        };
        let _res = obi.execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        // now zero authorizations
        let res: AuthorizationsResponse =
            from_binary(&obi.query(deps.as_ref(), mock_env(), query_msg).unwrap()).unwrap();
        assert_eq!(res.authorizations.len(), 0);

        //and action fails where before it succeeded
        let msg = QueryMsg::CanSpend {
            sender: "actor".to_string(),
            funds: vec![],
            msgs: vec![CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: "targetcontract".to_string(),
                msg: to_binary(&TestExecuteMsg {
                    foo: "bar".to_string(),
                })
                .unwrap(),
                funds: vec![],
            })],
        };
        let expected_res = CanSpendResponse {
            can_spend: false,
            reason: "Not an authorized action".to_string(),
        };
        let res: CanSpendResponse =
            from_binary(&obi.query(deps.as_ref(), mock_env(), msg).unwrap()).unwrap();
        assert_eq!(res, expected_res);
    }

    #[test]
    fn authorization_fields() {
        let mut deps = mock_dependencies_with_balance(&coins(2, "token"));

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

        let query_msg = QueryMsg::Authorizations {
            target_contract: Some("targetcontract".to_string()),
            limit: None,
            start_after: None,
        };

        // add authorization with fields
        let info = mock_info(OWNER, &coins(2, "token"));
        let msg = ExecuteMsg::AddAuthorization {
            new_authorization: Authorization {
                count: Uint128::from(0u128),
                actor: deps.api.addr_validate("actor").unwrap(),
                contract: deps.api.addr_validate("targetcontract").unwrap(),
                message_name: "test_fields_execute_msg".to_string(),
                fields: Some(
                    [
                        ("recipient".to_string(), "picard".to_string()),
                        ("strategy".to_string(), "engage".to_string()),
                    ]
                    .to_vec(),
                ),
            },
        };
        let _res = obi.execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        // given action should succeed if contract correct
        let msg = QueryMsg::CanSpend {
            sender: "actor".to_string(),
            funds: vec![],
            msgs: vec![CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: "targetcontract".to_string(),
                msg: to_binary(&TestFieldsExecuteMsg {
                    recipient: "picard".to_string(),
                    strategy: "engage".to_string(),
                })
                .unwrap(),
                funds: vec![],
            })],
        };
        let _res = obi.query(deps.as_ref(), mock_env(), msg).unwrap();

        // let's remove the authorization with no field checking... should fail
        let info = mock_info(OWNER, &coins(2, "token"));
        let msg = ExecuteMsg::RemoveAuthorization {
            authorization_to_remove: Authorization {
                count: Uint128::from(0u128),
                actor: deps.api.addr_validate("actor").unwrap(),
                contract: deps.api.addr_validate("targetcontract").unwrap(),
                message_name: "test_fields_execute_msg".to_string(),
                fields: None,
            },
        };
        let _res = obi
            .execute(deps.as_mut(), mock_env(), info, msg)
            .unwrap_err();

        // still one authorization
        let res: AuthorizationsResponse = from_binary(
            &obi.query(deps.as_ref(), mock_env(), query_msg.clone())
                .unwrap(),
        )
        .unwrap();
        assert_eq!(res.authorizations.len(), 1);

        // Now let's remove with fields specified
        let info = mock_info(OWNER, &coins(2, "token"));
        let msg = ExecuteMsg::RemoveAuthorization {
            authorization_to_remove: Authorization {
                count: Uint128::from(0u128),
                actor: deps.api.addr_validate("actor").unwrap(),
                contract: deps.api.addr_validate("targetcontract").unwrap(),
                message_name: "test_fields_execute_msg".to_string(),
                fields: Some(
                    [
                        ("recipient".to_string(), "picard".to_string()),
                        ("strategy".to_string(), "engage".to_string()),
                    ]
                    .to_vec(),
                ),
            },
        };
        let _res = obi.execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        // now zero authorizations
        let res: AuthorizationsResponse =
            from_binary(&obi.query(deps.as_ref(), mock_env(), query_msg).unwrap()).unwrap();
        assert_eq!(res.authorizations.len(), 0);

        // let's test with just strategy, and no qualification on recipient
        let info = mock_info(OWNER, &coins(2, "token"));
        let msg = ExecuteMsg::AddAuthorization {
            new_authorization: Authorization {
                count: Uint128::from(0u128),
                actor: deps.api.addr_validate("actor").unwrap(),
                contract: deps.api.addr_validate("targetcontract").unwrap(),
                message_name: "test_fields_execute_msg".to_string(),
                fields: Some([("strategy".to_string(), "engage".to_string())].to_vec()),
            },
        };
        let _res = obi.execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        // fails if strategy is wrong
        let msg = QueryMsg::CanSpend {
            sender: "actor".to_string(),
            funds: vec![],
            msgs: vec![CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: "targetcontract".to_string(),
                msg: to_binary(&TestFieldsExecuteMsg {
                    recipient: "picard".to_string(),
                    strategy: "assimiliate".to_string(),
                })
                .unwrap(),
                funds: vec![],
            })],
        };
        let expected_res = CanSpendResponse {
            can_spend: false,
            reason: "Not an authorized action".to_string(),
        };
        let res: CanSpendResponse =
            from_binary(&obi.query(deps.as_ref(), mock_env(), msg).unwrap()).unwrap();
        assert_eq!(res, expected_res);

        // succeeds if strategy is allowed
        let msg = QueryMsg::CanSpend {
            sender: "actor".to_string(),
            funds: vec![],
            msgs: vec![CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: "targetcontract".to_string(),
                msg: to_binary(&TestFieldsExecuteMsg {
                    recipient: "picard".to_string(),
                    strategy: "engage".to_string(),
                })
                .unwrap(),
                funds: vec![],
            })],
        };
        let _res = obi.query(deps.as_ref(), mock_env(), msg).unwrap();

        // remove fails with too many fields specified
        let info = mock_info(OWNER, &coins(2, "token"));
        let msg = ExecuteMsg::RemoveAuthorization {
            authorization_to_remove: Authorization {
                count: Uint128::from(0u128),
                actor: deps.api.addr_validate("actor").unwrap(),
                contract: deps.api.addr_validate("targetcontract").unwrap(),
                message_name: "test_fields_execute_msg".to_string(),
                fields: Some(
                    [
                        ("recipient".to_string(), "picard".to_string()),
                        ("strategy".to_string(), "engage".to_string()),
                    ]
                    .to_vec(),
                ),
            },
        };
        let _res = obi
            .execute(deps.as_mut(), mock_env(), info, msg)
            .unwrap_err();

        // remove succeeds with single field (strategy) specified
        let info = mock_info(OWNER, &coins(2, "token"));
        let msg = ExecuteMsg::RemoveAuthorization {
            authorization_to_remove: Authorization {
                count: Uint128::from(0u128),
                actor: deps.api.addr_validate("actor").unwrap(),
                contract: deps.api.addr_validate("targetcontract").unwrap(),
                message_name: "test_fields_execute_msg".to_string(),
                fields: Some([("strategy".to_string(), "engage".to_string())].to_vec()),
            },
        };
        let _res = obi.execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    }

    #[test]
    fn handling_repeat_authorization_fields() {
        let mut deps = mock_dependencies_with_balance(&coins(2, "token"));

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

        // add authorization with fields
        let info = mock_info(OWNER, &coins(2, "token"));
        let msg = ExecuteMsg::AddAuthorization {
            new_authorization: Authorization {
                count: Uint128::from(0u128),
                actor: deps.api.addr_validate("actor").unwrap(),
                contract: deps.api.addr_validate("targetcontract").unwrap(),
                message_name: "test_fields_execute_msg".to_string(),
                fields: Some(
                    [
                        ("recipient".to_string(), "picard".to_string()),
                        ("strategy".to_string(), "engage".to_string()),
                    ]
                    .to_vec(),
                ),
            },
        };
        let _res = obi.execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        // adding the same again should cause an error
        // in the future, maybe change this test to update expiration instead
        let info = mock_info(OWNER, &coins(2, "token"));
        let msg = ExecuteMsg::AddAuthorization {
            new_authorization: Authorization {
                count: Uint128::from(0u128),
                actor: deps.api.addr_validate("actor").unwrap(),
                contract: deps.api.addr_validate("targetcontract").unwrap(),
                message_name: "test_fields_execute_msg".to_string(),
                fields: Some(
                    [
                        ("recipient".to_string(), "picard".to_string()),
                        ("strategy".to_string(), "engage".to_string()),
                    ]
                    .to_vec(),
                ),
            },
        };
        let _res = obi
            .execute(deps.as_mut(), mock_env(), info, msg)
            .unwrap_err();
    }
}
