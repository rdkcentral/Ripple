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
    path::Path,
    sync::{Arc, RwLock},
};

use ripple_sdk::{
    api::{
        config::FEATURE_CLOUD_PERMISSIONS,
        device::device_apps::AppsRequest,
        //config::Config,
        distributor::distributor_permissions::{PermissionRequest, PermissionResponse},
        firebolt::{
            fb_capabilities::{
                DenyReason, DenyReasonWithCap, FireboltCap, FireboltPermission, RoleInfo,
            },
            fb_openrpc::CapabilitySet,
        },
        manifest::device_manifest::DeviceManifest,
    },
    extn::extn_client_message::{ExtnPayload, ExtnResponse},
    framework::{file_store::FileStore, RippleResponse},
    log::{debug, error, info},
    tokio,
    utils::error::RippleError,
};

use crate::state::platform_state::PlatformState;

type FireboltPermissionStore = Arc<RwLock<FileStore<HashMap<String, Vec<FireboltPermission>>>>>;

#[derive(Debug, Clone)]
pub struct PermittedState {
    permitted: FireboltPermissionStore,
}

impl PermittedState {
    pub fn new(manifest: DeviceManifest) -> PermittedState {
        let path = get_permissions_path(manifest.configuration.saved_dir);
        let store = if let Ok(v) = FileStore::load(path.clone()) {
            v
        } else {
            FileStore::new(path, HashMap::new())
        };

        PermittedState {
            permitted: Arc::new(RwLock::new(store)),
        }
    }

    fn ingest(&mut self, extend_perms: HashMap<String, Vec<FireboltPermission>>) {
        let mut perms = self.permitted.write().unwrap();
        perms.value.extend(extend_perms);
        perms.sync();
    }

    #[cfg(test)]
    pub fn set_permissions(&mut self, permissions: HashMap<String, Vec<FireboltPermission>>) {
        let mut perms = self.permitted.write().unwrap();
        perms.value = permissions;
        perms.sync();
    }

    fn get_all_permissions(&self) -> HashMap<String, Vec<FireboltPermission>> {
        self.permitted.read().unwrap().value.clone()
    }
    fn has_cached_permissions(&self, app_id: &String) -> bool {
        // check if the app has permissions cached
        self.permitted.read().unwrap().value.contains_key(app_id)
    }
    pub fn check_cap_role(&self, app_id: &str, role_info: &RoleInfo) -> Result<bool, RippleError> {
        let role = role_info
            .role
            .unwrap_or(ripple_sdk::api::firebolt::fb_capabilities::CapabilityRole::Use);
        if let Some(perms) = self.get_all_permissions().get(app_id) {
            for perm in perms {
                if perm.cap.as_str() == role_info.capability.as_str() && perm.role == role {
                    return Ok(true);
                }
            }
            Ok(false)
        } else {
            // Not cached prior
            Err(RippleError::InvalidAccess)
        }
    }

    pub fn check_multiple(&self, app_id: &str, request: Vec<RoleInfo>) -> HashMap<String, bool> {
        let mut map = HashMap::new();
        for role_info in request {
            map.insert(
                role_info.capability.as_str(),
                if let Ok(v) = self.check_cap_role(app_id, &role_info) {
                    v
                } else {
                    false
                },
            );
        }
        map
    }

    pub fn get_app_permissions(&self, app_id: &str) -> Option<Vec<FireboltPermission>> {
        if let Some(perms) = self.get_all_permissions().get(app_id) {
            return Some(perms.clone());
        }
        None
    }
}

fn get_permissions_path(saved_dir: String) -> String {
    let dir_path = Path::new(&saved_dir).join("app_perms");
    dir_path.into_os_string().into_string().unwrap()
}

pub struct PermissionHandler;

impl PermissionHandler {
    fn get_distributor_alias_for_app_id(ps: &PlatformState, app_id: &str) -> String {
        let dist_app_aliases = ps
            .get_device_manifest()
            .applications
            .distributor_app_aliases;
        if let Some(app_id_alias) = dist_app_aliases.get(&app_id.to_string()) {
            app_id_alias.to_string()
        } else {
            app_id.to_string()
        }
    }

