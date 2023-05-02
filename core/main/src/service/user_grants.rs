use tokio::sync::oneshot;

use crate::api::permissions::user_grants_data::{GrantPolicy, Lifespan, Scope};
use crate::helpers::ripple_helper::IRippleHelper;
use crate::managers::capability_manager::FireboltPermission;
use crate::managers::capability_resolver::ReqCaps;
use crate::managers::{
    capability_manager::{CapabilityRole, DenyReason, FireboltCap},
    storage::{storage_manager::StorageManager, storage_property::StorageProperty},
};
use crate::platform_state::PlatformState;

use crate::api::rpc::rpc_gateway::CallContext;
use crate::{
    apps::provider_broker::{self, ProviderRequestPayload, ProviderResponsePayload},
    helpers::channel_util::oneshot_send_and_log,
    managers::capability_manager::DenyReasonWithCap,
};
use crate::{helpers::ripple_helper::RippleHelper, managers::capability_manager::CapEventState};
use serde::{Deserialize, Serialize};
use std::time::{Duration, SystemTime};
use std::{
    collections::{HashMap, HashSet},
    hash::{Hash, Hasher},
    sync::{Arc, RwLock},
};
use tokio::sync::oneshot::Sender as OneShotSender;
use tracing::debug;

#[cfg(not(test))]
use crate::apps::provider_broker::ProviderBroker;

#[cfg(test)]
use tests::ProviderBroker;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Challenge {
    pub capability: String,
    pub requestor: ChallengeRequestor,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ChallengeRequestor {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChallengeResponse {
    pub granted: bool,
}

pub struct UserGrants {
    pub platform_state: PlatformState,
    pub helper: Box<RippleHelper>,
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

#[derive(Eq, Clone, Debug, Serialize, Deserialize)]
pub struct GrantEntry {
    pub role: CapabilityRole,
    pub capability: String,
    pub status: Option<GrantStatus>,
    pub lifespan: Option<Lifespan>,
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
            Some(Lifespan::Seconds) => match self.lifespan_ttl_in_secs {
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
            Some(Lifespan::Once) => true,
            _ => false,
        }
    }
}

/// UserGrantState represents the granted and denied entries of grants per app.
/// only Grants which are of type GrantPerApp User Grant scope will be stored here
#[derive(Clone)]
pub struct UserGrantState {
    grant_map: Arc<RwLock<HashMap<Option<String>, HashSet<GrantEntry>>>>,
}

#[derive(Debug, Clone)]
pub enum GrantActiveState {
    ActiveGrant(Result<(), DenyReason>),
    PendingGrant,
}

impl Default for UserGrantState {
    fn default() -> Self {
        let state = UserGrantState {
            grant_map: Arc::new(RwLock::new(HashMap::new())),
        };
        state
    }
}

impl std::fmt::Debug for UserGrantState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let grant_state = self.grant_map.read().unwrap();
        f.debug_map().entries(grant_state.iter()).finish()
    }
}
pub struct UserGrantStateUtils;

impl UserGrantStateUtils {
    pub async fn update_grant_entry(
        platform_state: &PlatformState,
        app_id: Option<String>, // None is for device
        new_entry: GrantEntry,
    ) -> bool {
        {
            let mut grant_state = platform_state.grant_state.grant_map.write().unwrap();
            //Get a mutable reference to the value associated with a key, create it if it doesn't exist,
            let entries = grant_state.entry(app_id).or_insert(HashSet::new());

            if entries.contains(&new_entry) {
                entries.remove(&new_entry);
            }
            entries.insert(new_entry);
        }
        // calling async function after releasing the write lock.
        let _ = UserGrantStateUtils::persist_user_grants(platform_state).await;
        true
    }

