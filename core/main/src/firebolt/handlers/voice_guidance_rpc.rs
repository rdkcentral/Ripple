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
use crate::{
    firebolt::rpc::RippleRPCProvider, service::apps::provider_broker::ProviderBroker,
    state::platform_state::PlatformState, utils::rpc_utils::rpc_err,
};

use jsonrpsee::{
    core::{async_trait, RpcResult},
    proc_macros::rpc,
    tracing::info,
    RpcModule,
};

use ripple_sdk::{
    api::{
        device::device_accessibility_data::{
            GetStorageProperty, SetBoolProperty, SetStorageProperty, SetU32Property,
            StorageRequest, VOICE_GUIDANCE_ENABLED_CHANGED_EVENT,
            VOICE_GUIDANCE_SETTINGS_CHANGED_EVENT, VOICE_GUIDANCE_SPEED_CHANGED_EVENT,
        },
        firebolt::fb_general::{ListenRequest, ListenerResponse},
        gateway::rpc_gateway_api::CallContext,
    },
    extn::extn_client_message::ExtnResponse,
    serde_json::json,
};

#[rpc(server)]
pub trait Voiceguidance {
    #[method(name = "accessibility.voiceGuidanceSettings")]
    async fn voice_guidance_settings(&self, ctx: CallContext) -> RpcResult<(bool, u32)>;
    #[method(name = "accessibility.onVoiceGuidanceSettingsChanged")]
    async fn on_voice_guidance_settings_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;
    #[method(name = "voiceguidance.enabled")]
    async fn voice_guidance_settings_enabled(&self, ctx: CallContext) -> RpcResult<bool>;
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
    async fn voice_guidance_settings_speed(&self, ctx: CallContext) -> RpcResult<u32>;
    #[method(name = "voiceguidance.setSpeed")]
    async fn voice_guidance_settings_speed_set(
        &self,
        ctx: CallContext,
        set_request: SetU32Property,
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

impl VoiceguidanceImpl {
    pub async fn on_request_app_event(
        &self,
        ctx: CallContext,
        request: ListenRequest,
        method: &'static str,
        event_name: &'static str,
    ) -> RpcResult<ListenerResponse> {
        let listen = request.listen;
        ProviderBroker::register_or_unregister_provider(
            &self.state,
            // TODO update with Firebolt Cap in later effort
            "xrn:firebolt:capability:accessibility:closedcaptions".into(),
            method.into(),
            event_name,
            ctx,
            request,
        )
        .await;

        Ok(ListenerResponse {
            listening: listen,
            event: event_name.into(),
        })
    }
}

#[async_trait]
impl VoiceguidanceServer for VoiceguidanceImpl {
    async fn voice_guidance_settings(&self, ctx: CallContext) -> RpcResult<(bool, u32)> {
        info!("Accessibility.voiceGuidanceSettings");
        Ok((
            self.voice_guidance_settings_enabled(ctx.clone()).await?,
            self.voice_guidance_settings_speed(ctx.clone()).await?,
        ))
    }

    async fn on_voice_guidance_settings_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        self.on_request_app_event(
            ctx,
            request,
            "VoiceGuidanceSettingsChanged",
            VOICE_GUIDANCE_SETTINGS_CHANGED_EVENT,
        )
        .await
    }

    async fn voice_guidance_settings_enabled(&self, _ctx: CallContext) -> RpcResult<bool> {
        info!("VoiceGuidance.enabled");
        let data = GetStorageProperty {
            namespace: "VoiceGuidance".to_string(),
            key: "enabled".to_string(),
        };
        if let Ok(response) = self
            .state
            .get_client()
            .send_extn_request(StorageRequest::Get(data))
            .await
        {
            if let Some(ExtnResponse::String(v)) = response.payload.clone().extract() {
                return Ok(v.parse::<bool>().unwrap());
            }
        }
        Err(rpc_err("VoiceGuidance.enabled is not available"))
    }

    async fn voice_guidance_settings_enabled_set(
        &self,
        _ctx: CallContext,
        set_request: SetBoolProperty,
    ) -> RpcResult<()> {
        info!("VoiceGuidance.setEnabled");
        let data = SetStorageProperty {
            namespace: "VoiceGuidance".to_string(),
            key: "enabled".to_string(),
            value: json!(set_request.value),
        };
        if let Ok(response) = self
            .state
            .get_client()
            .send_extn_request(StorageRequest::Set(data))
            .await
        {
            if let Some(ExtnResponse::Boolean(_v)) = response.payload.clone().extract() {
                return Ok(());
            }
        }
        Err(rpc_err("FB error response TBD"))
    }

    async fn voice_guidance_settings_enabled_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        self.on_request_app_event(
            ctx,
            request,
            "VoiceGuidanceEnabledChanged",
            VOICE_GUIDANCE_ENABLED_CHANGED_EVENT,
        )
        .await
    }

    async fn voice_guidance_settings_speed(&self, _ctx: CallContext) -> RpcResult<u32> {
        info!("VoiceGuidance.speed");
        let data = GetStorageProperty {
            namespace: "VoiceGuidance".to_string(),
            key: "speed".to_string(),
        };
        if let Ok(response) = self
            .state
            .get_client()
            .send_extn_request(StorageRequest::Get(data))
            .await
        {
            if let Some(ExtnResponse::String(v)) = response.payload.clone().extract() {
                return Ok(v.parse::<u32>().unwrap());
            }
        }
        Err(rpc_err("VoiceGuidance.speed is not available"))
    }

    async fn voice_guidance_settings_speed_set(
        &self,
        _ctx: CallContext,
        set_request: SetU32Property,
    ) -> RpcResult<()> {
        info!("VoiceGuidance.setSpeed");
        let data = SetStorageProperty {
            namespace: "VoiceGuidance".to_string(),
            key: "speed".to_string(),
            value: json!(set_request.value),
        };
        if let Ok(response) = self
            .state
            .get_client()
            .send_extn_request(StorageRequest::Set(data))
            .await
        {
            if let Some(ExtnResponse::Boolean(_v)) = response.payload.clone().extract() {
                return Ok(());
            }
        }
        Err(rpc_err("FB error response TBD"))
    }

    async fn voice_guidance_settings_speed_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        self.on_request_app_event(
            ctx,
            request,
            "VoiceGuidanceSpeedChanged",
            VOICE_GUIDANCE_SPEED_CHANGED_EVENT,
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
