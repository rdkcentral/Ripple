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

use std::{collections::HashMap, env, time::Duration};

use crate::{
    firebolt::rpc::RippleRPCProvider,
    processor::storage::storage_manager::StorageManager,
    service::apps::app_events::AppEvents,
    state::platform_state::PlatformState,
    utils::rpc_utils::{rpc_add_event_listener, rpc_err},
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
            device_events::{
                DeviceEvent, DeviceEventCallback, DeviceEventRequest, AUDIO_CHANGED_EVENT,
                HDCP_CHANGED_EVENT, HDR_CHANGED_EVENT, NETWORK_CHANGED_EVENT,
                SCREEN_RESOLUTION_CHANGED_EVENT, VIDEO_RESOLUTION_CHANGED_EVENT,
            },
            device_info_request::{DeviceInfoRequest, DeviceResponse, FirmwareInfo},
            device_operator::DEFAULT_DEVICE_OPERATION_TIMEOUT_SECS,
            device_peristence::SetStringProperty,
            device_request::{
                AudioProfile, DeviceVersionResponse, HdcpProfile, HdrProfile, NetworkResponse,
            },
        },
        distributor::distributor_encoder::EncoderRequest,
        firebolt::{
            fb_capabilities::CAPABILITY_NOT_SUPPORTED,
            fb_general::{ListenRequest, ListenerResponse},
        },
        gateway::rpc_gateway_api::CallContext,
        session::{AccountSessionRequest, ProvisionRequest},
        storage_property::{
            StorageProperty, StoragePropertyData, EVENT_DEVICE_DEVICE_NAME_CHANGED,
            EVENT_DEVICE_NAME_CHANGED, KEY_FIREBOLT_DEVICE_UID, SCOPE_DEVICE,
        },
    },
    extn::extn_client_message::ExtnResponse,
    log::error,
    tokio::time::timeout,
    uuid::Uuid,
};

include!(concat!(env!("OUT_DIR"), "/version.rs"));
pub const DEVICE_UID: &str = "device.uid";

// #[derive(Serialize, Clone, Debug, Deserialize)]
// #[serde(rename_all = "camelCase")]
// pub struct ProvisionRequest {
//     account_id: String,
//     device_id: String,
//     distributor_id: Option<String>,
// }

// impl ProvisionRequest {
//     fn get_session(self) -> DistributorSession {
//         DistributorSession {
//             id: None,
//             token: None,
//             account_id: Some(self.account_id.clone()),
//             device_id: Some(self.device_id.clone()),
//         }
//     }
// }

