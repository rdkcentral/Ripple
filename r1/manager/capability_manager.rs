use super::{
    capability::{
        available_cap_manager::NotAvailableState,
        grant_cap_manager::GrantCapManager,
        permitted_cap_manager::PermittedCapManager,
        supported_cap_manager::{SupportedCapHandler, SupportedState},
    },
    capability_resolver::CapabilityResolver,
    config_manager::{ConfigManager, ConfigResponse},
};
use crate::{
    api::{permissions::api_permissions::MethodPermissionRequest, rpc::rpc_gateway::CallContext},
    apps::app_events::{AppEvents, ListenRequest},
    helpers::{
        channel_util::oneshot_send_and_log,
        error_util::{
            CAPABILITY_GET_ERROR, CAPABILITY_NOT_AVAILABLE, CAPABILITY_NOT_PERMITTED,
            CAPABILITY_NOT_SUPPORTED,
        },
        ripple_helper::{IRippleHelper, RippleHelperFactory},
    },
    managers::capability::available_cap_manager::AvailableCapHandler,
    platform_state::PlatformState,
};
use dab::core::message::DabRequestPayload;
use dpab::core::message::{DpabRequest, Role};
use jsonrpsee::core::async_trait;
use regex::Regex;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::{
    collections::HashSet,
    fmt::Display,
    hash::{Hash, Hasher},
    sync::{Arc, RwLock},
};
use tokio::sync::{
    mpsc::{Receiver, Sender as MpscSender},
    oneshot::Sender,
};
use tracing::{debug, error, info, info_span, Instrument};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum DenyReason {
    Unpermitted,
    Unsupported,
    Disabled,
    Unavailable,
    GrantDenied,
    Ungranted,
}

pub type DenyReasonWithCap = (DenyReason, Option<Vec<String>>);

/// Deny for a single capability
pub fn deny_cap(reason: DenyReason, cap_str: String) -> Result<(), DenyReasonWithCap> {
    Err((reason, Some(vec![cap_str])))
}

/// Deny for a single permission
pub fn deny_perm(reason: DenyReason, perm: FireboltPermission) -> Result<(), DenyReasonWithCap> {
    Err((reason, Some(vec![perm.cap.as_str()])))
}

/// Deny for when unknown capability, like for system reasons
pub fn deny_generic(reason: DenyReason) -> Result<(), DenyReasonWithCap> {
    Err((reason, None))
}

/// Deny for multiple capabilities
pub fn deny_caps(reason: DenyReason, caps: Vec<FireboltCap>) -> Result<(), DenyReasonWithCap> {
    let cap_strs = caps.into_iter().map(|c| c.as_str()).collect();
    Err((reason, Some(cap_strs)))
}

/// Deny for multiple capabilities
pub fn deny_cap_set(reason: DenyReason, caps: HashSet<String>) -> Result<(), DenyReasonWithCap> {
    let cap_strs = caps.into_iter().map(|c| c.clone()).collect();
    Err((reason, Some(cap_strs)))
}

#[derive(Eq, Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum CapabilityRole {
    Use,
    Manage,
    Provide,
}

impl From<CapabilityRole> for Role {
    fn from(r: CapabilityRole) -> Self {
        match r {
            CapabilityRole::Use => Role::Use,
            CapabilityRole::Manage => Role::Manage,
            CapabilityRole::Provide => Role::Provide,
        }
    }
}

impl From<Role> for CapabilityRole {
    fn from(r: Role) -> Self {
        match r {
            Role::Use => CapabilityRole::Use,
            Role::Manage => CapabilityRole::Manage,
            Role::Provide => CapabilityRole::Provide,
        }
    }
}

impl CapabilityRole {
    pub fn as_string(&self) -> &'static str {
        match self {
            CapabilityRole::Use => "use",
            CapabilityRole::Manage => "manage",
            CapabilityRole::Provide => "provide",
        }
    }
}

impl Hash for CapabilityRole {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_u8(match self {
            CapabilityRole::Use => 0,
            CapabilityRole::Manage => 1,
            CapabilityRole::Provide => 2,
        });
    }
}
/// There are 5 types of Capabilities
/// 1. Supported = Device Supports or not
/// 2. Available = A Run time capability which might become unavailable
/// 3. Permitted = Needs cloud services to confirm if they are permitted
/// 4. Granted = Needs user to consent to use like camera access for certain app
/// 5. Not Available = Inverse of Available used for initial state of some capability providers like Session which is not available on boot
#[derive(Debug, Clone)]
pub enum CapClassifiedRequest {
    Supported(Vec<FireboltCap>),
    Available(Vec<FireboltCap>),
    NotAvailable(Vec<FireboltCap>),
    IsPermitted(CallContext, Vec<FireboltPermission>),
    Grant(CallContext, Vec<FireboltCap>),
    DenyGrant(CallContext, Vec<FireboltCap>),
}

