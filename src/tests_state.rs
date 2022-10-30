#[cfg(test)]
mod tests {
    use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
    use cosmwasm_std::testing::{mock_dependencies, mock_env};
    use cosmwasm_std::{Addr, Coin, Timestamp, Uint128};

    use crate::pair_contract::PairContracts;
    use crate::pair_contract_defaults::get_local_pair_contracts;
    use crate::permissioned_address::{
        CoinLimit, PeriodType, PermissionedAddress, PermissionedAddressParams,
    };
    use crate::signers::Signers;
    use crate::state::State;

    #[test]
    fn is_owner() {
        let now_env = mock_env();
        let deps = mock_dependencies();
        let owner: &str = "bob";
        let config = State {
            owner: Addr::unchecked(owner),
            owner_signers: Signers::new(
                deps.as_ref(),
                vec!["signer1".to_string(), "signer2".to_string()],
                vec!["type1".to_string(), "type2".to_string()],
            )
            .unwrap(),
            pending: Addr::unchecked(owner),
            permissioned_addresses: vec![],
            uusd_fee_debt: Uint128::from(0u128),
            fee_lend_repay_wallet: Addr::unchecked("test_repay_address"),
            home_network: "local".to_string(),
            pair_contracts: PairContracts {
                pair_contracts: get_local_pair_contracts().to_vec(),
            },
            update_delay_hours: 0u16,
            update_pending_time: now_env.block.time,
            auth_count: Uint128::from(0u128),
            frozen: false,
        };

        assert!(config.is_owner(owner.to_string()));
        assert!(!config.is_owner("other".to_string()));
    }

    #[test]
    fn daily_spend_limit() {
        let deps = mock_dependencies();
        let owner: &str = "bob";
        let spender = "owner";
        let bad_spender: &str = "medusa";
        let dt = NaiveDateTime::new(
            NaiveDate::from_ymd(2022, 6, 3),
            NaiveTime::from_hms_milli(12, 00, 00, 000),
        );
        let mut now_env = mock_env();
        now_env.block.time = Timestamp::from_seconds(dt.timestamp() as u64);
        // 3 day spend limit period
        let mut config = State {
            owner: Addr::unchecked(owner),
            pending: Addr::unchecked(owner),
            permissioned_addresses: vec![PermissionedAddress::new(PermissionedAddressParams {
                address: spender.to_string(),
                current_period_reset: dt.timestamp() as u64,
                period_type: PeriodType::DAYS,
                period_multiple: 3,
                spend_limits: vec![CoinLimit {
                    amount: 100_000_000u64,
                    denom: "ibc/EAC38D55372F38F1AFD68DF7FE9EF762DCF69F26520643CF3F9D292A738D8034"
                        .to_string(),
                    limit_remaining: 100_000_000u64,
                }],
                usdc_denom: Some("true".to_string()),
                default: Some(true),
                authorizations: None,
            })],
            uusd_fee_debt: Uint128::from(0u128),
            fee_lend_repay_wallet: Addr::unchecked("test_repay_address"),
            home_network: "local".to_string(),
            pair_contracts: PairContracts {
                pair_contracts: get_local_pair_contracts().to_vec(),
            },
            update_delay_hours: 0u16,
            update_pending_time: now_env.block.time,
            owner_signers: Signers::new(
                deps.as_ref(),
                vec!["signer1".to_string(), "signer2".to_string()],
                vec!["type1".to_string(), "type2".to_string()],
            )
            .unwrap(),
            auth_count: Uint128::from(0u128),
            frozen: false,
        };

        config
            .check_and_update_spend_limits(
                deps.as_ref(),
                now_env.block.time,
                spender.to_string(),
                vec![Coin {
                    denom: "ibc/EAC38D55372F38F1AFD68DF7FE9EF762DCF69F26520643CF3F9D292A738D8034"
                        .to_string(),
                    amount: Uint128::from(1_000_000u128),
                }],
            )
            .unwrap();
        config
            .check_and_update_spend_limits(
                deps.as_ref(),
                now_env.block.time,
                bad_spender.to_string(),
                vec![Coin {
                    denom: "ibc/EAC38D55372F38F1AFD68DF7FE9EF762DCF69F26520643CF3F9D292A738D8034"
                        .to_string(),
                    amount: Uint128::from(1_000_000u128),
                }],
            )
            .unwrap_err();
        // now we shouldn't be able to total over our spend limit
        config
            .check_and_update_spend_limits(
                deps.as_ref(),
                now_env.block.time,
                spender.to_string(),
                vec![Coin {
                    denom: "ibc/EAC38D55372F38F1AFD68DF7FE9EF762DCF69F26520643CF3F9D292A738D8034"
                        .to_string(),
                    amount: Uint128::from(99_500_000u128),
                }],
            )
            .unwrap_err();
        // our even 1 over our spend limit
        config
            .check_and_update_spend_limits(
                deps.as_ref(),
                now_env.block.time,
                spender.to_string(),
                vec![Coin {
                    denom: "ibc/EAC38D55372F38F1AFD68DF7FE9EF762DCF69F26520643CF3F9D292A738D8034"
                        .to_string(),
                    amount: Uint128::from(99_000_001u128),
                }],
            )
            .unwrap_err();

        // but go 3 days + 1 second into the future and we should
        let mut env_future = now_env;
        env_future.block.time =
            Timestamp::from_seconds(env_future.block.time.seconds() as u64 + 259206u64);
        config
            .check_and_update_spend_limits(
                deps.as_ref(),
                env_future.block.time,
                spender.to_string(),
                vec![Coin {
                    denom: "ibc/EAC38D55372F38F1AFD68DF7FE9EF762DCF69F26520643CF3F9D292A738D8034"
                        .to_string(),
                    amount: Uint128::from(100_000_000u128),
                }],
            )
            .unwrap();
    }

