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

use jsonrpsee::tracing::debug;
use ripple_sdk::{
    api::{
        apps::{AppManagerResponse, AppMethod, AppRequest, AppResponse},
        device::device_user_grants_data::{
            GrantLifespan, GrantPolicy, GrantPrivacySetting, GrantScope, GrantStep,
        },
        firebolt::{
            fb_capabilities::{
                CapEvent, CapabilityRole, DenyReason, DenyReasonWithCap, FireboltCap,
                FireboltPermission,
            },
            fb_openrpc::CapabilitySet,
            fb_pin::{PinChallengeConfiguration, PinChallengeRequest},
            provider::{
                Challenge, ChallengeRequestor, ProviderRequestPayload, ProviderResponsePayload,
            },
        },
        gateway::rpc_gateway_api::CallContext,
        manifest::device_manifest::DeviceManifest,
    },
    framework::file_store::FileStore,
    serde_json::Value,
    tokio::sync::oneshot,
};
use serde::{Deserialize, Serialize};

use crate::state::{cap::cap_state::CapState, platform_state::PlatformState};

use super::apps::provider_broker::{ProviderBroker, ProviderBrokerRequest};

pub struct UserGrants {}

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

#[derive(Debug, Clone)]
pub struct GrantState {
    device_grants: Arc<RwLock<FileStore<HashSet<GrantEntry>>>>,
    grant_app_map: Arc<RwLock<FileStore<HashMap<String, HashSet<GrantEntry>>>>>,
    caps_needing_grants: Vec<String>,
}

#[derive(Eq, Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum GrantStateModify {
    Grant,
    Deny,
    Clear,
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