    pub fn custom_delete_entries<F>(
        state: &UserGrantState,
        app_id: Option<String>,
        restrict_function: F,
    ) -> bool
    where
        F: FnMut(&GrantEntry) -> bool,
    {
        let mut deleted = false;
        let mut grant_state = state.grant_map.write().unwrap();
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
        state: &UserGrantState,
        app_id: Option<String>, // None is for device
    ) -> bool {
        let mut deleted = false;
        let mut grant_state = state.grant_map.write().unwrap();
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

    pub fn delete_all_expired_entries(state: &UserGrantState) -> bool {
        let mut deleted = false;
        let mut grant_state = state.grant_map.write().unwrap();
        for (_, entries) in grant_state.iter_mut() {
            let prev_len = entries.len();
            entries.retain(|entry| !entry.has_expired());
            if entries.len() < prev_len {
                deleted = true;
            }
        }
        deleted
    }

    // Returns all active and denied user grant entries for the given `app_id`.
    // Pass None for device scope
    pub fn get_grant_entries_for_app_id(
        state: &UserGrantState,
        app_id: Option<String>,
    ) -> Vec<GrantEntry> {
        UserGrantStateUtils::delete_expired_entries_for_app(state, app_id.clone());
        match state.grant_map.read().unwrap().get(&app_id) {
            Some(x) => x.iter().cloned().collect(),
            None => vec![],
        }
    }

    // Returns all active and denied user grant entries for the given `capability`
    pub fn get_grant_entries_for_capability(
        state: &UserGrantState,
        capability: &str,
    ) -> HashMap<Option<String>, Vec<GrantEntry>> {
        UserGrantStateUtils::delete_all_expired_entries(state);
        let grant_state = state.grant_map.read().unwrap();
        let mut grant_entry_map: HashMap<Option<String>, Vec<GrantEntry>> = HashMap::new();
        for (app_id, app_entries) in grant_state.iter() {
            for item in app_entries {
                if item.capability == capability {
                    grant_entry_map
                        .entry(app_id.clone())
                        .or_default()
                        .push(item.clone());
                }
            }
        }
        grant_entry_map
    }

    pub fn get_grant_entries_for_capability_for_app(
        state: &UserGrantState,
        app_id: Option<String>,
        capability: &str,
    ) -> Vec<GrantEntry> {
        UserGrantStateUtils::delete_expired_entries_for_app(state, app_id.clone());
        let grant_state = state.grant_map.read().unwrap();
        match grant_state.get(&app_id) {
            Some(app_entries) => app_entries
                .iter()
                .filter(|entry| entry.capability == capability)
                .cloned()
                .collect(),
            None => vec![],
        }
    }

    pub fn get_grant_status(
        state: &UserGrantState,
        app_id: Option<String>,
        role: CapabilityRole,
        capability: &str,
    ) -> Option<GrantStatus> {
        let grant_state = state.grant_map.read().unwrap();
        debug!("grant state: {:?}", grant_state);
        let entries = grant_state.get(&app_id)?;

        for entry in entries {
            if !entry.has_expired() && (entry.role == role) && (entry.capability == capability) {
                debug!("Stored grant status: {:?}", entry.status);
                return entry.status.clone();
            }
        }
        debug!("No stored grant status found");
        None
    }

    pub fn grant(
        platform_state: &PlatformState,
        app_id: Option<String>,
        role: CapabilityRole,
        capability: String,
    ) -> bool {
        let grant_status = GrantStatus::Allowed;
        // Get the GrantEntry from UserGrantState matching app_id, role & capability
        // If found update, GrantStatus to Allowed, otherwise create a new entry for active (What could be the default LifeSpan ?? )
        // persist_user_grants(). RPPL-66
        todo!()
    }

    pub fn deny(
        platform_state: &PlatformState,
        app_id: Option<String>,
        role: CapabilityRole,
        capability: String,
    ) {
        let grant_status = GrantStatus::Denied;
        // Get the GrantEntry from UserGrantState matching app_id, role & capability
        // If found update, GrantStatus to Denied, otherwise create a new entry for Denied (What could be the default LifeSpan ?? )
        // persist_user_grants(). RPPL-66
        todo!()
    }

    pub fn clear(
        platform_state: &PlatformState,
        app_id: Option<String>,
        role: CapabilityRole,
        capability: String,
    ) {
        // Get the GrantEntry from UserGrantState matching app_id, role & capability
        // If found update, unset the user grant and remove the entry.
        // persist_user_grants(). RPPL-66
        todo!()
    }
    fn serialize_user_grants_based_on_persistent_lifespan(
        platform_state: &PlatformState,
    ) -> Result<String, String> {
        let grant_entries = platform_state
            .grant_state
            .grant_map
            .read()
            .map_err(|_| "Error on acquiring read lock".to_string())?;

        let filtered_entries: HashMap<Option<String>, HashSet<GrantEntry>> = grant_entries
            .iter()
            .filter_map(|(app_id, entries)| {
                let filltered_app_entries: HashSet<GrantEntry> = entries
                    .iter()
                    .filter(|entry| {
                        entry.lifespan == Some(Lifespan::Forever)
                            || entry.lifespan == Some(Lifespan::Seconds)
                    })
                    .cloned()
                    .collect();
                if filltered_app_entries.is_empty() {
                    None
                } else {
                    Some((app_id.clone(), filltered_app_entries))
                }
            })
            .collect();

        let mut serialization_map = HashMap::new();
        for (app_id, entries) in filtered_entries {
            let key = match app_id {
                Some(app_id) => app_id,
                None => "".to_owned(),
            };
            serialization_map.insert(key, entries);
        }

        let serialized = serde_json::to_string(&serialization_map)
            .map_err(|e| format!("Error serializing {}", e))?;

        Ok(serialized.to_owned())
    }
    pub async fn persist_user_grants(platform_state: &PlatformState) -> Result<(), String> {
        UserGrantStateUtils::delete_all_expired_entries(&platform_state.grant_state);

        let serialized =
            match UserGrantStateUtils::serialize_user_grants_based_on_persistent_lifespan(
                platform_state,
            ) {
                Ok(s) => s,
                Err(e) => return Err(e),
            };

        // TBD RPPL-306: Use new StorageManager API to store UserGrant data into a file
        // rather than RDK's persistent storage. RDK Storage has a limit of 1000 bytes
        // per entry.
        let resp = StorageManager::set_string(
            platform_state,
            StorageProperty::UserGrants,
            serialized.to_owned(),
            None,
        )
        .await;

        match resp {
            Ok(_) => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    }

    fn deserialize_user_grants(
        platform_state: &PlatformState,
        json_str: &str,
    ) -> Result<(), String> {
        let deserialized: HashMap<String, HashSet<GrantEntry>> =
            serde_json::from_str(json_str).map_err(|e| e.to_string())?;

        debug!("User Grants from Storage : {:#?}", deserialized);

        let mut grant_entries = platform_state
            .grant_state
            .grant_map
            .write()
            .map_err(|_| "Error on acquiring write lock".to_string())?;

        grant_entries.clear();

        for (app_id, entries) in deserialized.into_iter() {
            let opt_app_id = if app_id.is_empty() {
                None
            } else {
                Some(app_id.to_owned())
            };
            grant_entries.insert(opt_app_id, entries);
        }
        Ok(())
    }

    pub async fn restore_user_grants_from_storage(
        platform_state: &PlatformState,
    ) -> Result<(), String> {
        let json_str =
            StorageManager::get_string(platform_state, StorageProperty::UserGrants).await;
        match json_str {
            Ok(s) => UserGrantStateUtils::deserialize_user_grants(platform_state, &s),
            Err(e) => Err(e.to_string()),
        }
    }
}

impl UserGrants {
    // This function will be modified or removed as part of RPPL-97
    async fn update(
        platform_state: &PlatformState,
        granted: bool,
        app_id: String,
        cap: FireboltCap,
    ) -> bool {
        let cap_s = cap.as_str();
        let cap_grant_entry = GrantEntry::get(CapabilityRole::Use, cap_s.to_owned());
        return UserGrantStateUtils::update_grant_entry(
            platform_state,
            Some(app_id),
            cap_grant_entry,
        )
        .await;
    }

    pub fn check_all(
        platform_state: &PlatformState,
        app_id: String,
        // role: CapabilityRole,  This will be added in RPPL-97
        caps: Vec<String>,
    ) -> HashMap<String, Result<(), DenyReason>> {
        let mut map = HashMap::new();

        let role = CapabilityRole::Use; // This will be collected as an input param in RPPL-97
        for cap in caps {
            let grant_status = UserGrantStateUtils::get_grant_status(
                &platform_state.grant_state,
                Some(app_id.clone()),
                role.clone(),
                &cap,
            );

            match grant_status {
                Some(GrantStatus::Allowed) => map.insert(cap.clone(), Ok(())),
                Some(GrantStatus::Denied) => map.insert(cap.clone(), Err(DenyReason::GrantDenied)),
                None => map.insert(cap.clone(), Err(DenyReason::Ungranted)),
            };
        }
        map
    }

    pub fn check(
        platform_state: &PlatformState,
        app_id: String,
        // role: CapabilityRole,  This will be added in RPPL-97
        cap: FireboltCap,
    ) -> Result<(), DenyReason> {
        let role = CapabilityRole::Use; // This will be collected as an input param in RPPL-97
        let grant_status = UserGrantStateUtils::get_grant_status(
            &platform_state.grant_state,
            Some(app_id),
            role,
            &cap.as_str(),
        );

        match grant_status {
            Some(GrantStatus::Allowed) => Ok(()),
            Some(GrantStatus::Denied) => Err(DenyReason::GrantDenied),
            None => Err(DenyReason::Ungranted),
        }
    }

    pub async fn are_caps_available(platform_state: &PlatformState, caps: &Vec<String>) -> bool {
        let mut all_available = true;
        for cap in caps {
            let firebolt_cap = FireboltCap::Full(cap.to_owned());
            if !platform_state.services.is_cap_available(firebolt_cap).await {
                all_available = false;
                break;
            }
        }
        all_available
    }
    pub async fn check_if_all_caps_available(
        platform_state: &PlatformState,
        call_ctx: &CallContext,
        req_caps: &ReqCaps,
    ) -> bool {
        let mut all_available = true;
        if let Some(use_caps) = &req_caps.use_caps {
            all_available &= UserGrants::are_caps_available(platform_state, use_caps).await;
        }
        if all_available {
            if let Some(manage_caps) = &req_caps.manage_caps {
                all_available &= UserGrants::are_caps_available(platform_state, manage_caps).await;
            }
        }
        if all_available {
            if let Some(provide_cap) = &req_caps.provide_caps {
                all_available &=
                    UserGrants::are_caps_available(platform_state, &vec![provide_cap.to_owned()])
                        .await;
            }
        }
        all_available
    }

    pub async fn get_grant_policy(
        platform_state: &PlatformState,
        call_ctx: &CallContext,
        permission: &FireboltPermission,
    ) -> Option<GrantPolicy> {
        let man_grant_policies_map = platform_state.services.cm.clone().get_grant_policies();
        let grant_policies = man_grant_policies_map.get(&permission.cap.as_str());
        if grant_policies.is_none()
            || permission.role == CapabilityRole::Use && grant_policies.unwrap()._use.is_none()
            || permission.role == CapabilityRole::Manage && grant_policies.unwrap().manage.is_none()
            || permission.role == CapabilityRole::Provide
                && grant_policies.unwrap().provide.is_none()
        {
            return None;
        }
        let mut grant_policy = match permission.role {
            CapabilityRole::Use => grant_policies.unwrap()._use.as_ref().unwrap().clone(),
            CapabilityRole::Manage => grant_policies.unwrap().manage.as_ref().unwrap().clone(),
            CapabilityRole::Provide => grant_policies.unwrap().provide.as_ref().unwrap().clone(),
        };
        grant_policy.override_policy(platform_state, call_ctx).await;
        Some(grant_policy)
    }
    pub async fn determine_grant_policies_for_permission(
        platform_state: &PlatformState,
        call_ctx: &CallContext,
        permission: &FireboltPermission,
    ) -> Result<(), DenyReason> {
        let grant_policy_opt = Self::get_grant_policy(platform_state, call_ctx, &permission).await;
        if grant_policy_opt.is_none() {
            return Ok(());
        }
        let grant_policy = grant_policy_opt.unwrap();
        if !grant_policy.is_valid(platform_state, call_ctx).await {
            return Err(DenyReason::Disabled);
        }
        let result = grant_policy.execute(platform_state, call_ctx).await;

        let mut grant_entry =
            GrantEntry::get(permission.role.clone(), permission.cap.as_str().to_owned());
        grant_entry.lifespan = Some(grant_policy.lifespan.clone());
        if grant_policy.lifespan_ttl.is_some() {
            grant_entry.lifespan_ttl_in_secs = grant_policy.lifespan_ttl.clone();
        }
        if result.is_ok() {
            grant_entry.status = Some(GrantStatus::Allowed);
        } else {
            grant_entry.status = Some(GrantStatus::Denied);
        }
        // If lifespan is once then no need to store it.
        if grant_policy.lifespan != Lifespan::Once {
            match grant_policy.scope {
                Scope::App => {
                    UserGrantStateUtils::update_grant_entry(
                        platform_state,
                        Some(call_ctx.app_id.to_owned()),
                        grant_entry,
                    )
                    .await;
                }
                Scope::Device => {
                    UserGrantStateUtils::update_grant_entry(platform_state, None, grant_entry)
                        .await;
                }
            }
        }
        result
    }

    pub async fn get_stored_grants_for_permission(
        platform_state: &PlatformState,
        call_ctx: &CallContext,
        permission: &FireboltPermission,
    ) -> GrantActiveState {
        let device_grant = UserGrantStateUtils::get_grant_status(
            &platform_state.grant_state,
            None,
            permission.role.clone(),
            permission.cap.as_str().as_str(),
        );
        if let Some(grant_status) = device_grant {
            return GrantActiveState::ActiveGrant(grant_status.into());
        }
        let app_grant = UserGrantStateUtils::get_grant_status(
            &platform_state.grant_state,
            Some(call_ctx.app_id.clone()),
            permission.role.clone(),
            permission.cap.as_str().as_str(),
        );
        if let Some(grant_status) = app_grant {
            return GrantActiveState::ActiveGrant(grant_status.into());
        }
        GrantActiveState::PendingGrant
    }

    pub async fn grant_multiple(
        platform_state: &PlatformState,
        call_ctx: CallContext,
        caps: Vec<FireboltCap>,
        callback: Option<OneShotSender<Result<(), DenyReasonWithCap>>>,
    ) {
        for cap in caps {
            if let Err(_) = Self::grant(platform_state, call_ctx.clone(), cap.clone()).await {
                if let Some(c) = callback {
                    oneshot_send_and_log(
                        c,
                        Err((DenyReason::GrantDenied, Some(vec![cap.as_str()]))),
                        "usergrant_grant_multiple_denied".into(),
                    );
                    return;
                }
                debug!("{} denied silently continue", cap.as_str());
            }
        }
        if let Some(c) = callback {
            oneshot_send_and_log(c, Ok(()), "usergrant_grant_multiple_denied".into());
        }
    }

    pub async fn grant(
        platform_state: &PlatformState,
        call_ctx: CallContext,
        cap: FireboltCap,
    ) -> Result<(), DenyReason> {
        debug!("inside grant");
        let (session_tx, session_rx) = oneshot::channel::<ProviderResponsePayload>();
        let app_id = call_ctx.clone().app_id;
        let p_app = app_id.clone();
        let p_cap = cap.clone();

        let challenge = Challenge {
            capability: cap.as_str(),
            requestor: ChallengeRequestor {
                id: app_id.clone(),
                name: app_id.clone(),
            },
        };
        let pr_msg = provider_broker::Request {
            capability: String::from("xrn:firebolt:capability:usergrant:acknowledgechallenge"),
            method: String::from("challenge"),
            caller: call_ctx,
            request: ProviderRequestPayload::AckChallenge(challenge),
            tx: session_tx,
            app_id: None,
        };

        ProviderBroker::invoke_method(&platform_state.clone(), pr_msg).await;
        match session_rx.await {
            Ok(result) => match result.as_challenge_response() {
                Some(res) => {
                    debug!("recieved challenge state");
                    let granted = res.clone().granted;
                    if UserGrants::update(
                        &platform_state.clone(),
                        res.clone().granted,
                        p_app,
                        p_cap.clone(),
                    )
                    .await
                    {
                        let ps2 = platform_state.clone();
                        tokio::spawn(async move {
                            CapEventState::emit(
                                ps2,
                                match granted {
                                    true => {
                                        crate::managers::capability_manager::CapEvent::OnGranted
                                    }
                                    false => {
                                        crate::managers::capability_manager::CapEvent::OnRevoked
                                    }
                                },
                                p_cap,
                            )
                            .await;
                        });
                    }
                    match res.granted {
                        true => Ok(()),
                        false => Err(DenyReason::GrantDenied),
                    }
                }
                None => {
                    return Err(DenyReason::Ungranted);
                }
            },
            Err(_) => {
                return Err(DenyReason::Ungranted);
            }
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {

    use crate::{
        api::permissions::user_grants_data::Lifespan,
        api::rpc::{firebolt_gateway::tests::TestGateway, rpc_gateway::CallContext},
        apps::provider_broker::Request,
        managers::capability_manager::CapabilityRole,
        platform_state::PlatformState,
    };
    use dab::core::message::{DabError, DabRequest, DabResponsePayload};
    use serde_json::{json, Value};
    use std::time::SystemTime;
    use tokio::sync::mpsc;

    use super::{GrantEntry, GrantStatus, UserGrantStateUtils, UserGrants};
    use std::collections::HashMap;
    use tracing::debug;
    pub struct ProviderBroker;
    static mut CHALLENGE_RESPONSE: bool = true;

    impl ProviderBroker {
        pub async fn invoke_method(_pst: &PlatformState, request: Request) {
            debug!("received invoke_method request");
            unsafe {
                request
                    .tx
                    .send(
                        crate::apps::provider_broker::ProviderResponsePayload::ChallengeResponse(
                            super::ChallengeResponse {
                                granted: CHALLENGE_RESPONSE,
                            },
                        ),
                    )
                    .unwrap();
            }
        }
    }
    fn grant_call_ctx() -> CallContext {
        CallContext {
            app_id: "app1".into(),
            method: "somemethod".into(),
            request_id: "somerequest".into(),
            session_id: "session".into(),
            call_id: 0,
            protocol: crate::api::rpc::api_messages::ApiProtocol::Badger,
        }
    }

    #[tokio::test]
    pub async fn test_grant() {
        let ps = PlatformState::default();
        let callcontext = grant_call_ctx();
        assert!(UserGrants::grant(
            &ps.clone(),
            callcontext,
            crate::managers::capability_manager::FireboltCap::Short("somecategory:somename".into())
        )
        .await
        .is_ok());
    }

    pub async fn test_add_user_grant_entries(ps: &PlatformState) {
        assert!(
            UserGrantStateUtils::update_grant_entry(
                &ps,
                Some("Hulu".to_owned()),
                GrantEntry {
                    role: CapabilityRole::Use,
                    capability: "xrn:firebolt:capability:localization:postal-code".to_owned(),
                    status: Some(GrantStatus::Allowed),
                    lifespan: Some(Lifespan::Seconds),
                    last_modified_time: SystemTime::now()
                        .duration_since(SystemTime::UNIX_EPOCH)
                        .unwrap(),
                    lifespan_ttl_in_secs: Some(300), // will be replaced
                },
            )
            .await
        );

        assert!(
            UserGrantStateUtils::update_grant_entry(
                &ps,
                Some("Hulu".to_owned()),
                GrantEntry {
                    role: CapabilityRole::Manage,
                    capability: "xrn:firebolt:capability:localization:postal-code".to_owned(),
                    status: Some(GrantStatus::Denied),
                    lifespan: Some(Lifespan::Forever),
                    last_modified_time: SystemTime::now()
                        .duration_since(SystemTime::UNIX_EPOCH)
                        .unwrap(),
                    lifespan_ttl_in_secs: Some(24 * 60 * 60),
                },
            )
            .await
        );

        assert!(
            UserGrantStateUtils::update_grant_entry(
                &ps,
                None,
                GrantEntry {
                    role: CapabilityRole::Use,
                    capability: "xrn:firebolt:capability:discovery:watch-next".to_owned(),
                    status: Some(GrantStatus::Denied),
                    lifespan: Some(Lifespan::Forever),
                    last_modified_time: SystemTime::now()
                        .duration_since(SystemTime::UNIX_EPOCH)
                        .unwrap(),
                    lifespan_ttl_in_secs: None,
                },
            )
            .await
        );

        assert!(
            UserGrantStateUtils::update_grant_entry(
                &ps,
                Some("Amazon-Prime".to_owned()),
                GrantEntry {
                    role: CapabilityRole::Use,
                    capability: "xrn:firebolt:capability:localization:postal-code".to_owned(),
                    status: Some(GrantStatus::Allowed),
                    lifespan: Some(Lifespan::AppActive),
                    last_modified_time: SystemTime::now()
                        .duration_since(SystemTime::UNIX_EPOCH)
                        .unwrap(),
                    lifespan_ttl_in_secs: None,
                },
            )
            .await
        );

        assert!(
            UserGrantStateUtils::update_grant_entry(
                &ps,
                Some("Amazon-Prime".to_owned()),
                GrantEntry {
                    role: CapabilityRole::Use,
                    capability: "xrn:firebolt:capability:discovery:watch-next".to_owned(),
                    status: Some(GrantStatus::Allowed),
                    lifespan: Some(Lifespan::Once),
                    last_modified_time: SystemTime::now()
                        .duration_since(SystemTime::UNIX_EPOCH)
                        .unwrap(),
                    lifespan_ttl_in_secs: None,
                },
            )
            .await
        );

        assert!(
            UserGrantStateUtils::update_grant_entry(
                &ps,
                Some("Hulu".to_owned()),
                GrantEntry {
                    role: CapabilityRole::Use,
                    capability: "xrn:firebolt:capability:localization:postal-code".to_owned(),
                    status: Some(GrantStatus::Allowed),
                    lifespan: Some(Lifespan::Seconds),
                    last_modified_time: SystemTime::now()
                        .duration_since(SystemTime::UNIX_EPOCH)
                        .unwrap(),
                    lifespan_ttl_in_secs: Some(24 * 60 * 60),
                },
            )
            .await
        );

        assert!(
            UserGrantStateUtils::update_grant_entry(
                &ps,
                Some("Hulu".to_owned()),
                GrantEntry {
                    role: CapabilityRole::Use,
                    capability: "xrn:firebolt:capability:discovery::launch".to_owned(),
                    status: Some(GrantStatus::Denied),
                    lifespan: Some(Lifespan::Seconds),
                    last_modified_time: SystemTime::now()
                        .duration_since(SystemTime::UNIX_EPOCH)
                        .unwrap(),
                    lifespan_ttl_in_secs: Some(24 * 60 * 60),
                },
            )
            .await
        );

        assert!(
            UserGrantStateUtils::update_grant_entry(
                &ps,
                Some("Xumo".to_owned()),
                GrantEntry {
                    role: CapabilityRole::Use,
                    capability: "xrn:firebolt:capability:localization:postal-code".to_owned(),
                    status: Some(GrantStatus::Allowed),
                    lifespan: Some(Lifespan::Seconds),
                    last_modified_time: SystemTime::now()
                        .duration_since(SystemTime::UNIX_EPOCH)
                        .unwrap(),
                    lifespan_ttl_in_secs: Some(1),
                },
            )
            .await
        );
    }

    fn start_local_storage_service(mut rx: tokio::sync::mpsc::Receiver<DabRequest>) {
        tokio::spawn(async move {
            let mut local_storage: HashMap<String, Value> = HashMap::new();
            while let Some(req) = rx.recv().await {
                match &req.payload {
                    dab::core::message::DabRequestPayload::Storage(sreq) => match &sreq {
                        dab::core::model::persistent_store::StorageRequest::Get(data) => {
                            let key = (data.namespace.to_owned() + data.key.as_str()).to_owned();
                            if let Some(value) = local_storage.get(key.as_str()) {
                                req.respond_and_log(Ok(DabResponsePayload::JsonValue(
                                    value.clone(),
                                )));
                            } else {
                                req.respond_and_log(Err(DabError::OsError));
                            }
                        }

                        dab::core::model::persistent_store::StorageRequest::Set(ssp) => {
                            local_storage.insert(
                                (ssp.namespace.to_owned() + &ssp.key).to_owned(),
                                ssp.data.value.clone(),
                            );
                            req.respond_and_log(Ok(DabResponsePayload::JsonValue(json!(true))));
                        }
                    },
                    _ => panic!("Not intended to handle request other than Storage!!"),
                }
            }
        });
    }
    pub async fn test_persist_and_restore_user_grant_entries(ps: &PlatformState) {
        assert!(UserGrantStateUtils::persist_user_grants(&ps).await.is_ok());
        assert!(UserGrantStateUtils::restore_user_grants_from_storage(&ps)
            .await
            .is_ok());
    }
    pub async fn test_update_user_grant_entries(ps: &PlatformState) {
        assert!(
            UserGrantStateUtils::update_grant_entry(
                &ps,
                None,
                GrantEntry {
                    role: CapabilityRole::Use,
                    capability: "xrn:firebolt:capability:localization:postal-code".to_owned(),
                    status: Some(GrantStatus::Allowed),
                    lifespan: Some(Lifespan::Seconds),
                    last_modified_time: SystemTime::now()
                        .duration_since(SystemTime::UNIX_EPOCH)
                        .unwrap(),
                    lifespan_ttl_in_secs: Some(1),
                },
            )
            .await
        );

        assert!(
            UserGrantStateUtils::update_grant_entry(
                &ps,
                Some("Xfinity Stream".to_owned()),
                GrantEntry {
                    role: CapabilityRole::Use,
                    capability: "xrn:firebolt:capability:localization:postal-code".to_owned(),
                    status: Some(GrantStatus::Allowed),
                    lifespan: Some(Lifespan::Seconds),
                    last_modified_time: SystemTime::now()
                        .duration_since(SystemTime::UNIX_EPOCH)
                        .unwrap(),
                    lifespan_ttl_in_secs: Some(1),
                },
            )
            .await
        );

        assert!(
            UserGrantStateUtils::update_grant_entry(
                &ps,
                Some("Xumo".to_owned()),
                GrantEntry {
                    role: CapabilityRole::Use,
                    capability: "xrn:firebolt:capability:localization:postal-code".to_owned(),
                    status: Some(GrantStatus::Allowed),
                    lifespan: Some(Lifespan::Seconds),
                    last_modified_time: SystemTime::now()
                        .duration_since(SystemTime::UNIX_EPOCH)
                        .unwrap(),
                    lifespan_ttl_in_secs: Some(24 * 60 * 60),
                },
            )
            .await
        );
        assert!(
            UserGrantStateUtils::update_grant_entry(
                &ps,
                None,
                GrantEntry {
                    role: CapabilityRole::Use,
                    capability: "xrn:firebolt:capability:privacy:test".to_owned(),
                    status: Some(GrantStatus::Allowed),
                    lifespan: Some(Lifespan::Seconds),
                    last_modified_time: SystemTime::now()
                        .duration_since(SystemTime::UNIX_EPOCH)
                        .unwrap(),
                    lifespan_ttl_in_secs: Some(24 * 60 * 60),
                },
            )
            .await
        );
    }
    pub async fn test_user_grant_query_apis(ps: &PlatformState) {
        let _ = UserGrantStateUtils::get_grant_entries_for_app_id(
            &ps.grant_state,
            Some("Hulu".to_owned()),
        );

        let _ = UserGrantStateUtils::get_grant_entries_for_app_id(&ps.grant_state, None);

        let _ = UserGrantStateUtils::get_grant_entries_for_capability(
            &ps.grant_state,
            "xrn:firebolt:capability:localization:postal-code".into(),
        );

        let _ = UserGrantStateUtils::get_grant_entries_for_capability_for_app(
            &ps.grant_state,
            Some("Hulu".to_owned()),
            "xrn:firebolt:capability:localization:postal-code".into(),
        );

        let _ = UserGrantStateUtils::get_grant_status(
            &ps.grant_state,
            Some("Hulu".to_owned()),
            CapabilityRole::Use,
            "xrn:firebolt:capability:localization:postal-code".into(),
        );
    }
    #[tokio::test]
    pub async fn test_add_user_grant_utils() {
        let ps = PlatformState::default();
        test_add_user_grant_entries(&ps).await;
    }

    #[tokio::test]
    pub async fn test_all_user_grant_utils() {
        let mut ps = PlatformState::default();
        let (_tx, mut _rx) = mpsc::channel::<String>(32);
        let (mock_dab_tx, mock_dab_rx) = mpsc::channel::<DabRequest>(32);
        ps.services.sender_hub.dab_tx = Some(mock_dab_tx.clone());
        let mut test_gateway = TestGateway::start(ps.clone()).await;
        test_gateway.pin_helper.sender_hub.dab_tx = Some(mock_dab_tx.clone());
        start_local_storage_service(mock_dab_rx);

        test_add_user_grant_entries(&ps).await;
        std::thread::sleep(std::time::Duration::from_secs(2));
        assert!(UserGrantStateUtils::delete_expired_entries_for_app(
            &ps.grant_state,
            Some("Xumo".to_owned()),
        ));
        test_update_user_grant_entries(&ps).await;
        std::thread::sleep(std::time::Duration::from_secs(2));
        assert!(UserGrantStateUtils::delete_all_expired_entries(
            &ps.grant_state
        ));
        test_user_grant_query_apis(&ps).await;
        test_persist_and_restore_user_grant_entries(&ps).await;
    }
}