#[rpc(server)]
pub trait Device {
    #[method(name = "device.name")]
    async fn name(&self, ctx: CallContext) -> RpcResult<String>;
    #[method(name = "device.setName")]
    async fn set_name(
        &self,
        ctx: CallContext,
        _setname_request: SetStringProperty,
    ) -> RpcResult<()>;
    #[method(name = "device.id")]
    async fn id(&self, ctx: CallContext) -> RpcResult<String>;
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
    #[method(name = "device.sku")]
    async fn sku(&self, ctx: CallContext) -> RpcResult<String>;
    #[method(name = "device.hdcp")]
    async fn hdcp(&self, ctx: CallContext) -> RpcResult<HashMap<HdcpProfile, bool>>;
    #[method(name = "device.onHdcpChanged")]
    async fn on_hdcp_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;
    #[method(name = "device.hdr")]
    async fn hdr(&self, ctx: CallContext) -> RpcResult<HashMap<HdrProfile, bool>>;
    #[method(name = "device.onHdrChanged")]
    async fn on_hdr_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;
    #[method(name = "device.screenResolution")]
    async fn screen_resolution(&self, ctx: CallContext) -> RpcResult<Vec<i32>>;
    #[method(name = "device.onScreenResolutionChanged")]
    async fn on_screen_resolution_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;
    #[method(name = "device.videoResolution")]
    async fn video_resolution(&self, ctx: CallContext) -> RpcResult<Vec<i32>>;
    #[method(name = "device.onVideoResolutionChanged")]
    async fn on_video_resolution_changed(
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
    #[method(name = "device.network")]
    async fn network(&self, ctx: CallContext) -> RpcResult<NetworkResponse>;
    #[method(name = "device.onNetworkChanged")]
    async fn on_network_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;
    #[method(name = "device.provision")]
    async fn provision(
        &self,
        ctx: CallContext,
        provision_request: ProvisionRequest,
    ) -> RpcResult<()>;
    #[method(name = "device.distributor")]
    async fn distributor(&self, ctx: CallContext) -> RpcResult<String>;
    #[method(name = "device.make")]
    async fn make(&self, ctx: CallContext) -> RpcResult<String>;
    #[method(name = "device.platform")]
    async fn platform(&self, ctx: CallContext) -> RpcResult<String>;
    #[method(name = "device.version")]
    async fn version(&self, ctx: CallContext) -> RpcResult<DeviceVersionResponse>;
}

pub fn filter_mac(mac_address: String) -> String {
    mac_address.replace(':', "")
}

pub async fn get_device_id(state: &PlatformState) -> RpcResult<String> {
    if let Some(session) = state.session_state.get_account_session() {
        return Ok(session.device_id);
    }
    match get_ll_mac_addr(state.clone()).await {
        Ok(device_id) => Ok(device_id),
        Err(_) => Err(rpc_err("parse error")),
    }
}

pub async fn get_uid(state: &PlatformState, app_id: String) -> RpcResult<String> {
    let data = StoragePropertyData {
        scope: Some(SCOPE_DEVICE.to_string()),
        key: KEY_FIREBOLT_DEVICE_UID, // Static string literal
        namespace: app_id.clone(),
        value: String::new(),
    };

    // Attempt to get the UID from storage
    if let Ok(uid) = StorageManager::get_string_for_scope(state, &data).await {
        return Ok(uid);
    }

    // check if the app has been migrated
    if state
        .app_manager_state
        .get_persisted_migrated_state_for_app_id(&app_id)
        .is_some()
    {
        let uid = Uuid::new_v4().to_string();
        let mut new_data = data.clone();
        new_data.value = uid.clone();
        StorageManager::set_string_for_scope(state, &new_data, None).await?;
        return Ok(uid);
    }

    // Fetch and handle the device ID
    let device_id = get_device_id(state)
        .await
        .map_err(|_| rpc_err("parse error"))?;
    // check if the state supports encoding
    if state.supports_encoding() {
        let response = state
            .get_client()
            .send_extn_request(EncoderRequest {
                reference: device_id,
                scope: app_id.clone(),
            })
            .await;

        if let Ok(resp) = response {
            if let Some(ExtnResponse::String(enc_device_id)) =
                resp.payload.extract::<ExtnResponse>()
            {
                let mut new_data = data.clone();
                new_data.value = enc_device_id.clone();
                StorageManager::set_string_for_scope(state, &new_data, None).await?;
                state
                    .app_manager_state
                    .persist_migrated_state(&app_id, DEVICE_UID.to_string());
                return Ok(enc_device_id);
            }
        }
    }

    Err(jsonrpsee::core::Error::Call(CallError::Custom {
        code: CAPABILITY_NOT_SUPPORTED,
        message: "capability device:uid is not supported".to_string(),
        data: None,
    }))
}

pub async fn get_ll_mac_addr(state: PlatformState) -> RpcResult<String> {
    let resp = state
        .get_client()
        .send_extn_request(DeviceInfoRequest::MacAddress)
        .await;
    match resp {
        Ok(response) => match response.payload.extract().unwrap() {
            ExtnResponse::String(value) => Ok(filter_mac(value)),
            _ => Err(jsonrpsee::core::Error::Custom(String::from(
                "MAC Info error response TBD",
            ))),
        },
        Err(_e) => Err(jsonrpsee::core::Error::Custom(String::from(
            "MAC Info error response TBD",
        ))),
    }
}

pub async fn set_device_name(state: &PlatformState, prop: SetStringProperty) -> RpcResult<()> {
    StorageManager::set_string(state, StorageProperty::DeviceName, prop.value, None).await
}

pub async fn get_device_name(state: &PlatformState) -> RpcResult<String> {
    StorageManager::get_string(state, StorageProperty::DeviceName).await
}

#[derive(Debug)]
pub struct DeviceImpl {
    pub state: PlatformState,
}

impl DeviceImpl {
    async fn firmware_info(&self, _ctx: CallContext) -> RpcResult<FirmwareInfo> {
        let resp = self
            .state
            .get_client()
            .send_extn_request(DeviceInfoRequest::FirmwareInfo)
            .await;

        match resp {
            Ok(dab_payload) => match dab_payload.payload.extract().unwrap() {
                DeviceResponse::FirmwareInfo(value) => Ok(value),
                _ => Err(jsonrpsee::core::Error::Custom(String::from(
                    "Firmware Info error response TBD",
                ))),
            },
            Err(_e) => Err(jsonrpsee::core::Error::Custom(String::from(
                "Firmware Info error response TBD",
            ))),
        }
    }
}

#[async_trait]
impl DeviceServer for DeviceImpl {
    async fn name(&self, _ctx: CallContext) -> RpcResult<String> {
        get_device_name(&self.state).await
    }

