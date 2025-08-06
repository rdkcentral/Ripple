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

use std::{collections::HashMap, env};

use crate::{
    firebolt::rpc::RippleRPCProvider,
    service::apps::app_events::AppEvents,
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
            device_events::{
                DeviceEvent, DeviceEventCallback, DeviceEventRequest, AUDIO_CHANGED_EVENT,
                HDCP_CHANGED_EVENT,
            },
            device_info_request::{DeviceInfoRequest, DeviceResponse, FirmwareInfo},
            device_request::{AudioProfile, DeviceVersionResponse, HdcpProfile},
        },
        firebolt::fb_general::{ListenRequest, ListenerResponse},
        gateway::rpc_gateway_api::CallContext,
        storage_property::{EVENT_DEVICE_DEVICE_NAME_CHANGED, EVENT_DEVICE_NAME_CHANGED},
    },
    extn::extn_client_message::ExtnResponse,
    log::error,
};

include!(concat!(env!("OUT_DIR"), "/version.rs"));

const KEY_FIREBOLT_DEVICE_UID: &str = "fireboltDeviceUid";

#[rpc(server)]
pub trait Device {
    #[method(name = "device.uid")]
    async fn uid(&self, ctx: CallContext) -> RpcResult<String>;
    #[method(name = "device.onNameChanged")]
    async fn on_name_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;
    #[method(name = "device.onDeviceNameChanged")]
    async fn on_device_name_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;
    #[method(name = "device.model")]
    async fn model(&self, ctx: CallContext) -> RpcResult<String>;
    #[method(name = "device.hdcp")]
    async fn hdcp(&self, ctx: CallContext) -> RpcResult<HashMap<HdcpProfile, bool>>;
    #[method(name = "device.onHdcpChanged")]
    async fn on_hdcp_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;
    #[method(name = "device.type")]
    async fn typ(&self, ctx: CallContext) -> RpcResult<String>;
    #[method(name = "device.audio")]
    async fn audio(&self, ctx: CallContext) -> RpcResult<HashMap<AudioProfile, bool>>;
    #[method(name = "device.onAudioChanged")]
    async fn on_audio_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;
    #[method(name = "device.platform")]
    async fn platform(&self, ctx: CallContext) -> RpcResult<String>;
    #[method(name = "device.version")]
    async fn version(&self, ctx: CallContext) -> RpcResult<DeviceVersionResponse>;
}

#[derive(Debug)]
pub struct DeviceImpl {
    pub state: PlatformState,
}

impl DeviceImpl {
    async fn firmware_info(&self, _ctx: CallContext) -> RpcResult<FirmwareInfo> {
        match self
            .state
            .extn_request(DeviceInfoRequest::FirmwareInfo)
            .await
        {
            Ok(response) => match response.payload.extract() {
                Some(DeviceResponse::FirmwareInfo(value)) => Ok(value),
                _ => Err(jsonrpsee::core::Error::Custom(String::from(
                    "device.hdcp error",
                ))),
            },
            Err(_e) => Err(jsonrpsee::core::Error::Custom(String::from(
                "device.hdcp error",
            ))),
        }
    }
}

