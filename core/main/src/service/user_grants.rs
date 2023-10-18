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
    firebolt::handlers::privacy_rpc::PrivacyImpl,
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

    pub fn delete_expired_entries_for_app(
        &self,
        app_id: String, // None is for device
    ) -> bool {
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

    pub fn delete_all_expired_entries(&self) -> bool {
        let mut deleted = false;
        let mut grant_state = self.grant_app_map.write().unwrap();
        for (_, entries) in grant_state.value.iter_mut() {
            let prev_len = entries.len();
            entries.retain(|entry| !entry.has_expired());
            if entries.len() < prev_len {
                deleted = true;
            }
        }
        grant_state.sync();
        deleted
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
    ) -> GrantActiveState {
        if let Some(status) = self.get_grant_status(app_id, permission) {
            GrantActiveState::ActiveGrant(status.into())
        } else {
            GrantActiveState::PendingGrant
        }
    }

    pub async fn check_with_roles(
        state: &PlatformState,
        caller_session: &CallerSession,
        app_requested_for: &AppIdentification,
        fb_perms: &[FireboltPermission],
        fail_on_first_error: bool,
    ) -> Result<(), DenyReasonWithCap> {
        /*
         * Instead of just checking for grants previously, if the user grants are not present,
         * we are taking necessary steps to get the user grant and send back the result.
         */
        let grant_state = state.clone().cap_state.grant_state;
        let app_id = app_requested_for.app_id.to_owned();
        let caps_needing_grants = grant_state.caps_needing_grants.clone();
        let caps_needing_grant_in_request: Vec<FireboltPermission> = fb_perms
            .iter()
            .cloned()
            .filter(|x| caps_needing_grants.contains(&x.cap.as_str()))
            .collect();
        let mut denied_caps = Vec::new();
        for permission in caps_needing_grant_in_request {
            let result = grant_state.get_grant_state(&app_id, &permission);
            match result {
                GrantActiveState::ActiveGrant(grant) => {
                    if grant.is_err() {
                        return Err(DenyReasonWithCap {
                            reason: grant.err().unwrap(),
                            caps: vec![permission.cap.clone()],
                        });
                    }
                }
                GrantActiveState::PendingGrant => {
                    let result = GrantPolicyEnforcer::determine_grant_policies_for_permission(
                        state,
                        caller_session,
                        app_requested_for,
                        &permission,
                    )
                    .await;
                    if result.is_err() {
                        if fail_on_first_error {
                            return result;
                        } else {
                            denied_caps.push(permission.cap.clone())
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

    pub fn check_granted(&self, app_id: &str, role_info: RoleInfo) -> Result<bool, RippleError> {
        if !self.caps_needing_grants.contains(&role_info.capability) {
            return Ok(true);
        }

        if let Ok(permission) = FireboltPermission::try_from(role_info) {
            let result = self.get_grant_state(app_id, &permission);

            match result {
                GrantActiveState::ActiveGrant(grant) => {
                    if grant.is_err() {
                        return Err(RippleError::Permission(DenyReason::GrantDenied));
                    } else {
                        return Ok(true);
                    }
                }
                GrantActiveState::PendingGrant => {
                    return Err(RippleError::Permission(DenyReason::Ungranted));
                }
            }
        }
        Err(RippleError::Permission(DenyReason::Ungranted))
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
    // Pass None for device scope
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
                            &grant_policy,
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
}

pub struct GrantPolicyEnforcer;

impl GrantPolicyEnforcer {
    pub async fn send_usergrants_for_cloud_storage(
        platform_state: &PlatformState,
        grant_policy: &GrantPolicy,
        grant_entry: &GrantEntry,
        app_id: &Option<String>,
    ) {
        if let Some(account_session) = platform_state.session_state.get_account_session() {
            let mut s = None;
            if grant_entry.status.is_some() {
                s = Some(grant_entry.status.to_owned().unwrap());
            };

            let usergrants_cloud_set_params = UserGrantsCloudSetParams {
                account_session,
                user_grant_info: UserGrantInfo {
                    role: grant_entry.role,
                    capability: grant_entry.capability.to_owned(),
                    status: s,
                    last_modified_time: Duration::new(0, 0),
                    expiry_time: match grant_policy.lifespan {
                        GrantLifespan::Seconds => {
                            let now = SystemTime::now().duration_since(UNIX_EPOCH);
                            if now.is_err() {
                                error!("Unable to sync usergrants to cloud. unable to get duration since epoch");
                                return;
                            }
                            let now_dur = now
                                .unwrap()
                                .checked_add(Duration::new(grant_policy.lifespan_ttl.unwrap(), 0));
                            if now_dur.is_none() {
                                error!("Unable to sync usergrants to cloud. unable to get duration since epoch for lifespan_ttl");
                                return;
                            }
                            Some(now_dur.unwrap())
                        }
                        _ => None,
                    },
                    app_name: app_id.to_owned(),
                    lifespan: grant_policy.lifespan.to_owned(),
                },
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
                grant_policy,
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
        if grant_policy.privacy_setting.is_some()
            && grant_policy
                .privacy_setting
                .as_ref()
                .unwrap()
                .update_property
        {
            Self::update_privacy_settings_with_grant(
                platform_state,
                grant_policy.privacy_setting.as_ref().unwrap(),
                result.is_ok(),
            )
            .await;
        }
        Self::store_user_grants(platform_state, permission, result, app_id, grant_policy).await;
    }

    pub async fn determine_grant_policies_for_permission(
        platform_state: &PlatformState,
        // call_context: &CallContext,
        caller_session: &CallerSession,
        app_requested_for: &AppIdentification,
        permission: &FireboltPermission,
    ) -> Result<(), DenyReasonWithCap> {
        let grant_policy_opt = platform_state.get_device_manifest().get_grant_policies();
        if grant_policy_opt.is_none() {
            debug!("There are no grant policies for the requesting cap so bailing out");
            return Ok(());
        }
        let grant_policies_map = grant_policy_opt.unwrap();
        let grant_policies_opt = grant_policies_map.get(&permission.cap.as_str());
        if grant_policies_opt.is_none() {
            debug!(
                "There are no policies for the cap: {} so granting request",
                permission.cap.as_str()
            );
            return Ok(());
        }
        let policies = grant_policies_opt.unwrap();
        let policy_opt = policies.get_policy(permission);
        if policy_opt.is_none() {
            debug!(
                "There are no polices for cap: {} for role: {:?}",
                permission.cap.as_str(),
                permission.role
            );
            return Ok(());
        }
        let policy = policy_opt.unwrap();
        if !Self::is_policy_valid(platform_state, &policy) {
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
            debug!("No grant policies configured");
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
        let allow_value_opt =
            Self::get_allow_value(platform_state, privacy_setting.property.as_str());
        if allow_value_opt.is_none() {
            debug!(
                "Allow value not present for property: {}",
                privacy_setting.property.as_str()
            );
            return None;
        }
        let allow_value = allow_value_opt.unwrap();
        // From privacyImpl make the call to the registered method for the configured privacy settings.
        let res_stored_value =
            PrivacyImpl::handle_allow_get_requests(&privacy_setting.property, platform_state).await;
        if res_stored_value.is_err() {
            debug!(
                "Unable to get stored value for privacy settings: {}",
                privacy_setting.property.as_str()
            );
            return None;
        }
        let stored_value = res_stored_value.unwrap();
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
        let method_name = Self::get_setter_method_name(platform_state, &privacy_setting.property);
        if method_name.is_none() {
            return;
        }
        let method_name = method_name.unwrap();
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
                    debug!("grant step execute Err. step={:?}", step);
                    CapState::emit(
                        platform_state,
                        CapEvent::OnRevoked,
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
                CapEvent::OnGranted,
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
            warn!(
                "capabilities are not supported on this device. {:?}",
                unsupported_caps
            );
            Err(DenyReasonWithCap {
                caps: unsupported_caps,
                reason: DenyReason::Unsupported,
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
        if policy.privacy_setting.is_some()
            && policy.privacy_setting.as_ref().unwrap().auto_apply_policy != AutoApplyPolicy::Never
        {
            if let Some(priv_sett_response) = Self::evaluate_privacy_settings(
                platform_state,
                policy.privacy_setting.as_ref().unwrap(),
            )
            .await
            {
                return priv_sett_response.map_err(|err| DenyReasonWithCap {
                    reason: err,
                    caps: vec![permission.cap.clone()],
                });
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
                .get_app_state(app_id.as_ref().unwrap().to_string())
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
                        reason: DenyReason::Unavailable,
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
                reason: DenyReason::GrantDenied,
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
        // call_ctx: &CallContext,
        caller_session: &CallerSession,
        app_requested_for: &AppIdentification,
        cap: &FireboltCap,
        param: &Option<Value>,
        permission: &FireboltPermission,
    ) -> Result<(), DenyReasonWithCap> {
        let (session_tx, session_rx) = oneshot::channel::<ProviderResponsePayload>();
        let p_cap = cap.clone();
        /*
         * We have a concrete struct defined for ack challenge and pin challenge hence handling them separately. If any new
         * caps are introduced in future, the assumption is that capability provider has a method "challenge" and it can
         * deduce its params from a string.
         */

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
                    method: String::from("challenge"),
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
                pin_space_res.map_or(None, |pin_conf| {
                    Some(ProviderBrokerRequest {
                        capability: p_cap.as_str(),
                        method: "challenge".to_owned(),
                        caller: caller_session.clone(),
                        request: ProviderRequestPayload::PinChallenge(PinChallengeRequest {
                            pin_space: pin_conf.pin_space,
                            requestor: ChallengeRequestor {
                                id: for_app_id.clone(),
                                name: app_name,
                            },
                            capability: Some(p_cap.as_str()),
                        }),
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
                let param_str = match param {
                    None => "".to_owned(),
                    Some(val) => val.to_string(),
                };
                Some(ProviderBrokerRequest {
                    capability: p_cap.as_str(),
                    method: String::from("challenge"),
                    caller: caller_session.clone(),
                    request: ProviderRequestPayload::Generic(param_str),
                    tx: session_tx,
                    app_id: None,
                })
            }
        };
        let result = if let Some(pr_msg) = pr_msg_opt {
            ProviderBroker::invoke_method(&platform_state.clone(), pr_msg).await;
            match session_rx.await {
                Ok(result) => match result.as_challenge_response() {
                    Some(res) => match res.granted {
                        Some(true) => {
                            debug!("returning ok from invoke_capability");
                            Ok(())
                        }
                        Some(false) => {
                            debug!("returning err from invoke_capability");
                            Err(DenyReason::GrantDenied)
                        }
                        None => {
                            debug!("returning err from invoke_capability");
                            Err(DenyReason::Ungranted)
                        }
                    },
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
                caps: vec![cap.clone()],
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
        use crate::utils::test_utils::{fb_perm, MockRuntime};
        use futures::FutureExt;
        use ripple_sdk::{
            api::{
                device::device_user_grants_data::GrantRequirements,
                firebolt::{
                    fb_pin::{PinChallengeResultReason, PIN_CHALLENGE_CAPABILITY},
                    provider::ACK_CHALLENGE_CAPABILITY,
                },
                gateway::rpc_gateway_api::CallContext,
            },
            tokio::{
                self, join,
                time::{self, Duration},
            },
            utils::logger::init_logger,
        };
        use serde_json::json;

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
            println!("result: {:?}", result);

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
            let pinchallenge_response = state
                .provider_broker_state
                .send_pinchallenge_success(&state, &ctx);

            let evaluate_options = GrantPolicyEnforcer::evaluate_options(
                &state,
                &caller_session,
                &app_identifier,
                &perm,
                &policy,
            );
            let (result, _) = join!(evaluate_options, pinchallenge_response);

            assert!(result.is_ok());
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
            let caller_session: CallerSession = CallerSession::default();
            let app_identifier: AppIdentification = ctx.clone().into();
            let pinchallenge_response = state
                .provider_broker_state
                .send_pinchallenge_success(&state, &ctx);

            let evaluate_options = GrantPolicyEnforcer::evaluate_options(
                &state,
                &caller_session,
                &app_identifier,
                &perm,
                &policy,
            );
            let (result, _) = join!(evaluate_options, pinchallenge_response);

            assert!(result.is_ok());
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
                    reason: DenyReason::Unsupported,
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
            let challenge_responses = state.provider_broker_state.send_pinchallenge_failure(
                &state,
                &ctx,
                PinChallengeResultReason::ExceededPinFailures,
            );

            let evaluate_options = GrantPolicyEnforcer::evaluate_options(
                &state,
                &caller_session,
                &app_identifier,
                &perm,
                &policy,
            );
            let (result, _) = join!(evaluate_options, challenge_responses);

            assert!(result.is_err());
            assert_eq!(
                result.err().unwrap(),
                DenyReasonWithCap {
                    reason: DenyReason::GrantDenied,
                    caps: vec![FireboltCap::Full(PIN_CHALLENGE_CAPABILITY.to_owned())]
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
            let challenge_responses = state
                .provider_broker_state
                .send_pinchallenge_success(&state, &ctx)
                .then(|_| async {
                    // TODO: workout how to do this without sleep
                    time::sleep(Duration::new(1, 0)).await;
                    state
                        .provider_broker_state
                        .send_ackchallenge_failure(&state, &ctx)
                        .await;
                });

            let evaluate_options = GrantPolicyEnforcer::evaluate_options(
                &state,
                &caller_session,
                &app_identifier,
                &perm,
                &policy,
            );

            let (result, _) = join!(evaluate_options, challenge_responses);

            assert!(result.is_err());
            assert_eq!(
                result.err().unwrap(),
                DenyReasonWithCap {
                    reason: DenyReason::GrantDenied,
                    caps: vec![FireboltCap::Full(ACK_CHALLENGE_CAPABILITY.to_owned())]
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
            let caller_session: CallerSession = CallerSession::default();
            let app_identifier: AppIdentification = ctx.clone().into();
            let challenge_responses = state
                .provider_broker_state
                .send_pinchallenge_success(&state, &ctx)
                .then(|_| async {
                    // TODO: workout how to do this without sleep
                    time::sleep(Duration::new(1, 0)).await;
                    state
                        .provider_broker_state
                        .send_ackchallenge_success(&state, &ctx)
                        .await;
                });

            let evaluate_options = GrantPolicyEnforcer::evaluate_options(
                &state,
                &caller_session,
                &app_identifier,
                &perm,
                &policy,
            );
            let (result, _) = join!(evaluate_options, challenge_responses);

            assert!(result.is_ok());
        }
    }
}
