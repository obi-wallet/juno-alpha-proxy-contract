#[cfg(test)]
mod tests {
    use cosmwasm_std::Timestamp;

    use crate::{
        constants::MAINNET_AXLUSDC_IBC,
        hot_wallet::{CoinLimit, HotWallet, PeriodType},
    };

    #[test]
    fn hot_wallet_check_is_valid() {
        let mut bad_wallet = HotWallet {
            address: "my_hot_wallet".to_string(),
            current_period_reset: 1510010, //seconds, meaningless here
            period_type: PeriodType::DAYS,
            period_multiple: 1,
            spend_limits: vec![
                CoinLimit {
                    denom: "non-usd-token".to_string(),
                    amount: 1_000_000u64,
                    limit_remaining: 1_000_000u64,
                },
                CoinLimit {
                    denom: MAINNET_AXLUSDC_IBC.to_string(),
                    amount: 1_000_000u64,
                    limit_remaining: 1_000_000u64,
                },
            ],
            usdc_denom: None,
            default: Some(true),
        };

        // multiple limits are no longer supported, so these should error
        bad_wallet.assert_is_valid().unwrap_err();
        bad_wallet.usdc_denom = Some("false".to_string());
        bad_wallet.assert_is_valid().unwrap_err();
        bad_wallet.usdc_denom = Some("true".to_string());
        bad_wallet.assert_is_valid().unwrap_err();
        bad_wallet.spend_limits = vec![bad_wallet.spend_limits[1].clone()];
        bad_wallet.assert_is_valid().unwrap();

        // now spend limits is fine; check the other usdc denom vals again
        bad_wallet.usdc_denom = Some("false".to_string());
        bad_wallet.assert_is_valid().unwrap_err();
        bad_wallet.usdc_denom = None;
        bad_wallet.assert_is_valid().unwrap_err();
    }

    #[test]
    fn hot_wallet_update_and_reset_spend_limit() {
        let starting_spend_limit = CoinLimit {
            denom: MAINNET_AXLUSDC_IBC.to_string(),
            amount: 1_000_000u64,
            limit_remaining: 1_000_000u64,
        };
        let mut hot_wallet = HotWallet {
            address: "my_hot_wallet".to_string(),
            current_period_reset: 1510010, //seconds, meaningless here
            period_type: PeriodType::DAYS,
            period_multiple: 1,
            spend_limits: vec![starting_spend_limit.clone()],
            usdc_denom: Some("true".to_string()),
            default: Some(true),
        };

        assert_eq!(hot_wallet.spend_limits, vec![starting_spend_limit.clone()]);

        let adjusted_spend_limit = CoinLimit {
            denom: MAINNET_AXLUSDC_IBC.to_string(),
            amount: 1_000_000u64,
            limit_remaining: 600_000u64,
        };

        hot_wallet
            .update_spend_limit(adjusted_spend_limit.clone())
            .unwrap();
        assert_eq!(hot_wallet.spend_limits, vec![adjusted_spend_limit]);

        hot_wallet.reset_limits();
        assert_eq!(hot_wallet.spend_limits, vec![starting_spend_limit]);

        let bigger_spend_limit = CoinLimit {
            denom: MAINNET_AXLUSDC_IBC.to_string(),
            amount: 420_000_000u64,
            limit_remaining: 420_000_000u64,
        };

        hot_wallet
            .update_spend_limit(bigger_spend_limit.clone())
            .unwrap();
        assert_eq!(hot_wallet.spend_limits, vec![bigger_spend_limit]);
    }

    #[test]
    fn hot_wallet_update_reset_time_period() {
        let starting_spend_limit = CoinLimit {
            denom: MAINNET_AXLUSDC_IBC.to_string(),
            amount: 1_000_000u64,
            limit_remaining: 1_000_000u64,
        };
        let mut hot_wallet = HotWallet {
            address: "my_hot_wallet".to_string(),
            current_period_reset: 1_510_010, //seconds
            period_type: PeriodType::DAYS,
            period_multiple: 1,
            spend_limits: vec![starting_spend_limit.clone()],
            usdc_denom: Some("true".to_string()),
            default: Some(true),
        };

        let adjusted_spend_limit = CoinLimit {
            denom: MAINNET_AXLUSDC_IBC.to_string(),
            amount: 1_000_000u64,
            limit_remaining: 600_000u64,
        };

        hot_wallet
            .update_spend_limit(adjusted_spend_limit.clone())
            .unwrap();
        assert_eq!(hot_wallet.spend_limits, vec![adjusted_spend_limit]);

        hot_wallet
            .reset_period(Timestamp::from_seconds(1_510_011))
            .unwrap();
        assert_eq!(hot_wallet.spend_limits, vec![starting_spend_limit]);
        assert_eq!(hot_wallet.current_period_reset, 1_510_011 + 86_400);
    }
}
