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

use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use ripple_sdk::{
    api::{
        firebolt::fb_capabilities::{
            CapabilityRole, DenyReason, DenyReasonWithCap, FireboltCap, FireboltPermission,
        },
        manifest::device_manifest::DeviceManifest,
    },
    log::debug,
    parking_lot::RwLock,
};

use crate::state::platform_state::PlatformState;

#[derive(Clone, Debug, Default)]
pub struct GenericCapState {
    supported: Arc<RwLock<HashSet<String>>>,
    // it consumes less memory and operations to store not_available vs available
    not_available: Arc<RwLock<HashSet<String>>>,
}

impl GenericCapState {
    pub fn new(manifest: DeviceManifest) -> GenericCapState {
        let cap_state = GenericCapState::default();
        cap_state.ingest_supported(manifest.get_supported_caps());
        let caps = vec![
            FireboltCap::Short("input:keyboard".to_owned()),
            FireboltCap::Short("token:account".to_owned()),
            FireboltCap::Short("token:platform".to_owned()),
            FireboltCap::Short("usergrant:acknowledgechallenge".to_owned()),
            FireboltCap::Short("usergrant:pinchallenge".to_owned()),
        ];
        cap_state.ingest_availability(caps, false);
        cap_state
    }

    pub fn ingest_supported(&self, request: Vec<FireboltCap>) {
        let mut supported = self.supported.write();
        supported.extend(
            request
                .iter()
                .map(|a| a.as_str())
                .collect::<HashSet<String>>(),
        )
    }

    pub fn ingest_availability(&self, request: Vec<FireboltCap>, is_available: bool) {
        let mut not_available = self.not_available.write();
        for cap in request {
            if is_available {
                not_available.remove(&cap.as_str());
            } else {
                not_available.insert(cap.as_str());
            }
        }
        debug!("Caps that are not available: {:?}", not_available);
    }

    pub fn check_for_processor(&self, request: Vec<String>) -> HashMap<String, bool> {
        let supported = self.supported.read();
        let mut result = HashMap::new();
        for cap in request {
            result.insert(cap.clone(), supported.contains(&cap));
        }
        result
    }

    pub fn check_supported(&self, request: &[FireboltPermission]) -> Result<(), DenyReasonWithCap> {
        let supported = self.supported.read();
        let not_supported: Vec<FireboltCap> = request
            .iter()
            .filter(|fb_perm| !supported.contains(&fb_perm.cap.as_str()))
            .map(|fb_perm| fb_perm.cap.clone())
            .collect();

        // debug!(
        //     "checking supported caps request={:?}, not_supported={:?}, supported: {:?}",
        //     request, not_supported, supported
        // );

        if !not_supported.is_empty() {
            return Err(DenyReasonWithCap::new(
                DenyReason::Unsupported,
                not_supported,
            ));
        }
        Ok(())
    }

    pub fn check_available(
        &self,
        request: &Vec<FireboltPermission>,
    ) -> Result<(), DenyReasonWithCap> {
        let not_available = self.not_available.read();
        let mut result: Vec<FireboltCap> = Vec::new();
        for fb_perm in request {
            if fb_perm.role == CapabilityRole::Use && not_available.contains(&fb_perm.cap.as_str())
            {
                result.push(fb_perm.cap.clone())
            }
        }
        debug!(
            "checking availability of caps request={:?}, not_available={:?}, result: {:?}",
            request, not_available, result
        );
        if !result.is_empty() {
            return Err(DenyReasonWithCap::new(DenyReason::Unavailable, result));
        }
        Ok(())
    }

    pub fn check_all(
        &self,
        permissions: &Vec<FireboltPermission>,
    ) -> Result<(), DenyReasonWithCap> {
        self.check_supported(permissions)?;
        self.check_available(permissions)
    }

    pub fn clear_non_negotiable_permission(
        &self,
        state: &PlatformState,
        permissions: &[FireboltPermission],
    ) -> Vec<FireboltPermission> {
        let filtered_permissions: Vec<FireboltPermission> = permissions
            .iter()
            .filter_map(|permission| {
                if let Some(cap_policy) = state
                    .open_rpc_state
                    .get_capability_policy(permission.cap.as_str())
                {
                    let role_policy = match permission.role {
                        CapabilityRole::Use => &cap_policy.use_role,
                        CapabilityRole::Manage => &cap_policy.manage,
                        CapabilityRole::Provide => &cap_policy.provide,
                    };

                    if let Some(perm_policy) = role_policy {
                        if perm_policy.public && !perm_policy.negotiable {
                            return None;
                        }
                    }
                }
                Some(permission.clone())
            })
            .collect();
        filtered_permissions
    }
}
