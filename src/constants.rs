use cosmwasm_std::{Coin, Uint128};

use crate::state::{SourcedCoin, SourcedSwap};

pub const MAINNET_AXLUSDC_IBC: &str =
    "ibc/EAC38D55372F38F1AFD68DF7FE9EF762DCF69F26520643CF3F9D292A738D8034";
// at a later date these will be retrieved from something like an on-chain registry
pub const TESTNET_DUMMY_CONTRACT: &str =
    "juno12em46sh7xx6qdgs0qgfn4yp67z7mzx4hnrstm7qjm7sd6ufjda2qv8hxrk";
pub const MAINNET_DEX_TOKEN_CONTRACT: &str =
    "juno1qsrercqegvs4ye0yqg93knv73ye5dc3prqwd6jcdcuj8ggp6w0us66deup";
pub const MAINNET_ID: &str = "juno-1";
pub const TESTNET_ID: &str = "uni-3";
pub const MAINNET_DENOM: &str = "ujuno";
pub const TESTNET_DENOM: &str = "ujunox";
pub const MAINNET_DEX_DENOM: &str = "uloop";

pub fn get_usdc_sourced_coin(amount: Uint128) -> SourcedCoin {
    SourcedCoin {
        coin: Coin {
            denom: MAINNET_AXLUSDC_IBC.to_string(),
            amount,
        },
        sources: vec![SourcedSwap {
            coin: Coin {
                denom: MAINNET_AXLUSDC_IBC.to_string(),
                amount,
            },
            contract_addr: "1 USDC is 1 USDC".to_string(),
        }],
    }
}
