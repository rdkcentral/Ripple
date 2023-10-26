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
    gateway::rpc_gateway_api::CallContext, storage_property::StorageProperty,
};

use crate::{
    firebolt::rpc::RippleRPCProvider, processor::storage::storage_manager::StorageManager,
    state::platform_state::PlatformState,
};

#[rpc(server)]
pub trait AudioDescription {
    #[method(name = "AudioDescriptions.enabled", aliases=["accessibility.audioDescriptionSettings"])]
    async fn ad_enabled(&self, ctx: CallContext) -> RpcResult<bool>;
    #[method(name = "AudioDescriptions.setEnabled")]
    async fn ad_enabled_set(
        &self,
        ctx: CallContext,
        set_request: AudioDescriptionSettings,
    ) -> RpcResult<()>;
}

#[derive(Debug)]
pub struct AudioDescriptionImpl {
    pub platform_state: PlatformState,
}

#[async_trait]
impl AudioDescriptionServer for AudioDescriptionImpl {
    async fn ad_enabled(&self, _ctx: CallContext) -> RpcResult<bool> {
        StorageManager::get_bool(
            &self.platform_state,
            StorageProperty::AudioDescriptionEnabled,
        )
        .await
    }

    async fn ad_enabled_set(
        &self,
        _ctx: CallContext,
        set_request: AudioDescriptionSettings,
    ) -> RpcResult<()> {
        StorageManager::set_bool(
            &self.platform_state,
            StorageProperty::AudioDescriptionEnabled,
            set_request.enabled,
            None,
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
