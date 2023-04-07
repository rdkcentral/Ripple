// If not stated otherwise in this file or this component's license file the
// following copyright and licenses apply:
//
// Copyright 2023 RDK Management
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
use ripple_sdk::api::firebolt::fb_capabilities::{DenyReason, DenyReasonWithCap};
use ripple_sdk::api::gateway::rpc_gateway_api::RpcRequest;

use crate::state::{
    cap::{grant_state::GrantState, permitted_state::PermissionHandler},
    platform_state::PlatformState,
};

pub struct FireboltGatekeeper {}

impl FireboltGatekeeper {
    // TODO return Deny Reason into ripple error
    pub async fn gate(state: PlatformState, request: RpcRequest) -> Result<(), DenyReasonWithCap> {
        let open_rpc_state = state.clone().open_rpc_state;
        if open_rpc_state.is_excluded(request.clone().method) {
            return Ok(());
        }
        if let Some(caps) = open_rpc_state.get_caps_for_method(request.clone().method) {
            // Supported and Availability checks
            if let Err(e) = state
                .clone()
                .cap_state
                .generic
                .check_all(&caps.clone().get_caps())
            {
                return Err(e);
            }
            // permission checks
            if let Err(e) =
                PermissionHandler::check_permitted(&state, &request.ctx.app_id, caps.clone()).await
            {
                return Err(e);
            }

            // grant checks
            if let Err(e) = GrantState::check_with_roles(&state, &request.ctx, caps.clone()).await {
                return Err(e);
            }
        } else {
            // Couldnt find any capabilities for the method
            return Err(DenyReasonWithCap {
                reason: DenyReason::Unsupported,
                caps: Vec::new(),
            });
        }
        Ok(())
    }
}
