#[cfg(test)]
mod tests {
    use cosmwasm_std::Uint128;

    use crate::{
        constants::{MAINNET_AXLUSDC_IBC, MAINNET_DENOM, MAINNET_DEX_DENOM},
        pair_contract::{PairContract, PairMessageType},
        simulation::{DexQueryMsg, DexQueryMsgType, FormatQueryMsg},
    };

    #[test]
    fn pair_contract_get_denoms() {
        let test_pair_contract = PairContract {
            contract_addr: String::from(
                "juno1ctsmp54v79x7ea970zejlyws50cj9pkrmw49x46085fn80znjmpqz2n642",
            ),
            denom1: String::from(MAINNET_DENOM),
            denom2: String::from(MAINNET_AXLUSDC_IBC),
            query_format: PairMessageType::JunoType,
        };

        assert_eq!(
            test_pair_contract.get_denoms().unwrap(),
            (MAINNET_DENOM.to_string(), MAINNET_AXLUSDC_IBC.to_string())
        );
    }

    #[test]
    fn pair_contract_create_loop_query_msg() {
        make_query_msg(
            false,
            ("testtokens".to_string(), MAINNET_DEX_DENOM.to_string()),
            "testtokens".to_string(),
            PairMessageType::LoopType,
            1_000_000u128,
        );
    }

    fn make_query_msg(
        flip_assets: bool,
        denoms: (String, String),
        expected_query_asset: String,
        ty: PairMessageType,
        amount: u128,
    ) {
        let test_pair_contract = PairContract {
            contract_addr: String::from(
                "juno1ctsmp54v79x7ea970zejlyws50cj9pkrmw49x46085fn80znjmpqz2n642",
            ),
            denom1: denoms.0,
            denom2: denoms.1,
            query_format: ty,
        };

        let this_amount = Uint128::from(amount);

        let query_msg = test_pair_contract
            .create_query_msg(this_amount, flip_assets)
            .unwrap();
        let dex_query_msg = DexQueryMsg {
            denom: expected_query_asset,
            amount: this_amount,
            ty: DexQueryMsgType::Simulation,
        };
        assert_eq!(query_msg.0, dex_query_msg.format_query_msg(false));
    }

    #[test]
    fn pair_contract_create_loop_reverse_query_msg() {
        let flip_assets = true; // going from dex tokens to test tokens
        let test_pair_contract = PairContract {
            contract_addr: String::from(
                "juno1ctsmp54v79x7ea970zejlyws50cj9pkrmw49x46085fn80znjmpqz2n642",
            ),
            denom1: "testtokens".to_string(),
            denom2: String::from(MAINNET_DEX_DENOM),
            query_format: PairMessageType::LoopType,
        };

        let amount = Uint128::from(1_000_000u128);

        let query_msg = test_pair_contract
            .create_query_msg(amount, flip_assets)
            .unwrap();
        let test_msg = DexQueryMsg {
            ty: DexQueryMsgType::ReverseSimulation,
            denom: "testtokens".to_string(),
            amount,
        };
        assert_eq!(query_msg.0, test_msg.format_query_msg(false));
    }

    #[test]
    fn pair_contract_create_juno_query_msg() {
        let flip_assets = false; // going from test tokens to usdc
        let test_pair_contract = PairContract {
            contract_addr: String::from(
                "juno1ctsmp54v79x7ea970zejlyws50cj9pkrmw49x46085fn80znjmpqz2n642",
            ),
            denom1: "testtokens".to_string(),
            denom2: String::from(MAINNET_AXLUSDC_IBC),
            query_format: PairMessageType::JunoType,
        };

        let amount = Uint128::from(1_000_000u128);

        let query_msg = test_pair_contract
            .create_query_msg(amount, flip_assets)
            .unwrap();
        let dex_query_msg = DexQueryMsg {
            denom: "testtokens".to_string(),
            amount,
            ty: DexQueryMsgType::Token1ForToken2Price,
        };
        assert_eq!(query_msg.0, dex_query_msg.format_query_msg(false));
    }

    #[test]
    fn pair_contract_create_juno_reverse_query_msg() {
        let flip_assets = true; // going from test tokens to usdc
        let test_pair_contract = PairContract {
            contract_addr: String::from(
                "juno1ctsmp54v79x7ea970zejlyws50cj9pkrmw49x46085fn80znjmpqz2n642",
            ),
            denom1: "testtokens".to_string(),
            denom2: String::from(MAINNET_AXLUSDC_IBC),
            query_format: PairMessageType::JunoType,
        };

        let amount = Uint128::from(1_000_000u128);

        let query_msg = test_pair_contract
            .create_query_msg(amount, flip_assets)
            .unwrap();
        let dex_query_msg = DexQueryMsg {
            denom: "testtokens".to_string(),
            amount,
            ty: DexQueryMsgType::Token2ForToken1Price,
        };
        assert_eq!(query_msg.0, dex_query_msg.format_query_msg(false));
    }
}
