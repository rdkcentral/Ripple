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
use jsonrpsee::{
    core::{async_trait, RpcResult},
    proc_macros::rpc,
    RpcModule,
};
use ripple_sdk::api::{
    device::device_accessibility_data::AudioDescriptionSettings,
    firebolt::fb_general::{ListenRequest, ListenerResponse},
    gateway::rpc_gateway_api::CallContext,
    storage_property::{StorageProperty, EVENT_AUDIO_DESCRIPTION_SETTINGS_CHANGED},
};

use crate::{
    firebolt::rpc::RippleRPCProvider,
    processor::storage::storage_manager::StorageManager,
    service::apps::app_events::{AppEventDecorationError, AppEventDecorator},
    state::platform_state::PlatformState,
    utils::rpc_utils::{rpc_add_event_listener, rpc_add_event_listener_with_decorator},
};

use ripple_sdk::serde_json::Value;

#[derive(Clone)]
struct AudioDescriptionEventDecorator {}

#[async_trait]
impl AppEventDecorator for AudioDescriptionEventDecorator {
    async fn decorate(
        &self,
        ps: &PlatformState,
        _ctx: &CallContext,
        _event_name: &str,
        _val_in: &Value,
    ) -> Result<Value, AppEventDecorationError> {
        if let Ok(enabled) =
            StorageManager::get_bool(ps, StorageProperty::AudioDescriptionEnabled).await
        {
            return Ok(serde_json::to_value(AudioDescriptionSettings { enabled }).unwrap());
        }

        Err(AppEventDecorationError {})
    }

    fn dec_clone(&self) -> Box<dyn AppEventDecorator + Send + Sync> {
        Box::new(self.clone())
    }
}

#[rpc(server)]
pub trait AudioDescription {
    #[method(name = "accessibility.audioDescriptionSettings")]
    async fn ad_settings_get(&self, ctx: CallContext) -> RpcResult<AudioDescriptionSettings>;

    #[method(name = "audiodescriptions.onEnabledChanged")]
    async fn on_audio_description_enabled_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;

    #[method(name = "accessibility.onAudioDescriptionSettingsChanged")]
    async fn on_audio_description_settings_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;
}

#[derive(Debug)]
pub struct AudioDescriptionImpl {
    pub platform_state: PlatformState,
}

#[async_trait]
impl AudioDescriptionServer for AudioDescriptionImpl {
    async fn ad_settings_get(&self, _ctx: CallContext) -> RpcResult<AudioDescriptionSettings> {
        let v = StorageManager::get_bool(
            &self.platform_state,
            StorageProperty::AudioDescriptionEnabled,
        )
        .await?;
        Ok(AudioDescriptionSettings { enabled: v })
    }

    async fn on_audio_description_settings_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        rpc_add_event_listener_with_decorator(
            &self.platform_state,
            ctx,
            request,
            EVENT_AUDIO_DESCRIPTION_SETTINGS_CHANGED,
            Some(Box::new(AudioDescriptionEventDecorator {})),
        )
        .await
    }

    async fn on_audio_description_enabled_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        rpc_add_event_listener(
            &self.platform_state,
            ctx,
            request,
            EVENT_AUDIO_DESCRIPTION_SETTINGS_CHANGED,
        )
        .await
    }
}

pub struct AudioDescriptionRPCProvider;
impl RippleRPCProvider<AudioDescriptionImpl> for AudioDescriptionRPCProvider {
    fn provide(platform_state: PlatformState) -> RpcModule<AudioDescriptionImpl> {
        (AudioDescriptionImpl { platform_state }).into_rpc()
    }
}