impl CapClassifiedRequest {
    pub fn all_caps(&self) -> Vec<FireboltCap> {
        match self {
            CapClassifiedRequest::Supported(c) => c.clone(),
            CapClassifiedRequest::Available(c) => c.clone(),
            CapClassifiedRequest::NotAvailable(c) => c.clone(),
            CapClassifiedRequest::IsPermitted(_, p) => {
                p.into_iter().map(|p| p.cap.clone()).collect()
            }
            CapClassifiedRequest::Grant(_, c) => c.clone(),
            CapClassifiedRequest::DenyGrant(_, c) => c.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityInfo {
    pub capability: String,
    pub supported: bool,
    pub available: bool,
    #[serde(rename = "use")]
    pub _use: RolePermission,
    pub manage: RolePermission,
    pub provide: RolePermission,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<Vec<DenyReason>>,
}

impl CapabilityInfo {
    fn get(cap: String, reason: Option<DenyReason>) -> CapabilityInfo {
        let (mut supported, mut available, mut permitted, mut granted) = (true, true, true, true);
        let mut details = None;
        if let Some(r) = reason.clone() {
            details = Some(vec![r.clone()]);
            match r {
                DenyReason::Unsupported => {
                    supported = false;
                    available = false;
                    permitted = false;
                    granted = false;
                }
                DenyReason::Unavailable => {
                    available = false;
                    permitted = false;
                    granted = false;
                }
                DenyReason::Unpermitted => {
                    permitted = false;
                    granted = false;
                }
                DenyReason::Ungranted => {
                    granted = false;
                }
                _ => {}
            }
        }
        CapabilityInfo {
            capability: cap,
            supported: supported,
            available: available,
            _use: RolePermission::get(permitted, granted),
            manage: RolePermission::get(permitted, granted),
            provide: RolePermission::get(permitted, granted),
            details,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RolePermission {
    pub permitted: bool,
    pub granted: bool,
}

impl RolePermission {
    pub fn get(permitted: bool, granted: bool) -> RolePermission {
        RolePermission { permitted, granted }
    }
}

#[derive(Debug, Clone)]
pub struct CapCheckRequest {
    pub caps: Vec<FireboltCap>,
    pub role: Option<CapabilityRole>,
}

impl DenyReason {
    pub fn get_rpc_error_code(&self) -> i32 {
        match self {
            Self::Unavailable => CAPABILITY_NOT_AVAILABLE,
            Self::Unsupported => CAPABILITY_NOT_SUPPORTED,
            Self::GrantDenied => CAPABILITY_NOT_PERMITTED,
            Self::Unpermitted => CAPABILITY_NOT_PERMITTED,
            _ => CAPABILITY_GET_ERROR,
        }
    }

    pub fn get_rpc_error_message(&self, caps: &Option<Vec<String>>) -> String {
        let caps_disp = caps.clone().unwrap_or_else(|| vec![]).clone().join(",");
        match self {
            Self::Unavailable => format!("{} is not available", caps_disp),
            Self::Unsupported => format!("{} is not supported", caps_disp),
            Self::GrantDenied => format!("The user denied access to {}", caps_disp),
            Self::Unpermitted => format!("{} is not permitted", caps_disp),
            _ => format!("Error with {}", caps_disp),
        }
    }
}

#[derive(Debug, Clone)]
pub enum Availability {
    NotReady,
    Ready,
    Failed,
}

#[derive(Debug, Clone)]
pub enum Support {
    Supported,
    NotSupported,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FireboltPermission {
    pub cap: FireboltCap,
    pub role: CapabilityRole,
}

impl Display for FireboltPermission {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = self.cap.as_str();
        let suffix = match self.role {
            crate::managers::capability_manager::CapabilityRole::Use => "",
            crate::managers::capability_manager::CapabilityRole::Manage => "[manage]",
            crate::managers::capability_manager::CapabilityRole::Provide => "[provide]",
        };
        write!(f, "{}{}", s, suffix)
    }
}

impl FireboltPermission {
    pub fn parse(cap: String, role: CapabilityRole) -> Option<FireboltPermission> {
        let fcap = FireboltCap::parse(cap);
        fcap.map(|c| FireboltPermission { cap: c, role })
    }

    pub fn parse_expect(cap: String, role: CapabilityRole) -> FireboltPermission {
        FireboltPermission::parse(cap.clone(), role).expect(&format!("Invalid capability {}", cap))
    }
}

impl Serialize for FireboltPermission {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for FireboltPermission {
    fn deserialize<D>(deserializer: D) -> Result<FireboltPermission, D::Error>
    where
        D: Deserializer<'de>,
    {
        let mut str = String::deserialize(deserializer)?;
        let mut role = CapabilityRole::Use;
        let mut cap = str.clone();
        if str.ends_with("[manage]") {
            role = CapabilityRole::Manage;
            str.truncate(str.len() - "[manage]".len());
            cap = str;
        } else if str.ends_with("[provide]") {
            role = CapabilityRole::Provide;
            str.truncate(str.len() - "[provide]".len());
            cap = str;
        }
        Ok(FireboltPermission {
            cap: FireboltCap::Full(cap),
            role,
        })
    }
}

/// There are many types of Firebolt Cap enums
/// 1. Short: `device:model` becomes = `xrn:firebolt:capability:account:session` its just a handy cap which helps us write less code
/// 2. Full: Contains the full string for capability typically loaded from Manifest and Firebolt SDK which contains the full string
/// 3. Dab: Converts `DabRequestPayload::Device(DeviceRequest::MacAddress)` to `xrn:firebolt:capability:device:mac`
#[derive(Debug, Clone)]
pub enum FireboltCap {
    Short(String),
    Full(String),
    Dab(DabRequestPayload),
}

impl FireboltCap {
    pub fn short<S>(s: S) -> FireboltCap
    where
        S: Into<String>,
    {
        FireboltCap::Short(s.into())
    }
}

impl Serialize for FireboltCap {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.as_str())
    }
}

impl<'de> Deserialize<'de> for FireboltCap {
    fn deserialize<D>(deserializer: D) -> Result<FireboltCap, D::Error>
    where
        D: Deserializer<'de>,
    {
        let cap = String::deserialize(deserializer)?;
        if let Some(fc) = FireboltCap::parse_long(cap.clone()) {
            Ok(fc)
        } else {
            Err(serde::de::Error::custom(format!(
                "Invalid capability: {}",
                cap.clone()
            )))
        }
    }
}

impl Eq for FireboltCap {}

impl PartialEq for FireboltCap {
    fn eq(&self, other: &Self) -> bool {
        self.as_str().eq(&other.as_str())
    }
}

impl Hash for FireboltCap {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_str().hash(state);
    }
}

impl FireboltCap {
    pub fn as_str(&self) -> String {
        let prefix = "xrn:firebolt:capability:";
        match self {
            Self::Full(s) => s.clone(),
            Self::Dab(drp) => match drp {
                DabRequestPayload::Device(drr) => {
                    format!("{}{}:{}", prefix, drp.as_ref(), drr.as_ref()).to_lowercase()
                }
                _ => "".into(),
            },
            Self::Short(s) => format!("{}{}", prefix, s).to_lowercase(),
        }
    }

    pub fn parse(cap: String) -> Option<FireboltCap> {
        let mut caps = cap.clone();
        if !cap.starts_with("xrn:firebolt:capability") {
            caps = "xrn:firebolt:capability:".to_string() + cap.as_str();
        }
        return FireboltCap::parse_long(caps);
    }

    pub fn parse_long(cap: String) -> Option<FireboltCap> {
        let pattern = r"^xrn:firebolt:capability:[a-z0-9\-]+:[a-z0-9\-]+(:[a-z0-9\-]+)?$";
        if !Regex::new(pattern).unwrap().is_match(cap.as_str()) {
            return None;
        }

        let prefix = vec!["xrn", "firebolt", "capability"];
        let c_a = cap.split(":");
        let mut cap_vec = Vec::<String>::new();
        for c in c_a.into_iter() {
            if !prefix.contains(&c) {
                cap_vec.push(String::from(c));
                if cap_vec.len() == 2 {
                    return Some(FireboltCap::Short(cap_vec.join(":")));
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod cap_tests {
    use super::FireboltCap;

    #[test]
    fn parse_cap() {
        assert_eq!(
            "xrn:firebolt:capability:input:keyboard",
            FireboltCap::parse("xrn:firebolt:capability:input:keyboard:email".into())
                .unwrap()
                .as_str()
                .as_str()
        );
        assert_eq!(
            "xrn:firebolt:capability:input:keyboard",
            FireboltCap::parse("input:keyboard:email".into())
                .unwrap()
                .as_str()
                .as_str()
        );
        assert_eq!(
            "xrn:firebolt:capability:input:keyboard",
            FireboltCap::parse("input:keyboard".into())
                .unwrap()
                .as_str()
                .as_str()
        );
    }
}

#[derive(Debug, Clone)]
pub struct CapAvailability {
    pub c: FireboltCap,
    pub a: Availability,
}

#[derive(Debug, Clone)]
pub struct CapSupport {
    pub dc: String,
    pub s: Support,
}

#[derive(Debug, Clone)]
pub struct BulkCapSupportProviderRequest {
    pub bulk_cap_support: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct CapManager {
    pub tx: MpscSender<CapRequest>,
}

#[derive(Debug)]
pub struct CapCallback {
    pub callback: Sender<Result<(), DenyReasonWithCap>>,
}

#[derive(Debug)]
pub struct CapMethodCheckCallback {
    pub callback: Sender<Result<(), DenyReasonWithCap>>,
}

#[derive(Debug)]
pub struct CapCheckCallback {
    pub callback: Sender<Result<Vec<CapabilityInfo>, DenyReason>>,
}

#[derive(Debug)]
pub enum CapRequest {
    IsAvailable(FireboltCap, CapCallback),
    UpdateAvailability(CapAvailability, Option<CapCallback>),
    IsSupported(FireboltCap, CapCallback),
    IsPermitted(CallContext, Vec<FireboltPermission>, CapCallback),
    PlainCheck(CapClassifiedRequest, CapCallback),
    BulkUpdateSupport(BulkCapSupportProviderRequest),
    UpdateHandlerSupport(Vec<RippleHandlerCaps>),
    CheckMethod(MethodPermissionRequest, CapMethodCheckCallback),
    CheckCaps(CallContext, Vec<String>, CapCheckCallback),
}

#[async_trait]
pub trait CapAvailabilityProvider<I = Self> {
    type Cap;
    fn get(self) -> Self::Cap;
    async fn update_cam(self, ca: Availability);
}

#[async_trait]
pub trait BulkCapSupportProvider<I = Self> {
    fn get(self) -> BulkCapSupportProviderRequest;
    async fn update_supported_capabilities(self, ca: CapManager);
}

#[derive(Debug, Clone, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum CapEvent {
    OnAvailable,
    OnUnavailable,
    OnGranted,
    OnRevoked,
}

impl CapEvent {
    pub fn as_str(self) -> String {
        serde_json::to_string(&self).unwrap()
    }
}

pub fn get_cap_event(event: &'static str) -> Option<CapEvent> {
    match event {
        "onAvailable" => Some(CapEvent::OnAvailable),
        "onUnavailable" => Some(CapEvent::OnUnavailable),
        "onGranted" => Some(CapEvent::OnGranted),
        "onRevoked" => Some(CapEvent::OnRevoked),
        _ => None,
    }
}

#[derive(Eq, Debug, Clone)]
pub struct CapEventEntry {
    pub cap: FireboltCap,
    pub event: CapEvent,
    pub app_id: String,
}

impl PartialEq for CapEventEntry {
    fn eq(&self, other: &Self) -> bool {
        self.cap.as_str().eq(&other.cap.as_str())
            && self.event == other.event
            && self.app_id.eq(&other.app_id)
    }
}

impl Hash for CapEventEntry {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.cap.as_str().hash(state);
        if let Ok(r) = serde_json::to_string(&self.event) {
            r.hash(state);
        }
        self.app_id.hash(state);
    }
}

#[derive(Debug, Clone, Default)]
pub struct CapEventState {
    primed_listeners: Arc<RwLock<HashSet<CapEventEntry>>>,
}

impl CapEventState {
    pub async fn setup_listener(
        ps: PlatformState,
        call_context: CallContext,
        event: CapEvent,
        cap: FireboltCap,
        listen: bool,
    ) {
        let mut r = ps.cap_state.events.primed_listeners.write().unwrap();
        let check = CapEventEntry {
            app_id: call_context.clone().app_id,
            cap: cap.clone(),
            event: event.clone(),
        };
        if listen {
            // Prime combo check
            // There are existing SDK protections against this scenario but this could happen when an app directly make requests
            // using WS. Ripple position with this scenario is Last in first out. It doesnt change the underlying impl
            if !r.contains(&check) {
                r.insert(check);
            }
        } else {
            r.remove(&check);
        }

        let event_name = format!(
            "{}.{}",
            "capabilities",
            serde_json::to_string(&event).unwrap()
        );
        debug!("setup event listener {}", event_name);
        AppEvents::add_listener(
            &ps.app_events_state,
            event_name,
            call_context.clone(),
            ListenRequest { listen },
        )
    }

    fn check_primed(
        ps: PlatformState,
        event: CapEvent,
        cap: FireboltCap,
        app_id: Option<String>,
    ) -> bool {
        let r = ps.cap_state.events.primed_listeners.read().unwrap();
        debug!("primed entries {:?}", r);
        if let Some(_) = r.iter().find(|x| {
            if x.event == event && x.cap == cap {
                if let Some(a) = app_id.clone() {
                    x.app_id.eq(&a)
                } else {
                    return true;
                }
            } else {
                return false;
            }
        }) {
            return true;
        }
        false
    }

    pub async fn emit(ps: PlatformState, event: CapEvent, cap: FireboltCap) {
        // check if given event and capability needs emitting
        if Self::check_primed(ps.clone(), event.clone(), cap.clone(), None) {
            let f = cap.clone().as_str();
            debug!("preparing cap event emit {}", f);
            // if its a grant or revoke it could be done per app
            // these require additional
            let is_app_check_necessary = match event.clone() {
                CapEvent::OnGranted | CapEvent::OnRevoked => true,
                _ => false,
            };
            let event_name = format!(
                "{}.{}",
                "capabilities",
                serde_json::to_string(&event).unwrap()
            );
            // App events current implementation can only send the same value for all the listeners
            // This wouldn't work for capability events because CapabilityInfo has information
            // pertaining to each app.
            // Additional processing and unique values are possible for the same event on each
            // listener
            // So Step 1: Get all listeners
            let listeners =
                AppEvents::get_listeners(&ps.app_events_state, event_name.as_str(), None);
            debug!("listener size {}", listeners.len());
            for listener in listeners {
                let cc = listener.call_ctx.clone();
                // Step 2: Check if the given event is valid for the app
                if is_app_check_necessary {
                    if !Self::check_primed(
                        ps.clone(),
                        event.clone(),
                        cap.clone(),
                        Some(cc.clone().app_id),
                    ) {
                        continue;
                    }
                }
                // Step 3: Get Capability info for each app based on context available in listener
                if let Ok(r) = ps.clone().services.get_cap_info(cc, vec![f.clone()]).await {
                    if let Some(cap_info) = r.get(0) {
                        if let Ok(data) = serde_json::to_value(cap_info) {
                            // Step 4: Send exclusive cap info data for each listener
                            AppEvents::send_event(&listener, &data).await;
                        }
                    }
                }
            }
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct CapState {
    supported: SupportedState,
    not_available: NotAvailableState,
    pub events: CapEventState,
}

pub fn handler_check_callback(
    cc: Option<CapCallback>,
    r: Result<(), DenyReasonWithCap>,
    channel_id: String,
) -> Option<Result<(), DenyReasonWithCap>> {
    if let Some(c) = cc {
        oneshot_send_and_log(c.callback, r, &channel_id);
        None
    } else {
        Some(r)
    }
}

pub fn cap_handler_callback<R>(cc: Option<Sender<R>>, r: R, channel_id: String) -> Option<R>
where
    R: std::fmt::Debug,
{
    if let Some(c) = cc {
        oneshot_send_and_log(c, r, &channel_id);
        None
    } else {
        Some(r)
    }
}

impl CapManager {
    pub fn get(
        tx: MpscSender<CapRequest>,
        rx: Receiver<CapRequest>,
        dpab_tx: MpscSender<DpabRequest>,
        config_manager: Box<ConfigManager>,
        ps: PlatformState,
    ) -> CapManager {
        tokio::spawn(async move {
            let cm = config_manager.clone();
            let exclusory = match cm
                .clone()
                .get_config(super::config_manager::ConfigRequest::Exclusory)
            {
                ConfigResponse::Exclusory(e) => e,
                _ => None,
            };
            debug!("Starting CamActor");
            Box::new(CamActor {
                resolver: CapabilityResolver::get(exclusory, cm.get_capability_dependencies()),
                permitted_cm: PermittedCapManager::get(dpab_tx, cm.clone(), ps.clone()),
                granted_cm: GrantCapManager::get(cm, ps.clone()),
                ps,
            })
            .run(rx)
            .await;
        });
        CapManager { tx }
    }
}

struct CamActor {
    resolver: CapabilityResolver,
    permitted_cm: PermittedCapManager,
    granted_cm: GrantCapManager,
    ps: PlatformState,
}

impl CamActor {
    async fn run(mut self: Box<Self>, mut rx: Receiver<CapRequest>) {
        loop {
            let rx_cam_listener = info_span!("cap availability");
            let cam_request = rx.recv().instrument(rx_cam_listener).await;
            if let Some(c) = cam_request {
                match c {
                    CapRequest::UpdateAvailability(ca, occ) => {
                        let ca_c = ca.clone();
                        info!("updating cam {} {:?}", ca.c.as_str(), ca_c.a.clone());
                        let firebolt_cap = ca.c;
                        let availability = ca.a;
                        let fc = firebolt_cap.clone();
                        let a = availability.clone();
                        // Only send events if availability is actually updated
                        // For cases like Keyboard there are multiple variants (Standard,Email,Password)
                        // for same capability. OpenRPC already makes sure that a provider has to
                        // implement all variants so keeping that good faith some resiliency logic
                        // is used here to avoid duplication
                        if let Some(_) = AvailableCapHandler::ingest(
                            self.ps.clone().cap_state.not_available,
                            match availability {
                                Availability::Ready => {
                                    CapClassifiedRequest::Available(vec![firebolt_cap])
                                }
                                _ => CapClassifiedRequest::NotAvailable(vec![firebolt_cap]),
                            },
                        ) {
                            let ps = self.ps.clone();
                            tokio::spawn(async move {
                                CapEventState::emit(
                                    ps,
                                    match a {
                                        Availability::Ready => CapEvent::OnAvailable,
                                        _ => CapEvent::OnUnavailable,
                                    },
                                    fc,
                                )
                                .await;
                            });
                        }

                        if let Some(cc) = occ {
                            oneshot_send_and_log(cc.callback, Ok(()), "cap_manager_callback");
                        }
                    }
                    CapRequest::IsAvailable(cap, cc) => {
                        AvailableCapHandler::check(
                            self.ps.clone().cap_state.not_available,
                            CapCheckRequest {
                                caps: vec![cap],
                                role: None,
                            },
                            Some(cc),
                        );
                    }
                    CapRequest::IsSupported(dc, cc) => {
                        SupportedCapHandler::check(
                            self.ps.clone().cap_state.supported,
                            CapCheckRequest {
                                caps: vec![dc],
                                role: None,
                            },
                            Some(cc),
                        );
                    }
                    CapRequest::IsPermitted(cc, cap, cb) => {
                        let r = self.permitted_cm.clone().check(cc, cap).await;
                        oneshot_send_and_log(cb.callback, r, "cap_manager_callback");
                    }
                    CapRequest::PlainCheck(c, cc) => match c {
                        CapClassifiedRequest::Supported(caps) => {
                            SupportedCapHandler::check(
                                self.ps.clone().cap_state.supported,
                                CapCheckRequest { caps, role: None },
                                Some(cc),
                            );
                        }
                        CapClassifiedRequest::Available(caps) => {
                            AvailableCapHandler::check(
                                self.ps.clone().cap_state.not_available,
                                CapCheckRequest { caps, role: None },
                                Some(cc),
                            );
                        }
                        CapClassifiedRequest::IsPermitted(c, r) => {
                            debug!("check_permission {:?}", r);
                            let r = self.permitted_cm.check(c, r).await;
                            oneshot_send_and_log(cc.callback, r, "cap_manager_callback");
                        }
                        CapClassifiedRequest::Grant(c, r) => {
                            // TODO why does this only run on the first capability??
                            if let Some(cap) = r.get(0) {
                                let fc = FireboltCap::Full(String::from(cap.as_str()));
                                let (r, gcm) = self.granted_cm.check(c.app_id, fc.clone());
                                let res = if r.is_err() {
                                    deny_cap(r.err().unwrap(), fc.as_str())
                                } else {
                                    Ok(())
                                };
                                self.granted_cm = gcm;
                                oneshot_send_and_log(cc.callback, res, "cap_manager_callback");
                            }
                        }
                        _ => todo!("TBD"),
                    },
                    CapRequest::BulkUpdateSupport(dc_a) => {
                        SupportedCapHandler::ingest(
                            self.ps.clone().cap_state.supported,
                            CapClassifiedRequest::Supported(
                                dc_a.bulk_cap_support
                                    .iter()
                                    .map(|dc_a_v| FireboltCap::Full(dc_a_v.into()))
                                    .collect(),
                            ),
                            None,
                        );
                    }
                    CapRequest::UpdateHandlerSupport(hs) => {
                        for handler in hs.iter() {
                            if let Some(caps) = handler.caps.clone() {
                                for cap in caps {
                                    let cap_vec = cap.all_caps();
                                    for c in cap_vec {
                                        let policy =
                                            self.resolver.get_capability_policy(c.as_str());
                                        if let None = policy {
                                            error!("Unknown capability being marked as supported or available: {}", c.as_str());
                                        }
                                    }
                                    SupportedCapHandler::ingest(
                                        self.ps.clone().cap_state.supported,
                                        cap.clone(),
                                        None,
                                    );

                                    AvailableCapHandler::ingest(
                                        self.ps.clone().cap_state.not_available,
                                        cap.clone(),
                                    );
                                }
                            }
                        }
                    }
                    CapRequest::CheckMethod(mpr, cc) => {
                        let rpc_request = mpr.clone().req;
                        let ctx = rpc_request.clone().ctx;
                        let m = rpc_request.method.clone();
                        let r = self.resolver.clone().get_caps_for_method(m);
                        /*
                        If capabilites are known for rpc_request.method, check
                        1) requested capability is in supported capabilities
                        2) Permission service grants call
                        */
                        let platform_state = self.ps.clone();
                        let permitted_cm = self.permitted_cm.clone();
                        let granted_cm = self.granted_cm.clone();
                        /*
                         * Spawning a new thread to do the heavy lifting in another thread and free this
                         * thread to read from the channel for processing the next request.
                         */
                        tokio::spawn(async move {
                            if let Some(rc) = r {
                                if let Some(cc) = SupportedCapHandler::check_caps_and_fail_close(
                                    platform_state.clone().cap_state.supported,
                                    rc.clone(),
                                    cc,
                                ) {
                                    if let Some(cc) = AvailableCapHandler::check_caps_and_fail_close(
                                        platform_state.clone().cap_state.not_available,
                                        rc.clone(),
                                        cc,
                                    ) {
                                        // Permissions needs better refactoring to use platform state mainly because it uses
                                        // DAB and DPAB we can revisit once Session is available in state. DPAB would then
                                        // be the only sender most probably available in state services
                                        let cr = permitted_cm.check(ctx.clone(), rc.clone()).await;
                                        if let Err(e) = cr {
                                            debug!("failed permitted");
                                            oneshot_send_and_log(
                                                cc.callback,
                                                Err(e),
                                                "cap_check_method",
                                            );
                                        } else {
                                            let cr =
                                                granted_cm.check_with_roles(&ctx, rc.clone()).await;

                                            oneshot_send_and_log(
                                                cc.callback,
                                                cr,
                                                "cap_check_method",
                                            );
                                        }
                                    } else {
                                        debug!("Caps are not available: {:?}", rc);
                                    }
                                } else {
                                    debug!("Caps are not supported: {:?}", rc);
                                }
                            } else {
                                debug!("No caps found");
                                oneshot_send_and_log(cc.callback, Ok(()), "cap_check_method");
                            }
                        });
                    }
                    CapRequest::CheckCaps(cc, caps, cb) => {
                        let sr = SupportedCapHandler::check_all(
                            self.ps.clone().cap_state.supported,
                            caps.clone(),
                        );
                        let ar = AvailableCapHandler::check_all(
                            self.ps.clone().cap_state.not_available,
                            caps.clone(),
                        );
                        let (pr, pcm) = self.permitted_cm.check_all(cc.clone(), caps.clone()).await;
                        self.permitted_cm = pcm;
                        let (gr, gcm) = self.granted_cm.check_all(cc.app_id, caps.clone());
                        self.granted_cm = gcm;
                        let cap_infos: Vec<CapabilityInfo> = caps
                            .into_iter()
                            .map(|x| {
                                let reason = if !sr.contains_key(&x) || !sr.get(&x).unwrap() {
                                    // Un supported
                                    Some(DenyReason::Unsupported)
                                } else if !ar.contains_key(&x) || !ar.get(&x).unwrap() {
                                    // Un Available
                                    Some(DenyReason::Unavailable)
                                } else if !pr.contains_key(&x) || !pr.get(&x).unwrap() {
                                    // Un Permitted
                                    Some(DenyReason::Unpermitted)
                                } else if let Some(r) = gr.get(&x) {
                                    if let Err(e) = r {
                                        let c_reason = e.clone();
                                        Some(c_reason)
                                    } else {
                                        None
                                    }
                                } else {
                                    None
                                };

                                CapabilityInfo::get(x.clone(), reason)
                            })
                            .collect();
                        oneshot_send_and_log(
                            cb.callback,
                            Ok(cap_infos),
                            "cap_manager_info_callback",
                        );
                    }
                }
            } else {
                error!("CamActor exiting");
                break;
            }
        }
    }
}

/// Any handler can provide 3 types of capabilities based on their inherent nature
/// 1. defined_caps: These are FireboltCap which has more information
/// 2. classified_caps: These are CapClassifiedRequest caps with more metadata on it which helps identify the type of capability
///
/// Handler provides information on what cap and classification they provide
/// # Examples
/// If Ripple handler for account wants to provide capabilities which it supports and the capabilities
/// it makes available. For Session when the device starts up there is no session then it becomes available from provider
/// ```
/// CapClassifiedRequest::Supported(vec![
/// FireboltCap::Short("account:session".into()),
/// FireboltCap::Short("account:id".into()),
/// FireboltCap::Short("account:uid".into()),
/// ]),
/// CapClassifiedRequest::NotAvailable(vec![FireboltCap::Session]),
/// ```
#[derive(Debug, Clone)]
pub struct RippleHandlerCaps {
    pub caps: Option<Vec<CapClassifiedRequest>>,
}

impl RippleHandlerCaps {
    // fn get_cap_request(self) -> Vec<String> {
    //     let mut available_caps: Vec<String> = Vec::new();
    //     let p_c = self.plain_caps;
    //     let d_c = self.defined_caps;
    //     if let Some(caps) = p_c {
    //         let mut r = caps.iter().map(|e| e.clone()).collect::<Vec<String>>();
    //         available_caps.append(&mut r);
    //     }

    //     if let Some(caps) = d_c {
    //         let mut r = caps
    //             .iter()
    //             .map(|e| e.as_str().clone())
    //             .collect::<Vec<String>>();
    //         available_caps.append(&mut r);
    //     }

    //     available_caps
    // }
}

pub struct CapInitParams {
    pub initial_caps: Option<CapClassifiedRequest>,
    pub rhf: Option<RippleHelperFactory>,
}

pub struct CheckWithRolesRequest {
    pub permissions: Vec<FireboltPermission>,
    pub grant_request: Option<MethodPermissionRequest>,
}

pub trait IGetLoadedCaps {
    fn get_loaded_caps(&self) -> RippleHandlerCaps;
}

#[cfg(test)]
mod tests {
    use crate::managers::capability::permission_store::AppPermissionsState;
    use crate::managers::event::event_manager_state::EventManagerState;
    use crate::platform_state::DeviceSessionIdentifier;
    use crate::{
        api::{
            permissions::{api_permissions::MethodPermissionRequest, user_grants::UserGrantState},
            rpc::{
                api_messages::{ApiProtocol, RpcRequest},
                rpc_gateway::CallContext,
            },
        },
        apps::{
            app_auth_sessions::AppAuthSessions,
            app_events::AppEventsState,
            app_library::AppLibraryState,
            app_mgr::{AppManager, AppSessionsState},
            provider_broker::ProviderBrokerState,
        },
        helpers::ripple_helper::RippleHelperFactory,
        managers::{
            capability_manager::{
                Availability, BulkCapSupportProvider, CapAvailability, CapCallback,
                CapCheckCallback, CapClassifiedRequest, CapManager, CapMethodCheckCallback,
                CapRequest, CapState, CapabilityInfo, DabRequestPayload, DenyReason,
                DenyReasonWithCap, FireboltCap, RippleHandlerCaps,
            },
            capability_resolver::FireboltOpenRpc,
            config_manager::ConfigManager,
            metrics_manager::MetricsRequest,
        },
        platform_state::PlatformState,
    };
    use dab::core::model::device::DeviceRequest;
    use dpab::core::message::Role;
    use tokio::sync::{
        mpsc,
        oneshot::{channel, Receiver},
    };
    use tracing::{debug, info};

    use super::{CapabilityRole, FireboltPermission};
    #[test]
    fn test_dp_string_enum() {
        assert_eq!(
            "xrn:firebolt:capability:device:model",
            FireboltCap::Dab(DabRequestPayload::Device(DeviceRequest::Model)).as_str()
        )
    }

    #[tokio::test]
    async fn test_cap_manager() {
        let test_thread = tokio::spawn(async move {
            run_test().await;
        });
        let _result = test_thread.await.unwrap();
    }

    async fn get_response(rx: Receiver<Result<(), DenyReasonWithCap>>) {
        if let Ok(cr) = rx.await {
            info!("Response {:?}", cr);
        } else {
            info!("Response is Unavailable");
        }
    }

    async fn run_test() {
        let cm = ConfigManager::get();
        let (cap_tx, cap_rx) = mpsc::channel::<CapRequest>(30);
        let (dab_tx, _dab_rx) = mpsc::channel(32);
        let (dpab_tx, _dpab_rx) = mpsc::channel(32);
        let (app_mgr_req_tx, _app_mgr_req_rx) = mpsc::channel(32);
        let default_apps = cm.clone().get_default_apps();
        let (metrics_events_tx, _metrics_events_rx) = mpsc::channel::<MetricsRequest>(32);
        #[cfg(any(feature = "only_launcher", feature = "gateway_with_launcher"))]
        let (container_mgr_req_tx, _container_mgr_req_rx) = mpsc::channel(32);
        #[cfg(any(feature = "only_launcher", feature = "gateway_with_launcher"))]
        let (view_mgr_req_tx, _view_mgr_req_rx) = mpsc::channel(32);

        #[cfg(any(feature = "only_launcher"))]
        let (gs, _gr) = mpsc::channel(32);

        let rhf = RippleHelperFactory {
            app_mgr_req_tx,
            cap_tx: cap_tx.clone(),
            cm: cm.clone(),
            dab_tx: dab_tx.clone(),
            dpab_tx: dpab_tx.clone(),
            #[cfg(any(feature = "only_launcher", feature = "gateway_with_launcher"))]
            container_mgr_req_tx: Some(container_mgr_req_tx.clone()),
            #[cfg(any(feature = "only_launcher", feature = "gateway_with_launcher"))]
            view_mgr_req_tx: Some(view_mgr_req_tx.clone()),
            #[cfg(feature = "only_launcher")]
            gateway_req_tx: Some(gs),
            metrics_context_tx: metrics_events_tx,
        };

        let platform_state = PlatformState {
            app_events_state: AppEventsState::default(),
            app_library_state: AppLibraryState::new(default_apps),
            provider_broker_state: ProviderBrokerState::default(),
            services: *rhf.clone().get_all(),
            app_sessions_state: AppSessionsState::default(),
            grant_state: UserGrantState::default(),
            cap_state: CapState::default(),
            app_auth_sessions: AppAuthSessions::default(),
            em_state: EventManagerState::default(),
            firebolt_open_rpc: FireboltOpenRpc::default(),
            device_session_id: DeviceSessionIdentifier::default(),
            app_permissions_state: AppPermissionsState::default(),
        };

        let cap = CapManager::get(
            cap_tx,
            cap_rx,
            dpab_tx.clone(),
            cm.clone(),
            platform_state.clone(),
        );
        let cap_cm = cm.clone();
        let cap_c = cap.clone();
        tokio::spawn(async move {
            Box::new(cap_cm).update_supported_capabilities(cap_c).await;
        });

        {
            let (tx, rx) = channel::<Result<(), DenyReasonWithCap>>();
            let cb = CapCallback { callback: tx };
            let c = FireboltCap::Short("closedcaptions:enabled".into());
            let req = CapRequest::IsAvailable(c, cb);
            let _result = cap.tx.clone().send(req).await.unwrap();
            get_response(rx).await;
        }
        {
            let (tx, rx) = channel::<Result<(), DenyReasonWithCap>>();
            let c = CapAvailability {
                c: FireboltCap::Short("account:session".into()),
                a: Availability::Ready,
            };
            let req = CapRequest::UpdateAvailability(c, Some(CapCallback { callback: tx }));
            let _result = cap.tx.clone().send(req).await.unwrap();
            get_response(rx).await;
        }
        {
            let (tx, rx) = channel::<Result<(), DenyReasonWithCap>>();
            let cb = CapCallback { callback: tx };
            let req = CapRequest::IsSupported(FireboltCap::short("remote:rf4ce"), cb);
            let _result = cap.tx.clone().send(req).await.unwrap();
            get_response(rx).await;
        }
        {
            let (tx, rx) = channel::<Result<(), DenyReasonWithCap>>();
            let cb = CapCallback { callback: tx };
            let c = CapClassifiedRequest::Supported(vec![FireboltCap::Full(String::from(
                "protocol:wifi",
            ))]);
            let req = CapRequest::PlainCheck(c, cb);
            let _result = cap.tx.clone().send(req).await.unwrap();
            get_response(rx).await;
        }
        {
            let (tx, rx) = channel::<Result<(), DenyReasonWithCap>>();
            let cb = CapCallback { callback: tx };
            let c = CapClassifiedRequest::Available(vec![FireboltCap::Full(String::from("test"))]);
            let req = CapRequest::PlainCheck(c, cb);
            let _result = cap.tx.clone().send(req).await.unwrap();
            get_response(rx).await;
        }
        {
            let app_session_state = AppSessionsState::default();
            AppManager::add_app_session(&app_session_state, "test".into(), "cert".into());

            let (tx, rx) = channel::<Result<Vec<CapabilityInfo>, DenyReason>>();
            let ccb = CapCheckCallback { callback: tx };

            let ctx = CallContext {
                session_id: String::from("test"),
                request_id: String::from("100"),
                app_id: String::from("cert"),
                call_id: rand::random::<u64>(),
                protocol: ApiProtocol::JsonRpc,
                method: "wif.scan".to_string(),
            };

            let vec = vec![
                String::from("wifi"),
                String::from("test1"),
                String::from("test2"),
                String::from("test3"),
            ];
            let req = CapRequest::CheckCaps(ctx, vec, ccb);
            let _result = cap.tx.clone().send(req).await;
            if let Ok(cr) = rx.await {
                info!("CheckCaps response {:?}", cr);
            } else {
                debug!("CheckCaps response is Unavailable");
            }
        }
        {
            let mut vec = Vec::new();
            let caps = RippleHandlerCaps {
                caps: Some(vec![CapClassifiedRequest::Supported(vec![
                    FireboltCap::Short("protocol:wifi".into()),
                ])]),
            };
            vec.push(caps);
            let req = CapRequest::UpdateHandlerSupport(vec);
            let _result = cap.tx.clone().send(req).await.unwrap();
        }
        {
            let (tx, rx) = channel::<Result<(), DenyReasonWithCap>>();
            let cmccb = CapMethodCheckCallback { callback: tx };

            let call_context = CallContext {
                session_id: String::from("session_id"),
                app_id: String::from("app_id"),
                call_id: 18_446_744_073_709_551_615u64,
                protocol: ApiProtocol::JsonRpc,
                method: String::from("method"),
                request_id: String::from("reqId"),
            };

            let rpc_request = RpcRequest {
                method: String::from("method"),
                params_json: String::from("json"),
                ctx: call_context,
            };

            let mpr = MethodPermissionRequest { req: rpc_request };

            let req = CapRequest::CheckMethod(mpr, cmccb);
            let _result = cap.tx.clone().send(req).await;
            if let Ok(cr) = rx.await {
                info!("CheckMethod response {:?}", cr);
            } else {
                debug!("CheckMethod response is Unavailable");
            }
        }
    }

    #[tokio::test]
    async fn test_cap_role_from() {
        let r: Role = CapabilityRole::Use.into();
        assert!(matches!(r, Role::Use));
        let r: Role = CapabilityRole::Manage.into();
        assert!(matches!(r, Role::Manage));
        let r: Role = CapabilityRole::Provide.into();
        assert!(matches!(r, Role::Provide));
    }

    #[tokio::test]
    async fn test_serialize_permission() {
        let perm = FireboltPermission {
            cap: FireboltCap::Short(String::from("device:id")),
            role: CapabilityRole::Use,
        };
        let json = serde_json::to_string(&perm).unwrap();
        assert_eq!(json, String::from("\"xrn:firebolt:capability:device:id\""));
        assert_eq!(
            serde_json::from_str::<FireboltPermission>(&json).unwrap(),
            perm
        );

        let perm = FireboltPermission {
            cap: FireboltCap::Short(String::from("input:keyboard")),
            role: CapabilityRole::Provide,
        };
        let json = serde_json::to_string(&perm).unwrap();
        assert_eq!(
            json,
            String::from("\"xrn:firebolt:capability:input:keyboard[provide]\"")
        );
        assert_eq!(
            serde_json::from_str::<FireboltPermission>(&json).unwrap(),
            perm
        );

        let perm = FireboltPermission {
            cap: FireboltCap::Short(String::from("device:name")),
            role: CapabilityRole::Manage,
        };
        let json = serde_json::to_string(&perm).unwrap();
        assert_eq!(
            json,
            String::from("\"xrn:firebolt:capability:device:name[manage]\"")
        );
        assert_eq!(
            serde_json::from_str::<FireboltPermission>(&json).unwrap(),
            perm
        );
    }

    #[tokio::test]
    async fn test_firebolt_full_cap() {
        let fc = FireboltCap::Full(String::from("xrn:firebolt:capability:device:id"));
        let json = serde_json::to_string(&fc).unwrap();
        assert_eq!(json, String::from("\"xrn:firebolt:capability:device:id\""));
        let cap = serde_json::from_str::<FireboltCap>(&json);
        assert!(cap.is_ok());
        assert_eq!(cap.unwrap(), fc);
    }

    #[tokio::test]
    async fn test_firebolt_short_cap() {
        let fc = FireboltCap::Short(String::from("device:mac"));
        let json = serde_json::to_string(&fc).unwrap();
        assert_eq!(json, String::from("\"xrn:firebolt:capability:device:mac\""));
        let cap = serde_json::from_str::<FireboltCap>(&json);
        assert!(cap.is_ok());
        assert_eq!(cap.unwrap(), fc);
    }

    #[tokio::test]
    async fn test_firebolt_cap_alpha_num() {
        let fc = FireboltCap::Short(String::from("device123:mac123"));
        let json = serde_json::to_string(&fc).unwrap();
        let cap = serde_json::from_str::<FireboltCap>(&json);
        assert!(cap.is_ok());
        assert_eq!(cap.unwrap(), fc);
    }

    #[tokio::test]
    async fn test_firebolt_cap_invalid() {
        let fc = FireboltCap::Short(String::from("device"));
        let json = serde_json::to_string(&fc).unwrap();
        let cap = serde_json::from_str::<FireboltCap>(&json);
        assert!(cap.is_err());
    }

    #[tokio::test]
    async fn test_firebolt_cap_special_chars_invalid() {
        let fc = FireboltCap::Short(String::from("device123*:mac123"));
        let json = serde_json::to_string(&fc).unwrap();
        let cap = serde_json::from_str::<FireboltCap>(&json);
        assert!(cap.is_err());
    }

    #[tokio::test]
    async fn test_firebolt_cap_special_chars_invalid1() {
        let fc = FireboltCap::Short(String::from("device123:mac123*"));
        let json = serde_json::to_string(&fc).unwrap();
        let cap = serde_json::from_str::<FireboltCap>(&json);
        assert!(cap.is_err());
    }
}
