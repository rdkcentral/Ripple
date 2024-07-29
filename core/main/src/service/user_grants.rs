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
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use ripple_sdk::{
    api::{
        apps::{AppManagerResponse, AppMethod, AppRequest, AppResponse},
        device::{
            device_peristence::SetBoolProperty,
            device_user_grants_data::{
                AutoApplyPolicy, GrantActiveState, GrantEntry, GrantLifespan, GrantPolicy,
                GrantPrivacySetting, GrantScope, GrantStateModify, GrantStatus, GrantStep,
                PolicyPersistenceType,
            },
        },
        distributor::distributor_usergrants::{
            UserGrantsCloudSetParams, UserGrantsCloudStoreRequest,
        },
        firebolt::{
            fb_capabilities::{
                CapEvent, CapabilityRole, DenyReason, DenyReasonWithCap, FireboltCap,
                FireboltPermission, RoleInfo,
            },
            fb_lifecycle::LifecycleState,
            fb_openrpc::{CapabilitySet, FireboltOpenRpcMethod},
            fb_pin::{PinChallengeConfiguration, PinChallengeRequest},
            provider::{
                Challenge, ChallengeRequestor, ProviderRequestPayload, ProviderResponsePayload,
            },
        },
        gateway::rpc_gateway_api::{AppIdentification, CallerSession},
        manifest::device_manifest::DeviceManifest,
        usergrant_entry::UserGrantInfo,
    },
    framework::file_store::FileStore,
    log::{debug, error, warn},
    serde_json::Value,
    tokio::sync::oneshot,
    utils::error::RippleError,
};
use serde::Deserialize;

use crate::{
    firebolt::{firebolt_gatekeeper::FireboltGatekeeper, handlers::privacy_rpc::PrivacyImpl},
    state::{cap::cap_state::CapState, platform_state::PlatformState},
};

use super::apps::provider_broker::{ProviderBroker, ProviderBrokerRequest};

pub struct UserGrants {}

type GrantAppMap = Arc<RwLock<FileStore<HashMap<String, HashSet<GrantEntry>>>>>;

#[derive(Debug, Clone)]
pub struct GrantState {
    device_grants: Arc<RwLock<FileStore<HashSet<GrantEntry>>>>,
    grant_app_map: GrantAppMap,
    caps_needing_grants: Vec<String>,
}

impl GrantState {
    pub fn new(manifest: DeviceManifest) -> GrantState {
        let saved_dir = manifest.clone().configuration.saved_dir;
        let dir_path = Path::new(&saved_dir).join("device_grants");
        let device_grant_path = dir_path.into_os_string().into_string();
        let dev_grant_store = if let Ok(v) = FileStore::load(device_grant_path.clone().unwrap()) {
            v
        } else {
            FileStore::new(device_grant_path.unwrap(), HashSet::new())
        };
        let dir_path = Path::new(&saved_dir).join("app_grants");
        let app_grant_path = dir_path.into_os_string().into_string();
        let app_grant_store = if let Ok(v) = FileStore::load(app_grant_path.clone().unwrap()) {
            v
        } else {
            FileStore::new(app_grant_path.unwrap(), HashMap::new())
        };

        GrantState {
            grant_app_map: Arc::new(RwLock::new(app_grant_store)),
            caps_needing_grants: manifest.get_caps_requiring_grant(),
            device_grants: Arc::new(RwLock::new(dev_grant_store)),
        }
    }

    pub fn cleanup_user_grants(&self) {
        self.delete_all_expired_entries();
        self.delete_all_entries_for_lifespan(&GrantLifespan::PowerActive);
        self.delete_all_entries_for_lifespan(&GrantLifespan::AppActive);
    }

    fn check_device_grants(&self, grant_entry: &GrantEntry) -> Option<GrantStatus> {
        let device_grants = self.device_grants.read().unwrap();
        if let Some(v) = device_grants.value.get(grant_entry) {
            return v.status.clone();
        }
        None
    }

    fn check_app_grants(&self, grant_entry: &GrantEntry, app_id: &str) -> Option<GrantStatus> {
        let grant_app_map = self.grant_app_map.read().unwrap();
        if let Some(app_map) = grant_app_map.value.get(app_id) {
            if let Some(v) = app_map.get(grant_entry) {
                return v.status.clone();
            }
        }
        None
    }

    pub async fn sync_grant_map_with_grant_policy(&self, platform_state: &PlatformState) {
        //Remove app user grants
        let grant_entries_to_remove = Self::fetch_app_grant_entry_to_remove(platform_state);
        for (app_id, entry_set) in grant_entries_to_remove.iter() {
            for entry in entry_set {
                Self::force_delete_user_grant_from_local_sources(
                    platform_state,
                    Some(app_id),
                    entry,
                )
                .await;
            }
        }

        //Remove device user grants
        let grant_entries_to_remove = Self::fetch_device_grant_entry_to_remove(platform_state);
        for entry in grant_entries_to_remove {
            Self::force_delete_user_grant_from_local_sources(platform_state, None, &entry).await;
        }
    }

    fn fetch_app_grant_entry_to_remove(
        platform_state: &PlatformState,
    ) -> HashMap<String, HashSet<GrantEntry>> {
        let mut grant_entries_to_remove: HashMap<String, HashSet<GrantEntry>> = HashMap::new();
        let grant_policies_map = if let Some(grant_policies) =
            platform_state.get_device_manifest().get_grant_policies()
        {
            grant_policies
        } else {
            HashMap::default()
        };
        let grant_entries = platform_state
            .cap_state
            .grant_state
            .grant_app_map
            .read()
            .unwrap();
        for (app_id, app_entries) in grant_entries.value.iter() {
            let mut grant_entry_set_to_remove: HashSet<GrantEntry> = HashSet::new();
            for grant_entry in app_entries.iter() {
                if !grant_policies_map.contains_key(&grant_entry.capability) {
                    grant_entry_set_to_remove.insert(grant_entry.clone());
                }
            }
            if !grant_entry_set_to_remove.is_empty() {
                grant_entries_to_remove.insert(app_id.clone(), grant_entry_set_to_remove);
            }
        }
        debug!(
            "sync: app grant entries to be removed {:#?}",
            grant_entries_to_remove
        );
        grant_entries_to_remove
    }

    fn fetch_device_grant_entry_to_remove(platform_state: &PlatformState) -> HashSet<GrantEntry> {
        let grant_policies_map = if let Some(grant_policies) =
            platform_state.get_device_manifest().get_grant_policies()
        {
            grant_policies
        } else {
            HashMap::default()
        };
        let grant_entries = platform_state
            .cap_state
            .grant_state
            .device_grants
            .read()
            .unwrap();
        let mut grant_entry_set_to_remove: HashSet<GrantEntry> = HashSet::new();
        for grant_entry in grant_entries.value.iter() {
            if !grant_policies_map.contains_key(&grant_entry.capability) {
                grant_entry_set_to_remove.insert(grant_entry.clone());
            };
        }
        debug!(
            "sync: device grant entries to be removed {:#?}",
            grant_entry_set_to_remove
        );
        grant_entry_set_to_remove
    }

    #[allow(clippy::eq_op)]
    async fn force_delete_user_grant_from_local_sources(
        platform_state: &PlatformState,
        app_id: Option<&str>,
        entry: &GrantEntry,
    ) {
        let mut gc = entry.clone();
        let mut app_id_opt: Option<String> = None;
        if let Some(id) = app_id {
            // Delete app grant in local storage and grant state by app id
            let grant_entry_to_remove =
                Self::force_delete_user_grant(platform_state, id.to_string(), entry);

            if let Some(gc_opt) = grant_entry_to_remove {
                gc = gc_opt;
            }
            app_id_opt = Some(id.to_string());
        } else {
            // Delete device grant in local storage and grant state
            let mut device_grant_map_write = platform_state
                .cap_state
                .grant_state
                .device_grants
                .write()
                .unwrap();

            device_grant_map_write
                .value
                .retain(|entry: &GrantEntry| entry.capability != entry.capability);
            device_grant_map_write.sync();
        }
        gc.status = None;

        // Remove grant entry from cloud
        GrantPolicyEnforcer::send_usergrants_for_cloud_storage(
            platform_state,
            None,
            &gc,
            &app_id_opt,
        )
        .await
    }

    fn force_delete_user_grant(
        platform_state: &PlatformState,
        app_id: String,
        entry: &GrantEntry,
    ) -> Option<GrantEntry> {
        let mut gc_opt: Option<GrantEntry> = None;
        {
            let mut grant_app_map_write = platform_state
                .cap_state
                .grant_state
                .grant_app_map
                .write()
                .unwrap();
            let entries = grant_app_map_write.value.entry(app_id).or_default();
            if entries.contains(entry) {
                gc_opt = Some(entry.clone());
                entries.remove(entry);
            }
            grant_app_map_write.sync();
            gc_opt
        }
    }

    pub fn update_grant_entry(
        &self,
        app_id: Option<String>, // None is for device
        new_entry: GrantEntry,
    ) {
        if let Some(app_id) = app_id {
            let mut grant_state = self.grant_app_map.write().unwrap();
            //Get a mutable reference to the value associated with a key, create it if it doesn't exist,
            let entries = grant_state.value.entry(app_id).or_default();
            if entries.contains(&new_entry) {
                entries.remove(&new_entry);
            }
            if new_entry.status.is_some() {
                entries.insert(new_entry);
            }
            grant_state.sync();
        } else {
            self.add_device_entry(new_entry)
        }
    }

