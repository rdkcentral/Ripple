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

use std::collections::HashMap;

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
            device_info_request::{DeviceInfoRequest, DeviceResponse},
            device_request::{
                AudioProfile, DeviceVersionResponse, HdcpProfile, HdrProfile, NetworkResponse,
            },
            device_storage::SetStringProperty,
        },
        firebolt::{
            fb_general::{ListenRequest, ListenerResponse},
            fb_openrpc::FireboltSemanticVersion,
        },
        gateway::rpc_gateway_api::CallContext,
        session::{AccountSessionRequest, ProvisionRequest},
        storage_property::{
            StorageProperty, EVENT_DEVICE_DEVICE_NAME_CHANGED, EVENT_DEVICE_NAME_CHANGED,
        },
    },
    extn::extn_client_message::ExtnResponse,
    log::error,
    uuid::Uuid,
};

include!(concat!(env!("OUT_DIR"), "/version.rs"));

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
    #[method(name = "device.model", aliases = ["device.sku"])]
    async fn model(&self, ctx: CallContext) -> RpcResult<String>;
    //    #[method(name = "device.sku")]
    //    async fn sku(&self, ctx: CallContext) -> RpcResult<String>;
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
    // #[method(name = "device.distributor")]
    // async fn distributor(&self, ctx: CallContext) -> RpcResult<String>;
    #[method(name = "device.make")]
    async fn make(&self, ctx: CallContext) -> RpcResult<String>;
    #[method(name = "device.platform")]
    async fn platform(&self, ctx: CallContext) -> RpcResult<String>;
    #[method(name = "device.version")]
    async fn version(&self, ctx: CallContext) -> RpcResult<DeviceVersionResponse>;
}

pub fn filter_mac(mac_address: String) -> String {
    String::from(mac_address.replace(":", ""))
}

pub async fn get_device_id(state: &PlatformState) -> RpcResult<String> {
    match get_ll_mac_addr(state.clone()).await {
        Ok(device_id) => Ok(device_id),
        Err(_) => Err(rpc_err("parse error").into()),
    }
}

