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
        distributor::distributor_privacy::ContentListenRequest,
        firebolt::{
            fb_capabilities::{CapabilityRole, FireboltCap, RoleInfo},
            fb_general::ListenRequest,
        },
        gateway::rpc_gateway_api::CallContext,
        settings::{SettingKey, SettingValue, SettingsRequest, SettingsRequestParam},
        storage_property::{
            EVENT_ALLOW_PERSONALIZATION_CHANGED, EVENT_ALLOW_WATCH_HISTORY_CHANGED,
            EVENT_SHARE_WATCH_HISTORY,
        },
    },
    async_trait::async_trait,
    extn::{
        client::extn_processor::{
            DefaultExtnStreamer, ExtnRequestProcessor, ExtnStreamProcessor, ExtnStreamer,
        },
        extn_client_message::{ExtnMessage, ExtnResponse},
    },
    log::{debug, warn},
    tokio::sync::mpsc::{Receiver as MReceiver, Sender as MSender},
    utils::error::RippleError,
};
use serde_json::{json, Value};

use crate::{
    broker::broker_utils::{self, BrokerUtils},
    firebolt::handlers::{
        capabilities_rpc::is_permitted, closed_captions_rpc::ClosedcaptionsImpl,
        discovery_rpc::DiscoveryImpl, privacy_rpc::PrivacyImpl,
        voice_guidance_rpc::voice_guidance_settings_enabled,
    },
    service::apps::app_events::{AppEventDecorationError, AppEventDecorator, AppEvents},
    state::platform_state::PlatformState,
};

#[derive(Clone, Debug)]
struct SettingsChangeEventDecorator {
    request: SettingsRequestParam,
}

#[async_trait]
impl AppEventDecorator for SettingsChangeEventDecorator {
    async fn decorate(
        &self,
        state: PlatformState,
        _ctx: &CallContext,
        _event_name: &str,
        _val: &Value,
    ) -> Result<Value, AppEventDecorationError> {
        if let Ok(v) = SettingsProcessor::get_settings_map(state, &self.request).await {
            Ok(serde_json::to_value(v).unwrap())
        } else {
            Ok(json!({}))
        }
    }