    pub fn clear_local_entries(&self, ps: &PlatformState, persistence_type: PolicyPersistenceType) {
        let mut app_grant_state = self.grant_app_map.write().unwrap();
        for (_, entries) in app_grant_state.value.iter_mut() {
            entries.retain(|entry| {
                !self.check_grant_policy_persistence(
                    ps,
                    entry.capability.clone(),
                    entry.role,
                    persistence_type.clone(),
                )
            });
        }
        app_grant_state.sync();

        let mut device_grant_state = self.device_grants.write().unwrap();
        device_grant_state.value.retain(|entry: &GrantEntry| {
            !self.check_grant_policy_persistence(
                ps,
                entry.capability.clone(),
                entry.role,
                persistence_type.clone(),
            )
        });
        device_grant_state.sync();
    }

    pub fn check_grant_policy_persistence(
        &self,
        ps: &PlatformState,
        capability: String,
        role: CapabilityRole,
        persistence: PolicyPersistenceType,
    ) -> bool {
        // retrieve the grant policy for the given cap and role.
        let permission = FireboltPermission {
            cap: FireboltCap::Full(capability),
            role,
        };

        if let Some(grant_policy_map) = ps.get_device_manifest().get_grant_policies() {
            let result = grant_policy_map.get(&permission.cap.as_str());
            if let Some(policies) = result {
                if let Some(grant_policy) = policies.get_policy(&permission) {
                    let grant_policy_persistence = grant_policy.persistence;
                    return grant_policy_persistence == persistence;
                }
            }
        }
        false
    }

    pub fn custom_delete_entries<F>(&self, app_id: String, restrict_function: F) -> bool
    where
        F: FnMut(&GrantEntry) -> bool,
    {
        let mut deleted = false;
        let mut grant_state = self.grant_app_map.write().unwrap();
        let entries = match grant_state.value.get_mut(&app_id) {
            Some(entries) => entries,
            None => return false,
        };
        let prev_len = entries.len();
        entries.retain(restrict_function);
        if entries.len() < prev_len {
            deleted = true;
        }
        grant_state.sync();
        deleted
    }

    /**
     *  Delete all matching entries based on the lifespan
     */
    pub fn delete_all_entries_for_lifespan(&self, lifespan: &GrantLifespan) -> bool {
        let mut deleted = false;
        {
            let mut grant_state = self.grant_app_map.write().unwrap();

            for set in grant_state.value.values_mut() {
                let prev_len = set.len();
                set.retain(|entry| entry.lifespan.as_ref().map_or(false, |l| l != lifespan));
                if set.len() < prev_len {
                    deleted = true;
                }
            }
            if deleted {
                grant_state.sync()
            }
        }
        {
            let mut grant_state = self.device_grants.write().unwrap();
            let prev_len = grant_state.value.len();
            grant_state
                .value
                .retain(|entry| entry.lifespan.as_ref().map_or(false, |l| l != lifespan));
            if grant_state.value.len() < prev_len {
                deleted = true;
            }

            if deleted {
                grant_state.sync()
            }
        }

        deleted
    }

    pub fn delete_expired_entries_for_app(&self, app_id: String) -> bool {
        let mut deleted = false;
        let mut grant_state = self.grant_app_map.write().unwrap();
        let entries = match grant_state.value.get_mut(&app_id) {
            Some(entries) => entries,
            None => return false,
        };
        let prev_len = entries.len();
        entries.retain(|entry| !entry.has_expired());
        if entries.len() < prev_len {
            deleted = true;
        }
        grant_state.sync();
        deleted
    }

    pub fn delete_expired_entries_for_device(&self) -> bool {
        let mut deleted = false;
        let mut grant_state = self.device_grants.write().unwrap();
        let prev_len = grant_state.value.len();
        grant_state.value.retain(|entry| !entry.has_expired());
        if grant_state.value.len() < prev_len {
            deleted = true;
        }
        grant_state.sync();
        deleted
    }

    pub fn delete_all_expired_entries(&self) -> bool {
        // delete expired entries for app
        let mut grant_state = self.grant_app_map.write().unwrap();
        for (_, entries) in grant_state.value.iter_mut() {
            entries.retain(|entry| !entry.has_expired());
        }
        grant_state.sync();

        // delete expired entries for device
        self.delete_expired_entries_for_device();
        true
    }

    fn add_device_entry(&self, entry: GrantEntry) {
        let mut device_grants = self.device_grants.write().unwrap();
        if entry.status.is_none() {
            device_grants.value.remove(&entry);
        } else {
            device_grants.value.replace(entry);
        }
        device_grants.sync();
    }

