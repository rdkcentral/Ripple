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
    processor::storage::storage_manager::StorageManager,
    service::apps::app_events::{AppEventDecorationError, AppEventDecorator, AppEvents},
    state::platform_state::PlatformState,
    utils::rpc_utils::{rpc_add_event_listener, rpc_err},
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
                DeviceEvent, DeviceEventCallback, DeviceEventRequest, VOICE_GUIDANCE_CHANGED,
            },
            device_info_request::DeviceInfoRequest,
            device_peristence::{SetBoolProperty, SetF32Property},
        },
        firebolt::fb_general::{ListenRequest, ListenerResponse},
        gateway::rpc_gateway_api::CallContext,
        storage_property::{StorageProperty, EVENT_VOICE_GUIDANCE_SPEED_CHANGED},
    },
    extn::extn_client_message::ExtnResponse,
    log::error,
};
use serde_json::Value;

#[derive(Clone)]
struct VGEventDecorator {}

#[async_trait]
impl AppEventDecorator for VGEventDecorator {
    async fn decorate(
        &self,
        ps: &PlatformState,
        _ctx: &CallContext,
        _event_name: &str,
        _val_in: &Value,
    ) -> Result<Value, AppEventDecorationError> {
        let enabled = voice_guidance_settings_enabled(ps).await?;
        let speed = voice_guidance_settings_speed(ps).await?;
        Ok(serde_json::to_value(VoiceGuidanceSettings { enabled, speed }).unwrap())
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

/*
Free function to allow variability of event_name
*/
pub async fn voice_guidance_settings_enabled_changed(
    platform_state: &PlatformState,
    ctx: &CallContext,
    request: &ListenRequest,
    event_name: &str,
) -> RpcResult<ListenerResponse> {
    let listen = request.listen;

    // Register for individual change events (no-op if already registered), handlers emit VOICE_GUIDANCE_SETTINGS_CHANGED_EVENT.
    if platform_state
        .get_client()
        .send_extn_request(DeviceEventRequest {
            event: DeviceEvent::VoiceGuidanceChanged,
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
        event_name.to_string(),
        ctx.clone(),
        request.clone(),
        Some(Box::new(VGEventDecorator {})),
    );

    Ok(ListenerResponse {
        listening: listen,
        event: event_name.to_string(),
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
        // Register for individual change events (no-op if already registered), handlers emit VOICE_GUIDANCE_SETTINGS_CHANGED_EVENT.
        voice_guidance_settings_enabled_changed(&self.state, &ctx, &request, VOICE_GUIDANCE_CHANGED)
            .await
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
        // Register for individual change events (no-op if already registered), handlers emit VOICE_GUIDANCE_SETTINGS_CHANGED_EVENT.
        voice_guidance_settings_enabled_changed(&self.state, &ctx, &request, VOICE_GUIDANCE_CHANGED)
            .await
    }

    async fn voice_guidance_settings_speed_rpc(&self, _ctx: CallContext) -> RpcResult<f32> {
        StorageManager::get_number_as_f32(&self.state, StorageProperty::VoiceguidanceSpeed).await
    }

    async fn voice_guidance_settings_speed_set(
        &self,
        _ctx: CallContext,
        set_request: SetF32Property,
    ) -> RpcResult<()> {
        if set_request.value >= 0.1 && set_request.value <= 10.0 {
            StorageManager::set_number_as_f32(
                &self.state,
                StorageProperty::VoiceguidanceSpeed,
                set_request.value,
                None,
            )
            .await?;

            let resp = self
                .state
                .get_client()
                .send_extn_request(DeviceInfoRequest::SetVoiceGuidanceSpeed(set_request.value))
                .await;
            if resp.is_ok() {
                Ok(())
            } else {
                Err(jsonrpsee::core::Error::Custom(String::from(
                    "Voice guidance speed error response TBD2",
                )))
            }
        } else {
            Err(rpc_err("Invalid Value for set speed".to_owned()))
        }
    }

    async fn voice_guidance_settings_speed_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        rpc_add_event_listener(
            &self.state,
            ctx,
            request,
            EVENT_VOICE_GUIDANCE_SPEED_CHANGED,
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