    fn dec_clone(&self) -> Box<dyn AppEventDecorator + Send + Sync> {
        Box::new(self.clone())
    }
}

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

    async fn get_settings_map(
        state: PlatformState,
        request: &SettingsRequestParam,
    ) -> Result<HashMap<String, SettingValue>, RippleError> {
        let ctx = request.context.clone();
        if let Ok(cp) = DiscoveryImpl::get_content_policy(
            &request.context,
            state.clone(),
            &request.context.app_id,
        )
        .await
        {
            let mut settings = HashMap::default();
            for sk in request.keys.clone() {
                let val = match sk {
                    SettingKey::VoiceGuidanceEnabled => {
                        let enabled = voice_guidance_settings_enabled(state.clone())
                            .await
                            .unwrap_or(false);
                        Some(SettingValue::bool(enabled))
                    }
                    SettingKey::ClosedCaptions => {
                        let enabled = ClosedcaptionsImpl::cc_enabled(state.clone())
                            .await
                            .unwrap_or(false);
                        Some(SettingValue::bool(enabled))
                    }
                    SettingKey::AllowPersonalization => {
                        Some(SettingValue::bool(cp.enable_recommendations))
                    }
                    SettingKey::AllowWatchHistory => {
                        Some(SettingValue::bool(cp.remember_watched_programs))
                    }
                    SettingKey::ShareWatchHistory => {
                        Some(SettingValue::bool(cp.share_watch_history))
                    }
                    SettingKey::DeviceName => Some(SettingValue::string(
                        broker_utils::BrokerUtils::process_internal_main_request(
                            state.clone(),
                            "device.name",
                            None,
                        )
                        .await
                        .unwrap_or_else(|_| "".into())
                        .to_string(),
                    )),
                    SettingKey::PowerSaving => Some(SettingValue::bool(true)),
                    SettingKey::LegacyMiniGuide => Some(SettingValue::bool(false)),
                };

                if let Some(v) = val {
                    let role_info = RoleInfo {
                        role: Some(CapabilityRole::Use),
                        capability: FireboltCap::Short(sk.use_capability().into()),
                    };
                    if let Ok(result) = is_permitted(state.clone(), &ctx, &role_info).await {
                        if result {
                            settings.insert(request.get_alias(&sk), v);
                        }
                    }
                }
            }
            return Ok(settings);
        }
        Err(RippleError::InvalidOutput)
    }

    async fn get(state: PlatformState, msg: ExtnMessage, request: SettingsRequestParam) -> bool {
        if let Ok(settings) = Self::get_settings_map(state.clone(), &request).await {
            return Self::respond(
                state.clone().get_client().get_extn_client(),
                msg,
                ExtnResponse::Settings(settings),
            )
            .await
            .is_ok();
        }

        Self::handle_error(
            state.clone().get_client().get_extn_client(),
            msg,
            RippleError::ProcessorError,
        )
        .await
    }

    fn subscribe_event(
        state: PlatformState,
        ctx: CallContext,
        event_name: &str,
        request: SettingsRequestParam,
    ) -> bool {
        AppEvents::add_listener_with_decorator(
            state,
            event_name.to_string(),
            ctx,
            ListenRequest { listen: true },
            Some(Box::new(SettingsChangeEventDecorator { request })),
        );
        true
    }

    async fn subscribe_to_settings(
        state: PlatformState,
        msg: ExtnMessage,
        request: SettingsRequestParam,
    ) -> bool {
        let mut resp = true;
        let ctx = &request.context;
        debug!("Incoming subscribe request");
        for key in &request.keys {
            debug!("Checking Key {:?}", key);
            match key {
                SettingKey::VoiceGuidanceEnabled => {
                    resp = BrokerUtils::process_internal_request(
                        state.clone(),
                        Some(request.context.clone()),
                        "sts.voiceguidance.onEnabledChanged",
                        serde_json::to_value(json!({"listen": true})).ok(),
                    )
                    .await
                    .is_ok();
                }
                SettingKey::ClosedCaptions => {
                    resp = BrokerUtils::process_internal_request(
                        state.clone(),
                        Some(request.context.clone()),
                        "sts.closedcaptions.onEnabledChanged",
                        serde_json::to_value(json!({"listen": true})).ok(),
                    )
                    .await
                    .is_ok();
                }
                SettingKey::AllowPersonalization => {
                    if PrivacyImpl::listen_content_policy_changed(
                        state.clone(),
                        true,
                        ctx,
                        EVENT_ALLOW_PERSONALIZATION_CHANGED,
                        Some(ContentListenRequest {
                            listen: true,
                            app_id: None,
                        }),
                        Some(Box::new(SettingsChangeEventDecorator {
                            request: request.clone(),
                        })),
                    )
                    .is_err()
                    {
                        resp = false;
                    }
                }
                SettingKey::AllowWatchHistory => {
                    if PrivacyImpl::listen_content_policy_changed(
                        state.clone(),
                        true,
                        ctx,
                        EVENT_ALLOW_WATCH_HISTORY_CHANGED,
                        Some(ContentListenRequest {
                            listen: true,
                            app_id: None,
                        }),
                        Some(Box::new(SettingsChangeEventDecorator {
                            request: request.clone(),
                        })),
                    )
                    .is_err()
                    {
                        resp = false;
                    }
                }
                SettingKey::ShareWatchHistory => {
                    if !Self::subscribe_event(
                        state.clone(),
                        ctx.clone(),
                        EVENT_SHARE_WATCH_HISTORY,
                        request.clone(),
                    ) {
                        resp = false;
                    }
                }
                SettingKey::DeviceName => {
                    resp = BrokerUtils::process_internal_request(
                        state.clone(),
                        Some(request.context.clone()),
                        "sts.device.onNameChanged",
                        serde_json::to_value(json!({"listen": true})).ok(),
                    )
                    .await
                    .is_ok();
                }
                SettingKey::PowerSaving | SettingKey::LegacyMiniGuide => {
                    warn!("{} Not implemented", key.to_string());
                }
            }
        }
        Self::respond(
            state.clone().get_client().get_extn_client(),
            msg,
            if resp {
                ExtnResponse::None(())
            } else {
                ExtnResponse::Error(RippleError::ProcessorError)
            },
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
            SettingsRequest::Get(request) => Self::get(state.clone(), msg, request).await,
            SettingsRequest::Subscribe(request) => {
                Self::subscribe_to_settings(state.clone(), msg, request).await
            }
        }
    }
}
