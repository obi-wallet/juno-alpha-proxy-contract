use cosmwasm_std::{BankMsg, Coin, CosmosMsg, DepsMut, Env, MessageInfo, Response};

use crate::contract::{execute, execute_execute};
use crate::error::ContractError;
use crate::hot_wallet::{CoinLimit, HotWallet};
use crate::msg::ExecuteMsg;
use crate::{contract::query_hot_wallets, hot_wallet::PeriodType};

pub fn add_test_hotwallet(
    mut deps: DepsMut,
    address: String,
    current_env: Env,
    info: MessageInfo,
    period_multiple: u16,
    period_type: PeriodType,
    limit: u64,
) -> Result<Response, ContractError> {
    let res = query_hot_wallets(deps.as_ref()).unwrap();
    let old_length = res.hot_wallets.len();
    let execute_msg = ExecuteMsg::AddHotWallet {
        new_hot_wallet: HotWallet {
            address,
            current_period_reset: current_env.block.time.seconds() as u64,
            period_type,
            period_multiple,
            spend_limits: vec![CoinLimit {
                denom: "ibc/EAC38D55372F38F1AFD68DF7FE9EF762DCF69F26520643CF3F9D292A738D8034"
                    .to_string(),
                amount: limit,
                limit_remaining: limit,
            }],
            usdc_denom: Some("true".to_string()),
        },
    };

    let _res = execute(deps.branch(), current_env.clone(), info, execute_msg).unwrap();
    let res = query_hot_wallets(deps.as_ref()).unwrap();
    assert!(res.hot_wallets.len() == old_length + 1);
    Ok(Response::new())
}

pub fn test_spend_bank(
    mut deps: DepsMut,
    current_env: Env,
    to_address: String,
    amount: Vec<Coin>,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    let send_msg = CosmosMsg::Bank(BankMsg::Send { to_address, amount });
    let res = execute_execute(
        &mut deps.branch(),
        current_env.clone(),
        info,
        vec![send_msg],
        false,
    );
    let unwrapped_res = match res {
        Ok(res) => res,
        Err(e) => {
            return Err(e);
        }
    };
    println!("{:?}", unwrapped_res);
    assert!(unwrapped_res.messages.len() == 1);
    let submsg = unwrapped_res.messages[0].clone();
    match submsg.msg {
        CosmosMsg::Bank(BankMsg::Send {
            to_address: _,
            amount: _,
        }) => (),
        _ => {
            panic!("We sent a send bankmsg but that's not the first submessage for some reason");
        }
    }
    Ok(unwrapped_res)
}
