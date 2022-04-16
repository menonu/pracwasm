use cosmwasm_std::{StdError, Uint128};
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("Invalid State")]
    InvalidState {},

    #[error("Bet amount must be >0")]
    BetAmountZero {},

    #[error("Balance is too low")]
    ShortBalance { balance: Uint128 },

    #[error("Custom Error val: {val:?}")]
    CustomError { val: String },

    #[error("Action before bet is not allowed")]
    ActionBeforeBetError {},

    #[error("Wrong double down amount")]
    WrongDoublDownAmount { amount: Uint128 },

    #[error("DoubleDown is not allowed")]
    DoubleDownNotAllowed,
    // Add any other custom errors you like here.
    // Look at https://docs.rs/thiserror/1.0.21/thiserror/ for details.
}
