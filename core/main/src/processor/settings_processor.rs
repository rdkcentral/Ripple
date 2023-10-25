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

use std::collections::HashMap;

use ripple_sdk::{
    api::{
        device::device_events::VOICE_GUIDANCE_ENABLED_CHANGED,
        firebolt::{
            fb_capabilities::{CapabilityRole, FireboltCap, RoleInfo},
            fb_general::ListenRequest,
        },
        gateway::rpc_gateway_api::CallContext,
        settings::{SettingKey, SettingValue, SettingsRequest},
        storage_property::{
            EVENT_ALLOW_PERSONALIZATION_CHANGED, EVENT_ALLOW_WATCH_HISTORY_CHANGED,
            EVENT_CLOSED_CAPTIONS_ENABLED, EVENT_DEVICE_NAME_CHANGED, EVENT_SHARE_WATCH_HISTORY,
        },
    },
    async_trait::async_trait,
    extn::{
        client::extn_processor::{
            DefaultExtnStreamer, ExtnRequestProcessor, ExtnStreamProcessor, ExtnStreamer,
        },
        extn_client_message::{ExtnMessage, ExtnResponse},
    },
    log::warn,
    tokio::sync::mpsc::{Receiver as MReceiver, Sender as MSender},
    utils::error::RippleError,
};
use serde_json::{json, Value};

use crate::{
    firebolt::handlers::{
        capabilities_rpc::is_permitted,
        closed_captions_rpc::ClosedcaptionsImpl,
        device_rpc::get_device_name,
        discovery_rpc::DiscoveryImpl,
        voice_guidance_rpc::{
            voice_guidance_settings_enabled, voice_guidance_settings_enabled_changed,
        },
    },
    service::apps::app_events::{AppEventDecorationError, AppEventDecorator, AppEvents},
    state::platform_state::PlatformState,
};

#[derive(Clone, Debug)]
struct SettingsChangeEventDecorator {
    keys: Vec<String>,
}

#[async_trait]
impl AppEventDecorator for SettingsChangeEventDecorator {
    async fn decorate(
        &self,
        _state: &PlatformState,
        _ctx: &CallContext,
        _event_name: &str,
        val: &Value,
    ) -> Result<Value, AppEventDecorationError> {
        let keys = self.keys.clone();
        let data = match val {
            Value::Bool(bool_val) => {
                let objects = keys
                    .iter()
                    .filter_map(|key| {
                        if !key.is_empty() {
                            Some((key.clone(), json!({ "enabled": bool_val })))
                        } else {
                            None
                        }
                    })
                    .collect::<serde_json::Map<String, Value>>();

                json!(objects)
            }
            Value::String(string_val) => {
                let key = keys.last().cloned().unwrap_or_default();
                json!({ key: { "value": string_val } })
            }
            _ => json!({}),
        };

        Ok(serde_json::to_value(data).unwrap())
    }

    fn dec_clone(&self) -> Box<dyn AppEventDecorator + Send + Sync> {
        Box::new(self.clone())
    }
}

/// Supports processing of [Config] request from extensions and also
/// internal services.
#[derive(Debug)]
pub struct SettingsProcessor {
    state: PlatformState,
    streamer: DefaultExtnStreamer,
}

impl SettingsProcessor {
    pub fn new(state: PlatformState) -> SettingsProcessor {
        SettingsProcessor {
            state,
            streamer: DefaultExtnStreamer::new(),
        }
    }

    async fn get(
        state: &PlatformState,
        msg: ExtnMessage,
        ctx: CallContext,
        request: Vec<SettingKey>,
    ) -> bool {
        if let Ok(cp) = DiscoveryImpl::get_content_policy(&ctx, state, &ctx.app_id).await {
            let mut settings = HashMap::default();
            for sk in request {
                use SettingKey::*;
                let val = match sk.clone() {
                    VoiceGuidanceEnabled => {
                        let enabled = voice_guidance_settings_enabled(state)
                            .await
                            .unwrap_or(false);
                        Some(SettingValue::bool(enabled))
                    }
                    ClosedCaptions => {
                        let enabled = ClosedcaptionsImpl::cc_enabled(state).await.unwrap_or(false);
                        Some(SettingValue::bool(enabled))
                    }
                    AllowPersonalization => Some(SettingValue::bool(cp.enable_recommendations)),
                    AllowWatchHistory => Some(SettingValue::bool(cp.remember_watched_programs)),
                    ShareWatchHistory => Some(SettingValue::bool(cp.share_watch_history)),
                    DeviceName => Some(SettingValue::string(
                        get_device_name(state).await.unwrap_or_else(|_| "".into()),
                    )),
                    PowerSaving => Some(SettingValue::bool(true)),
                    LegacyMiniGuide => Some(SettingValue::bool(false)),
                };

                if let Some(v) = val {
                    let role_info = RoleInfo {
                        role: Some(CapabilityRole::Use),
                        capability: FireboltCap::Short(sk.use_capability().into()).as_str(),
                    };
                    if let Ok(result) = is_permitted(state.clone(), ctx.clone(), role_info).await {
                        if result {
                            settings.insert(sk.to_string(), v);
                        }
                    }
                }
            }
            return Self::respond(
                state.get_client().get_extn_client(),
                msg,
                ExtnResponse::Settings(settings),
            )
            .await
            .is_ok();
        }

        Self::handle_error(
            state.get_client().get_extn_client(),
            msg,
            RippleError::ProcessorError,
        )
        .await
    }

