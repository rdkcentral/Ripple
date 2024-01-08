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
    firebolt::rpc::RippleRPCProvider,
    service::apps::app_events::{AppEventDecorationError, AppEventDecorator, AppEvents},
    state::platform_state::PlatformState,
    utils::rpc_utils::rpc_add_event_listener,
};

use jsonrpsee::{
    core::{async_trait, RpcResult},
    proc_macros::rpc,
    types::error::CallError,
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
            device_peristence::{SetBoolProperty, SetF32Property},
        },
        firebolt::{
            fb_capabilities::JSON_RPC_STANDARD_ERROR_INVALID_PARAMS,
            fb_general::{ListenRequest, ListenerResponse},
        },
        gateway::rpc_gateway_api::CallContext,
    },
    extn::extn_client_message::ExtnResponse,
    log::error,
};
use serde_json::{json, Value};

#[derive(Clone)]
struct VGEnabledEventDecorator {}

#[async_trait]
impl AppEventDecorator for VGEnabledEventDecorator {
    async fn decorate(
        &self,
        ps: &PlatformState,
        _ctx: &CallContext,
        _event_name: &str,
        _val_in: &Value,
    ) -> Result<Value, AppEventDecorationError> {
        let enabled = voice_guidance_settings_enabled(ps).await?;
        Ok(json!(enabled))
    }

    fn dec_clone(&self) -> Box<dyn AppEventDecorator + Send + Sync> {
        Box::new(self.clone())
    }
}

#[rpc(server)]
pub trait Voiceguidance {
    #[method(name = "accessibility.voiceGuidanceSettings", aliases = ["accessibility.voiceGuidance"])]
    async fn voice_guidance_settings(&self, ctx: CallContext) -> RpcResult<VoiceGuidanceSettings>;
    #[method(name = "accessibility.onVoiceGuidanceSettingsChanged")]
    async fn on_voice_guidance_settings_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;

    #[method(name = "voiceguidance.enabled")]
    async fn voice_guidance_settings_enabled_rpc(&self, ctx: CallContext) -> RpcResult<bool>;
    #[method(name = "voiceguidance.setEnabled")]
    async fn voice_guidance_settings_enabled_set(
        &self,
        ctx: CallContext,
        set_request: SetBoolProperty,
    ) -> RpcResult<()>;
    #[method(name = "voiceguidance.onEnabledChanged")]
    async fn voice_guidance_settings_enabled_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;

    #[method(name = "voiceguidance.speed")]
    async fn voice_guidance_settings_speed_rpc(&self, ctx: CallContext) -> RpcResult<f32>;
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

pub async fn voice_guidance_settings_enabled(state: &PlatformState) -> RpcResult<bool> {
    let resp = state
        .get_client()
        .send_extn_request(DeviceInfoRequest::VoiceGuidanceEnabled)
        .await;
    match resp {
        Ok(value) => {
            if let Some(ExtnResponse::Boolean(v)) = value.payload.extract() {
                Ok(v)
            } else {
                Err(jsonrpsee::core::Error::Custom(String::from(
                    "Voice guidance enabled error response TBD1",
                )))
            }
        }
        Err(_) => Err(jsonrpsee::core::Error::Custom(String::from(
            "Voice guidance enabled error response TBD2",
        ))),
    }
}

pub async fn voice_guidance_settings_speed(state: &PlatformState) -> RpcResult<f32> {
    let resp = state
        .get_client()
        .send_extn_request(DeviceInfoRequest::VoiceGuidanceSpeed)
        .await;
    match resp {
        Ok(value) => {
            if let Some(ExtnResponse::Float(v)) = value.payload.extract() {
                Ok(v)
            } else {
                Err(jsonrpsee::core::Error::Custom(String::from(
                    "Voice guidance enabled error response TBD1",
                )))
            }
        }
        Err(_) => Err(jsonrpsee::core::Error::Custom(String::from(
            "Voice guidance enabled error response TBD2",
        ))),
    }
}

pub async fn voice_guidance_settings_enabled_changed(
    platform_state: &PlatformState,
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
    async fn voice_guidance_settings(&self, _ctx: CallContext) -> RpcResult<VoiceGuidanceSettings> {
        Ok(VoiceGuidanceSettings {
            enabled: voice_guidance_settings_enabled(&self.state).await?,
            speed: voice_guidance_settings_speed(&self.state).await?,
        })
    }

    async fn on_voice_guidance_settings_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        if self
            .state
            .get_client()
            .send_extn_request(DeviceEventRequest {
                event: DeviceEvent::VoiceGuidanceEnabledChanged,
                subscribe: true,
                callback_type: DeviceEventCallback::FireboltAppEvent(ctx.app_id.to_owned()),
            })
            .await
            .is_err()
        {
            error!("on_voice_guidance_settings_changed: Error while registration");
        }

        rpc_add_event_listener(&self.state, ctx, request, VOICE_GUIDANCE_SETTINGS_CHANGED).await
    }

    async fn voice_guidance_settings_enabled_rpc(&self, _ctx: CallContext) -> RpcResult<bool> {
        voice_guidance_settings_enabled(&self.state).await
    }

    async fn voice_guidance_settings_enabled_set(
        &self,
        _ctx: CallContext,
        set_request: SetBoolProperty,
    ) -> RpcResult<()> {
        let resp = self
            .state
            .get_client()
            .send_extn_request(DeviceInfoRequest::SetVoiceGuidanceEnabled(
                set_request.value,
            ))
            .await;
        if resp.is_ok() {
            Ok(())
        } else {
            Err(jsonrpsee::core::Error::Custom(String::from(
                "Voice guidance enabled error response TBD2",
            )))
        }
    }

    async fn voice_guidance_settings_enabled_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        voice_guidance_settings_enabled_changed(
            &self.state,
            &ctx,
            &request,
            Some(Box::new(VGEnabledEventDecorator {})),
        )
        .await
    }

    async fn voice_guidance_settings_speed_rpc(&self, _ctx: CallContext) -> RpcResult<f32> {
        voice_guidance_settings_speed(&self.state).await
    }

    async fn voice_guidance_settings_speed_set(
        &self,
        _ctx: CallContext,
        set_request: SetF32Property,
    ) -> RpcResult<()> {
        if (0.50..=2.0).contains(&set_request.value) {
            let resp = self
                .state
                .get_client()
                .send_extn_request(DeviceInfoRequest::SetVoiceGuidanceSpeed(set_request.value))
                .await;
            if resp.is_ok() {
                AppEvents::emit(
                    &self.state,
                    VOICE_GUIDANCE_SPEED_CHANGED,
                    &json!(set_request.value),
                )
                .await;

                let enabled = voice_guidance_settings_enabled(&self.state).await?;
                let voice_guidance_settings = VoiceGuidanceSettings {
                    enabled,
                    speed: set_request.value,
                };

                AppEvents::emit(
                    &self.state,
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
            Err(jsonrpsee::core::Error::Call(CallError::Custom {
                code: JSON_RPC_STANDARD_ERROR_INVALID_PARAMS,
                message: "Invalid Value for set speed".to_owned(),
                data: None,
            }))
        }
    }

    async fn voice_guidance_settings_speed_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        rpc_add_event_listener(&self.state, ctx, request, VOICE_GUIDANCE_SPEED_CHANGED).await
    }
}

pub struct VoiceguidanceRPCProvider;
impl RippleRPCProvider<VoiceguidanceImpl> for VoiceguidanceRPCProvider {
    fn provide(state: PlatformState) -> RpcModule<VoiceguidanceImpl> {
        (VoiceguidanceImpl { state }).into_rpc()
    }
}
