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

use crate::{
    broker::broker_utils::BrokerUtils,
    firebolt::rpc::RippleRPCProvider,
    service::apps::app_events::{AppEventDecorator, AppEvents},
    state::platform_state::PlatformState,
    utils::rpc_utils::rpc_add_event_listener,
};

use jsonrpsee::{
    core::{async_trait, RpcResult},
    proc_macros::rpc,
    RpcModule,
};

use ripple_sdk::{
    api::{
        device::{
            device_accessibility_data::VoiceGuidanceSettings,
            device_events::{
                DeviceEvent, DeviceEventCallback, DeviceEventRequest,
                VOICE_GUIDANCE_ENABLED_CHANGED, VOICE_GUIDANCE_SETTINGS_CHANGED,
                VOICE_GUIDANCE_SPEED_CHANGED,
            },
            device_info_request::DeviceInfoRequest,
            device_peristence::SetF32Property,
        },
        firebolt::{
            fb_capabilities::JSON_RPC_STANDARD_ERROR_INVALID_PARAMS,
            fb_general::{ListenRequest, ListenerResponse},
        },
        gateway::rpc_gateway_api::CallContext,
    },
    log::error,
    utils::rpc_utils::rpc_error_with_code_result,
};
use serde_json::{json, Value};

#[rpc(server)]
pub trait Voiceguidance {
    #[method(name = "voiceguidance.setSpeed")]
    async fn voice_guidance_settings_speed_set(
        &self,
        ctx: CallContext,
        set_request: SetF32Property,
    ) -> RpcResult<()>;
    #[method(name = "voiceguidance.onSpeedChanged")]
    async fn voice_guidance_settings_speed_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;
}

#[derive(Debug)]
pub struct VoiceguidanceImpl {
    pub state: PlatformState,
}

pub async fn voice_guidance_settings_enabled(state: PlatformState) -> RpcResult<bool> {
    match BrokerUtils::process_internal_main_request(state.clone(), "voiceguidance.enabled", None)
        .await
    {
        Ok(enabled_value) => {
            if let Value::Bool(enabled) = enabled_value {
                Ok(enabled)
            } else {
                Err(jsonrpsee::core::Error::Custom(String::from(
                    "voice_guidance_settings_enabled: Unexpected value type",
                )))
            }
        }
        Err(e) => Err(jsonrpsee::core::Error::Custom(format!(
            "voice_guidance_settings_enabled: e={}",
            e
        ))),
    }
}

pub async fn voice_guidance_settings_enabled_changed(
    platform_state: PlatformState,
    ctx: &CallContext,
    request: &ListenRequest,
    dec: Option<Box<dyn AppEventDecorator + Send + Sync>>,
) -> RpcResult<ListenerResponse> {
    let listen = request.listen;
    // Register for individual change events (no-op if already registered), handlers emit VOICE_GUIDANCE_SETTINGS_CHANGED_EVENT.
    if platform_state
        .get_client()
        .send_extn_request(DeviceEventRequest {
            event: DeviceEvent::VoiceGuidanceEnabledChanged,
            subscribe: listen,
            callback_type: DeviceEventCallback::FireboltAppEvent(ctx.app_id.to_owned()),
        })
        .await
        .is_err()
    {
        error!("Error while registration");
    }

    /*
    Add decorated listener after call to voice_guidance_settings_enabled_changed to make decorated listener current  */
    AppEvents::add_listener_with_decorator(
        platform_state,
        VOICE_GUIDANCE_ENABLED_CHANGED.to_string(),
        ctx.clone(),
        request.clone(),
        dec,
    );

    Ok(ListenerResponse {
        listening: listen,
        event: VOICE_GUIDANCE_ENABLED_CHANGED.to_string(),
    })
}

#[async_trait]
impl VoiceguidanceServer for VoiceguidanceImpl {
    async fn voice_guidance_settings_speed_set(
        &self,
        _ctx: CallContext,
        set_request: SetF32Property,
    ) -> RpcResult<()> {
        if (0.50..=2.0).contains(&set_request.value) {
            let resp = self
                .state
                .clone()
                .get_client()
                .send_extn_request(DeviceInfoRequest::SetVoiceGuidanceSpeed(set_request.value))
                .await;
            if resp.is_ok() {
                AppEvents::emit(
                    self.state.clone(),
                    VOICE_GUIDANCE_SPEED_CHANGED,
                    &json!(set_request.value),
                )
                .await;

                let enabled = voice_guidance_settings_enabled(self.state.clone()).await?;
                let voice_guidance_settings = VoiceGuidanceSettings {
                    enabled,
                    speed: set_request.value,
                };

                AppEvents::emit(
                    self.state.clone(),
                    VOICE_GUIDANCE_SETTINGS_CHANGED,
                    &serde_json::to_value(voice_guidance_settings).unwrap_or_default(),
                )
                .await;
                Ok(())
            } else {
                Err(jsonrpsee::core::Error::Custom(String::from(
                    "Voice guidance speed error response TBD2",
                )))
            }
        } else {
            rpc_error_with_code_result(
                "Invalid Value for set speed".to_owned(),
                JSON_RPC_STANDARD_ERROR_INVALID_PARAMS,
            )
        }
    }

    async fn voice_guidance_settings_speed_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        rpc_add_event_listener(
            self.state.clone(),
            ctx,
            request,
            VOICE_GUIDANCE_SPEED_CHANGED,
        )
        .await
    }
}

pub struct VoiceguidanceRPCProvider;
impl RippleRPCProvider<VoiceguidanceImpl> for VoiceguidanceRPCProvider {
    fn provide(state: PlatformState) -> RpcModule<VoiceguidanceImpl> {
        (VoiceguidanceImpl { state }).into_rpc()
    }
}
