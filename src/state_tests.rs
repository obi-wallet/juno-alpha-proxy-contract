#[cfg(test)]
mod tests {
    use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
    use cosmwasm_std::testing::{mock_dependencies, mock_env};
    use cosmwasm_std::{Addr, Coin, Timestamp, Uint128};

    use crate::state::{CoinLimit, HotWallet, PeriodType, State};

    #[test]
    fn is_admin() {
        let admin: &str = "bob";
        let config = State {
            admin: Addr::unchecked(admin),
            pending: Addr::unchecked(admin),
            hot_wallets: vec![],
            uusd_fee_debt: Uint128::from(0u128),
            fee_lend_repay_wallet: Addr::unchecked("test_repay_address"),
            home_network: "local".to_string(),
        };

        assert!(config.is_admin(admin.to_string()));
        assert!(!config.is_admin("other".to_string()));
    }

    #[test]
    fn daily_spend_limit() {
        let deps = mock_dependencies();
        let admin: &str = "bob";
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
            admin: Addr::unchecked(admin),
            pending: Addr::unchecked(admin),
            hot_wallets: vec![HotWallet {
                address: spender.to_string(),
                current_period_reset: dt.timestamp() as u64,
                period_type: PeriodType::DAYS,
                period_multiple: 3,
                spend_limits: vec![
                    CoinLimit {
                        amount: 100_000_000u64,
                        denom: "ujuno".to_string(),
                        limit_remaining: 100_000_000u64,
                    },
                    CoinLimit {
                        amount: 100_000_000u64,
                        denom: "uaxlusdc".to_string(),
                        limit_remaining: 100_000_000u64,
                    },
                    CoinLimit {
                        amount: 9_000_000_000u64,
                        denom: "uloop".to_string(),
                        limit_remaining: 9_000_000_000u64,
                    },
                ],
                usdc_denom: Some("false".to_string()), // to avoid breaking local tests for now
                                                       // 100 JUNO, 100 axlUSDC, 9000 LOOP – but really only the USDC matters
                                                       // since usdc_denom is true
            }],
            uusd_fee_debt: Uint128::from(0u128),
            fee_lend_repay_wallet: Addr::unchecked("test_repay_address"),
            home_network: "local".to_string(),
        };

        println!("Spending 1,000,000 now");
        config
            .check_spend_limits(
                deps.as_ref(),
                now_env.block.time,
                spender.to_string(),
                vec![Coin {
                    denom: "ujuno".to_string(),
                    amount: Uint128::from(1_000_000u128),
                }],
            )
            .unwrap();
        println!("Trying 1,000,000 from bad sender");
        config
            .check_spend_limits(
                deps.as_ref(),
                now_env.block.time,
                bad_spender.to_string(),
                vec![Coin {
                    denom: "ujuno".to_string(),
                    amount: Uint128::from(1_000_000u128),
                }],
            )
            .unwrap_err();
        // now we shouldn't be able to total over our spend limit
        println!("Trying 99,500,000 (over limit)");
        config
            .check_spend_limits(
                deps.as_ref(),
                now_env.block.time,
                spender.to_string(),
                vec![Coin {
                    denom: "ujuno".to_string(),
                    amount: Uint128::from(99_500_000u128),
                }],
            )
            .unwrap_err();
        // our even 1 over our spend limit
        println!("Trying 99,000,001 (over limit)");
        config
            .check_spend_limits(
                deps.as_ref(),
                now_env.block.time,
                spender.to_string(),
                vec![Coin {
                    denom: "ujuno".to_string(),
                    amount: Uint128::from(99_000_001u128),
                }],
            )
            .unwrap_err();

        // but go 3 days + 1 second into the future and we should
        println!("Spending in future! 100,000,000 should pass now");
        let mut env_future = now_env;
        env_future.block.time =
            Timestamp::from_seconds(env_future.block.time.seconds() as u64 + 259206u64);
        config
            .check_spend_limits(
                deps.as_ref(),
                env_future.block.time,
                spender.to_string(),
                vec![Coin {
                    denom: "ujuno".to_string(),
                    amount: Uint128::from(100_000_000u128),
                }],
            )
            .unwrap();
    }

    #[test]
    fn monthly_spend_limit() {
        let deps = mock_dependencies();
        let admin: &str = "bob";
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
            admin: Addr::unchecked(admin),
            pending: Addr::unchecked(admin),
            hot_wallets: vec![HotWallet {
                address: spender.to_string(),
                current_period_reset: dt.timestamp() as u64,
                period_type: PeriodType::MONTHS,
                period_multiple: 38,
                spend_limits: vec![
                    CoinLimit {
                        amount: 7_000_000_000u64,
                        denom: "ujuno".to_string(),
                        limit_remaining: 100_000_000u64,
                    },
                    CoinLimit {
                        amount: 100_000_000u64,
                        denom: "uaxlusdc".to_string(),
                        limit_remaining: 100_000_000u64,
                    },
                    CoinLimit {
                        amount: 999_000_000_000u64,
                        denom: "juno1mrshruqvgctq5wah5plpe5wd97pq32f6ysc97tzxyd89gj8uxa7qcdwmnm"
                            .to_string(),
                        limit_remaining: 999_000_000_000u64,
                    },
                ],
                usdc_denom: None, // 100 JUNO, 100 axlUSDC, 9000 LOOP
            }],
            uusd_fee_debt: Uint128::from(0u128),
            fee_lend_repay_wallet: Addr::unchecked("test_repay_address"),
            home_network: "local".to_string(),
        };

        config
            .check_spend_limits(
                deps.as_ref(),
                now_env.block.time,
                spender.to_string(),
                vec![Coin {
                    denom: "juno1mrshruqvgctq5wah5plpe5wd97pq32f6ysc97tzxyd89gj8uxa7qcdwmnm"
                        .to_string(),
                    amount: Uint128::from(9_000_000_000u128),
                }],
            )
            .unwrap();
        config
            .check_spend_limits(
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
            .check_spend_limits(
                deps.as_ref(),
                now_env.block.time,
                spender.to_string(),
                vec![Coin {
                    denom: "juno1mrshruqvgctq5wah5plpe5wd97pq32f6ysc97tzxyd89gj8uxa7qcdwmnm"
                        .to_string(),
                    amount: Uint128::from(999_000_000_000u128),
                }],
            )
            .unwrap_err();

        // now we shouldn't be able to total just over our spend limit
        config
            .check_spend_limits(
                deps.as_ref(),
                now_env.block.time,
                spender.to_string(),
                vec![Coin {
                    denom: "juno1mrshruqvgctq5wah5plpe5wd97pq32f6ysc97tzxyd89gj8uxa7qcdwmnm"
                        .to_string(),
                    amount: Uint128::from(990_000_000_001u128),
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
            .check_spend_limits(
                deps.as_ref(),
                env_future.block.time,
                spender.to_string(),
                vec![Coin {
                    denom: "juno1mrshruqvgctq5wah5plpe5wd97pq32f6ysc97tzxyd89gj8uxa7qcdwmnm"
                        .to_string(),
                    amount: Uint128::from(990_000_000_001u128),
                }],
            )
            .unwrap();
    }
}