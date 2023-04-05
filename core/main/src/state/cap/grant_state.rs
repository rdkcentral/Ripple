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
use std::{
    collections::{HashMap, HashSet},
    hash::{Hash, Hasher},
    sync::{Arc, RwLock},
    time::{Duration, SystemTime},
};

use ripple_sdk::{
    api::{
        firebolt::{
            fb_capabilities::{CapabilityRole, DenyReason, DenyReasonWithCap, FireboltPermission},
            fb_openrpc::CapabilitySet,
        },
        gateway::rpc_gateway_api::CallContext,
        manifest::device_manifest::{DeviceManifest, GrantLifespan, GrantPolicy, GrantScope},
    },
    log::debug,
};
use serde::{Deserialize, Serialize};

use crate::{
    service::user_grants::grant_policy_enforcer::GrantPolicyEnforcer,
    state::platform_state::PlatformState,
};

#[derive(Debug, Clone)]
pub struct GrantState {
    device_grants: Arc<RwLock<HashSet<GrantEntry>>>,
    grant_app_map: Arc<RwLock<HashMap<String, HashSet<GrantEntry>>>>,
    caps_needing_grants: Vec<String>,
}

#[derive(Debug, Clone)]
pub enum GrantActiveState {
    ActiveGrant(Result<(), DenyReason>),
    PendingGrant,
}

impl GrantState {
    pub fn new(manifest: DeviceManifest) -> GrantState {
        GrantState {
            grant_app_map: Arc::new(RwLock::new(HashMap::new())),
            caps_needing_grants: manifest.get_caps_requiring_grant(),
            device_grants: Arc::new(RwLock::new(HashSet::new())),
        }
    }

    fn check_device_grants(&self, grant_entry: &GrantEntry) -> Option<GrantStatus> {
        let device_grants = self.device_grants.read().unwrap();
        if let Some(v) = device_grants.get(grant_entry) {
            return v.status.clone();
        }
        None
    }

    fn check_app_grants(&self, grant_entry: &GrantEntry, app_id: &str) -> Option<GrantStatus> {
        let grant_app_map = self.grant_app_map.read().unwrap();
        if let Some(app_map) = grant_app_map.get(app_id) {
            if let Some(v) = app_map.get(grant_entry) {
                return v.status.clone();
            }
        }
        None
    }

    pub fn update_grant_entry(
        &self,
        app_id: Option<String>, // None is for device
        new_entry: GrantEntry,
    ) {
        if app_id.is_some() {
            let app_id = app_id.unwrap();
            let mut grant_state = self.grant_app_map.write().unwrap();
            //Get a mutable reference to the value associated with a key, create it if it doesn't exist,
            let entries = grant_state.entry(app_id).or_insert(HashSet::new());

            if entries.contains(&new_entry) {
                entries.remove(&new_entry);
            }
            entries.insert(new_entry);
        } else {
            self.add_device_entry(new_entry)
        }
    }

    pub fn custom_delete_entries<F>(&self, app_id: String, restrict_function: F) -> bool
    where
        F: FnMut(&GrantEntry) -> bool,
    {
        let mut deleted = false;
        let mut grant_state = self.grant_app_map.write().unwrap();
        let entries = match grant_state.get_mut(&app_id) {
            Some(entries) => entries,
            None => return false,
        };
        let prev_len = entries.len();
        entries.retain(restrict_function);
        if entries.len() < prev_len {
            deleted = true;
        }
        deleted
    }

    pub fn delete_expired_entries_for_app(
        &self,
        app_id: String, // None is for device
    ) -> bool {
        let mut deleted = false;
        let mut grant_state = self.grant_app_map.write().unwrap();
        let entries = match grant_state.get_mut(&app_id) {
            Some(entries) => entries,
            None => return false,
        };
        let prev_len = entries.len();
        entries.retain(|entry| !entry.has_expired());
        if entries.len() < prev_len {
            deleted = true;
        }
        deleted
    }

