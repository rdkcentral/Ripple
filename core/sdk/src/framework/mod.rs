use crate::utils::error::RippleError;

pub mod bootstrap;

pub type RippleResponse = Result<(), RippleError>;
