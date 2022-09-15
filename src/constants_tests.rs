use cosmwasm_std::{Coin, Uint128};

use crate::{
    constants::MAINNET_AXLUSDC_IBC,
    state::{SourcedCoin, SourcedSwap},
};

pub fn get_test_sourced_swap(
    denoms: (String, String),
    amount: Uint128,
    reverse: bool,
) -> SourcedSwap {
    println!(
        "getting test swap for {:?} {} with reverse {}",
        denoms, amount, reverse
    );
    match denoms.clone() {
        val if val == ("testtokens".to_string(), "uloop".to_string()) => SourcedSwap {
            coin: Coin {
                amount,
                denom: denoms.1,
            },
            contract_addr: "test conversion localjuno to loop".to_string(),
        },
        val if val
            == (
                "testtokens".to_string(),
                "ibc/EAC38D55372F38F1AFD68DF7FE9EF762DCF69F26520643CF3F9D292A738D8034".to_string(),
            )
            || val
                == (
                    "uloop".to_string(),
                    "ibc/EAC38D55372F38F1AFD68DF7FE9EF762DCF69F26520643CF3F9D292A738D8034"
                        .to_string(),
                ) =>
        {
            let this_amount = if reverse {
                amount.checked_div(Uint128::from(10000u128)).unwrap()
            } else {
                amount.checked_mul(Uint128::from(100u128)).unwrap()
            };
            SourcedSwap {
                coin: Coin {
                    amount: this_amount,
                    denom: denoms.1,
                },
                contract_addr: "test conversion loop to dollars".to_string(),
            }
        }
        val if val == ("uloop".to_string(), "testtokens".to_string()) => SourcedSwap {
            coin: Coin {
                amount,
                denom: denoms.1,
            },
            contract_addr: "test conversion loop to localjuno".to_string(),
        },
        val if val
            == (
                "ibc/EAC38D55372F38F1AFD68DF7FE9EF762DCF69F26520643CF3F9D292A738D8034".to_string(),
                "uloop".to_string(),
            ) =>
        {
            let this_amount = if !reverse {
                amount.checked_mul(Uint128::from(10000u128)).unwrap()
            } else {
                amount.checked_div(Uint128::from(10000u128)).unwrap()
            };
            SourcedSwap {
                coin: Coin {
                    amount: this_amount,
                    denom: denoms.1,
                },
                contract_addr: "test conversion dollars to loop".to_string(),
            }
        }
        _ => {
            panic!("unexpected unit test swap denoms: {:?}", denoms);
        }
    }
}

pub fn get_test_sourced_coin(spend: Coin) -> SourcedCoin {
    let amount = match &*spend.denom {
        "testtokens" => spend.amount.saturating_mul(Uint128::from(100u128)),
        _ => spend.amount,
    };
    SourcedCoin {
        coin: Coin {
            denom: MAINNET_AXLUSDC_IBC.to_string(),
            amount,
        },
        sources: vec![SourcedSwap {
            coin: Coin {
                amount: spend.amount,
                denom: "test_1".to_string(),
            },
            contract_addr: "automatic mul by 100".to_string(),
        }],
    }
}
