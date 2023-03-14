use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use ripple_sdk::{
    api::{
        distributor::distributor_permissions::PermissionRequest,
        firebolt::{
            fb_capabilities::{DenyReasonWithCap, FireboltPermission, RoleInfo},
            fb_openrpc::CapabilitySet,
        },
        manifest::device_manifest::DeviceManifest,
    },
    framework::{file_store::FileStore, RippleResponse},
    log::{info},
};

use crate::state::platform_state::PlatformState;

#[derive(Debug, Clone)]
pub struct PermittedState {
    permitted: Arc<RwLock<FileStore<HashMap<String, Vec<FireboltPermission>>>>>,
}

impl PermittedState {
    pub fn new(manifest: DeviceManifest) -> PermittedState {
        let path = get_permissions_path(manifest.configuration.saved_dir);
        let mut map = HashMap::new();
        let store = if let Ok(v) = FileStore::load(path.clone()) {
            map.extend(v.clone().value);
            v
        } else {
            FileStore::new(path.clone(), HashMap::new())
        };

        PermittedState {
            permitted: Arc::new(RwLock::new(store)),
        }
    }

    fn ingest(&mut self, extend_perms: HashMap<String, Vec<FireboltPermission>>) {
        self.permitted.write().unwrap().update(extend_perms.clone());
    }

    fn get_all_permissions(&self) -> HashMap<String, Vec<FireboltPermission>> {
        self.permitted.read().unwrap().clone().value
    }

    pub fn check_cap_role(&self, app_id: &str, role_info: RoleInfo) -> bool {
        if let Some(perms) =  self.get_all_permissions().get(app_id) {
            for perm in perms {
                return perm.cap.as_str() == role_info.capability && perm.role == role_info.role;
            }
        }
    false
    }

    pub fn get_app_permissions(&self, app_id: &str) -> Option<Vec<FireboltPermission>> {
        if let Some(perms) =  self.get_all_permissions().get(app_id) {
            return Some(perms.clone());
        }
        None
    }
}

fn get_permissions_path(saved_dir: String) -> String {
    format!("{}/{}", saved_dir, "app_perms")
}

pub struct PermissionHandler;

impl PermissionHandler {
    pub async fn fetch_and_store(state: PlatformState, app_id: String) -> RippleResponse {
        if let Some(session) = state.session_state.get_distributor_session() {
            if let Ok(extn_response) = state
                .get_client()
                .send_extn_request(PermissionRequest {
                    app_id: app_id.clone(),
                    session,
                })
                .await
            {
                if let Some(permission_response) = extn_response.payload.extract() {
                    let mut map = HashMap::new();
                    map.insert(app_id.clone(), permission_response);
                    let mut permitted_state = state.cap_state.permitted_state.clone();
                    permitted_state.ingest(map);
                    info!("Permissions fetched for {}", app_id);
                }
            }
        }

        Err(ripple_sdk::utils::error::RippleError::InvalidOutput)
    }

    pub async fn check_permitted(
        state: &PlatformState,
        app_id: String,
        request: CapabilitySet,
    ) -> Result<(), DenyReasonWithCap> {
        if let Some(permitted) = state.cap_state.permitted_state.get_app_permissions(&app_id) {
            let permission_set: CapabilitySet = permitted.clone().into();
            return permission_set.check(request);
        } else {
            // check to retrieve it one more time
            if let Ok(_) = Self::fetch_and_store(state.clone(), app_id.clone()).await {
                // cache primed try again
                if let Some(permitted) =
                    state.cap_state.permitted_state.get_app_permissions(&app_id)
                {
                    let permission_set: CapabilitySet = permitted.clone().into();
                    return permission_set.check(request);
                }
            }
        }

        Err(DenyReasonWithCap {
            reason: ripple_sdk::api::firebolt::fb_capabilities::DenyReason::Unpermitted,
            caps: request.get_caps(),
        })
    }
}
