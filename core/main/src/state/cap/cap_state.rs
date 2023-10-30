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
    collections::HashSet,
    hash::{Hash, Hasher},
    sync::{Arc, RwLock},
};

use crate::{
    service::{apps::app_events::AppEvents, user_grants::GrantState},
    state::platform_state::PlatformState,
};
use ripple_sdk::{api::firebolt::fb_capabilities::RolePermission, serde_json};
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
    pub grant_state: GrantState,
}

impl CapState {
    pub fn new(manifest: DeviceManifest) -> Self {
        CapState {
            generic: GenericCapState::new(manifest.clone()),
            permitted_state: PermittedState::new(manifest.clone()),
            primed_listeners: Arc::new(RwLock::new(HashSet::new())),
            grant_state: GrantState::new(manifest),
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
                cap,
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
                ps,
                event_name,
                call_context,
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
        if r.iter().any(|x| {
            if x.event == event && x.cap == cap {
                if let Some(a) = app_id.clone() {
                    x.app_id.eq(&a)
                } else {
                    true
                }
            } else {
                false
            }
        }) {
            return true;
        }
        false
    }

    pub async fn emit(
        ps: &PlatformState,
        event: CapEvent,
        cap: FireboltCap,
        role: Option<CapabilityRole>,
    ) {
        match event.clone() {
            CapEvent::OnAvailable => ps
                .clone()
                .cap_state
                .generic
                .ingest_availability(vec![cap.clone()], true),
            CapEvent::OnUnavailable => ps
                .clone()
                .cap_state
                .generic
                .ingest_availability(vec![cap.clone()], false),
            _ => {}
        }
        // check if given event and capability needs emitting
        if Self::check_primed(ps, event.clone(), cap.clone(), None) {
            let f = cap.clone().as_str();
            debug!("preparing cap event emit {}", f);
            // if its a grant or revoke it could be done per app
            // these require additional
            let is_app_check_necessary =
                matches!(event.clone(), CapEvent::OnGranted | CapEvent::OnRevoked);
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
                if is_app_check_necessary
                    && !Self::check_primed(ps, event.clone(), cap.clone(), Some(cc.clone().app_id))
                {
                    continue;
                }
                let caps = vec![cap.clone()];
                let request =
                    CapabilitySet::get_from_role(caps, Some(role.unwrap_or(CapabilityRole::Use)));

                // Step 3: Get Capability info for each app based on context available in listener
                if let Ok(r) = Self::get_cap_info(ps, cc, &request.get_caps()).await {
                    if let Some(cap_info) = r.get(0) {
                        if let Ok(data) = serde_json::to_value(cap_info) {
                            debug!("data={:?}", data);
                            // Step 4: Send exclusive cap info data for each listener
                            AppEvents::send_event(ps, &listener, &data).await;
                        }
                    }
                }
            }
        }
    }

    pub async fn get_cap_info(
        state: &PlatformState,
        call_context: CallContext,
        firebolt_caps: &Vec<FireboltCap>,
    ) -> Result<Vec<CapabilityInfo>, RippleError> {
        // let mut unsupported_caps = Vec::new();
        let mut capability_infos = Vec::new();
        for cap in firebolt_caps {
            let mut capability_info = CapabilityInfo {
                capability: cap.as_str(),
                supported: false,
                available: false,
                _use: RolePermission {
                    permitted: false,
                    granted: None,
                },
                manage: RolePermission {
                    permitted: false,
                    granted: None,
                },
                provide: RolePermission {
                    permitted: false,
                    granted: None,
                },
                details: None,
            };
            capability_info.supported = state
                .cap_state
                .generic
                .check_supported(&[cap.clone().into()])
                .is_ok();

            capability_info.available = state
                .cap_state
                .generic
                .check_available(&vec![cap.clone().into()])
                .is_ok();
            (
                capability_info._use.permitted,
                capability_info.manage.permitted,
                capability_info.provide.permitted,
            ) = PermissionHandler::check_all_permitted(state, &call_context.app_id, &cap.as_str())
                .await;

            (
                capability_info._use.granted,
                capability_info.manage.granted,
                capability_info.provide.granted,
            ) = GrantState::check_all_granted(state, &call_context.app_id, &cap.as_str());
            let mut deny_reasons = Vec::new();
            if !capability_info.supported {
                deny_reasons.push(DenyReason::Unsupported);
            }
            if !capability_info.available {
                deny_reasons.push(DenyReason::Unavailable);
            }
            if !capability_info._use.permitted
                || !capability_info.manage.permitted
                || !capability_info.provide.permitted
            {
                deny_reasons.push(DenyReason::Unpermitted);
            }
            if capability_info._use.granted.is_none()
                || capability_info.manage.granted.is_none()
                || capability_info.provide.granted.is_none()
            {
                deny_reasons.push(DenyReason::Ungranted);
            }
            if (capability_info._use.granted.is_some() && !capability_info._use.granted.unwrap())
                || (capability_info.manage.granted.is_some()
                    && !capability_info.manage.granted.unwrap())
                || (capability_info.provide.granted.is_some()
                    && !capability_info.provide.granted.unwrap())
            {
                deny_reasons.push(DenyReason::GrantDenied);
            }
            if !deny_reasons.is_empty() {
                let _ = capability_info.details.insert(deny_reasons);
            }
            capability_infos.push(capability_info);
        }
        Ok(capability_infos)
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