    pub async fn fetch_and_store(
        state: &PlatformState,
        app_id: &str,
        allow_cached: bool,
    ) -> RippleResponse {
        if state.open_rpc_state.is_app_excluded(app_id) {
            return Ok(());
        }

        if state
            .get_client()
            .get_extn_client()
            .get_features()
            .contains(&String::from(FEATURE_CLOUD_PERMISSIONS))
        {
            if allow_cached {
                if let Some(permissions) =
                    state.cap_state.permitted_state.get_app_permissions(app_id)
                {
                    let mut permissions_copy = permissions;
                    return Self::process_permissions(state, app_id, &mut permissions_copy);
                }
            }
            Self::cloud_fetch_and_store(state, app_id).await
        } else {
            // Never use cache, always fetch from device.
            Self::device_fetch_and_store(state, app_id).await
        }
    }

    pub async fn cloud_fetch_and_store(state: &PlatformState, app_id: &str) -> RippleResponse {
        // This function will always get the permissions from server and update the local cache
        let app_id_alias = Self::get_distributor_alias_for_app_id(state, app_id);
        if let Some(session) = state.session_state.get_account_session() {
            match state
                .get_client()
                .send_extn_request(PermissionRequest {
                    app_id: app_id_alias,
                    session,
                    payload: None,
                })
                .await
                .ok()
            {
                Some(extn_response) => {
                    if let Some(permission_response) =
                        extn_response.payload.extract::<PermissionResponse>()
                    {
                        let mut permission_response_copy = permission_response;
                        return Self::process_permissions(
                            state,
                            app_id,
                            &mut permission_response_copy,
                        );
                    }
                    Err(RippleError::InvalidOutput)
                }
                None => Err(RippleError::InvalidOutput),
            }
        } else {
            Err(RippleError::InvalidOutput)
        }
    }

    pub async fn device_fetch_and_store(state: &PlatformState, app_id: &str) -> RippleResponse {
        let mut client = state.get_client().get_extn_client();
        let resp = client
            .request(AppsRequest::GetFireboltPermissions(app_id.to_string()))
            .await?;

        let mut permissions = match resp.payload {
            ExtnPayload::Response(response) => match response {
                ExtnResponse::Permission(perms) => perms,
                ExtnResponse::Error(e) => {
                    error!("device_fetch_and_store: e={:?}", e);
                    return Err(e);
                }
                _ => {
                    error!("device_fetch_and_store: Unexpected response");
                    return Err(RippleError::ExtnError);
                }
            },
            _ => {
                error!("device_fetch_and_store: Unexpected payload");
                return Err(RippleError::ExtnError);
            }
        };

        Self::process_permissions(state, app_id, &mut permissions)
    }

    fn process_permissions(
        state: &PlatformState,
        app_id: &str,
        permissions: &mut Vec<FireboltPermission>,
    ) -> RippleResponse {
        info!("Permissions fetched for {}", app_id);
        let dep_lookup = &state.get_device_manifest().capabilities.dependencies;
        let mut deps = HashSet::new();

        // Create a HashSet to track unique permissions
        let mut unique_permissions = HashSet::new();

        for p in permissions.iter() {
            if let Some(dependent_list) = dep_lookup.get(p) {
                deps.extend(dependent_list.iter().cloned());
            }
        }

        // Filter out duplicates and insert only unique permissions
        for permission in deps {
            if !unique_permissions.contains(&permission) {
                permissions.push(permission.clone()); // Clone the permission to avoid the moved value error
                unique_permissions.insert(permission);
            }
        }

        let map = vec![(app_id.to_owned(), permissions.clone())]
            .into_iter()
            .collect::<HashMap<_, _>>();

        let mut permitted_state = state.cap_state.permitted_state.clone();
        permitted_state.ingest(map.clone());
        info!("Permissions: {:?}", map);

        Ok(())
    }