    pub fn get_grant_status(
        &self,
        app_id: &str,
        permission: &FireboltPermission,
    ) -> Option<GrantStatus> {
        let role = permission.role;
        let capability = permission.cap.as_str();
        let result = self.get_generic_grant_status(role, &capability);
        if result.is_some() {
            return result;
        }

        let grant_state = self.grant_app_map.read().unwrap();
        debug!("grant state: {:?}", grant_state);
        let entries = grant_state.value.get(app_id)?;

        for entry in entries {
            debug!(
                "Stored grant entry: {:?} requested cap: {:?} & role: {:?}",
                entry, role, capability
            );
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

        for entry in grant_state.value.iter() {
            if !entry.has_expired() && (entry.role == role) && (entry.capability == capability) {
                debug!("Stored grant status: {:?}", entry.status);
                return entry.status.clone();
            }
        }
        debug!("No stored device grant status found");
        None
    }

    pub fn get_grant_state(
        &self,
        app_id: &str,
        permission: &FireboltPermission,
        platform_state: Option<&PlatformState>,
    ) -> GrantActiveState {
        if let Some(status) = self.get_grant_status(app_id, permission) {
            GrantActiveState::ActiveGrant(status.into())
        } else {
            // This flow is when the capability is not in grant state.
            // Check if there is a grant policy for the capability.
            // if no, return allowed as active grant state;
            // otherwise, return pending grant.
            if let Some(state) = platform_state {
                let grant_polices_map_opt = state.get_device_manifest().capabilities.grant_policies;
                if let Some(grant_polices_map) = grant_polices_map_opt.as_ref() {
                    let capability = permission.cap.as_str();
                    let filtered_policy_opt = grant_polices_map.get(capability.as_str());
                    if filtered_policy_opt.is_none() {
                        debug!(
                            "Capability {:?} is not in grant policy",
                            capability.as_str()
                        );
                        return GrantActiveState::ActiveGrant((GrantStatus::Allowed).into());
                    }
                } else {
                    debug!("grant_polices_map is not available");
                    return GrantActiveState::ActiveGrant((GrantStatus::Allowed).into());
                }
            }
            GrantActiveState::PendingGrant
        }
    }

    pub async fn check_with_roles(
        state: &PlatformState,
        caller_session: &CallerSession,
        app_requested_for: &AppIdentification,
        fb_perms: &[FireboltPermission],
        fail_on_first_error: bool,
        apply_exclusion_filter: bool,
        force: bool,
    ) -> Result<(), DenyReasonWithCap> {
        /*
         * Instead of just checking for grants previously, if the user grants are not present,
         * we are taking necessary steps to get the user grant and send back the result.
         */
        let grant_state = state.clone().cap_state.grant_state;
        let app_id = app_requested_for.app_id.to_owned();
        let caps_needing_grants = grant_state.caps_needing_grants.clone();
        let mut caps_needing_grant_in_request: Vec<FireboltPermission> = fb_perms
            .iter()
            .filter(|&x| caps_needing_grants.contains(&x.cap.as_str()))
            .cloned()
            .collect();

        if apply_exclusion_filter {
            caps_needing_grant_in_request = GrantPolicyEnforcer::apply_grant_exclusion_filters(
                state,
                &app_requested_for.app_id,
                None,
                &caps_needing_grant_in_request,
            )
            .await;

            if caps_needing_grant_in_request.is_empty() {
                debug!("check_with_roles: caps_needing_grant_in_request list is empty() after applying grant_exclusion_filters");
                return Ok(());
            }
        }

        let mut denied_caps = Vec::new();
        for permission in caps_needing_grant_in_request {
            let result = if force {
                GrantActiveState::PendingGrant
            } else {
                grant_state.get_grant_state(&app_id, &permission, None)
            };

            match result {
                GrantActiveState::ActiveGrant(grant) => grant.map_err(|err| DenyReasonWithCap {
                    reason: err,
                    caps: vec![permission.cap.clone()],
                })?,
                GrantActiveState::PendingGrant => {
                    let result = GrantPolicyEnforcer::determine_grant_policies_for_permission(
                        state,
                        caller_session,
                        app_requested_for,
                        &permission,
                    )
                    .await;

                    match result {
                        Ok(_) => {}
                        Err(deny_reason_with_cap) => {
                            if fail_on_first_error
                                || deny_reason_with_cap.reason == DenyReason::AppNotInActiveState
                            {
                                return Err(deny_reason_with_cap);
                            } else {
                                denied_caps.push(permission.cap.clone())
                            }
                        }
                    }
                }
            }
        }

        if !denied_caps.is_empty() {
            return Err(DenyReasonWithCap {
                reason: DenyReason::GrantDenied,
                caps: denied_caps,
            });
        }

        Ok(())

        // UserGrants::determine_grant_policies(&self.ps.clone(), call_ctx, &r).await
    }

    pub fn check_granted(
        &self,
        state: &PlatformState,
        app_id: &str,
        role_info: RoleInfo,
    ) -> Result<bool, RippleError> {
        debug!(
            "Incoming grant check for app_id: {:?} and role: {:?}",
            app_id, role_info
        );
        for perm in FireboltGatekeeper::resolve_dependencies(
            state,
            &vec![FireboltPermission::from(role_info)],
        ) {
            match self.get_grant_state(app_id, &perm, Some(state)) {
                GrantActiveState::ActiveGrant(grant) => {
                    if grant.is_err() {
                        return Err(RippleError::Permission(DenyReason::GrantDenied));
                    }
                }
                GrantActiveState::PendingGrant => {
                    return Err(RippleError::Permission(DenyReason::Ungranted));
                }
            }
        }
        Ok(true)
    }

    pub fn update(
        &self,
        permission: &FireboltPermission,
        grant_policy: &GrantPolicy,
        is_allowed: bool,
        app_id: &str,
    ) {
        debug!("Update called to store user grant with grant = {is_allowed}");
        let mut grant_entry = GrantEntry::get(permission.role, permission.cap.as_str());
        grant_entry.lifespan = Some(grant_policy.lifespan.clone());
        if grant_policy.lifespan_ttl.is_some() {
            grant_entry.lifespan_ttl_in_secs = grant_policy.lifespan_ttl;
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

    // Returns all active and denied user grant entries for the given `app_id`.
    pub fn get_grant_entries_for_app_id(&self, app_id: String) -> HashSet<GrantEntry> {
        self.delete_expired_entries_for_app(app_id.clone());
        match self.grant_app_map.read().unwrap().value.get(&app_id) {
            Some(x) => x.iter().cloned().collect(),
            None => HashSet::new(),
        }
    }

    // Returns all active and denied user grant entries for the given `app_id`.
    // Pass None for device scope
    pub fn get_device_entries(&self) -> HashSet<GrantEntry> {
        self.delete_expired_entries_for_device();
        self.device_grants.read().unwrap().value.clone()
    }

    // Returns all active and denied user grant entries for the given `capability`
    pub fn get_grant_entries_for_capability(
        &self,
        capability: &str,
    ) -> HashMap<String, HashSet<GrantEntry>> {
        self.delete_all_expired_entries();
        let grant_state = self.grant_app_map.read().unwrap();
        let mut grant_entry_map: HashMap<String, HashSet<GrantEntry>> = HashMap::new();
        for (app_id, app_entries) in grant_state.value.iter() {
            for item in app_entries {
                if item.capability == capability {
                    grant_entry_map
                        .entry(app_id.clone())
                        .or_default()
                        .insert(item.clone());
                }
            }
        }
        let device_grants = self.device_grants.read().unwrap();
        let grant_sets = device_grants
            .value
            .iter()
            .filter(|elem| elem.capability == capability)
            .cloned()
            .collect();
        grant_entry_map.insert("device".to_owned(), grant_sets);
        grant_entry_map
    }

    fn get_mapped_grant_status(
        platform_state: &PlatformState,
        app_id: &str,
        capability: &str,
        role: CapabilityRole,
    ) -> Option<bool> {
        let grant_state = &platform_state.cap_state.grant_state;
        grant_state
            .get_grant_status(
                app_id,
                &FireboltPermission {
                    cap: FireboltCap::Full(capability.to_owned()),
                    role,
                },
            )
            .map(|grant_status| match grant_status {
                GrantStatus::Allowed => true,
                GrantStatus::Denied => false,
            })
    }
    pub fn check_all_granted(
        platform_state: &PlatformState,
        app_id: &str,
        capability: &str,
    ) -> (Option<bool>, Option<bool>, Option<bool>) {
        let use_granted =
            Self::get_mapped_grant_status(platform_state, app_id, capability, CapabilityRole::Use);
        let manage_granted = Self::get_mapped_grant_status(
            platform_state,
            app_id,
            capability,
            CapabilityRole::Manage,
        );
        let provide_granted = Self::get_mapped_grant_status(
            platform_state,
            app_id,
            capability,
            CapabilityRole::Provide,
        );
        (use_granted, manage_granted, provide_granted)
    }

    pub async fn grant_modify(
        platform_state: &PlatformState,
        modify_operation: GrantStateModify,
        app_id: Option<String>,
        role: CapabilityRole,
        capability: String,
    ) -> bool {
        // Get the GrantEntry from UserGrantState matching app_id, role & capability
        let mut entry_modified = false;

        // retrieve the grant policy for the given cap and role.
        let permission = FireboltPermission {
            cap: FireboltCap::Full(capability.clone()),
            role,
        };
        match modify_operation {
            GrantStateModify::Grant | GrantStateModify::Deny => {}
            GrantStateModify::Clear => {}
        }

        if let Some(grant_policy_map) = platform_state.get_device_manifest().get_grant_policies() {
            let result = grant_policy_map.get(&permission.cap.as_str());
            if let Some(policies) = result {
                if let Some(grant_policy) = policies.get_policy(&permission) {
                    // Do the scope validation here and return false, if there is any scope mismatch.
                    if app_id.is_some() && grant_policy.scope != GrantScope::App {
                        return false;
                    }

                    let grant_policy_persistence = grant_policy.persistence.clone();
                    let mut new_entry = GrantEntry {
                        role,
                        capability,
                        status: None, // status will be updated later based on the modify operation.
                        lifespan: Some(grant_policy.lifespan.clone()),
                        lifespan_ttl_in_secs: grant_policy.lifespan_ttl,
                        last_modified_time: SystemTime::now()
                            .duration_since(SystemTime::UNIX_EPOCH)
                            .unwrap(),
                    };

                    match modify_operation {
                        GrantStateModify::Grant => {
                            // insert the allowed GrantEntry
                            new_entry.status = Some(GrantStatus::Allowed);
                            entry_modified = true;
                        }
                        GrantStateModify::Deny => {
                            // insert the denied GrantEntry
                            new_entry.status = Some(GrantStatus::Denied);
                            entry_modified = true;
                        }
                        GrantStateModify::Clear => {
                            // No op as the entry is already removed.
                            entry_modified = true;
                        }
                    }

                    debug!("user grant modified with new entry:{:?}", new_entry.clone());
                    platform_state
                        .cap_state
                        .grant_state
                        .update_grant_entry(app_id.clone(), new_entry.clone());

                    debug!(
                        "Sync user grant modified with new entry:{:?} to cloud",
                        new_entry.clone()
                    );
                    if grant_policy_persistence == PolicyPersistenceType::Account {
                        GrantPolicyEnforcer::send_usergrants_for_cloud_storage(
                            platform_state,
                            Some(&grant_policy),
                            &new_entry,
                            &app_id,
                        )
                        .await;
                    }
                }
            }
        }
        entry_modified
    }
    pub fn get_grant_policy(
        platform_state: &PlatformState,
        permission: &FireboltPermission,
        _app_id: &Option<String>,
    ) -> Option<GrantPolicy> {
        if let Some(grant_policy_map) = platform_state.get_device_manifest().get_grant_policies() {
            let grant_policies = grant_policy_map.get(&permission.cap.as_str()).cloned()?;
            grant_policies.get_policy(permission)
        } else {
            None
        }
    }
    pub async fn update_grant_as_per_policy(
        platform_state: &PlatformState,
        granted: GrantStateModify,
        app_id: &Option<String>,
        // permission: &FireboltPermission,
        role: CapabilityRole,
        capability: String,
    ) -> Result<(), &'static str> {
        let permission = FireboltPermission {
            cap: FireboltCap::Full(capability),
            role,
        };

        let grant_policy =
            Self::get_grant_policy(platform_state, &permission, app_id).ok_or_else(|| {
                error!(
                    "There are no grant polices for the requesting cap so cant update user grant"
                );
                "There are no grant polices for the requesting cap so cant update user grant"
            })?;
        if !GrantPolicyEnforcer::is_policy_valid(platform_state, &grant_policy) {
            return Err(
                "There are no valid grant polices for the requesting cap so cant update user grant",
            );
        }
        if grant_policy.scope == GrantScope::App && app_id.is_none()
            || grant_policy.scope == GrantScope::Device && app_id.is_some()
        {
            error!("Grant policy scope and request scope doesn't match!");
            return Err("Grant policy scope and request scope doesn't match!");
        }
        // let result = match granted {
        //     GrantStateModify::Grant =
        // };
        if granted != GrantStateModify::Clear {
            let result = match granted {
                GrantStateModify::Grant => Ok(()),
                _ => Err(DenyReasonWithCap {
                    reason: DenyReason::GrantDenied,
                    caps: vec![permission.cap.clone()],
                }),
            };
            if grant_policy.privacy_setting.is_none()
                || !grant_policy
                    .privacy_setting
                    .as_ref()
                    .unwrap()
                    .update_property
            {
                //There is no need to update privacy settings so it is enough to update only user grants storage.

                GrantPolicyEnforcer::store_user_grants(
                    platform_state,
                    &permission,
                    &result,
                    app_id,
                    &grant_policy,
                )
                .await;
                return Ok(());
            } else {
                GrantPolicyEnforcer::update_privacy_settings_and_user_grants(
                    platform_state,
                    &permission,
                    &result,
                    app_id,
                    &grant_policy,
                )
                .await;
                return Ok(());
            }
        } else {
            // For clear user grants, we will only clear the stored user grants.
            // It is will not affect the privacy settings.
            let new_entry = GrantEntry {
                role: permission.role,
                capability: permission.cap.clone().as_str(),
                status: None, // status will be updated later based on the modify operation.
                lifespan: Some(grant_policy.lifespan.clone()),
                lifespan_ttl_in_secs: grant_policy.lifespan_ttl,
                last_modified_time: SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap(),
            };
            debug!("user grant modified with new entry:{:?}", new_entry.clone());
            platform_state
                .cap_state
                .grant_state
                .update_grant_entry(app_id.clone(), new_entry.clone());

            debug!(
                "Sync user grant modified with new entry:{:?} to cloud",
                new_entry.clone()
            );
            let grant_policy_persistence = grant_policy.persistence.clone();
            if grant_policy_persistence == PolicyPersistenceType::Account {
                GrantPolicyEnforcer::send_usergrants_for_cloud_storage(
                    platform_state,
                    Some(&grant_policy),
                    &new_entry,
                    app_id,
                )
                .await;
            }
        }
        Ok(())
    }
}

pub struct GrantHandler;

impl GrantHandler {
    pub async fn grant_info(
        state: &PlatformState,
        app_id: String,
        request: CapabilitySet,
    ) -> Result<(), DenyReasonWithCap> {
        let caps_needing_grants = state.clone().cap_state.grant_state.caps_needing_grants;
        let user_grant = state.clone().cap_state.grant_state;
        let permissions = request.into_firebolt_permissions_vec();
        let caps_needing_grant_in_request: Vec<FireboltPermission> = permissions
            .into_iter()
            .filter(|x| caps_needing_grants.contains(&x.cap.as_str()))
            .collect();
        let mut denied_caps = Vec::new();
        for permission in caps_needing_grant_in_request {
            let grant_entry = GrantEntry::get(permission.role, permission.cap.as_str());
            if let Some(v) = user_grant.check_device_grants(&grant_entry) {
                if let GrantStatus::Denied = v {
                    denied_caps.push(permission.cap.clone());
                }
            } else if let Some(GrantStatus::Denied) =
                user_grant.check_app_grants(&grant_entry, &app_id)
            {
                denied_caps.push(permission.cap.clone());
            }
        }

        if !denied_caps.is_empty() {
            return Err(DenyReasonWithCap {
                reason: ripple_sdk::api::firebolt::fb_capabilities::DenyReason::GrantDenied,
                caps: denied_caps,
            });
        }

        Ok(())
    }

    pub fn are_all_user_grants_resolved(
        state: &PlatformState,
        app_id: &str,
        permissions: Vec<FireboltPermission>,
    ) -> Vec<FireboltPermission> {
        let user_grant = state.clone().cap_state.grant_state;
        let mut final_perms = Vec::new();
        for permission in permissions {
            let result = user_grant.get_grant_state(app_id, &permission, Some(state));
            if let GrantActiveState::PendingGrant = result {
                final_perms.push(permission.clone());
            }
        }
        final_perms
    }
}

pub struct GrantPolicyEnforcer;

impl GrantPolicyEnforcer {
    pub async fn send_usergrants_for_cloud_storage(
        platform_state: &PlatformState,
        grant_policy: Option<&GrantPolicy>,
        grant_entry: &GrantEntry,
        app_id: &Option<String>,
    ) {
        if let Some(account_session) = platform_state.session_state.get_account_session() {
            let s = grant_entry.status.to_owned();

            let user_grant_info = if let Some(policy) = grant_policy {
                // Modifying user grant flow goes in here
                UserGrantInfo {
                    role: grant_entry.role,
                    capability: grant_entry.capability.to_owned(),
                    status: s,
                    last_modified_time: Duration::new(0, 0),
                    expiry_time: match policy.lifespan {
                        GrantLifespan::Seconds => {
                            let now = match SystemTime::now().duration_since(UNIX_EPOCH) {
                                Ok(now) => now,
                                Err(_) => {
                                    error!("Unable to sync usergrants to cloud. unable to get duration since epoch");
                                    return;
                                }
                            };

                            let now_dur =
                                now.checked_add(Duration::new(policy.lifespan_ttl.unwrap(), 0));

                            match now_dur {
                                Some(now_dur) => Some(now_dur),
                                None => {
                                    error!("Unable to sync usergrants to cloud. unable to get duration since epoch for lifespan_ttl");
                                    return;
                                }
                            }
                        }
                        _ => None,
                    },
                    app_name: app_id.to_owned(),
                    lifespan: policy.lifespan.to_owned(),
                }
            } else {
                // Deleting user grant after grant has been remove from grant policy flow goes in here
                // Create a dummy user grant info just to pass in app id to the cloud service
                UserGrantInfo {
                    role: grant_entry.role,
                    capability: grant_entry.capability.to_owned(),
                    status: s,
                    last_modified_time: Duration::new(0, 0),
                    expiry_time: None,
                    app_name: app_id.to_owned(),
                    lifespan: GrantLifespan::Forever,
                }
            };

            if matches!(
                user_grant_info.lifespan,
                GrantLifespan::PowerActive | GrantLifespan::AppActive | GrantLifespan::Once
            ) {
                warn!("Can't sync user grant info with lifespan other than forever or seconds");
                return;
            }

            let usergrants_cloud_set_params = UserGrantsCloudSetParams {
                account_session,
                user_grant_info,
            };
            let request =
                UserGrantsCloudStoreRequest::SetCloudUserGrants(usergrants_cloud_set_params);
            let resp = platform_state.get_client().send_extn_request(request).await;
            if resp.is_err() {
                error!("Unable to sync usergrants to cloud. Unable to send request to extn");
            }
        }
    }

    pub async fn store_user_grants(
        platform_state: &PlatformState,
        permission: &FireboltPermission,
        result: &Result<(), DenyReasonWithCap>,
        app_id: &Option<String>,
        grant_policy: &GrantPolicy,
    ) -> bool {
        let mut ret_val = false;
        let mut grant_entry = GrantEntry::get(permission.role, permission.cap.as_str());
        grant_entry.lifespan = Some(grant_policy.lifespan.clone());
        if grant_policy.lifespan_ttl.is_some() {
            grant_entry.lifespan_ttl_in_secs = grant_policy.lifespan_ttl;
        }
        if result.is_ok() {
            grant_entry.status = Some(GrantStatus::Allowed);
        } else {
            grant_entry.status = Some(GrantStatus::Denied);
        }
        debug!("created grant_entry: {:?}", grant_entry);
        let grant_entry_c = grant_entry.clone();
        // let grant_entry_c = grant_entry.clone();
        // If lifespan is once then no need to store it.
        if grant_policy.lifespan != GrantLifespan::Once {
            match grant_policy.scope {
                GrantScope::App => {
                    if app_id.is_some() {
                        platform_state
                            .cap_state
                            .grant_state
                            .update_grant_entry(app_id.to_owned(), grant_entry);
                        ret_val = true;
                    }
                }
                GrantScope::Device => {
                    platform_state
                        .cap_state
                        .grant_state
                        .update_grant_entry(None, grant_entry);
                    ret_val = true;
                }
            }
        }
        if grant_policy.persistence == PolicyPersistenceType::Account {
            Self::send_usergrants_for_cloud_storage(
                platform_state,
                Some(grant_policy),
                &grant_entry_c,
                app_id,
            )
            .await;
        }
        ret_val
    }

    pub async fn update_privacy_settings_and_user_grants(
        platform_state: &PlatformState,
        permission: &FireboltPermission,
        result: &Result<(), DenyReasonWithCap>,
        app_id: &Option<String>,
        grant_policy: &GrantPolicy,
    ) {
        // Updating privacy settings
        if let Some(privacy_setting) = &grant_policy.privacy_setting {
            if privacy_setting.update_property {
                Self::update_privacy_settings_with_grant(
                    platform_state,
                    privacy_setting,
                    result.is_ok(),
                )
                .await;
            }
        }

        Self::store_user_grants(platform_state, permission, result, app_id, grant_policy).await;
    }

    // Helper function to check
    // 1. Privacy settings for autoApplyPolicy
    // 2. Availability of providers for user grant resolution.
    pub async fn can_proceed_with_user_grant_resolution(
        platform_state: &PlatformState,
        permissions: &Vec<FireboltPermission>,
    ) -> bool {
        let mut check_status = false;
        let grant_policies_map = match platform_state.get_device_manifest().get_grant_policies() {
            Some(policy) => policy,
            None => {
                debug!("There are no grant policies for the requesting cap so bailing out");
                return false;
            }
        };

        for permission in permissions {
            let policy_opt = grant_policies_map
                .get(&permission.cap.as_str())
                .and_then(|policies| policies.get_policy(permission));

            if let Some(policy) = policy_opt {
                if let Some(privacy_setting) = policy.privacy_setting {
                    if privacy_setting.auto_apply_policy != AutoApplyPolicy::Never {
                        let result = GrantPolicyEnforcer::evaluate_privacy_settings(
                            platform_state,
                            &privacy_setting,
                        )
                        .await;

                        if result.is_some() {
                            // already resolved.
                            return false;
                        }
                    }
                }

                // Now check if provider and capability are available.
                for grant_requirements in &policy.options {
                    let step_caps: Vec<FireboltPermission> = grant_requirements
                        .steps
                        .iter()
                        .map(|step| FireboltPermission {
                            cap: step.capability_as_fb_cap(),
                            role: CapabilityRole::Use,
                        })
                        .collect();

                    if platform_state
                        .cap_state
                        .generic
                        .check_all(&step_caps)
                        .is_ok()
                    {
                        check_status = true
                    }
                }
            }
        }
        check_status
    }

    pub async fn determine_grant_policies_for_permission(
        platform_state: &PlatformState,
        caller_session: &CallerSession,
        app_requested_for: &AppIdentification,
        permission: &FireboltPermission,
    ) -> Result<(), DenyReasonWithCap> {
        // 1. If the call is coming from method invocation then check if app is in active state.
        // This check is omitted if the call is coming from Lifecyclemanagement.session because this check happens right
        // before the app goes active, so this check would fail.
        // 2. Check if the capability is supported and available.
        // 3. Call the capability,
        // 4. Get the user response and return

        let CallerSession { session_id, app_id } = caller_session;
        if session_id.is_some() && app_id.is_some() {
            // session id is some, so caller is from method invoke
            debug!("Method invoke caller, check if app is in foreground state");
            let app_state = platform_state
                .ripple_client
                .get_app_state(app_id.as_ref().unwrap())
                .await;

            match app_state {
                Ok(state) => {
                    if state != LifecycleState::Foreground.as_string() {
                        debug!("App is not in foreground state");
                        return Err(DenyReasonWithCap {
                            reason: DenyReason::AppNotInActiveState,
                            caps: vec![permission.cap.clone()],
                        });
                    }
                }
                Err(_) => {
                    error!("Unable to get app state");
                    return Err(DenyReasonWithCap {
                        reason: DenyReason::AppNotInActiveState,
                        caps: vec![permission.cap.clone()],
                    });
                }
            }
            debug!(
                "Requesting app is in active state, now has to check if cap is supported and available"
            );
        } else {
            // session id is None, so caller is from lifecyclemanagement.session
            debug!("Lifecyclemanagement.session caller, skip app state check. Check if cap is supported and available")
        }

        let grant_policy_opt = platform_state.get_device_manifest().get_grant_policies();
        let grant_policies_map = match grant_policy_opt {
            Some(map) => map,
            None => {
                debug!("There are no grant policies for the requesting cap so bailing out");
                return Ok(());
            }
        };

        let grant_policies_opt = grant_policies_map.get(&permission.cap.as_str());
        let policies = match grant_policies_opt {
            Some(policies) => policies,
            None => {
                debug!(
                    "There are no policies for the cap: {} so granting request",
                    permission.cap.as_str()
                );
                return Ok(());
            }
        };

        let policy_opt = policies.get_policy(permission);
        let policy = match policy_opt {
            Some(policy) => policy,
            None => {
                debug!(
                    "There are no polices for cap: {} for role: {:?}",
                    permission.cap.as_str(),
                    permission.role
                );
                return Ok(());
            }
        };

        if !Self::is_policy_valid(platform_state, &policy) {
            error!("Configured policy is not valid {:?}", policy);
            return Err(DenyReasonWithCap {
                caps: vec![permission.clone().cap],
                reason: DenyReason::Disabled,
            });
        }
        let result = GrantPolicyEnforcer::execute(
            platform_state,
            caller_session,
            app_requested_for,
            permission,
            &policy,
        )
        .await;

        if let Err(e) = &result {
            // if the grant failed because we don't have a provider or because requested app is not in active state to
            // fulfill the grant , do not store the result in grant state
            if e.reason == DenyReason::Ungranted
                || e.reason == DenyReason::GrantProviderMissing
                || e.reason == DenyReason::AppNotInActiveState
            {
                return result;
            }
        } else {
            // TODO: This debug statement looks incorrect as it would trigger if all grants were successful
            debug!("Grant policies executed successfully. Result: {:?}", result);
        }
        Self::update_privacy_settings_and_user_grants(
            platform_state,
            permission,
            &result,
            &Some(app_requested_for.app_id.to_owned()),
            &policy,
        )
        .await;

        result
    }

    fn is_policy_valid(platform_state: &PlatformState, policy: &GrantPolicy) -> bool {
        // Privacy settings in a policy takes higher precedence and we are
        // evaluating first.
        if let Some(privacy) = &policy.privacy_setting {
            let privacy_property = &privacy.property;
            return platform_state
                .open_rpc_state
                .check_privacy_property(privacy_property);
        }
        if policy.get_steps_without_grant().is_some() {
            // If any cap in any step in a policy is not starting with
            // "xrn:firebolt:capability:usergrant:" pattern,
            // we should treat it as a invalid policy.
            return false;
        }
        // If a policy doesn't have a privacy settings and caps in all steps starts with
        //xrn:firebolt:capability:usergrant: then the policy deemed to be valid.
        true
    }

    pub fn get_allow_value(platform_state: &PlatformState, property_name: &str) -> Option<bool> {
        // Find the rpc method which has same name as that mentioned in privacy settings, and has allow property.
        platform_state
            .open_rpc_state
            .get_method_with_allow_value_property(String::from(property_name))
            .map(|method| method.get_allow_value().unwrap()) // We can safely unwrap because the previous step in the chain ensures x-allow-value is present
    }

    async fn evaluate_privacy_settings(
        platform_state: &PlatformState,
        privacy_setting: &GrantPrivacySetting,
    ) -> Option<Result<(), DenyReason>> {
        let allow_value = Self::get_allow_value(platform_state, privacy_setting.property.as_str())
            .or_else(|| {
                debug!(
                    "Allow value not present for property: {}",
                    privacy_setting.property.as_str()
                );
                None
            })?;

        // From privacyImpl make the call to the registered method for the configured privacy settings.
        let stored_value =
            PrivacyImpl::handle_allow_get_requests(&privacy_setting.property, platform_state)
                .await
                .ok()
                .or_else(|| {
                    debug!(
                        "Unable to get stored value for privacy settings: {}",
                        privacy_setting.property.as_str()
                    );

                    None
                })?;

        debug!(
            "auto apply policy: {:?}, stored_value: {}, allow_value: {}",
            privacy_setting.auto_apply_policy, stored_value, allow_value
        );
        match privacy_setting.auto_apply_policy {
            AutoApplyPolicy::Always => {
                if stored_value == allow_value {
                    // Silently Grant if the stored value is same as that of allowed value
                    debug!("Silently Granting");
                    Some(Ok(()))
                } else {
                    // Silently Deny if the stored value is inverse of allowed value
                    debug!("Silently Denying");
                    Some(Err(DenyReason::GrantDenied))
                }
            }
            AutoApplyPolicy::Allowed => {
                // Silently Grant if the stored value is same as that of allowed value
                if stored_value == allow_value {
                    debug!("Silently Granting");
                    Some(Ok(()))
                } else {
                    debug!("Cant Silently determine");
                    None
                }
            }
            AutoApplyPolicy::Disallowed => {
                // Silently Deny if the stored value is inverse of allowed value
                if stored_value != allow_value {
                    debug!("Silently Denying");
                    Some(Err(DenyReason::GrantDenied))
                } else {
                    debug!("Cant Silently determine");
                    None
                }
            }
            AutoApplyPolicy::Never => {
                // This is already handled during start of the function.
                // Do nothing using privacy settings get it by evaluating options
                debug!("Cant Silently determine");
                None
            }
        }
    }

    pub async fn update_privacy_settings_with_grant(
        platform_state: &PlatformState,
        privacy_setting: &GrantPrivacySetting,
        grant: bool,
    ) {
        let allow_value_opt =
            Self::get_allow_value(platform_state, privacy_setting.property.as_str());
        let allow_value = allow_value_opt.unwrap_or(true);
        /*
         * We have to update the privacy settings such that it matches x-allow-value when cap is granted
         * and we have to set it to !x-allow-value when the grant is denied.
         *  ┌───────────────┬──────────────┬─────────────────┐
         *  │ X-allow-value │  User Grant  │ Property value  │
         *  ├───────────────┼──────────────┼─────────────────┤
         *  │    True       │   Granted    │    True         │
         *  │    True       │   Denied     │    False        │
         *  │    False      │   Granted    │    False        │
         *  │    False      │   Denied     │    True         │
         *  └───────────────┴──────────────┴─────────────────┘
         */
        let set_value = match (allow_value, grant) {
            (true, true) => true,
            (true, false) => false,
            (false, true) => false,
            (false, false) => true,
        };
        debug!(
            "x-allow-value: {}, grant: {}, set_value: {}",
            allow_value, grant, set_value
        );
        let method_name =
            match Self::get_setter_method_name(platform_state, &privacy_setting.property) {
                Some(method_name) => method_name,
                None => {
                    error!(
                        "Unable to find setter method for property: {}",
                        privacy_setting.property.as_str()
                    );
                    return;
                }
            };
        debug!("Resolved method_name: {}", &method_name);
        let set_request = SetBoolProperty { value: set_value };
        let _res =
            PrivacyImpl::handle_allow_set_requests(&method_name, platform_state, set_request).await;
    }

    pub fn get_setter_method_name(
        platform_state: &PlatformState,
        property: &str,
    ) -> Option<String> {
        let firebolt_rpc_method_opt = platform_state
            .open_rpc_state
            .get_open_rpc()
            .get_setter_method_for_getter(property);
        firebolt_rpc_method_opt.map(|firebolt_openrpc_method| {
            FireboltOpenRpcMethod::name_with_lowercase_module(&firebolt_openrpc_method.name)
        })
    }

    async fn evaluate_options(
        platform_state: &PlatformState,
        // call_ctx: &CallContext,
        caller_session: &CallerSession,
        app_requested_for: &AppIdentification,
        permission: &FireboltPermission,
        policy: &GrantPolicy,
    ) -> Result<(), DenyReasonWithCap> {
        // TODO: is this check duplicated on line 1032?
        platform_state
            .cap_state
            .generic
            .check_all(&vec![permission.clone()])?;

        if policy.options.is_empty() {
            return Ok(());
        }
        let first_supported_option = policy.options.iter().find(|grant_requirements| {
            let step_caps = grant_requirements
                .steps
                .iter()
                .map(|step| FireboltPermission {
                    cap: step.capability_as_fb_cap(),
                    role: CapabilityRole::Use,
                })
                .collect();
            platform_state
                .cap_state
                .generic
                .check_all(&step_caps)
                .is_ok()
        });

        if let Some(first_supported_option) = first_supported_option {
            for step in &first_supported_option.steps {
                if let Err(e) = GrantStepExecutor::execute(
                    step,
                    platform_state,
                    caller_session,
                    app_requested_for,
                    permission,
                )
                .await
                {
                    debug!("grant step execute Err. step={:?} e={:?}", step, e);
                    CapState::emit(
                        platform_state,
                        &CapEvent::OnRevoked,
                        permission.cap.clone(),
                        Some(permission.role),
                    )
                    .await;
                    return Err(e);
                } else {
                    debug!("grant step execute OK. step={:?}", step);
                }
            }

            debug!("all grants ok emitting cap");

            CapState::emit(
                platform_state,
                &CapEvent::OnGranted,
                permission.cap.clone(),
                Some(permission.role),
            )
            .await;

            Ok(())
        } else {
            let unsupported_caps = policy
                .options
                .iter()
                .flat_map(|grant_requirements| {
                    grant_requirements
                        .steps
                        .iter()
                        .map(|step| step.capability_as_fb_cap())
                })
                .collect();
            warn!("Required provider is missing {:?}", unsupported_caps);
            Err(DenyReasonWithCap {
                caps: unsupported_caps,
                reason: DenyReason::GrantProviderMissing,
            })
        }
    }

    async fn execute(
        platform_state: &PlatformState,
        // call_ctx: &CallContext,
        caller_session: &CallerSession,
        app_requested_for: &AppIdentification,
        permission: &FireboltPermission,
        policy: &GrantPolicy,
    ) -> Result<(), DenyReasonWithCap> {
        if let Some(privacy_setting) = &policy.privacy_setting {
            if privacy_setting.auto_apply_policy != AutoApplyPolicy::Never {
                if let Some(priv_sett_response) =
                    Self::evaluate_privacy_settings(platform_state, privacy_setting).await
                {
                    return priv_sett_response.map_err(|err| DenyReasonWithCap {
                        reason: err,
                        caps: vec![permission.cap.clone()],
                    });
                }
            }
        }

        Self::evaluate_options(
            platform_state,
            caller_session,
            app_requested_for,
            permission,
            policy,
        )
        .await
    }

    fn get_app_content_catalog(platform_state: &PlatformState, app_id: &str) -> Option<String> {
        if let Some(app) = platform_state.app_manager_state.get(app_id) {
            return app.initial_session.app.catalog;
        }
        None
    }

    pub async fn apply_grant_exclusion_filters(
        platform_state: &PlatformState,
        app_id: &str,
        catalog: Option<String>,
        permissions: &Vec<FireboltPermission>,
    ) -> Vec<FireboltPermission> {
        // If catalog is None, then get catalog from AppSession
        let grant_exclusion_filters = platform_state
            .get_device_manifest()
            .get_grant_exclusion_filters();

        if grant_exclusion_filters.is_empty() {
            return permissions.clone();
        }

        let mut filtered_permissions = Vec::new();

        for permission in permissions {
            let mut exclude_permission = false;
            for grant_exclusion_filter in &grant_exclusion_filters {
                let id_and_catalog_match = match (
                    app_id,
                    grant_exclusion_filter.id.as_ref(),
                    &catalog,
                    &grant_exclusion_filter.catalog,
                ) {
                    // 1. id & catalog filters are given. check if both filers are matching
                    (id, Some(filter_id), Some(app_catalog), Some(filter_catalog)) => {
                        filter_id.as_str() == id && *filter_catalog == *app_catalog
                    }
                    //
                    // 2. id & catalog filters are given. But app_catalog for this app is not available.
                    // get catalog from AppSession and check if both filers are matching
                    (id, Some(filter_id), None, Some(filter_catalog)) => {
                        if let Some(app_catalog) =
                            Self::get_app_content_catalog(platform_state, app_id)
                        {
                            filter_id.as_str() == id && *filter_catalog == app_catalog
                        } else {
                            false
                        }
                    }
                    //
                    // 3. Only id filter is given. Check if id filter matches
                    (id, Some(filter_id), _, None) => filter_id.as_str() == id,
                    //
                    // 4. Only catalog filter is given. Check if catalog filter matches
                    (_id, None, Some(app_catalog), Some(filter_catalog)) => {
                        *filter_catalog == *app_catalog
                    }
                    //
                    // 5. Only catalog filter is given. But app_catalog for this app is not available.
                    // get catalog from AppSession and check if catalog filter matches
                    (_id, None, None, Some(filter_catalog)) => {
                        if let Some(app_catalog) =
                            Self::get_app_content_catalog(platform_state, app_id)
                        {
                            *filter_catalog == app_catalog
                        } else {
                            false
                        }
                    }
                    _ => false,
                };

                if id_and_catalog_match {
                    if grant_exclusion_filter.capability.is_none() {
                        return Vec::new(); // Return empty vector if filter capability is None
                    }
                    exclude_permission = match (
                        grant_exclusion_filter.capability.as_ref(),
                        permission.cap.as_str(),
                    ) {
                        (Some(filter_capability), permission_capability) => {
                            *filter_capability == permission_capability
                        }
                        _ => false, // No capability filtering in this case
                    };
                    if exclude_permission {
                        // Not required to iterate further, break from the loop
                        break;
                    }
                }
            }

            if !exclude_permission {
                filtered_permissions.push(permission.clone());
            }
        }

        filtered_permissions
    }
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GrantStepExecutor;

impl GrantStepExecutor {
    pub async fn execute(
        step: &GrantStep,
        platform_state: &PlatformState,
        // call_ctx: &CallContext,
        caller_session: &CallerSession,
        app_requested_for: &AppIdentification,
        permission: &FireboltPermission,
    ) -> Result<(), DenyReasonWithCap> {
        let capability = step.capability.clone();
        let configuration = step.configuration.clone();
        debug!(
            "Reached execute phase of step for capability: {}",
            capability
        );

        let firebolt_cap = FireboltCap::Full(capability.to_owned());
        if let Err(e) = platform_state
            .cap_state
            .generic
            .check_all(&vec![FireboltPermission {
                cap: firebolt_cap.clone(),
                role: CapabilityRole::Use,
            }])
        {
            return Err(DenyReasonWithCap {
                reason: DenyReason::GrantProviderMissing,
                caps: e.caps,
            });
        }
        debug!("Cap is supported and available, invoking capability");

        Self::invoke_capability(
            platform_state,
            // call_ctx,
            caller_session,
            app_requested_for,
            &firebolt_cap,
            &configuration,
            permission,
        )
        .await
    }

    async fn get_app_name(platform_state: &PlatformState, app_id: String) -> String {
        let mut app_name: String = Default::default();
        let (tx, rx) = oneshot::channel::<AppResponse>();
        let app_request = AppRequest::new(AppMethod::GetAppName(app_id), tx);
        let send_result = platform_state.get_client().send_app_request(app_request);
        if send_result.is_err() {
            return app_name;
        }
        if let Ok(Ok(AppManagerResponse::AppName(name))) = rx.await {
            app_name = name.unwrap_or_default();
        }

        app_name
    }

    pub async fn invoke_capability(
        platform_state: &PlatformState,
        caller_session: &CallerSession,
        app_requested_for: &AppIdentification,
        cap: &FireboltCap,
        param: &Option<Value>,
        permission: &FireboltPermission,
    ) -> Result<(), DenyReasonWithCap> {
        let (session_tx, session_rx) = oneshot::channel::<ProviderResponsePayload>();
        let p_cap = cap.clone();

        let mut method_key: Option<String> = None;

        for (key, provider_relation_set) in platform_state
            .open_rpc_state
            .get_provider_relation_map()
            .iter()
        {
            if let Some(provides) = &provider_relation_set.provides {
                if provides.eq(&cap.as_str()) {
                    method_key = Some(key.clone());
                    break;
                }
            }
        }

        if method_key.is_none() {
            error!(
                "invoke_capability: Could not find provider for capability {}",
                p_cap.as_str()
            );
            return Err(DenyReasonWithCap {
                reason: DenyReason::Ungranted,
                caps: vec![permission.cap.clone()],
            });
        }

        let method = method_key.unwrap();

        /*
         * this might be weird looking as_str().as_str(), FireboltCap returns String but has a function named as_str.
         * We call as_str on String to convert String to str to perform our match
         */
        let for_app_id = &app_requested_for.app_id;
        let app_name = Self::get_app_name(platform_state, for_app_id.clone()).await;
        let pr_msg_opt = match p_cap.as_str().as_str() {
            "xrn:firebolt:capability:usergrant:acknowledgechallenge" => {
                let challenge = Challenge {
                    capability: permission.cap.as_str(),
                    requestor: ChallengeRequestor {
                        id: for_app_id.clone(),
                        name: app_name,
                    },
                };
                Some(ProviderBrokerRequest {
                    capability: p_cap.as_str(),
                    method,
                    caller: caller_session.to_owned(),
                    request: ProviderRequestPayload::AckChallenge(challenge),
                    tx: session_tx,
                    app_id: None,
                })
            }
            "xrn:firebolt:capability:usergrant:pinchallenge" => {
                let pin_space_res = serde_json::from_value::<PinChallengeConfiguration>(
                    param.as_ref().unwrap_or(&Value::Null).clone(),
                );
                if pin_space_res.is_err() {
                    error!("Missing pin space for {}", permission.cap.as_str());
                }
                pin_space_res.map_or(None, |pin_conf| {
                    let challenge = PinChallengeRequest {
                        pin_space: pin_conf.pin_space,
                        requestor: ChallengeRequestor {
                            id: for_app_id.clone(),
                            name: app_name,
                        },
                        capability: Some(permission.cap.as_str()),
                    };
                    Some(ProviderBrokerRequest {
                        capability: p_cap.as_str(),
                        method,
                        caller: caller_session.clone(),
                        request: ProviderRequestPayload::PinChallenge(challenge),
                        tx: session_tx,
                        app_id: None,
                    })
                })
            }
            _ => {
                /*
                 * This is for any other capability, hoping it to deduce its necessary params from a json string
                 * and has a challenge method.
                 */
                Some(ProviderBrokerRequest {
                    capability: p_cap.as_str(),
                    method,
                    caller: caller_session.clone(),
                    request: ProviderRequestPayload::Generic(param.clone().unwrap_or(Value::Null)),
                    tx: session_tx,
                    app_id: None,
                })
            }
        };

        let result = if let Some(pr_msg) = pr_msg_opt {
            ProviderBroker::invoke_method(&platform_state.clone(), pr_msg).await;
            match session_rx.await {
                Ok(result) => match result.as_challenge_response() {
                    Some(res) => {
                        match res.granted {
                            Some(true) => {
                                debug!("returning ok from invoke_capability");
                                Ok(())
                            }
                            Some(false) => {
                                debug!("returning err from invoke_capability");
                                Err(DenyReason::GrantDenied)
                            }
                            None => {
                                debug!("Challenge left unanswered. Returning err from invoke_capability");
                                Err(DenyReason::Ungranted)
                            }
                        }
                    }
                    None => {
                        debug!("Received reponse that is not convertable to challenge response");
                        Err(DenyReason::Ungranted)
                    }
                },
                Err(_) => {
                    debug!("Receive error in channel");
                    Err(DenyReason::Ungranted)
                }
            }
        } else {
            /*
             * We would reach here if the cap is ack or pin
             * and we are not able to parse the configuration in the manifest
             * as pinchallenge or ackchallenge.
             */
            Err(DenyReason::Ungranted)
        };

        if let Err(reason) = result {
            Err(DenyReasonWithCap {
                reason,
                caps: vec![permission.cap.clone()],
            })
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    mod test_grant_policy_enforcer {
        use super::*;
        use crate::{
            state::session_state::Session,
            utils::test_utils::{fb_perm, MockRuntime},
        };
        use ripple_sdk::{
            api::{
                apps::EffectiveTransport,
                device::device_user_grants_data::GrantRequirements,
                firebolt::{
                    fb_general::ListenRequest,
                    fb_pin::{
                        PinChallengeResponse, PinChallengeResultReason, PIN_CHALLENGE_CAPABILITY,
                        PIN_CHALLENGE_EVENT,
                    },
                    provider::{
                        ChallengeResponse, ExternalProviderRequest, ProviderResponse,
                        ACK_CHALLENGE_CAPABILITY, ACK_CHALLENGE_EVENT,
                    },
                },
                gateway::rpc_gateway_api::CallContext,
            },
            tokio,
            utils::logger::init_logger,
        };
        use serde_json::json;
        struct ProviderApp;

        impl ProviderApp {
            pub async fn start(
                state: PlatformState,
                ctx: CallContext,
                pin_response: PinChallengeResponse,
                ack_response: ChallengeResponse,
            ) {
                let (tx, mut rx) = tokio::sync::mpsc::channel(32);
                let sample_app_session =
                    Session::new("app_id".to_owned(), Some(tx), EffectiveTransport::Websocket);
                let state_c = state.clone();
                let ctx_c = ctx.clone();
                state
                    .session_state
                    .add_session(ctx.session_id.clone(), sample_app_session);

                ProviderBroker::register_or_unregister_provider(
                    &state_c,
                    String::from(PIN_CHALLENGE_CAPABILITY),
                    String::from(PIN_CHALLENGE_EVENT),
                    String::from(PIN_CHALLENGE_EVENT),
                    ctx_c.clone(),
                    ListenRequest { listen: true },
                )
                .await;

                ProviderBroker::register_or_unregister_provider(
                    &state_c,
                    String::from(ACK_CHALLENGE_CAPABILITY),
                    String::from(ACK_CHALLENGE_EVENT),
                    String::from(ACK_CHALLENGE_EVENT),
                    ctx_c.clone(),
                    ListenRequest { listen: true },
                )
                .await;

                let platform_state = state.clone();

                tokio::spawn(async move {
                    while let Some(message) = rx.recv().await {
                        let json_msg = serde_json::from_str::<Value>(&message.jsonrpc_msg).unwrap();
                        let msg = json_msg.get("result").cloned().unwrap();
                        let req = serde_json::from_value::<
                            ExternalProviderRequest<PinChallengeRequest>,
                        >(msg.clone());
                        if let Ok(prov_req) = req {
                            let corr_id = prov_req.correlation_id.clone();
                            let msg = ProviderResponse {
                                correlation_id: corr_id,
                                result: ProviderResponsePayload::PinChallengeResponse(
                                    pin_response.clone(),
                                ),
                            };
                            ProviderBroker::provider_response(&platform_state, msg).await;
                        } else {
                            let req =
                                serde_json::from_value::<ExternalProviderRequest<Challenge>>(msg);
                            if let Ok(prov_req) = req {
                                let corr_id = prov_req.correlation_id.clone();
                                let msg = ProviderResponse {
                                    correlation_id: corr_id,
                                    result: ProviderResponsePayload::ChallengeResponse(
                                        ack_response.clone(),
                                    ),
                                };
                                ProviderBroker::provider_response(&platform_state, msg).await;
                            }
                        }
                    }
                });
            }
        }

        fn setup(
            policy_options: Vec<GrantRequirements>,
        ) -> (PlatformState, CallContext, FireboltPermission, GrantPolicy) {
            let _ = init_logger("tests".into());
            let runtime = MockRuntime::new();
            let perm = fb_perm(
                "xrn:firebolt:capability:localization:postal-code",
                Some(CapabilityRole::Use),
            );
            let policy = GrantPolicy {
                options: policy_options,
                ..Default::default()
            };
            let ctx = runtime.call_context;
            let mut platform_state = runtime.platform_state;
            let caps = vec![
                FireboltCap::Short("input:keyboard".to_owned()),
                FireboltCap::Short("token:account".to_owned()),
                FireboltCap::Short("usergrant:acknowledgechallenge".to_owned()),
                FireboltCap::Short("usergrant:pinchallenge".to_owned()),
            ];
            platform_state
                .cap_state
                .generic
                .ingest_availability(caps, true);

            let mut permissions = HashMap::new();
            permissions.insert(ctx.app_id.to_owned(), vec![perm.clone()]);
            platform_state
                .cap_state
                .permitted_state
                .set_permissions(permissions);

            (platform_state, ctx, perm, policy)
        }

        #[tokio::test]
        async fn test_evaluate_options_no_options() {
            let (state, ctx, perm, policy) = setup(vec![]);
            let caller_session: CallerSession = ctx.clone().into();
            let app_identifier: AppIdentification = ctx.clone().into();

            let result = GrantPolicyEnforcer::evaluate_options(
                &state,
                &caller_session,
                &app_identifier,
                &perm,
                &policy,
            )
            .await;

            assert!(result.is_ok());
        }

        #[tokio::test]
        async fn test_evaluate_options_no_steps_for_option() {
            let (state, ctx, perm, policy) = setup(vec![GrantRequirements { steps: vec![] }]);
            let caller_session: CallerSession = ctx.clone().into();
            let app_identifier: AppIdentification = ctx.clone().into();

            let result = GrantPolicyEnforcer::evaluate_options(
                &state,
                &caller_session,
                &app_identifier,
                &perm,
                &policy,
            )
            .await;

            assert!(result.is_ok());
        }

        #[tokio::test]
        async fn test_evaluate_options_fb_perm_denied() {
            let (state, ctx, _, policy) = setup(vec![GrantRequirements { steps: vec![] }]);
            let caller_session: CallerSession = ctx.clone().into();
            let app_identifier: AppIdentification = ctx.clone().into();
            let perm = FireboltPermission {
                cap: FireboltCap::Full("xrn:firebolt:capability:something:unknown".to_owned()),
                role: CapabilityRole::Use,
            };

            let result = GrantPolicyEnforcer::evaluate_options(
                &state,
                &caller_session,
                &app_identifier,
                &perm,
                &policy,
            )
            .await;

            assert!(result.is_err());
            assert_eq!(
                result.err().unwrap(),
                DenyReasonWithCap {
                    reason: DenyReason::Unsupported,
                    caps: vec![perm.cap.clone()]
                }
            );
        }

        #[tokio::test]
        async fn test_evaluate_options_first_option_suitable() {
            let (state, ctx, perm, policy) = setup(vec![
                GrantRequirements {
                    steps: vec![GrantStep {
                        capability: PIN_CHALLENGE_CAPABILITY.to_owned(),
                        configuration: Some(json!({ "pinSpace": "purchase" })),
                    }],
                },
                GrantRequirements {
                    steps: vec![GrantStep {
                        capability: "xrn:firebolt:capability:usergrant:notavailableonplatform"
                            .to_owned(),
                        configuration: None,
                    }],
                },
            ]);
            let caller_session: CallerSession = CallerSession::default();
            let app_identifier: AppIdentification = ctx.clone().into();
            // let pinchallenge_response = state
            //     .provider_broker_state
            //     .send_pinchallenge_success(&state, &ctx);
            ProviderApp::start(
                state.clone(),
                ctx.clone(),
                PinChallengeResponse {
                    granted: Some(true),
                    reason: PinChallengeResultReason::CorrectPin,
                },
                ChallengeResponse {
                    granted: Some(true),
                },
            )
            .await;

            let evaluate_options = GrantPolicyEnforcer::evaluate_options(
                &state,
                &caller_session,
                &app_identifier,
                &perm,
                &policy,
            )
            .await;
            // let (result, _) = join!(evaluate_options, pinchallenge_response);

            assert!(evaluate_options.is_ok());
        }

        #[tokio::test]
        async fn test_evaluate_options_second_option_suitable() {
            let (state, ctx, perm, policy) = setup(vec![
                GrantRequirements {
                    steps: vec![GrantStep {
                        capability: "xrn:firebolt:capability:usergrant:notavailableonplatform"
                            .to_owned(),
                        configuration: None,
                    }],
                },
                GrantRequirements {
                    steps: vec![GrantStep {
                        capability: PIN_CHALLENGE_CAPABILITY.to_owned(),
                        configuration: Some(json!({ "pinSpace": "purchase" })),
                    }],
                },
            ]);
            ProviderApp::start(
                state.clone(),
                ctx.clone(),
                PinChallengeResponse {
                    granted: Some(true),
                    reason: PinChallengeResultReason::CorrectPin,
                },
                ChallengeResponse {
                    granted: Some(true),
                },
            )
            .await;
            let caller_session: CallerSession = CallerSession::default();
            let app_identifier: AppIdentification = ctx.clone().into();
            // let pinchallenge_response = state
            //     .provider_broker_state
            //     .send_pinchallenge_success(&state, &ctx);

            let evaluate_options = GrantPolicyEnforcer::evaluate_options(
                &state,
                &caller_session,
                &app_identifier,
                &perm,
                &policy,
            )
            .await;
            // let (result, _) = join!(evaluate_options, pinchallenge_response);

            assert!(evaluate_options.is_ok());
        }

        #[tokio::test]
        async fn test_evaluate_options_no_options_supported() {
            let (state, ctx, perm, policy) = setup(vec![
                GrantRequirements {
                    steps: vec![GrantStep {
                        capability: "xrn:firebolt:capability:usergrant:notavailableonplatform"
                            .to_owned(),
                        configuration: None,
                    }],
                },
                GrantRequirements {
                    steps: vec![
                        GrantStep {
                            capability: "xrn:firebolt:capability:usergrant:notavailableonplatform"
                                .to_owned(),
                            configuration: None,
                        },
                        GrantStep {
                            capability: ACK_CHALLENGE_CAPABILITY.to_owned(),
                            configuration: None,
                        },
                    ],
                },
            ]);
            let caller_session: CallerSession = ctx.clone().into();
            let app_identifier: AppIdentification = ctx.clone().into();

            let result = GrantPolicyEnforcer::evaluate_options(
                &state,
                &caller_session,
                &app_identifier,
                &perm,
                &policy,
            )
            .await;

            assert!(result.is_err());
            assert_eq!(
                result.err().unwrap(),
                DenyReasonWithCap {
                    reason: DenyReason::GrantProviderMissing,
                    caps: vec![
                        FireboltCap::Full(
                            "xrn:firebolt:capability:usergrant:notavailableonplatform".to_owned()
                        ),
                        FireboltCap::Full(
                            "xrn:firebolt:capability:usergrant:notavailableonplatform".to_owned()
                        ),
                        FireboltCap::Full(ACK_CHALLENGE_CAPABILITY.to_owned())
                    ]
                }
            );
        }

        #[tokio::test]
        async fn test_evaluate_options_first_step_denied() {
            let (state, ctx, perm, policy) = setup(vec![GrantRequirements {
                steps: vec![
                    GrantStep {
                        capability: PIN_CHALLENGE_CAPABILITY.to_owned(),
                        configuration: Some(json!({ "pinSpace": "purchase" })),
                    },
                    GrantStep {
                        capability: ACK_CHALLENGE_CAPABILITY.to_owned(),
                        configuration: None,
                    },
                ],
            }]);
            let caller_session: CallerSession = CallerSession::default();
            let app_identifier: AppIdentification = ctx.clone().into();
            // let challenge_responses = state.provider_broker_state.send_pinchallenge_failure(
            //     &state,
            //     &ctx,
            //     PinChallengeResultReason::ExceededPinFailures,
            // );
            ProviderApp::start(
                state.clone(),
                ctx.clone(),
                PinChallengeResponse {
                    granted: Some(false),
                    reason: PinChallengeResultReason::ExceededPinFailures,
                },
                ChallengeResponse {
                    granted: Some(true),
                },
            )
            .await;

            let result = GrantPolicyEnforcer::evaluate_options(
                &state,
                &caller_session,
                &app_identifier,
                &perm,
                &policy,
            )
            .await;
            // let (result, _) = join!(evaluate_options, challenge_responses);

            assert!(result.is_err());
            assert_eq!(
                result.err().unwrap(),
                DenyReasonWithCap {
                    reason: DenyReason::GrantDenied,
                    caps: vec![FireboltCap::Full(
                        "xrn:firebolt:capability:localization:postal-code".to_owned()
                    )]
                }
            );
        }

        #[tokio::test]
        async fn test_evaluate_options_second_step_denied() {
            let (state, ctx, perm, policy) = setup(vec![GrantRequirements {
                steps: vec![
                    GrantStep {
                        capability: PIN_CHALLENGE_CAPABILITY.to_owned(),
                        configuration: Some(json!({ "pinSpace": "purchase" })),
                    },
                    GrantStep {
                        capability: ACK_CHALLENGE_CAPABILITY.to_owned(),
                        configuration: None,
                    },
                ],
            }]);
            let caller_session: CallerSession = CallerSession::default();
            let app_identifier: AppIdentification = ctx.clone().into();
            // let challenge_responses = state
            //     .provider_broker_state
            //     .send_pinchallenge_success(&state, &ctx)
            //     .then(|_| async {
            //         // TODO: workout how to do this without sleep
            //         time::sleep(Duration::new(1, 0)).await;
            //         state
            //             .provider_broker_state
            //             .send_ackchallenge_failure(&state, &ctx)
            //             .await;
            //     });
            ProviderApp::start(
                state.clone(),
                ctx.clone(),
                PinChallengeResponse {
                    granted: Some(true),
                    reason: PinChallengeResultReason::CorrectPin,
                },
                ChallengeResponse {
                    granted: Some(false),
                },
            )
            .await;

            let result = GrantPolicyEnforcer::evaluate_options(
                &state,
                &caller_session,
                &app_identifier,
                &perm,
                &policy,
            )
            .await;

            // let (result, _) = join!(evaluate_options, challenge_responses);

            assert!(result.is_err());
            assert_eq!(
                result.err().unwrap(),
                DenyReasonWithCap {
                    reason: DenyReason::GrantDenied,
                    caps: vec![FireboltCap::Full(
                        "xrn:firebolt:capability:localization:postal-code".to_owned()
                    )]
                }
            );
        }

        #[tokio::test]
        pub async fn test_evaluate_options_all_steps_granted() {
            let (state, ctx, perm, policy) = setup(vec![GrantRequirements {
                steps: vec![
                    GrantStep {
                        capability: PIN_CHALLENGE_CAPABILITY.to_owned(),
                        configuration: Some(json!({ "pinSpace": "purchase" })),
                    },
                    GrantStep {
                        capability: ACK_CHALLENGE_CAPABILITY.to_owned(),
                        configuration: None,
                    },
                ],
            }]);
            ProviderApp::start(
                state.clone(),
                ctx.clone(),
                PinChallengeResponse {
                    granted: Some(true),
                    reason: PinChallengeResultReason::CorrectPin,
                },
                ChallengeResponse {
                    granted: Some(true),
                },
            )
            .await;

            // state
            //     .session_state
            //     .add_session("app_id".to_owned(), sample_app_session);

            let caller_session: CallerSession = CallerSession::default();
            let app_identifier: AppIdentification = ctx.clone().into();
            // let challenge_responses = state
            //     .provider_broker_state
            //     .send_pinchallenge_success(&state, &ctx)
            //     .then(|_| async {
            //         // TODO: workout how to do this without sleep
            //         time::sleep(Duration::new(1, 0)).await;
            //         state
            //             .provider_broker_state
            //             .send_ackchallenge_success(&state, &ctx)
            //             .await;
            //     });

            let evaluate_options = GrantPolicyEnforcer::evaluate_options(
                &state,
                &caller_session,
                &app_identifier,
                &perm,
                &policy,
            )
            .await;
            debug!("result of evaluate option: {:?}", evaluate_options);

            assert!(evaluate_options.is_ok());
        }
    }
}
