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
    firebolt::rpc::RippleRPCProvider,
    processor::storage::{
        storage_manager::StorageManager,
        storage_property::{
            StorageProperty, EVENT_VOICE_GUIDANCE_ENABLED_CHANGED,
            EVENT_VOICE_GUIDANCE_SETTINGS_CHANGED, EVENT_VOICE_GUIDANCE_SPEED_CHANGED,
        },
    },
    state::platform_state::PlatformState,
    utils::rpc_utils::{rpc_add_event_listener, rpc_err}, service::apps::app_events::AppEvents,
};

use jsonrpsee::{
    core::{async_trait, RpcResult},
    proc_macros::rpc,
    RpcModule,
};

use ripple_sdk::api::{
    device::{
        device_accessibility_data::VoiceGuidanceSettings,
        device_storage::{SetBoolProperty, SetF32Property}, device_events::VOICE_GUIDANCE_CHANGED,
    },
    firebolt::fb_general::{ListenRequest, ListenerResponse},
    gateway::rpc_gateway_api::CallContext,
};

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

/*
Free function to allow variability of event_name
*/
async fn voice_guidance_settings_enabled_changed(
    platform_state: &PlatformState,
    ctx: &CallContext,
    request: &ListenRequest,
    event_name: &str,
) -> RpcResult<ListenerResponse> {
    let listen = request.listen;
    AppEvents::add_listener(
        &platform_state.app_events_state,
        String::from(event_name),
        ctx.clone(),
        request.clone(),
    );

    if listen {
        let _ = DabEventAdministrator::get(platform_state.clone())
            .dab_event_subscribe(
                Some(ctx.clone()),
                String::from(event_name),
                None,
                Some(Box::new(VoiceGuidanceEnabledChangedEventHandler {})),
            )
            .await;
    } else {
        let _ = DabEventAdministrator::get(platform_state.clone())
            .dab_event_unsubscribe(
                Some(ctx.clone()),
                String::from(event_name),
                Some(Box::new(VoiceGuidanceEnabledChangedEventHandler {})),
            )
            .await;
    }

    Ok(ListenerResponse {
        listening: listen,
        event: VOICE_GUIDANCE_ENABLED_CHANGED_EVENT,
    })
}

#[async_trait]
impl VoiceguidanceServer for VoiceguidanceImpl {
    async fn voice_guidance_settings(&self, ctx: CallContext) -> RpcResult<VoiceGuidanceSettings> {
        Ok(VoiceGuidanceSettings {
            enabled: self
                .voice_guidance_settings_enabled_rpc(ctx.clone())
                .await?,
            speed: self.voice_guidance_settings_speed_rpc(ctx.clone()).await?,
        })
    }

    async fn on_voice_guidance_settings_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        let listen = request.listen;

        // Register for individual change events (no-op if already registered), handlers emit VOICE_GUIDANCE_SETTINGS_CHANGED_EVENT.
        voice_guidance_settings_enabled_changed(
            &self.platform_state,
            &ctx,
            &request,
            VOICE_GUIDANCE_SETTINGS_CHANGED_EVENT,
        )
        .await
        .ok();
        /*
        Add decorated listener after call to voice_guidance_settings_enabled_changed to make decorated listener current  */
        AppEvents::add_listener_with_decorator(
            &&self.platform_state.app_events_state,
            VOICE_GUIDANCE_SETTINGS_CHANGED_EVENT.to_string(),
            ctx.clone(),
            request.clone(),
            Some(Box::new(VGEventDecorator {})),
        );

        Ok(ListenerResponse {
            listening: listen,
            event: VOICE_GUIDANCE_SETTINGS_CHANGED_EVENT,
        })
    }

    async fn voice_guidance_settings_enabled_rpc(&self, _ctx: CallContext) -> RpcResult<bool> {
        StorageManager::get_bool(&self.state, StorageProperty::VoiceguidanceEnabled).await
    }

    async fn voice_guidance_settings_enabled_set(
        &self,
        _ctx: CallContext,
        set_request: SetBoolProperty,
    ) -> RpcResult<()> {
        StorageManager::set_bool(
            &self.state,
            StorageProperty::VoiceguidanceEnabled,
            set_request.value,
            None,
        )
        .await
    }

    async fn voice_guidance_settings_enabled_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        rpc_add_event_listener(
            &self.state,
            ctx,
            request,
            EVENT_VOICE_GUIDANCE_ENABLED_CHANGED,
        )
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
            Ok(())
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
