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
use ripple_sdk::log::trace;

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
        let perm_based_on_spec = platform_state
            .open_rpc_state
            .get_perms_for_method(method, api_surface)?;

        if perm_based_on_spec.is_empty() {
            return Some(perm_based_on_spec);
        }
        Some(Self::resolve_dependencies(
            platform_state,
            &perm_based_on_spec,
        ))
    }
    // TODO return Deny Reason into ripple error
    pub async fn gate(
        state: PlatformState,
        request: RpcRequest,
    ) -> Result<Vec<FireboltPermission>, DenyReasonWithCap> {
        let caps =
            Self::get_resolved_caps_for_method(&state, &request.method, request.ctx.gateway_secure)
                .ok_or(DenyReasonWithCap {
                    reason: DenyReason::NotFound,
                    caps: Vec::new(),
                })?;

        if caps.is_empty() {
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
        let filtered_perm_list = state
            .clone()
            .cap_state
            .generic
            .clear_non_negotiable_permission(&state, &caps);
        if filtered_perm_list.is_empty() {
            trace!("Role/Capability is cleared based on non-negotiable policy");
            return Ok(caps);
        }
        // Supported and Availability checks
        trace!(
            "Required caps for method:{} Caps: [{:?}]",
            request.method,
            filtered_perm_list
        );
        if let Err(e) = state
            .clone()
            .cap_state
            .generic
            .check_all(&filtered_perm_list)
        {
            trace!("check_all for caps[{:?}] failed", filtered_perm_list);
            return Err(e);
        }
        // permission checks
        Self::permissions_check(state, request, filtered_perm_list).await?;
        Ok(caps)
    }

    async fn permissions_check(
        state: PlatformState,
        request: RpcRequest,
        filtered_perm_list: Vec<FireboltPermission>,
    ) -> Result<(), DenyReasonWithCap> {
        // permission checks
        let open_rpc_state = state.clone().open_rpc_state;
        // check if the app or method is in permission exclusion list
        if open_rpc_state.is_excluded(request.clone().method, request.clone().ctx.app_id) {
            trace!("Method is exluded from permission check {}", request.method);
        } else if let Err(e) =
            PermissionHandler::check_permitted(&state, &request.ctx.app_id, &filtered_perm_list)
                .await
        {
            trace!(
                "check_permitted for method ({}) failed. Error: {:?}",
                request.method,
                e
            );
            return Err(e);
        }

        trace!("check_permitted for method ({}) succeded", request.method);
        //usergrants check
        if let Err(e) = GrantState::check_with_roles(
            &state,
            &request.ctx.clone().into(),
            &request.ctx.clone().into(),
            &filtered_perm_list,
            true,
            true,
            false,
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

        Ok(())
    }
}
