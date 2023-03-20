use crate::utils::error::RippleError;

pub mod bootstrap;
pub mod ripple_contract;

pub type RippleResponse = Result<(), RippleError>;