impl GrantState {
    pub fn new(manifest: DeviceManifest) -> GrantState {
        let saved_dir = manifest.clone().configuration.saved_dir;
        let device_grant_path = format!("{}device_grants", saved_dir);
        let dev_grant_store = if let Ok(v) = FileStore::load(device_grant_path.clone()) {
            v
        } else {
            FileStore::new(device_grant_path.clone(), HashSet::new())
        };

        let app_grant_path = format!("{}app_grants", saved_dir);
        let app_grant_store = if let Ok(v) = FileStore::load(app_grant_path.clone()) {
            v
        } else {
            FileStore::new(app_grant_path.clone(), HashMap::new())
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
        if app_id.is_some() {
            let app_id = app_id.unwrap();
            let mut grant_state = self.grant_app_map.write().unwrap();
            //Get a mutable reference to the value associated with a key, create it if it doesn't exist,
            let entries = grant_state.value.entry(app_id).or_insert(HashSet::new());

            if entries.contains(&new_entry) {
                entries.remove(&new_entry);
            }
            entries.insert(new_entry);
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
        device_grants.value.insert(entry);
        device_grants.sync();
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

    pub fn get_info(
        state: &PlatformState,
        call_ctx: &CallContext,
        r: CapabilitySet,
    ) -> Result<(), GrantErrors> {
        /*
         * Instead of just checking for grants previously, if the user grants are not present,
         * we are taking necessary steps to get the user grant and send back the result.
         */
        let grant_state = state.clone().cap_state.grant_state;
        let app_id = call_ctx.app_id.clone();
        let caps_needing_grants = grant_state.caps_needing_grants.clone();
        let caps_needing_grant_in_request: Vec<FireboltPermission> = r
            .into_firebolt_permissions_vec()
            .clone()
            .into_iter()
            .filter(|x| caps_needing_grants.contains(&x.cap.as_str()))
            .collect();
        let mut grant_errors = GrantErrors::default();
        for permission in caps_needing_grant_in_request {
            let result = grant_state.get_grant_state(&app_id, &permission);
            match result {
                GrantActiveState::ActiveGrant(grant) => {
                    if grant.is_err() {
                        grant_errors.add_denied(permission.cap.clone())
                    }
                }
                GrantActiveState::PendingGrant => {
                    grant_errors.add_ungranted(permission.cap.clone())
                }
            }
        }
        if grant_errors.has_errors() {
            Err(grant_errors)
        } else {
            Ok(())
        }
        // UserGrants::determine_grant_policies(&self.ps.clone(), call_ctx, &r).await
    }

    pub async fn check_with_roles(
        state: &PlatformState,
        call_ctx: &CallContext,
        r: CapabilitySet,
    ) -> Result<(), DenyReasonWithCap> {
        /*
         * Instead of just checking for grants previously, if the user grants are not present,
         * we are taking necessary steps to get the user grant and send back the result.
         */
        let grant_state = state.clone().cap_state.grant_state;
        let app_id = call_ctx.app_id.clone();
        let caps_needing_grants = grant_state.caps_needing_grants.clone();
        let caps_needing_grant_in_request: Vec<FireboltPermission> = r
            .into_firebolt_permissions_vec()
            .clone()
            .into_iter()
            .filter(|x| caps_needing_grants.contains(&x.cap.as_str()))
            .collect();
        for permission in caps_needing_grant_in_request {
            let result = grant_state.get_grant_state(&app_id, &permission);
            match result {
                GrantActiveState::ActiveGrant(grant) => {
                    if grant.is_err() {
                        return Err(DenyReasonWithCap {
                            reason: DenyReason::Ungranted,
                            caps: vec![permission.cap.clone()],
                        });
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
        grant_entry_map
    }

    pub fn grant_modify(
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
            role: role.clone(),
        };

        if let Some(grant_policy_map) = platform_state.get_device_manifest().get_grant_policies() {
            let result = grant_policy_map.get(&permission.cap.as_str());
            if let Some(policies) = result {
                if let Some(grant_policy) = policies.get_policy(&permission) {
                    // Do the scope validation here and return false, if there is any scope mismatch.
                    if app_id.is_some() && grant_policy.scope != GrantScope::App {
                        return false;
                    }

                    let mut new_entry = GrantEntry {
                        role,
                        capability,
                        status: None, // status will be updated later based on the modify operation.
                        lifespan: Some(grant_policy.lifespan),
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

                    platform_state
                        .cap_state
                        .grant_state
                        .update_grant_entry(app_id, new_entry);
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

#[derive(Debug, Clone)]
pub enum GrantActiveState {
    ActiveGrant(Result<(), DenyReason>),
    PendingGrant,
}

#[derive(Debug, Default)]
pub struct GrantErrors {
    pub ungranted: HashSet<FireboltCap>,
    pub denied: HashSet<FireboltCap>,
}

impl GrantErrors {
    fn add_ungranted(&mut self, cap: FireboltCap) {
        self.ungranted.insert(cap);
    }

    fn add_denied(&mut self, cap: FireboltCap) {
        self.denied.insert(cap);
    }

    fn has_errors(&self) -> bool {
        self.ungranted.len() > 0 || self.denied.len() > 0
    }

    pub fn get_reason(&self, cap: &FireboltCap) -> Option<DenyReason> {
        if self.ungranted.contains(cap) {
            Some(DenyReason::Ungranted)
        } else if self.denied.contains(cap) {
            Some(DenyReason::GrantDenied)
        } else {
            None
        }
    }
}

pub struct GrantPolicyEnforcer;

impl GrantPolicyEnforcer {
    pub async fn determine_grant_policies_for_permission(
        platform_state: &PlatformState,
        call_context: &CallContext,
        permission: &FireboltPermission,
    ) -> Result<(), DenyReasonWithCap> {
        if let Some(grant_policy_map) = platform_state.get_device_manifest().get_grant_policies() {
            let result = grant_policy_map.get(&permission.cap.as_str());
            if let Some(policies) = result {
                if let Some(policy) = policies.get_policy(permission) {
                    if Self::is_policy_valid(platform_state, &policy) {
                        return Err(DenyReasonWithCap {
                            caps: vec![permission.clone().cap],
                            reason: DenyReason::Disabled,
                        });
                    }
                    let result = GrantPolicyEnforcer::execute(
                        platform_state,
                        call_context,
                        permission,
                        &policy,
                    )
                    .await;
                    platform_state.cap_state.grant_state.update(
                        permission,
                        &policy,
                        result.is_ok(),
                        &call_context.app_id,
                    )
                }
            }
        }
        Ok(())
    }

    fn is_policy_valid(platform_state: &PlatformState, policy: &GrantPolicy) -> bool {
        if let Some(privacy) = &policy.privacy_setting {
            let privacy_property = &privacy.property;
            return platform_state
                .open_rpc_state
                .check_privacy_property(privacy_property);
        } else {
            if let Some(grant_steps) = policy.get_steps_without_grant() {
                for step in grant_steps {
                    if platform_state
                        .open_rpc_state
                        .get_capability_policy(step.capability.clone())
                        .is_some()
                    {
                        return false;
                    }
                }
                return true;
            }
        }

        false
    }

    async fn evaluate_privacy_settings(
        _platform_state: &PlatformState,
        _privacy_setting: &GrantPrivacySetting,
        _call_ctx: &CallContext,
    ) -> Option<Result<(), DenyReasonWithCap>> {
        // TODO add Privacy logic
        None
    }

    async fn evaluate_options(
        platform_state: &PlatformState,
        call_ctx: &CallContext,
        permission: &FireboltPermission,
        policy: &GrantPolicy,
    ) -> Result<(), DenyReasonWithCap> {
        let generic_cap_state = platform_state.clone().cap_state.generic;
        for grant_requirements in &policy.options {
            for step in &grant_requirements.steps {
                let cap = FireboltCap::Full(step.capability.to_owned());
                let firebolt_cap = vec![cap.clone()];
                debug!(
                    "checking if the cap is supported & available: {:?}",
                    firebolt_cap
                );
                if let Err(e) = generic_cap_state.check_all(&firebolt_cap) {
                    return Err(DenyReasonWithCap {
                        caps: e.caps,
                        reason: DenyReason::GrantDenied,
                    });
                } else {
                    match GrantStepExecutor::execute(step, platform_state, call_ctx, permission)
                        .await
                    {
                        Ok(_) => {
                            CapState::emit(
                                platform_state,
                                CapEvent::OnGranted,
                                cap,
                                Some(permission.role.clone()),
                            )
                            .await;
                            return Ok(());
                        }
                        Err(e) => {
                            CapState::emit(
                                platform_state,
                                CapEvent::OnRevoked,
                                cap,
                                Some(permission.role.clone()),
                            )
                            .await;
                            return Err(e);
                        }
                    }
                }
            }
        }

        Ok(())
    }

    async fn execute(
        platform_state: &PlatformState,
        call_ctx: &CallContext,
        permission: &FireboltPermission,
        policy: &GrantPolicy,
    ) -> Result<(), DenyReasonWithCap> {
        if let Some(privacy_setting) = policy.clone().privacy_setting {
            let resp =
                Self::evaluate_privacy_settings(platform_state, &privacy_setting, call_ctx).await;

            if resp.is_some() {
                return resp.unwrap();
            }
        }
        Self::evaluate_options(platform_state, call_ctx, permission, policy).await
    }
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GrantStepExecutor;

impl GrantStepExecutor {
    pub async fn execute(
        step: &GrantStep,
        platform_state: &PlatformState,
        call_ctx: &CallContext,
        permission: &FireboltPermission,
    ) -> Result<(), DenyReasonWithCap> {
        let capability = step.capability.clone();
        let configuration = step.configuration.clone();
        debug!(
            "Reached execute phase of step for capability: {}",
            capability
        );
        // 1. Check if the capability is supported and available.
        // 2. Call the capability,
        // 3. Get the user response and return
        let firebolt_cap = FireboltCap::Full(capability.to_owned());
        if let Err(e) = platform_state
            .cap_state
            .generic
            .check_all(&vec![firebolt_cap.clone()])
        {
            return Err(DenyReasonWithCap {
                reason: DenyReason::GrantDenied,
                caps: e.caps,
            });
        }

        Self::invoke_capability(
            platform_state,
            call_ctx,
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
        if let Ok(app_response_res) = rx.await {
            if let Ok(app_response) = app_response_res {
                if let AppManagerResponse::AppName(name) = app_response {
                    app_name = name.unwrap_or_default();
                }
            }
        }
        app_name
    }

    pub async fn invoke_capability(
        platform_state: &PlatformState,
        call_ctx: &CallContext,
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
        let app_name = Self::get_app_name(platform_state, call_ctx.app_id.clone()).await;
        let pr_msg_opt = match p_cap.as_str().as_str() {
            "xrn:firebolt:capability:usergrant:acknowledgechallenge" => {
                let challenge = Challenge {
                    capability: permission.cap.as_str(),
                    requestor: ChallengeRequestor {
                        id: call_ctx.app_id.clone(),
                        name: app_name,
                    },
                };
                Some(ProviderBrokerRequest {
                    capability: p_cap.as_str(),
                    method: String::from("challenge"),
                    caller: call_ctx.clone(),
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
                        caller: call_ctx.clone(),
                        request: ProviderRequestPayload::PinChallenge(PinChallengeRequest {
                            pin_space: pin_conf.pin_space,
                            requestor: call_ctx.clone(),
                            capability: Some(call_ctx.app_id.clone()),
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
                    caller: call_ctx.clone(),
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
                        true => {
                            debug!("returning ok from invoke_capability");
                            Ok(())
                        }
                        false => {
                            debug!("returning err from invoke_capability");
                            Err(DenyReason::GrantDenied)
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
