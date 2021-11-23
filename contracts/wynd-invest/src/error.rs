use cosmwasm_std::StdError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("Invalid R3 Index: {0}")]
    InvalidR3(String),

    #[error("Cannot pay with token: {0}")]
    InvalidToken(String),

    #[error("Cannot invest in a location without oracle data")]
    NoDataPresent,

    #[error("Last measurement was more than {days} days ago, cannot use")]
    DataTooOld { days: u64 },

    // TODO: remove when done
    #[error("Unimplemented")]
    Unimplemented,
}