    fn subscribe_event(
        state: &PlatformState,
        ctx: CallContext,
        event_name: &str,
        keys: Vec<String>,
    ) {
        AppEvents::add_listener_with_decorator(
            state,
            event_name.to_string(),
            ctx,
            ListenRequest { listen: true },
            Some(Box::new(SettingsChangeEventDecorator { keys })),
        );
    }

    async fn subscribe_to_settings(
        state: &PlatformState,
        msg: ExtnMessage,
        ctx: CallContext,
        req: Vec<SettingKey>,
    ) -> bool {
        let mut vg_keys: Vec<String> = Vec::new();
        let mut cc_keys: Vec<String> = Vec::new();
        for key in req {
            match key {
                SettingKey::VoiceGuidanceEnabled => {
                    vg_keys.push(key.clone().to_string());
                    voice_guidance_settings_enabled_changed(
                        state,
                        &ctx,
                        &ListenRequest { listen: true },
                    )
                    .await
                    .ok();
                    Self::subscribe_event(
                        state,
                        ctx.clone(),
                        VOICE_GUIDANCE_ENABLED_CHANGED,
                        vg_keys.clone(),
                    )
                }
                SettingKey::ClosedCaptions => {
                    cc_keys.push(key.clone().to_string());
                    Self::subscribe_event(
                        state,
                        ctx.clone(),
                        EVENT_CLOSED_CAPTIONS_ENABLED,
                        cc_keys.clone(),
                    )
                }
                SettingKey::AllowPersonalization => Self::subscribe_event(
                    state,
                    ctx.clone(),
                    EVENT_ALLOW_PERSONALIZATION_CHANGED,
                    vec![key.to_string()],
                ),
                SettingKey::AllowWatchHistory => Self::subscribe_event(
                    state,
                    ctx.clone(),
                    EVENT_ALLOW_WATCH_HISTORY_CHANGED,
                    vec![key.to_string()],
                ),
                SettingKey::ShareWatchHistory => Self::subscribe_event(
                    state,
                    ctx.clone(),
                    EVENT_SHARE_WATCH_HISTORY,
                    vec![key.to_string()],
                ),
                SettingKey::DeviceName => Self::subscribe_event(
                    state,
                    ctx.clone(),
                    EVENT_DEVICE_NAME_CHANGED,
                    vec![key.to_string()],
                ),
                SettingKey::PowerSaving | SettingKey::LegacyMiniGuide => {
                    warn!("{} Not implemented", key.to_string())
                }
            }
        }
        Self::respond(
            state.get_client().get_extn_client(),
            msg,
            ExtnResponse::None(()),
        )
        .await
        .is_ok()
    }
}

impl ExtnStreamProcessor for SettingsProcessor {
    type STATE = PlatformState;
    type VALUE = SettingsRequest;
    fn get_state(&self) -> Self::STATE {
        self.state.clone()
    }

    fn sender(&self) -> MSender<ExtnMessage> {
        self.streamer.sender()
    }

    fn receiver(&mut self) -> MReceiver<ExtnMessage> {
        self.streamer.receiver()
    }
}

#[async_trait]
impl ExtnRequestProcessor for SettingsProcessor {
    fn get_client(&self) -> ripple_sdk::extn::client::extn_client::ExtnClient {
        self.state.get_client().get_extn_client()
    }

    async fn process_request(
        state: Self::STATE,
        msg: ExtnMessage,
        extracted_message: Self::VALUE,
    ) -> bool {
        match extracted_message {
            SettingsRequest::Get(ctx, request) => Self::get(&state, msg, ctx, request).await,
            SettingsRequest::Subscribe(ctx, request) => {
                Self::subscribe_to_settings(&state, msg, ctx, request).await
            }
        }
    }
}
