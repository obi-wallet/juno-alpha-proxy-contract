use cosmwasm_std::{from_binary, BankMsg, Binary, Coin, CosmosMsg, StdError, WasmMsg};
use cw20::Cw20ExecuteMsg;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct PendingSubmsg {
    pub msg: CosmosMsg,
    pub contract_addr: Option<String>,
    pub binarymsg: Option<Binary>,
    pub funds: Vec<Coin>,
    pub ty: SubmsgType,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct PendingSubmsgGroup {
    msgs: Vec<PendingSubmsg>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub enum SubmsgType {
    BankSend,
    BankBurn,
    ExecuteWasm(WasmmsgType),
    Unknown,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub enum WasmmsgType {
    Cw20Transfer,
    Cw20Burn,
    Cw20Send,
    Cw20IncreaseAllowance,
    Cw20DecreaseAllowance,
    Cw20TransferFrom,
    Cw20SendFrom,
    Cw20BurnFrom,
    Cw20Mint,
    Cw20UpdateMarketing,
    Cw20UploadLogo,
}

impl PendingSubmsg {
    pub fn add_funds(&mut self, funds: Vec<Coin>) {
        for fund in funds {
            self.funds.push(fund);
        }
    }

    pub fn process_and_get_msg_type(&mut self) -> SubmsgType {
        match &self.msg {
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr,
                msg,
                funds,
            }) => {
                self.contract_addr = Some(contract_addr.to_string());
                self.binarymsg = Some(msg.clone());
                self.funds = funds.clone();
                // note that parent message may have more funds attached
                self.ty = self.process_execute_type();
                self.ty.clone()
            }
            CosmosMsg::Bank(_) => {
                self.contract_addr = None;
                self.binarymsg = None;
                self.funds = vec![];
                // note that parent message may have more funds attached
                self.ty = self.process_bank_type();
                self.ty.clone()
            }
            _ => SubmsgType::Unknown,
        }
    }

    pub fn process_execute_type(&mut self) -> SubmsgType {
        let msg_de: Result<cw20::Cw20ExecuteMsg, StdError> = match &self.binarymsg {
            None => Err(StdError::GenericErr {
                msg: "Message does not exist as struct member".to_string(),
            }),
            Some(msg) => from_binary(msg),
        };
        match msg_de {
            Ok(msg_contents) => {
                // must be Transfer or Send if permissioned address
                match msg_contents {
                    Cw20ExecuteMsg::Transfer {
                        recipient: _,
                        amount,
                    } => {
                        if let Some(denom) = self.contract_addr.clone() {
                            // maybe this needs better handling
                            self.funds.push(Coin { amount, denom });
                        }
                        SubmsgType::ExecuteWasm(WasmmsgType::Cw20Transfer)
                    }
                    Cw20ExecuteMsg::Burn { amount } => {
                        if let Some(denom) = self.contract_addr.clone() {
                            // maybe this needs better handling
                            self.funds.push(Coin { amount, denom });
                        }
                        SubmsgType::ExecuteWasm(WasmmsgType::Cw20Burn)
                    }
                    Cw20ExecuteMsg::Send {
                        contract: _,
                        amount,
                        msg: _,
                    } => {
                        if let Some(denom) = self.contract_addr.clone() {
                            // maybe this needs better handling
                            self.funds.push(Coin { amount, denom });
                        }
                        SubmsgType::ExecuteWasm(WasmmsgType::Cw20Send)
                    }
                    Cw20ExecuteMsg::IncreaseAllowance {
                        spender: _,
                        amount,
                        expires: _,
                    } => {
                        if let Some(denom) = self.contract_addr.clone() {
                            // maybe this needs better handling
                            self.funds.push(Coin { amount, denom });
                        }
                        SubmsgType::ExecuteWasm(WasmmsgType::Cw20IncreaseAllowance)
                    }
                    Cw20ExecuteMsg::DecreaseAllowance {
                        spender: _,
                        amount: _,
                        expires: _,
                    } => SubmsgType::ExecuteWasm(WasmmsgType::Cw20DecreaseAllowance),
                    Cw20ExecuteMsg::TransferFrom {
                        owner: _,
                        recipient: _,
                        amount: _,
                    } => SubmsgType::ExecuteWasm(WasmmsgType::Cw20TransferFrom),
                    Cw20ExecuteMsg::SendFrom {
                        owner: _,
                        contract: _,
                        amount: _,
                        msg: _,
                    } => SubmsgType::ExecuteWasm(WasmmsgType::Cw20SendFrom),
                    Cw20ExecuteMsg::BurnFrom {
                        owner: _,
                        amount: _,
                    } => SubmsgType::ExecuteWasm(WasmmsgType::Cw20BurnFrom),
                    Cw20ExecuteMsg::Mint {
                        recipient: _,
                        amount: _,
                    } => SubmsgType::ExecuteWasm(WasmmsgType::Cw20Mint),
                    Cw20ExecuteMsg::UpdateMarketing {
                        project: _,
                        description: _,
                        marketing: _,
                    } => SubmsgType::ExecuteWasm(WasmmsgType::Cw20UpdateMarketing),
                    Cw20ExecuteMsg::UploadLogo(_) => {
                        SubmsgType::ExecuteWasm(WasmmsgType::Cw20UploadLogo)
                    }
                }
            }
            Err(_) => SubmsgType::Unknown,
        }
    }

    pub fn process_bank_type(&mut self) -> SubmsgType {
        match self.msg.clone() {
            CosmosMsg::Bank(BankMsg::Send {
                to_address: _,
                amount,
            }) => {
                for coin in amount {
                    self.funds.push(coin);
                }
                SubmsgType::BankSend
            }
            CosmosMsg::Bank(BankMsg::Burn { amount }) => {
                for coin in amount {
                    self.funds.push(coin);
                }
                SubmsgType::BankBurn
            }
            _ => SubmsgType::Unknown,
        }
    }
}