#[async_trait]
impl DeviceServer for DeviceImpl {
    async fn on_name_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        rpc_add_event_listener(self.state.clone(), ctx, request, EVENT_DEVICE_NAME_CHANGED).await
    }

    async fn on_device_name_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        rpc_add_event_listener(
            self.state.clone(),
            ctx,
            request,
            EVENT_DEVICE_DEVICE_NAME_CHANGED,
        )
        .await
    }

    async fn uid(&self, ctx: CallContext) -> RpcResult<String> {
        crate::utils::common::get_uid(self.state.clone(), ctx.app_id, KEY_FIREBOLT_DEVICE_UID).await
    }

    async fn platform(&self, _ctx: CallContext) -> RpcResult<String> {
        Ok("WPE".into())
    }

    async fn version(&self, ctx: CallContext) -> RpcResult<DeviceVersionResponse> {
        let firmware_info = self.firmware_info(ctx).await?;
        let open_rpc_state = self.state.clone().open_rpc_state.clone();
        let api = &*open_rpc_state.get_open_rpc().clone();
        let api = api.info.clone();

        // os is deprecated, for now senidng firmware ver in os as well
        let os_ver = firmware_info.clone().version;

        Ok(DeviceVersionResponse {
            api,
            firmware: firmware_info.version,
            os: os_ver,
            debug: self
                .state
                .version
                .clone()
                .unwrap_or_else(|| env!("CARGO_PKG_VERSION").to_string()),
        })
    }

    async fn model(&self, _ctx: CallContext) -> RpcResult<String> {
        if let Ok(response) = self
            .state
            .get_client()
            .send_extn_request(DeviceInfoRequest::Model)
            .await
        {
            if let Some(ExtnResponse::String(v)) = response.payload.extract() {
                if let Some(f) = self
                    .state
                    .get_device_manifest()
                    .get_model_friendly_names()
                    .get(&v)
                {
                    return Ok(f.clone());
                }
                return Ok(v);
            }
        }
        Err(rpc_err("FB error response TBD"))
    }

    async fn hdcp(&self, _ctx: CallContext) -> RpcResult<HashMap<HdcpProfile, bool>> {
        match self
            .state
            .extn_request(DeviceInfoRequest::HdcpSupport)
            .await
        {
            Ok(response) => match response.payload.extract() {
                Some(DeviceResponse::HdcpSupportResponse(value)) => Ok(value),
                _ => Err(jsonrpsee::core::Error::Custom(String::from(
                    "device.hdcp error",
                ))),
            },
            Err(_e) => Err(jsonrpsee::core::Error::Custom(String::from(
                "device.hdcp error",
            ))),
        }
    }

    async fn on_hdcp_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        let listen = request.listen;

        AppEvents::add_listener(
            self.state.clone(),
            HDCP_CHANGED_EVENT.to_string(),
            ctx.clone(),
            request,
        );

        if self
            .state
            .get_client()
            .send_extn_request(DeviceEventRequest {
                event: DeviceEvent::InputChanged,
                subscribe: listen,
                callback_type: DeviceEventCallback::FireboltAppEvent(ctx.app_id),
            })
            .await
            .is_err()
        {
            error!("Error while registration");
        }

        Ok(ListenerResponse {
            listening: listen,
            event: HDCP_CHANGED_EVENT.to_string(),
        })
    }

    async fn typ(&self, _ctx: CallContext) -> RpcResult<String> {
        Ok(self.state.clone().get_device_manifest().get_form_factor())
    }

    async fn audio(&self, _ctx: CallContext) -> RpcResult<HashMap<AudioProfile, bool>> {
        let resp = self
            .state
            .get_client()
            .send_extn_request(DeviceInfoRequest::Audio)
            .await;

        match resp {
            Ok(response) => match response.payload.extract() {
                Some(DeviceResponse::AudioProfileResponse(audio)) => {
                    return Ok(audio);
                }
                _ => Err(jsonrpsee::core::Error::Custom(String::from(
                    "device.audio error",
                ))),
            },
            Err(_e) => Err(jsonrpsee::core::Error::Custom(String::from(
                "device.audio error",
            ))),
        }
    }

    async fn on_audio_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        let listen = request.listen;

        AppEvents::add_listener(
            self.state.clone(),
            AUDIO_CHANGED_EVENT.to_string(),
            ctx.clone(),
            request,
        );

        if self
            .state
            .get_client()
            .send_extn_request(DeviceEventRequest {
                event: DeviceEvent::AudioChanged,
                subscribe: listen,
                callback_type: DeviceEventCallback::FireboltAppEvent(ctx.app_id),
            })
            .await
            .is_err()
        {
            error!("Error while registration");
        }

        Ok(ListenerResponse {
            listening: listen,
            event: AUDIO_CHANGED_EVENT.to_string(),
        })
    }
}

pub struct DeviceRPCProvider;
impl RippleRPCProvider<DeviceImpl> for DeviceRPCProvider {
    fn provide(state: PlatformState) -> RpcModule<DeviceImpl> {
        (DeviceImpl { state }).into_rpc()
    }
}
