use ripple_sdk::api::{
    firebolt::fb_capabilities::DenyReasonWithCap, gateway::rpc_gateway_api::RpcRequest,
};

use crate::state::{cap::permitted_state::PermissionHandler, platform_state::PlatformState};

pub struct FireboltGatekeeper {}

impl FireboltGatekeeper {
    // TODO return Deny Reason into ripple error
    pub async fn gate(state: PlatformState, request: RpcRequest) -> Result<(), DenyReasonWithCap> {
        if let Some(caps) = state
            .clone()
            .open_rpc_state
            .get_caps_for_method(request.clone().method)
        {
            // Supported and Availability checks
            if let Err(e) = state
                .clone()
                .cap_state
                .generic
                .check_all(&caps.clone().get_caps())
            {
                return Err(e);
            } else {
                // permission checks
                if let Err(e) = PermissionHandler::check_permitted(
                    &state,
                    request.clone().ctx.app_id,
                    caps.clone(),
                )
                .await
                {
                    return Err(e);
                }
            }
        }
        Ok(())
    }
}
