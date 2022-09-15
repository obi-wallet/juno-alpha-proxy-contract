use cosmwasm_std::OverflowError;
use cosmwasm_std::StdError;
use thiserror::Error;

use crate::state::PairContract;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("{0}")]
    Overflow(#[from] OverflowError),

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

    #[error("Unable to get current asset price to check spend limit for asset. If this transaction is urgent, use your multisig to sign. Submsg: {0} Error: {1}")]
    PriceCheckFailed(String, String),

    #[error("Please repay your fee debt (USD {0}) before sending funds.")]
    RepayFeesFirst(u128),

    #[error("Spend limit and fee repay unsupported: Unknown home network")]
    UnknownHomeNetwork(String),

    #[error("Multiple spend limits are no longer supported. Remove this wallet and re-add with a USD spend limit.")]
    MultiSpendLimitsNotSupported {},

    #[error("Mismatched pair contract.")]
    MismatchedPairContract {},

    #[error("Pair contract for asset {0} to {1} not found, DUMP: {:3}")]
    PairContractNotFound(String, String, Vec<PairContract>),

    #[error("Semver parsing error: {0}")]
    SemVer(String),
}

impl From<semver::Error> for ContractError {
    fn from(err: semver::Error) -> Self {
        Self::SemVer(err.to_string())
    }
}
