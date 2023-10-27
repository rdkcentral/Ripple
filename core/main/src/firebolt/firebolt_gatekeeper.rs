// Copyright 2023 Comcast Cable Communications Management, LLC
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
//
// SPDX-License-Identifier: Apache-2.0
//

use std::collections::HashMap;

use ripple_sdk::api::firebolt::fb_capabilities::{
    DenyReason, DenyReasonWithCap, FireboltPermission,
};
use ripple_sdk::api::gateway::rpc_gateway_api::RpcRequest;
use ripple_sdk::log::{debug, trace};

use crate::service::user_grants::GrantState;
use crate::state::openrpc_state::ApiSurface;
use crate::state::{cap::permitted_state::PermissionHandler, platform_state::PlatformState};

pub struct FireboltGatekeeper {}

impl FireboltGatekeeper {
    pub fn resolve_dependencies(
        platform_state: &PlatformState,
        perm_set: &Vec<FireboltPermission>,
    ) -> Vec<FireboltPermission> {
        let mut resolved_perm_set: Vec<FireboltPermission> = Default::default();
        let cap_dependencies: &HashMap<FireboltPermission, Vec<FireboltPermission>> =
            &platform_state
                .get_device_manifest()
                .capabilities
                .dependencies;
        for perm in perm_set {
            if let Some(dep_perm) = cap_dependencies.get(perm) {
                resolved_perm_set.append(&mut dep_perm.clone());
            } else {
                resolved_perm_set.push(perm.clone());
            }
        }
        resolved_perm_set
    }
    fn get_resolved_caps_for_method(
        platform_state: &PlatformState,
        method: &str,
        secure: bool,
    ) -> Option<Vec<FireboltPermission>> {
        trace!(
            "get_resolved_caps_for_method called with params: method {}, secure: {}",
            method,
            secure,
        );
        let mut api_surface = vec![ApiSurface::Firebolt];
        if !secure {
            api_surface.push(ApiSurface::Ripple);
        }
        let perm_based_on_spec_opt = platform_state
            .open_rpc_state
            .get_perms_for_method(method, api_surface);
        perm_based_on_spec_opt.as_ref()?;
        let perm_based_on_spec = perm_based_on_spec_opt.unwrap();
        if perm_based_on_spec.is_empty() {
            return Some(perm_based_on_spec);
        }
        Some(Self::resolve_dependencies(
            platform_state,
            &perm_based_on_spec,
        ))
    }
    // TODO return Deny Reason into ripple error
    pub async fn gate(state: PlatformState, request: RpcRequest) -> Result<(), DenyReasonWithCap> {
        let open_rpc_state = state.clone().open_rpc_state;
        if open_rpc_state.is_excluded(request.clone().method, request.clone().ctx.app_id) {
            trace!("Method is exluded from gating {}", request.method);
            return Ok(());
        }
        // if let Some(caps) = open_rpc_state.get_caps_for_method(&request.method) {
        let caps_opt =
            Self::get_resolved_caps_for_method(&state, &request.method, request.ctx.gateway_secure);
        if caps_opt.is_none() {
            return Err(DenyReasonWithCap {
                reason: DenyReason::NotFound,
                caps: Vec::new(),
            });
        }
        let caps = caps_opt.unwrap();
        if !state
            .clone()
            .cap_state
            .generic
            .clear_non_negotiable_permission(&state, &caps)
        {
            if !caps.is_empty() {
                // Supported and Availability checks
                trace!(
                    "Required caps for method:{} Caps: [{:?}]",
                    request.method,
                    caps
                );
                if let Err(e) = state.clone().cap_state.generic.check_all(&caps) {
                    trace!("check_all for caps[{:?}] failed", caps);
                    return Err(e);
                }
                // permission checks
                if let Err(e) =
                    PermissionHandler::check_permitted(&state, &request.ctx.app_id, &caps).await
                {
                    trace!(
                        "check_permitted for method ({}) failed. Error: {:?}",
                        request.method,
                        e
                    );
                    return Err(e);
                } else {
                    trace!("check_permitted for method ({}) succeded", request.method);
                    //usergrants check
                    if let Err(e) = GrantState::check_with_roles(
                        &state,
                        &request.ctx.clone().into(),
                        &request.ctx.clone().into(),
                        &caps,
                        true,
                    )
                    .await
                    {
                        trace!(
                            "check_with_roles for method ({}) failed. Error: {:?}",
                            request.method,
                            e
                        );
                        return Err(e);
                    } else {
                        trace!("check_with_roles for method ({}) succeded", request.method);
                    }
                }
            } else {
                // Couldnt find any capabilities for the method
                trace!(
                    "Unable to find any caps for the method ({})",
                    request.method
                );
                return Err(DenyReasonWithCap {
                    reason: DenyReason::Unsupported,
                    caps: Vec::new(),
                });
            }
        } else {
            debug!("Role/Capability is cleared based on non-negotiable policy");
        }
        Ok(())
    }
}
