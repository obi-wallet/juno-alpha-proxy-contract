use cosmwasm_std::testing::{mock_env, mock_info, MockApi, MockQuerier};
use cosmwasm_std::{
    Attribute, BankMsg, Coin, CosmosMsg, DepsMut, Empty, Env, MemoryStorage, MessageInfo,
    OwnedDeps, Response,
};

use crate::contract::{execute, execute_execute, instantiate};
use crate::error::ContractError;
use crate::hot_wallet::{CoinLimit, HotWallet};
use crate::msg::{ExecuteMsg, InstantiateMsg};
use crate::{contract::query_hot_wallets, hot_wallet::PeriodType};

use crate::tests_contract::{ADMIN, HOT_WALLET};

pub fn instantiate_contract(
    deps: &mut OwnedDeps<MemoryStorage, MockApi, MockQuerier<Empty>, Empty>,
    env: Env,
    starting_debt: Coin,
) {
    // instantiate the contract
    let instantiate_msg = InstantiateMsg {
        admin: ADMIN.to_string(),
        hot_wallets: vec![HotWallet {
            address: HOT_WALLET.to_string(),
            current_period_reset: env.block.time.seconds() as u64, // this is fine since it will calc on first spend
            period_type: PeriodType::DAYS,
            period_multiple: 1,
            spend_limits: vec![CoinLimit {
                denom: "testtokens".to_string(),
                amount: 1_000_000u64,
                limit_remaining: 1_000_000u64,
            }],
            usdc_denom: Some("true".to_string()),
        }],
        uusd_fee_debt: starting_debt.amount,
        fee_lend_repay_wallet: "test_repay_address".to_string(),
        home_network: "local".to_string(),
        signers: [
            "testsigner1".to_string(),
            "testsigner2".to_string(),
            "testsigner3".to_string(),
        ]
        .to_vec(),
    };
    let info = mock_info(ADMIN, &[]);
    let res = instantiate(deps.as_mut(), mock_env(), info, instantiate_msg).unwrap();
    println!("events: {:?}", res.events);
    assert_eq!(res.events.len(), 1);
    assert_eq!(
        res.events[0].attributes[1],
        Attribute::new("signer".to_string(), "testsigner2".to_string())
    );
}

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

    let _res = execute(deps.branch(), current_env, info, execute_msg).unwrap();
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
    let res = execute_execute(&mut deps.branch(), current_env, info, vec![send_msg], false);
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