    async fn set_name(
        &self,
        _ctx: CallContext,
        setname_request: SetStringProperty,
    ) -> RpcResult<()> {
        set_device_name(&self.state, setname_request).await
    }

    async fn on_name_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        rpc_add_event_listener(&self.state, ctx, request, EVENT_DEVICE_NAME_CHANGED).await
    }

    async fn on_device_name_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        rpc_add_event_listener(&self.state, ctx, request, EVENT_DEVICE_DEVICE_NAME_CHANGED).await
    }

    async fn id(&self, _ctx: CallContext) -> RpcResult<String> {
        get_device_id(&self.state).await
    }

    async fn uid(&self, ctx: CallContext) -> RpcResult<String> {
        get_uid(&self.state, ctx.app_id).await
    }

    async fn platform(&self, _ctx: CallContext) -> RpcResult<String> {
        Ok("WPE".into())
    }

    async fn version(&self, ctx: CallContext) -> RpcResult<DeviceVersionResponse> {
        let firmware_info = self.firmware_info(ctx).await?;
        let open_rpc_state = self.state.clone().open_rpc_state;
        let api = open_rpc_state.get_open_rpc().info;

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

    async fn sku(&self, _ctx: CallContext) -> RpcResult<String> {
        if let Ok(response) = self
            .state
            .get_client()
            .send_extn_request(DeviceInfoRequest::Model)
            .await
        {
            if let Some(ExtnResponse::String(v)) = response.payload.extract() {
                return Ok(v);
            }
        }
        Err(rpc_err("FB error response TBD"))
    }

    async fn hdcp(&self, _ctx: CallContext) -> RpcResult<HashMap<HdcpProfile, bool>> {
        let resp = self
            .state
            .get_client()
            .send_extn_request(DeviceInfoRequest::HdcpSupport)
            .await;

        match resp {
            Ok(payload) => match payload.payload.extract().unwrap() {
                DeviceResponse::HdcpSupportResponse(value) => Ok(value),
                _ => Err(jsonrpsee::core::Error::Custom(String::from(
                    "Hdcp capabilities error response TBD",
                ))),
            },
            Err(_e) => Err(jsonrpsee::core::Error::Custom(String::from(
                "Hdcp capabilities error response TBD",
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
            &self.state,
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

    async fn hdr(&self, _ctx: CallContext) -> RpcResult<HashMap<HdrProfile, bool>> {
        let resp = self
            .state
            .get_client()
            .send_extn_request(DeviceInfoRequest::Hdr)
            .await;

        match resp {
            Ok(response) => match response.payload.extract().unwrap() {
                DeviceResponse::HdrResponse(value) => Ok(value),
                _ => Err(jsonrpsee::core::Error::Custom(String::from(
                    "Hdr capabilities error response TBD",
                ))),
            },
            Err(_e) => Err(jsonrpsee::core::Error::Custom(String::from(
                "Hdr capabilities error response TBD",
            ))),
        }
    }

    async fn on_hdr_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        let listen = request.listen;
        AppEvents::add_listener(
            &self.state,
            HDR_CHANGED_EVENT.to_string(),
            ctx.clone(),
            request,
        );

        if self
            .state
            .get_client()
            .send_extn_request(DeviceEventRequest {
                event: DeviceEvent::HdrChanged,
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
            event: HDR_CHANGED_EVENT.to_string(),
        })
    }

    async fn screen_resolution(&self, _ctx: CallContext) -> RpcResult<Vec<i32>> {
        if let Ok(Ok(resp)) = timeout(
            Duration::from_secs(DEFAULT_DEVICE_OPERATION_TIMEOUT_SECS),
            self.state
                .get_client()
                .send_extn_request(DeviceInfoRequest::ScreenResolution),
        )
        .await
        {
            if let Some(DeviceResponse::ScreenResolutionResponse(value)) = resp.payload.extract() {
                return Ok(value);
            }
        }

        Err(jsonrpsee::core::Error::Custom(String::from(
            "screen_resolution error response TBD",
        )))
    }

    async fn on_screen_resolution_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        let listen = request.listen;
        AppEvents::add_listener(
            &self.state,
            SCREEN_RESOLUTION_CHANGED_EVENT.to_string(),
            ctx.clone(),
            request,
        );

        if self
            .state
            .get_client()
            .send_extn_request(DeviceEventRequest {
                event: DeviceEvent::ScreenResolutionChanged,
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
            event: SCREEN_RESOLUTION_CHANGED_EVENT.to_string(),
        })
    }

    async fn video_resolution(&self, _ctx: CallContext) -> RpcResult<Vec<i32>> {
        if let Ok(Ok(resp)) = timeout(
            Duration::from_secs(DEFAULT_DEVICE_OPERATION_TIMEOUT_SECS),
            self.state
                .get_client()
                .send_extn_request(DeviceInfoRequest::VideoResolution),
        )
        .await
        {
            if let Some(DeviceResponse::VideoResolutionResponse(value)) = resp.payload.extract() {
                return Ok(value);
            }
        }

        Err(jsonrpsee::core::Error::Custom(String::from(
            "video_resolution error response TBD",
        )))
    }

    async fn on_video_resolution_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        let listen = request.listen;
        AppEvents::add_listener(
            &self.state,
            VIDEO_RESOLUTION_CHANGED_EVENT.to_string(),
            ctx.clone(),
            request,
        );

        if self
            .state
            .get_client()
            .send_extn_request(DeviceEventRequest {
                event: DeviceEvent::VideoResolutionChanged,
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
            event: VIDEO_RESOLUTION_CHANGED_EVENT.to_string(),
        })
    }

    async fn make(&self, _ctx: CallContext) -> RpcResult<String> {
        if let Ok(response) = self
            .state
            .get_client()
            .send_extn_request(DeviceInfoRequest::Make)
            .await
        {
            if let Some(ExtnResponse::String(v)) = response.payload.extract() {
                return Ok(v);
            }
        }
        Err(rpc_err("FB error response TBD"))
    }

    async fn typ(&self, _ctx: CallContext) -> RpcResult<String> {
        Ok(self.state.get_device_manifest().get_form_factor())
    }

    async fn audio(&self, _ctx: CallContext) -> RpcResult<HashMap<AudioProfile, bool>> {
        let resp = self
            .state
            .get_client()
            .send_extn_request(DeviceInfoRequest::Audio)
            .await;

        match resp {
            Ok(response) => match response.payload.extract().unwrap() {
                DeviceResponse::AudioProfileResponse(audio) => Ok(audio),
                _ => Err(jsonrpsee::core::Error::Custom(String::from(
                    "Audio error response TBD",
                ))),
            },
            Err(_e) => {
                // TODO: What do error responses look like?
                Err(jsonrpsee::core::Error::Custom(String::from(
                    "Audio error response TBD",
                )))
            }
        }
    }

    async fn on_audio_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        let listen = request.listen;

        AppEvents::add_listener(
            &self.state,
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

    async fn network(&self, _ctx: CallContext) -> RpcResult<NetworkResponse> {
        let resp = self
            .state
            .get_client()
            .send_extn_request(DeviceInfoRequest::Network)
            .await;

        match resp {
            Ok(response) => match response.payload.extract().unwrap() {
                ExtnResponse::NetworkResponse(value) => Ok(value),
                _ => Err(jsonrpsee::core::Error::Custom(String::from(
                    "Network Status error response TBD",
                ))),
            },
            Err(_e) => Err(jsonrpsee::core::Error::Custom(String::from(
                "Network status error response TBD",
            ))),
        }
    }

    async fn on_network_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        let listen = request.listen;
        AppEvents::add_listener(
            &self.state,
            NETWORK_CHANGED_EVENT.to_string(),
            ctx.clone(),
            request,
        );

        if self
            .state
            .get_client()
            .send_extn_request(DeviceEventRequest {
                event: DeviceEvent::NetworkChanged,
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
            event: NETWORK_CHANGED_EVENT.to_string(),
        })
    }

    async fn provision(
        &self,
        _ctx: CallContext,
        provision_request: ProvisionRequest,
    ) -> RpcResult<()> {
        // clear the cached distributor session
        self.state
            .session_state
            .update_account_session(provision_request.clone());

        let resp = self
            .state
            .get_client()
            .send_extn_request(AccountSessionRequest::Provision(provision_request))
            .await;
        match resp {
            Ok(payload) => match payload.payload.extract().unwrap() {
                ExtnResponse::None(()) => Ok(()),
                _ => Err(rpc_err("Provision Status error response TBD")),
            },
            Err(_e) => Err(jsonrpsee::core::Error::Custom(String::from(
                "Provision Status error response TBD",
            ))),
        }
    }

    async fn distributor(&self, _ctx: CallContext) -> RpcResult<String> {
        if let Some(session) = self.state.session_state.get_account_session() {
            Ok(session.id)
        } else {
            Err(jsonrpsee::core::Error::Custom(String::from(
                "Account session is not available",
            )))
        }
    }
}

pub struct DeviceRPCProvider;
impl RippleRPCProvider<DeviceImpl> for DeviceRPCProvider {
    fn provide(state: PlatformState) -> RpcModule<DeviceImpl> {
        (DeviceImpl { state }).into_rpc()
    }
}
