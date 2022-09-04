pub const USDC: &str = "ibc/EAC38D55372F38F1AFD68DF7FE9EF762DCF69F26520643CF3F9D292A738D8034";
pub mod contract;
#[cfg(test)]
mod contract_tests;
pub mod error;
pub mod helpers;
#[cfg(test)]
mod integration_tests;
pub mod msg;
pub mod state;

pub use crate::error::ContractError;
