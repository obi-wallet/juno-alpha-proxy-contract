use cosmwasm_std::{Coin, Uint128};

use crate::{
    state::Source,
    sourced_coin::SourcedCoin,
    ContractError,
};

pub fn get_test_sourced_coin(
    denoms: (String, String),
    amount: Uint128,
    reverse: bool,
) -> Result<SourcedCoin, ContractError> {
    println!(
        "getting test swap for {:?} {} with reverse {}",
        denoms, amount, reverse
    );
    match denoms.clone() {
        val if val == ("testtokens".to_string(), "uloop".to_string()) => Ok(SourcedCoin {
            coin: Coin {
                amount,
                denom: denoms.1,
            },
            sources: vec![Source {
                contract_addr: "test conversion localjuno to loop".to_string(),
                query_msg: format!("converted {} to {}", amount, amount),
            }],
        }),
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
                println!("div uloop-usdc by 10000");
                amount.checked_div(Uint128::from(10000u128)).unwrap()
            } else {
                println!("mul uloop-usdc by 100");
                amount.checked_mul(Uint128::from(100u128)).unwrap()
            };
            Ok(SourcedCoin {
                coin: Coin {
                    amount: this_amount,
                    denom: denoms.1,
                },
                sources: vec![Source {
                    contract_addr: "test conversion loop to dollars".to_string(),
                    query_msg: format!("converted {} to {}", amount, this_amount),
                }],
            })
        }
        val if val == ("uloop".to_string(), "testtokens".to_string()) => Ok(SourcedCoin {
            coin: Coin {
                amount,
                denom: denoms.1,
            },
            sources: vec![Source {
                contract_addr: "test conversion loop to localjuno".to_string(),
                query_msg: format!("converted {} to {}", amount, amount),
            }],
        }),
        val if val
            == (
                "ibc/EAC38D55372F38F1AFD68DF7FE9EF762DCF69F26520643CF3F9D292A738D8034".to_string(),
                "uloop".to_string(),
            ) =>
        {
            let this_amount = if !reverse {
                println!("div usdc-uloop by 10000");
                amount.checked_div(Uint128::from(10000u128)).unwrap()
            } else {
                println!("div usdc-uloop by 100");
                amount.checked_div(Uint128::from(100u128)).unwrap()
            };
            Ok(SourcedCoin {
                coin: Coin {
                    amount: this_amount,
                    denom: denoms.1,
                },
                sources: vec![Source {
                    contract_addr: "test conversion dollars to loop".to_string(),
                    query_msg: format!("converted {} to {}", amount, this_amount),
                }],
            })
        }
        val if val
            == (
                "ibc/EAC38D55372F38F1AFD68DF7FE9EF762DCF69F26520643CF3F9D292A738D8034".to_string(),
                "testtokens".to_string(),
            ) =>
        {
            let this_amount = if !reverse {
                println!("div usdc-ujuno by 100");
                amount.checked_mul(Uint128::from(100u128)).unwrap()
            } else {
                println!("mul usdc-ujuno by 100");
                amount.checked_div(Uint128::from(10000u128)).unwrap()
            };
            Ok(SourcedCoin {
                coin: Coin {
                    amount: this_amount,
                    denom: denoms.1,
                },
                sources: vec![Source {
                    contract_addr: "test conversion dollars to juno".to_string(),
                    query_msg: format!("converted {} to {}", amount, this_amount),
                }],
            })
        }
        _ => Err(ContractError::BadSwapDenoms(format!(
            "unexpected unit test swap denoms: {:?} with amount {} and reverse {}",
            denoms, amount, reverse
        ))),
    }
}
