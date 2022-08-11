use cosmwasm_std::StdError;
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("Caller is not admin.")]
    Unauthorized {},

    #[error("Spend-limited cw20 transactions cannot have additional funds attached.")]
    AttachedFundsNotAllowed {},

    #[error("Spend-limited WasmMsg txes must be cw20 Send or Transfer messages.")]
    OnlyTransferSendAllowed {},

    #[error("Message deserialization error.  Spend-limited WasmMsg txes are limited to a Cw20ExecuteMsg Send or Transfer.")]
    ErrorDeserializingCw20Message {},

    #[error("WASM message is not Execute. Spend-limited WasmMsg txes are limited to a Cw20ExecuteMsg Send or Transfer.")]
    WasmMsgMustBeExecute {},

    #[error(
        "This address is not permitted to spend this token, or to spend this many of this token."
    )]
    SpendNotAuthorized {},

    #[error(
        "Spend-limited transactions must be BankMsg or WasmMsg (Cw20ExecuteMsg Send or Transfer)."
    )]
    BadMessageType {},

    #[error("This address is already authorized as a Hot Wallet. Remove it first in order to update it.")]
    HotWalletExists {},

    #[error("This address is not authorized as a spend limit Hot Wallet.")]
    HotWalletDoesNotExist {},

    #[error("Failed to advance the reset day.")]
    DayUpdateError {},

    #[error("Failed to advance the reset month.")]
    MonthUpdateError {},

    #[error("Hot wallet does not have a spend limit for asset {0}.")]
    CannotSpendThisAsset(String),

    #[error("You cannot spend more than your available spend limit.")]
    CannotSpendMoreThanLimit {},

    #[error("Uninitialized message.")]
    UninitializedMessage {},

    #[error("Caller is not pending new admin. Propose new admin first.")]
    CallerIsNotPendingNewAdmin {},
}