    pub fn get_permitted_info(
        state: &PlatformState,
        app_id: &str,
        request: CapabilitySet,
    ) -> Result<(), DenyReasonWithCap> {
        if let Some(permitted) = state.cap_state.permitted_state.get_app_permissions(app_id) {
            let permission_set: CapabilitySet = permitted.into();
            permission_set.check(request)
        } else {
            Err(DenyReasonWithCap {
                reason: ripple_sdk::api::firebolt::fb_capabilities::DenyReason::Unpermitted,
                caps: request.get_caps(),
            })
        }
    }
    pub fn is_all_permitted(
        permitted: &[FireboltPermission],
        request: &[FireboltPermission],
    ) -> Result<(), DenyReasonWithCap> {
        let mut unpermitted_caps: Vec<FireboltCap> = Vec::new();
        let _all_permitted = request.iter().all(|perm| {
            let present = permitted.contains(perm);
            if !present {
                unpermitted_caps.push(perm.cap.clone());
            }
            present
        });
        if unpermitted_caps.is_empty() {
            Ok(())
        } else {
            Err(DenyReasonWithCap {
                reason: DenyReason::Unpermitted,
                caps: unpermitted_caps,
            })
        }
    }

    pub async fn get_cached_app_permissions(
        state: &PlatformState,
        app_id: &str,
    ) -> Vec<FireboltPermission> {
        // This would always return cached permissions. Empty vector if no permissions are cached
        state
            .cap_state
            .permitted_state
            .get_app_permissions(app_id)
            .map_or(Vec::new(), |v| v)
    }

    pub async fn fetch_permission_for_app_session(
        state: &PlatformState,
        app_id: &String,
    ) -> Result<(), RippleError> {
        // This call should hit the server and fetch permissions for the app.
        // Local cache will be updated with the fetched permissions
        let has_stored = state
            .cap_state
            .permitted_state
            .has_cached_permissions(app_id);
        let ps_c = state.clone();
        let app_id_c = app_id.clone();
        let handle = tokio::spawn(async move {
            let perm_res = Self::fetch_and_store(&ps_c, &app_id_c, false).await;
            if perm_res.is_err() {
                if has_stored {
                    error!(
                        "Failed to get permissions for {}, possibly using stale permissions",
                        app_id_c
                    )
                } else {
                    error!("Failed to get permissions for {} and no previous permissions store, app may not be able to access capabilities", app_id_c)
                }
            }
            perm_res
        });
        let result: Result<(), RippleError> = if !has_stored {
            // app has no stored permissions, wait until it does
            debug!(
                "{} did not have any permissions, waiting until permissions are fetched from cloud",
                app_id
            );
            match handle.await {
                Ok(handle_result) => handle_result,
                Err(e) => {
                    error!("fetch_permission_for_app_session: e={:?}", e);
                    Err(RippleError::NotAvailable)
                }
            }
        } else {
            Ok(())
        };
        result
    }

    pub async fn check_permitted(
        state: &PlatformState,
        app_id: &str,
        request: &[FireboltPermission],
    ) -> Result<(), DenyReasonWithCap> {
        if let Some(permitted) = state.cap_state.permitted_state.get_app_permissions(app_id) {
            // return request.has_permissions(&permitted);
            return Self::is_all_permitted(&permitted, request);
        } else {
            // check to retrieve it one more time
            if (Self::fetch_and_store(state, app_id, true).await).is_ok() {
                // cache primed try again
                if let Some(permitted) = state.cap_state.permitted_state.get_app_permissions(app_id)
                {
                    return Self::is_all_permitted(&permitted, request);
                }
            }
        }

        Err(DenyReasonWithCap {
            reason: ripple_sdk::api::firebolt::fb_capabilities::DenyReason::Unpermitted,
            caps: request
                .iter()
                .map(|fb_perm| fb_perm.cap.to_owned())
                .collect(),
        })
    }

    pub async fn check_all_permitted(
        platform_state: &PlatformState,
        app_id: &str,
        capability: &str,
    ) -> (bool, bool, bool) {
        let mut use_granted = false;
        let mut manage_granted = false;
        let mut provide_granted = false;
        let granted_permissions = Self::get_cached_app_permissions(platform_state, app_id).await;
        for perm in granted_permissions {
            if perm.cap.as_str() == capability {
                match perm.role {
                    ripple_sdk::api::firebolt::fb_capabilities::CapabilityRole::Use => {
                        use_granted = true
                    }
                    ripple_sdk::api::firebolt::fb_capabilities::CapabilityRole::Manage => {
                        manage_granted = true
                    }
                    ripple_sdk::api::firebolt::fb_capabilities::CapabilityRole::Provide => {
                        provide_granted = true
                    }
                }
            }
        }
        (use_granted, manage_granted, provide_granted)
    }
}
