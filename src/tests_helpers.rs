use cosmwasm_std::{BankMsg, Coin, CosmosMsg, DepsMut, Env, MessageInfo, Response};

use crate::error::ContractError;
use crate::msg::{ExecuteMsg, InstantiateMsg};
use crate::permissioned_address::{CoinLimit, PeriodType, PermissionedAddressParams};
use crate::state::ObiProxyContract;

use crate::tests_contract::{OWNER, PERMISSIONED_ADDRESS};

pub fn get_test_instantiate_message(
    env: Env,
    starting_debt: Coin,
    obi_is_signer: bool,
) -> InstantiateMsg {
    let signer2: String = if obi_is_signer {
        "juno17w77rnps59cnallfskg42s3ntnlhrzu2mjkr3e".to_string()
    } else {
        "signer2".to_string()
    };
    // instantiate the contract

    InstantiateMsg {
        owner: OWNER.to_string(),
        permissioned_addresses: vec![PermissionedAddressParams {
            address: PERMISSIONED_ADDRESS.to_string(),
            current_period_reset: env.block.time.seconds() as u64, // this is fine since it will calc on first spend
            period_type: PeriodType::DAYS,
            period_multiple: 1,
            spend_limits: vec![CoinLimit {
                denom: "ibc/EAC38D55372F38F1AFD68DF7FE9EF762DCF69F26520643CF3F9D292A738D8034"
                    .to_string(),
                amount: 1_000_000u64,
                limit_remaining: 1_000_000u64,
            }],
            usdc_denom: Some("true".to_string()),
            default: Some(true),
            authorizations: None,
        }],
        uusd_fee_debt: starting_debt.amount,
        fee_lend_repay_wallet: "test_repay_address".to_string(),
        home_network: "local".to_string(),
        signers: [
            "testsigner1".to_string(),
            signer2,
            "testsigner3".to_string(),
        ]
        .to_vec(),
        update_delay_hours: if obi_is_signer { Some(24u16) } else { None },
        signer_types: vec![
            "type1".to_string(),
            "type2".to_string(),
            "type3".to_string(),
        ],
    }
}

#[allow(clippy::too_many_arguments)]
pub fn add_test_permissioned_address(
    mut deps: DepsMut,
    obi: &mut ObiProxyContract,
    address: String,
    current_env: Env,
    info: MessageInfo,
    period_multiple: u16,
    period_type: PeriodType,
    limit: u64,
) -> Result<Response, ContractError> {
    let res = obi.query_permissioned_addresses(deps.as_ref()).unwrap();
    let old_length = res.permissioned_addresses.len();
    let execute_msg = ExecuteMsg::AddPermissionedAddress {
        new_permissioned_address: PermissionedAddressParams {
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
            default: Some(true),
            authorizations: None,
        },
    };

    let _res = obi
        .execute(deps.branch(), current_env, info, execute_msg)
        .unwrap();
    let res = obi.query_permissioned_addresses(deps.as_ref()).unwrap();
    assert!(res.permissioned_addresses.len() == old_length + 1);
    Ok(Response::new())
}

pub fn test_spend_bank(
    deps: DepsMut,
    obi: &mut ObiProxyContract,
    current_env: Env,
    to_address: String,
    amount: Vec<Coin>,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    let send_msg = CosmosMsg::Bank(BankMsg::Send { to_address, amount });
    let res = obi.execute_execute(deps, current_env, info, vec![send_msg]);
    let unwrapped_res = match res {
        Ok(res) => res,
        Err(e) => {
            return Err(e);
        }
    };
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
