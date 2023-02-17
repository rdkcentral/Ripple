use serde::{Deserialize, Serialize};

use crate::api::gateway::rpc_error::DenyReason;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum RippleError {
    MissingInput,
    InvalidInput,
    InvalidOutput,
    SenderMissing,
    SendFailure,
    ApiAuthenticationFailed,
    ExtnError,
    BootstrapError,
    ParseError,
    ProcessorError,
    ClientMissing,
    NoResponse,
    Permission(DenyReason),
}
