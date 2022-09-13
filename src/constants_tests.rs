use cosmwasm_std::{Coin, Uint128};

use crate::{state::{SourcedCoin, SourcedSwap}, constants::MAINNET_AXLUSDC_IBC};

pub fn get_test_sourced_swap() -> SourcedSwap {
  SourcedSwap {
    coin: Coin {
        amount: Uint128::from(100u128),
        denom: "testtokens".to_string(),
    },
    contract_addr: "local test path 1".to_string(),
  }
}

pub fn get_test_sourced_coin(spend: Coin) -> SourcedCoin {
    let amount = match &*spend.denom {
      "testtokens" => {
        spend.amount.saturating_mul(Uint128::from(100u128))
      }
      _ => spend.amount
    };
    SourcedCoin {
        coin: Coin {
            denom: MAINNET_AXLUSDC_IBC.to_string(),
            amount: amount,
        },
        top: SourcedSwap {
            coin: Coin {
                amount: spend.amount,
                denom: "test_1".to_string(),
            },
            contract_addr: "automatic mul by 100".to_string(),
        },
        bottom: SourcedSwap {
            coin: Coin {
                amount: Uint128::from(0u128),
                denom: "test_2".to_string(),
            },
            contract_addr: "test".to_string(),
        },
    }
}
