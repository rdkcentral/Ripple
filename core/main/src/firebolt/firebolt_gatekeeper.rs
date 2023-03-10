use ripple_sdk::{api::gateway::rpc_gateway_api::RpcRequest, utils::error::RippleError};

use crate::state::{cap::permitted_state::PermissionHandler, platform_state::PlatformState};

pub struct FireboltGatekeeper {}

impl FireboltGatekeeper {
    // TODO return Deny Reason into ripple error
    pub async fn gate(state: PlatformState, request: RpcRequest) -> Result<(), RippleError> {
        if let Some(caps) = state
            .clone()
            .open_rpc_state
            .get_caps_for_method(request.clone().method)
        {
            // Supported and Availability checks
            if let Err(e) = state.clone().cap_state.check_all(caps.clone()) {
                return Err(RippleError::Permission(e.reason));
            } else {
                // permission checks
                if let Err(e) = PermissionHandler::check_permitted(
                    state.clone(),
                    request.clone().ctx.app_id,
                    caps.clone(),
                )
                .await
                {
                    return Err(RippleError::Permission(e.reason));
                }
            }
        }
        Ok(())
    }
}
