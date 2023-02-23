use ripple_sdk::{api::gateway::rpc_gateway_api::RpcRequest, utils::error::RippleError};

use crate::state::cap::cap_state::CapState;

pub struct FireboltGatekeeper {}

impl FireboltGatekeeper {
    // TODO return Deny Reason into ripple error
    pub async fn gate(_cap_state: CapState, _request: RpcRequest) -> Result<(), RippleError> {
        Ok(())
    }
}
