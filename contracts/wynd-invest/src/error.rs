use cosmwasm_std::{OverflowError, StdError};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("Invalid R3 Index: {0}")]
    InvalidR3(String),

    #[error("Location not registered during initialization: {0}")]
    UnregisteredLocation(String),

    #[error("Cannot pay with token: {0}")]
    InvalidToken(String),

    #[error("Oracle setting data from the future, unix time: {0}")]
    OracleFromTheFuture(u64),

    #[error("Cannot invest in a location without oracle data")]
    NoDataPresent,

    #[error("Last measurement was more than {days} days ago, cannot use")]
    DataTooOld { days: u64 },

    // TODO: remove when done
    #[error("Unimplemented")]
    Unimplemented,
}

impl From<OverflowError> for ContractError {
    fn from(other: OverflowError) -> ContractError {
        ContractError::Std(other.into())
    }
}