pub async fn get_uid(state: &PlatformState) -> RpcResult<String> {
    let mac_id = get_ll_mac_addr(state.clone()).await.unwrap();
    let prefix = "00000000000000000000";
    let uuid_string = format!("{}{}", prefix, mac_id);
    let uuid = Uuid::parse_str(&uuid_string);

    match uuid {
        Ok(uid) => Ok(uid.to_string()),
        Err(_) => Err(rpc_err("parse error").into()),
    }
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
    async fn firmware_info(&self, _ctx: CallContext) -> RpcResult<FireboltSemanticVersion> {
        let resp = self
            .state
            .get_client()
            .send_extn_request(DeviceInfoRequest::Version)
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

    async fn uid(&self, _ctx: CallContext) -> RpcResult<String> {
        get_uid(&self.state).await
    }

    async fn platform(&self, _ctx: CallContext) -> RpcResult<String> {
        Ok("WPE".into())
    }

    async fn version(&self, ctx: CallContext) -> RpcResult<DeviceVersionResponse> {
        let mut os = FireboltSemanticVersion::new(
            env!("CARGO_PKG_VERSION_MAJOR").parse().unwrap(),
            env!("CARGO_PKG_VERSION_MINOR").parse().unwrap(),
            env!("CARGO_PKG_VERSION_PATCH").parse().unwrap(),
            "".to_string(),
        );
        os.readable = format!("Firebolt OS v{}", env!("CARGO_PKG_VERSION"));

        let firmware = self.firmware_info(ctx.clone()).await?;

        let open_rpc_state = self.state.clone().open_rpc_state;
        let api = open_rpc_state.get_open_rpc().info.clone();
        Ok(DeviceVersionResponse {
            api,
            firmware,
            os,
            debug: format!("{} ({})", env!("CARGO_PKG_VERSION"), SHA_SHORT),
        })
    }

    async fn model(&self, _ctx: CallContext) -> RpcResult<String> {
        if let Ok(response) = self
            .state
            .get_client()
            .send_extn_request(DeviceInfoRequest::Model)
            .await
        {
            if let Some(ExtnResponse::String(v)) = response.payload.clone().extract() {
                return Ok(v);
            }
        }
        Err(rpc_err("FB error response TBD"))
    }

    // async fn sku(&self, _ctx: CallContext) -> RpcResult<String> {
    //     if let Ok(response) = self
    //         .state
    //         .get_client()
    //         .send_extn_request(DeviceInfoRequest::Model)
    //         .await
    //     {
    //         if let Some(ExtnResponse::String(v)) = response.payload.clone().extract() {
    //             return Ok(v);
    //         }
    //     }
    //     Err(rpc_err("FB error response TBD"))
    // }

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
            &&self.state,
            HDCP_CHANGED_EVENT.to_string(),
            ctx.clone(),
            request,
        );

        if let Err(_) = self
            .state
            .get_client()
            .send_extn_request(DeviceEventRequest {
                event: DeviceEvent::InputChanged,
                id: ctx.app_id,
                subscribe: listen,
                callback_type: DeviceEventCallback::FireboltAppEvent,
            })
            .await
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
            &&self.state,
            HDR_CHANGED_EVENT.to_string(),
            ctx.clone(),
            request,
        );

        if let Err(_) = self
            .state
            .get_client()
            .send_extn_request(DeviceEventRequest {
                event: DeviceEvent::HdrChanged,
                id: ctx.app_id,
                subscribe: listen,
                callback_type: DeviceEventCallback::FireboltAppEvent,
            })
            .await
        {
            error!("Error while registration");
        }

        Ok(ListenerResponse {
            listening: listen,
            event: HDR_CHANGED_EVENT.to_string(),
        })
    }

    async fn screen_resolution(&self, _ctx: CallContext) -> RpcResult<Vec<i32>> {
        let resp = self
            .state
            .get_client()
            .send_extn_request(DeviceInfoRequest::ScreenResolution)
            .await;

        match resp {
            Ok(response) => match response.payload.extract().unwrap() {
                DeviceResponse::ScreenResolutionResponse(value) => Ok(value),
                _ => Err(jsonrpsee::core::Error::Custom(String::from(
                    "screen_resolution error response TBD",
                ))),
            },
            Err(_e) => Err(jsonrpsee::core::Error::Custom(String::from(
                "screen_resolution error response TBD",
            ))),
        }
    }

    async fn on_screen_resolution_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        let listen = request.listen;
        AppEvents::add_listener(
            &&self.state,
            SCREEN_RESOLUTION_CHANGED_EVENT.to_string(),
            ctx.clone(),
            request,
        );

        if let Err(_) = self
            .state
            .get_client()
            .send_extn_request(DeviceEventRequest {
                event: DeviceEvent::ScreenResolutionChanged,
                id: ctx.app_id,
                subscribe: listen,
                callback_type: DeviceEventCallback::FireboltAppEvent,
            })
            .await
        {
            error!("Error while registration");
        }

        Ok(ListenerResponse {
            listening: listen,
            event: SCREEN_RESOLUTION_CHANGED_EVENT.to_string(),
        })
    }

    async fn video_resolution(&self, _ctx: CallContext) -> RpcResult<Vec<i32>> {
        let resp = self
            .state
            .get_client()
            .send_extn_request(DeviceInfoRequest::VideoResolution)
            .await;

        match resp {
            Ok(response) => match response.payload.extract().unwrap() {
                DeviceResponse::VideoResolutionResponse(value) => Ok(value),
                _ => Err(jsonrpsee::core::Error::Custom(String::from(
                    "video_resolution cap error response TBD",
                ))),
            },
            Err(_e) => Err(jsonrpsee::core::Error::Custom(String::from(
                "video_resolution cap error response TBD",
            ))),
        }
    }

    async fn on_video_resolution_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        let listen = request.listen;
        AppEvents::add_listener(
            &&self.state,
            VIDEO_RESOLUTION_CHANGED_EVENT.to_string(),
            ctx.clone(),
            request,
        );

        if let Err(_) = self
            .state
            .get_client()
            .send_extn_request(DeviceEventRequest {
                event: DeviceEvent::VideoResolutionChanged,
                id: ctx.app_id,
                subscribe: listen,
                callback_type: DeviceEventCallback::FireboltAppEvent,
            })
            .await
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
            if let Some(ExtnResponse::String(v)) = response.payload.clone().extract() {
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
            &&self.state,
            AUDIO_CHANGED_EVENT.to_string(),
            ctx.clone(),
            request,
        );

        if let Err(_) = self
            .state
            .get_client()
            .send_extn_request(DeviceEventRequest {
                event: DeviceEvent::AudioChanged,
                id: ctx.clone().app_id,
                subscribe: listen,
                callback_type: DeviceEventCallback::FireboltAppEvent,
            })
            .await
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
            &&self.state,
            NETWORK_CHANGED_EVENT.to_string(),
            ctx.clone(),
            request,
        );

        if let Err(_) = self
            .state
            .get_client()
            .send_extn_request(DeviceEventRequest {
                event: DeviceEvent::NetworkChanged,
                id: ctx.app_id,
                subscribe: listen,
                callback_type: DeviceEventCallback::FireboltAppEvent,
            })
            .await
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
        ctx: CallContext,
        provision_request: ProvisionRequest,
    ) -> RpcResult<()> {
        let resp = self
            .state
            .get_client()
            .send_extn_request(AccountSessionRequest::Provision(provision_request))
            .await;
        // clear the cached distributor session
        self.state
            .session_state
            .clear_session(&ctx.session_id.clone());

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

    // async fn distributor(&self, _ctx: CallContext) -> RpcResult<String> {
    // TODO:device not provisioned
    //}
}

pub struct DeviceRPCProvider;
impl RippleRPCProvider<DeviceImpl> for DeviceRPCProvider {
    fn provide(state: PlatformState) -> RpcModule<DeviceImpl> {
        (DeviceImpl { state }).into_rpc()
    }
}
