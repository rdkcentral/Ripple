use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use ripple_sdk::{
    api::{
        distributor::distributor_permissions::PermissionRequest,
        firebolt::{
            fb_capabilities::{DenyReasonWithCap, FireboltPermission, CapabilityRole, RoleInfo},
            fb_openrpc::CapabilitySet,
        },
        manifest::device_manifest::DeviceManifest,
    },
    framework::{file_store::FileStore, RippleResponse},
    log::error,
};

use crate::state::platform_state::PlatformState;

#[derive(Debug, Clone)]
pub struct PermittedState {
    permitted: Arc<RwLock<HashMap<String, Vec<FireboltPermission>>>>,
    store: FileStore<HashMap<String, Vec<FireboltPermission>>>,
}

impl PermittedState {
    pub fn new(manifest: DeviceManifest) -> PermittedState {
        let path = get_permissions_path(manifest.configuration.saved_dir);
        let store = if let Ok(v) = FileStore::load(path.clone()) {
            v
        } else {
            FileStore::new(path, HashMap::new()).expect("Needs permission for state")
        };

        PermittedState {
            store,
            permitted: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    fn ingest(&mut self, extend_perms: HashMap<String, Vec<FireboltPermission>>) {
        {
            self.permitted.write().unwrap().extend(extend_perms.clone());
        }
        let new_value = self.get_all_permissions();
        if let Err(e) = self.store.update(new_value) {
            error!("Error during update to local file store {:?}", e);
        }
    }

    fn get_all_permissions(&self) -> HashMap<String, Vec<FireboltPermission>> {
        self.permitted.read().unwrap().clone()
    }

    fn get_app_permissions(&self, app_id: &str) -> Option<Vec<FireboltPermission>> {
        self.permitted.read().unwrap().get(app_id).cloned()
    }

    pub fn check_cap_role(&self, app_id: &str, role_info:RoleInfo) -> bool {
        if let Some(perms) = self.get_app_permissions(app_id) {
            for perm in perms {
                return perm.cap.as_str() == role_info.capability && perm.role == role_info.role;
            }
        }
        false
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
                    let mut permitted_state = state.permitted_state.clone();
                    permitted_state.ingest(map);
                }
            }
        }

        Err(ripple_sdk::utils::error::RippleError::InvalidOutput)
    }

    pub async fn check_permitted(
        state: PlatformState,
        app_id: String,
        request: CapabilitySet,
    ) -> Result<(), DenyReasonWithCap> {
        if let Some(permitted) = state.permitted_state.get_app_permissions(&app_id) {
            let permission_set: CapabilitySet = permitted.clone().into();
            return permission_set.check(request);
        } else {
            // check to retrieve it one more time
            if let Ok(_) = Self::fetch_and_store(state.clone(), app_id.clone()).await {
                // cache primed try again
                if let Some(permitted) = state.permitted_state.get_app_permissions(&app_id) {
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