    pub fn delete_all_expired_entries(&self) -> bool {
        let mut deleted = false;
        let mut grant_state = self.grant_app_map.write().unwrap();
        for (_, entries) in grant_state.iter_mut() {
            let prev_len = entries.len();
            entries.retain(|entry| !entry.has_expired());
            if entries.len() < prev_len {
                deleted = true;
            }
        }
        deleted
    }

    fn add_device_entry(&self, entry: GrantEntry) {
        let mut device_grants = self.device_grants.write().unwrap();
        device_grants.insert(entry);
    }

    pub fn get_grant_status(
        &self,
        app_id: &str,
        permission: &FireboltPermission,
    ) -> Option<GrantStatus> {
        let role = permission.role.clone();
        let capability = permission.cap.as_str();
        let result = self.get_generic_grant_status(role.clone(), &capability);
        if result.is_some() {
            return result;
        }

        let grant_state = self.grant_app_map.read().unwrap();
        debug!("grant state: {:?}", grant_state);
        let entries = grant_state.get(app_id)?;

        for entry in entries {
            if !entry.has_expired() && (entry.role == role) && (entry.capability == capability) {
                debug!("Stored grant status: {:?}", entry.status);
                return entry.status.clone();
            }
        }
        debug!("No stored grant status found");
        None
    }

    fn get_generic_grant_status(
        &self,
        role: CapabilityRole,
        capability: &str,
    ) -> Option<GrantStatus> {
        let grant_state = self.device_grants.read().unwrap();
        debug!("grant state: {:?}", grant_state);

        for entry in grant_state.iter() {
            if !entry.has_expired() && (entry.role == role) && (entry.capability == capability) {
                debug!("Stored grant status: {:?}", entry.status);
                return entry.status.clone();
            }
        }
        debug!("No stored grant status found");
        None
    }

    pub fn get_grant_state(
        &self,
        app_id: &str,
        permission: &FireboltPermission,
    ) -> GrantActiveState {
        if let Some(status) = self.get_grant_status(app_id, permission) {
            GrantActiveState::ActiveGrant(status.into())
        } else {
            GrantActiveState::PendingGrant
        }
    }

    pub async fn check_with_roles(
        state: &PlatformState,
        call_ctx: &CallContext,
        r: CapabilitySet,
    ) -> Result<(), DenyReasonWithCap> {
        /*
         * Instead of just checking for grants perviously, if the user grants are not present,
         * we are taking necessary steps to get the user grant and send back the result.
         */
        let grant_state = state.clone().cap_state.grant_state;
        let app_id = call_ctx.app_id.clone();
        for permission in r.into_firebolt_permissions_vec() {
            let result = grant_state.get_grant_state(&app_id, &permission);
            match result {
                GrantActiveState::ActiveGrant(grant) => {
                    if grant.is_err() {
                        return Ok(());
                    }
                }
                GrantActiveState::PendingGrant => {
                    let _result = GrantPolicyEnforcer::determine_grant_policies_for_permission(
                        &state,
                        &call_ctx,
                        &permission,
                    )
                    .await?;
                }
            }
        }

        Ok(())

        // UserGrants::determine_grant_policies(&self.ps.clone(), call_ctx, &r).await
    }

    pub fn update(
        &self,
        permission: &FireboltPermission,
        grant_policy: &GrantPolicy,
        is_allowed: bool,
        app_id: &str,
    ) {
        let mut grant_entry =
            GrantEntry::get(permission.role.clone(), permission.cap.as_str().to_owned());
        grant_entry.lifespan = Some(grant_policy.lifespan.clone());
        if grant_policy.lifespan_ttl.is_some() {
            grant_entry.lifespan_ttl_in_secs = grant_policy.lifespan_ttl.clone();
        }
        if is_allowed {
            grant_entry.status = Some(GrantStatus::Allowed);
        } else {
            grant_entry.status = Some(GrantStatus::Denied);
        }

        if grant_policy.lifespan != GrantLifespan::Once {
            self.update_grant_entry(
                match grant_policy.scope {
                    GrantScope::App => Some(app_id.into()),
                    GrantScope::Device => None,
                },
                grant_entry,
            )
        }
    }
}

