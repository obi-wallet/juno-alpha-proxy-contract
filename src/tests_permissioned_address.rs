#[cfg(test)]
mod tests {
    use cosmwasm_std::Timestamp;

    use crate::{
        constants::MAINNET_AXLUSDC_IBC,
        permissioned_address::{
            CoinLimit, PeriodType, PermissionedAddress, PermissionedAddressParams,
        },
    };

    #[test]
    fn permissioned_address_check_is_valid() {
        let bad_wallet_params = PermissionedAddressParams {
            address: "my_permissioned_address".to_string(),
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
            authorizations: None,
        };

        let mut bad_wallet = PermissionedAddress::new(bad_wallet_params);

        // multiple limits are no longer supported, so these should error
        bad_wallet.get_params().assert_is_valid().unwrap_err();
        bad_wallet
            .set_usdc_denom(Some("false".to_string()))
            .unwrap();
        bad_wallet.get_params().assert_is_valid().unwrap_err();
        bad_wallet.set_usdc_denom(Some("true".to_string())).unwrap();
        bad_wallet.get_params().assert_is_valid().unwrap_err();
        bad_wallet
            .update_spend_limit(bad_wallet.spend_limits()[1].clone())
            .unwrap();
        bad_wallet.get_params().assert_is_valid().unwrap();

        // now spend limits is fine; check the other usdc denom vals again
        bad_wallet
            .set_usdc_denom(Some("false".to_string()))
            .unwrap();
        bad_wallet.get_params().assert_is_valid().unwrap_err();
        bad_wallet.set_usdc_denom(None).unwrap();
        bad_wallet.get_params().assert_is_valid().unwrap_err();
    }

    #[test]
    fn permissioned_address_update_and_reset_spend_limit() {
        let starting_spend_limit = CoinLimit {
            denom: MAINNET_AXLUSDC_IBC.to_string(),
            amount: 1_000_000u64,
            limit_remaining: 1_000_000u64,
        };
        let mut permissioned_address = PermissionedAddress::new(PermissionedAddressParams {
            address: "my_permissioned_address".to_string(),
            current_period_reset: 1510010, //seconds, meaningless here
            period_type: PeriodType::DAYS,
            period_multiple: 1,
            spend_limits: vec![starting_spend_limit.clone()],
            usdc_denom: Some("true".to_string()),
            default: Some(true),
            authorizations: None,
        });

        assert_eq!(
            permissioned_address.spend_limits(),
            vec![starting_spend_limit.clone()]
        );

        let adjusted_spend_limit = CoinLimit {
            denom: MAINNET_AXLUSDC_IBC.to_string(),
            amount: 1_000_000u64,
            limit_remaining: 600_000u64,
        };

        permissioned_address
            .update_spend_limit(adjusted_spend_limit.clone())
            .unwrap();
        assert_eq!(
            permissioned_address.spend_limits(),
            vec![adjusted_spend_limit]
        );

        permissioned_address.reset_limits();
        assert_eq!(
            permissioned_address.spend_limits(),
            vec![starting_spend_limit]
        );

        let bigger_spend_limit = CoinLimit {
            denom: MAINNET_AXLUSDC_IBC.to_string(),
            amount: 420_000_000u64,
            limit_remaining: 420_000_000u64,
        };

        permissioned_address
            .update_spend_limit(bigger_spend_limit.clone())
            .unwrap();
        assert_eq!(
            permissioned_address.spend_limits(),
            vec![bigger_spend_limit]
        );
    }

    #[test]
    fn permissioned_address_update_reset_time_period() {
        let starting_spend_limit = CoinLimit {
            denom: MAINNET_AXLUSDC_IBC.to_string(),
            amount: 1_000_000u64,
            limit_remaining: 1_000_000u64,
        };
        let mut permissioned_address = PermissionedAddress::new(PermissionedAddressParams {
            address: "my_permissioned_address".to_string(),
            current_period_reset: 1_510_010, //seconds
            period_type: PeriodType::DAYS,
            period_multiple: 1,
            spend_limits: vec![starting_spend_limit.clone()],
            usdc_denom: Some("true".to_string()),
            default: Some(true),
            authorizations: None,
        });

        let adjusted_spend_limit = CoinLimit {
            denom: MAINNET_AXLUSDC_IBC.to_string(),
            amount: 1_000_000u64,
            limit_remaining: 600_000u64,
        };

        permissioned_address
            .update_spend_limit(adjusted_spend_limit.clone())
            .unwrap();
        assert_eq!(
            permissioned_address.spend_limits(),
            vec![adjusted_spend_limit]
        );

        permissioned_address
            .reset_period(Timestamp::from_seconds(1_510_011))
            .unwrap();
        assert_eq!(
            permissioned_address.spend_limits(),
            vec![starting_spend_limit]
        );
        assert_eq!(
            permissioned_address.current_period_reset(),
            1_510_011 + 86_400
        );
    }
}
