use std::{
    collections::HashSet,
    hash::{Hash, Hasher},
    sync::{Arc, RwLock},
};

use crate::{service::apps::app_events::AppEvents, state::platform_state::PlatformState};
use ripple_sdk::serde_json;
use ripple_sdk::{
    api::{
        firebolt::{
            fb_capabilities::{
                CapEvent, CapListenRPCRequest, CapabilityInfo, CapabilityRole, DenyReason,
                FireboltCap,
            },
            fb_general::ListenRequest,
            fb_openrpc::CapabilitySet,
        },
        gateway::rpc_gateway_api::CallContext,
        manifest::device_manifest::DeviceManifest,
    },
    log::debug,
    utils::error::RippleError,
};

use super::{
    generic_cap_state::GenericCapState,
    permitted_state::{PermissionHandler, PermittedState},
};

#[derive(Debug, Clone)]
pub struct CapState {
    pub generic: GenericCapState,
    pub permitted_state: PermittedState,
    primed_listeners: Arc<RwLock<HashSet<CapEventEntry>>>,
    // add user grant state here
}

impl CapState {
    pub fn new(manifest: DeviceManifest) -> Self {
        CapState {
            generic: GenericCapState::default(),
            permitted_state: PermittedState::new(manifest),
            primed_listeners: Arc::new(RwLock::new(HashSet::new())),
        }
    }

    pub async fn setup_listener(
        ps: &PlatformState,
        call_context: CallContext,
        event: CapEvent,
        request: CapListenRPCRequest,
    ) {
        let mut r = ps.cap_state.primed_listeners.write().unwrap();
        if let Some(cap) = FireboltCap::parse(request.clone().capability) {
            let check = CapEventEntry {
                app_id: call_context.clone().app_id,
                cap: cap,
                event: event.clone(),
                role: request.role,
            };
            if request.listen {
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
                ListenRequest {
                    listen: request.listen,
                },
            )
        }
    }

    fn check_primed(
        ps: &PlatformState,
        event: CapEvent,
        cap: FireboltCap,
        app_id: Option<String>,
    ) -> bool {
        let r = ps.cap_state.primed_listeners.read().unwrap();
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

    pub async fn emit(ps: &PlatformState, event: CapEvent, cap: FireboltCap) {
        // check if given event and capability needs emitting
        if Self::check_primed(ps, event.clone(), cap.clone(), None) {
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
                    if !Self::check_primed(ps, event.clone(), cap.clone(), Some(cc.clone().app_id))
                    {
                        continue;
                    }
                }
                // Step 3: Get Capability info for each app based on context available in listener
                if let Ok(r) = Self::get_cap_info(ps, cc, vec![f.clone()]).await {
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

    pub async fn get_cap_info(
        state: &PlatformState,
        call_context: CallContext,
        caps: Vec<String>,
    ) -> Result<Vec<CapabilityInfo>, RippleError> {
        let mut unsupported_caps = Vec::new();
        let generic_caps = FireboltCap::from_vec_string(caps.clone());
        if let Err(e) = GenericCapState::check_supported(&state.cap_state.generic, &generic_caps) {
            unsupported_caps.extend(e.caps);
        }

        let mut unavailable_caps = Vec::new();
        if let Err(e) = GenericCapState::check_supported(&state.cap_state.generic, &generic_caps) {
            unavailable_caps.extend(e.caps);
        }

        let mut unpermitted_caps = Vec::new();
        let cap_set = CapabilitySet {
            use_caps: Some(generic_caps.clone()),
            manage_caps: None,
            provide_cap: None,
        };
        if let Err(e) =
            PermissionHandler::check_permitted(state, call_context.app_id, cap_set).await
        {
            unpermitted_caps.extend(e.caps);
        }

        let cap_infos: Vec<CapabilityInfo> = generic_caps
            .into_iter()
            .map(|x| {
                let reason = if unsupported_caps.contains(&x) {
                    // Un supported
                    Some(DenyReason::Unsupported)
                } else if unavailable_caps.contains(&x) {
                    // Un Available
                    Some(DenyReason::Unavailable)
                } else if unpermitted_caps.contains(&x) {
                    // Un Permitted
                    Some(DenyReason::Unpermitted)
                } else {
                    None
                };

                CapabilityInfo::get(x.as_str(), reason)
            })
            .collect();

        return Ok(cap_infos);
    }
}

#[derive(Eq, Debug, Clone)]
pub struct CapEventEntry {
    pub cap: FireboltCap,
    pub event: CapEvent,
    pub app_id: String,
    pub role: Option<CapabilityRole>,
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