#[derive(Eq, Clone, Debug, Serialize, Deserialize)]
pub struct GrantEntry {
    pub role: CapabilityRole,
    pub capability: String,
    pub status: Option<GrantStatus>,
    pub lifespan: Option<GrantLifespan>,
    pub last_modified_time: Duration,
    pub lifespan_ttl_in_secs: Option<u64>,
}

impl PartialEq for GrantEntry {
    fn eq(&self, other: &Self) -> bool {
        self.role == other.role && self.capability == other.capability
    }
}

impl Hash for GrantEntry {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.role.hash(state);
        self.capability.hash(state);
    }
}

impl GrantEntry {
    pub fn get(role: CapabilityRole, capability: String) -> GrantEntry {
        GrantEntry {
            role,
            capability,
            status: None,
            lifespan: None,
            last_modified_time: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap(),
            lifespan_ttl_in_secs: None,
        }
    }

    fn has_expired(&self) -> bool {
        match self.lifespan {
            Some(GrantLifespan::Seconds) => match self.lifespan_ttl_in_secs {
                None => true,
                Some(ttl) => {
                    let elapsed_time = SystemTime::now()
                        .duration_since(SystemTime::UNIX_EPOCH)
                        .unwrap()
                        .checked_sub(self.last_modified_time)
                        .unwrap_or(Duration::from_secs(0));

                    elapsed_time > Duration::from_secs(ttl)
                }
            },
            Some(GrantLifespan::Once) => true,
            _ => false,
        }
    }
}

#[derive(Eq, Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum GrantStatus {
    Allowed,
    Denied,
}

impl GrantStatus {
    pub fn as_string(&self) -> &'static str {
        match self {
            GrantStatus::Allowed => "granted",
            GrantStatus::Denied => "denied",
        }
    }
}

impl From<GrantStatus> for Result<(), DenyReason> {
    fn from(grant_status: GrantStatus) -> Self {
        match grant_status {
            GrantStatus::Allowed => Ok(()),
            GrantStatus::Denied => Err(DenyReason::GrantDenied),
        }
    }
}

impl Hash for GrantStatus {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_u8(match self {
            GrantStatus::Allowed => 0,
            GrantStatus::Denied => 1,
        });
    }
}

pub struct GrantHandler;

impl GrantHandler {
    pub async fn check_granted(
        state: &PlatformState,
        app_id: String,
        request: CapabilitySet,
    ) -> Result<(), DenyReasonWithCap> {
        let caps_needing_grants = state.clone().cap_state.grant_state.caps_needing_grants;
        let user_grant = state.clone().cap_state.grant_state;
        let permissions = request.into_firebolt_permissions_vec();
        let caps_needing_grant_in_request: Vec<FireboltPermission> = permissions
            .clone()
            .into_iter()
            .filter(|x| caps_needing_grants.contains(&x.cap.as_str()))
            .collect();
        let mut denied_caps = Vec::new();
        for permission in caps_needing_grant_in_request {
            let grant_entry = GrantEntry::get(permission.role.clone(), permission.cap.as_str());
            if let Some(v) = user_grant.check_device_grants(&grant_entry) {
                if let GrantStatus::Denied = v {
                    denied_caps.push(permission.cap.clone())
                }
            } else if let Some(v) = user_grant.check_app_grants(&grant_entry, &app_id) {
                if let GrantStatus::Denied = v {
                    denied_caps.push(permission.cap.clone())
                }
            }
        }

        if denied_caps.len() > 0 {
            return Err(DenyReasonWithCap {
                reason: ripple_sdk::api::firebolt::fb_capabilities::DenyReason::GrantDenied,
                caps: denied_caps,
            });
        }

        Ok(())
    }
}