    #[test]
    fn monthly_spend_limit() {
        let deps = mock_dependencies();
        let owner: &str = "bob";
        let spender = "owner";
        let bad_spender: &str = "medusa";
        let dt = NaiveDateTime::new(
            NaiveDate::from_ymd(2022, 6, 3),
            NaiveTime::from_hms_milli(12, 00, 00, 000),
        );
        let mut now_env = mock_env();
        now_env.block.time = Timestamp::from_seconds(dt.timestamp() as u64);

        // Let's do a 38 month spend limit period
        // and for kicks use a contract address for LOOP
        let mut config = State {
            owner: Addr::unchecked(owner),
            pending: Addr::unchecked(owner),
            permissioned_addresses: vec![PermissionedAddress::new(PermissionedAddressParams {
                address: spender.to_string(),
                current_period_reset: dt.timestamp() as u64,
                period_type: PeriodType::MONTHS,
                period_multiple: 38,
                spend_limits: vec![CoinLimit {
                    amount: 100_000_000u64,
                    denom: "ibc/EAC38D55372F38F1AFD68DF7FE9EF762DCF69F26520643CF3F9D292A738D8034"
                        .to_string(),
                    limit_remaining: 100_000_000u64,
                }],
                usdc_denom: None, // 100 JUNO, 100 axlUSDC, 9000 LOOP
                default: Some(true),
                authorizations: None,
            })],
            uusd_fee_debt: Uint128::from(0u128),
            fee_lend_repay_wallet: Addr::unchecked("test_repay_address"),
            home_network: "local".to_string(),
            pair_contracts: PairContracts {
                pair_contracts: get_local_pair_contracts().to_vec(),
            },
            update_delay_hours: 0u16,
            update_pending_time: now_env.block.time,
            owner_signers: Signers::new(
                deps.as_ref(),
                vec!["signer1".to_string(), "signer2".to_string()],
                vec!["type1".to_string(), "type2".to_string()],
            )
            .unwrap(),
            auth_count: Uint128::from(0u128),
            frozen: false,
        };

        config
            .check_and_update_spend_limits(
                deps.as_ref(),
                now_env.block.time,
                spender.to_string(),
                vec![Coin {
                    denom: "ibc/EAC38D55372F38F1AFD68DF7FE9EF762DCF69F26520643CF3F9D292A738D8034"
                        .to_string(),
                    amount: Uint128::from(1_000_000u128),
                }],
            )
            .unwrap();
        config
            .check_and_update_spend_limits(
                deps.as_ref(),
                now_env.block.time,
                bad_spender.to_string(),
                vec![Coin {
                    denom: "ujuno".to_string(),
                    amount: Uint128::from(1_000_000u128),
                }],
            )
            .unwrap_err();
        config
            .check_and_update_spend_limits(
                deps.as_ref(),
                now_env.block.time,
                spender.to_string(),
                vec![Coin {
                    denom: "ibc/EAC38D55372F38F1AFD68DF7FE9EF762DCF69F26520643CF3F9D292A738D8034"
                        .to_string(),
                    amount: Uint128::from(99_000_001u128),
                }],
            )
            .unwrap_err();

        // but go 38 months (minus a couple of days - reset is the 1st, not the 3rd)
        // into the future and we should be able to spend
        let dt = NaiveDateTime::new(
            NaiveDate::from_ymd(2025, 8, 1),
            NaiveTime::from_hms_milli(12, 00, 00, 000),
        );
        let mut env_future = mock_env();
        env_future.block.time = Timestamp::from_seconds(dt.timestamp() as u64);
        config
            .check_and_update_spend_limits(
                deps.as_ref(),
                env_future.block.time,
                spender.to_string(),
                vec![Coin {
                    denom: "ibc/EAC38D55372F38F1AFD68DF7FE9EF762DCF69F26520643CF3F9D292A738D8034"
                        .to_string(),
                    amount: Uint128::from(99_000_001u128),
                }],
            )
            .unwrap();
    }
}
