use crate::utils::error::RippleError;

pub mod bootstrap;
pub mod file_store;
pub type RippleResponse = Result<(), RippleError>;
