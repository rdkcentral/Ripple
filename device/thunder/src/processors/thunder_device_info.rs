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

use std::{
    collections::HashMap,
    thread,
    time::{self, Duration},
};

use crate::processors::thunder_device_info::ThunderPlugin::LocationSync;

use serde::{Deserialize, Serialize};
use serde_json::json;
use thunder_ripple_sdk::ripple_sdk::{
    self,
    api::{
        device::{
            device_info_request::{DeviceInfoRequest, DeviceResponse},
            device_operator::{
                DeviceCallRequest, DeviceChannelParams, DeviceChannelRequest, DeviceOperator,
                DeviceResponseMessage, DeviceSubscribeRequest, DeviceUnsubscribeRequest,
            },
            device_request::{
                AudioProfile, HDCPStatus, HdcpProfile, HdrProfile, NetworkResponse, NetworkState,
                NetworkType, Resolution,
            },
        },
        firebolt::fb_openrpc::FireboltSemanticVersion,
    },
    async_trait::async_trait,
    extn::{
        client::extn_processor::{
            DefaultExtnStreamer, ExtnRequestProcessor, ExtnStreamProcessor, ExtnStreamer,
        },
        extn_client_message::{ExtnMessage, ExtnPayload, ExtnPayloadProvider, ExtnResponse},
    },
    log::{error, info},
    serde_json, tokio,
    utils::error::RippleError,
};
use thunder_ripple_sdk::{
    client::thunder_plugin::ThunderPlugin,
    ripple_sdk::{extn::client::extn_client::ExtnClient, tokio::sync::mpsc},
    thunder_state::ThunderState,
};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ThunderHDCPStatus {
    #[serde(rename = "HDCPStatus")]
    pub hdcp_status: HDCPStatus,
    pub success: bool,
}

pub mod hdr_flags {
    pub const HDRSTANDARD_NONE: u32 = 0x00;
    pub const HDRSTANDARD_HDR10: u32 = 0x01;
    pub const HDRSTANDARD_HLG: u32 = 0x02;
    pub const HDRSTANDARD_DOLBY_VISION: u32 = 0x04;
    pub const HDRSTANDARD_TECHNICOLOR_PRIME: u32 = 0x08;
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "UPPERCASE")]
enum ThunderInterfaceType {
    Wifi,
    Ethernet,
    None,
}

impl ThunderInterfaceType {
    fn to_network_type(self: Box<Self>) -> NetworkType {
        match *self {
            Self::Wifi => NetworkType::Wifi,
            Self::Ethernet => NetworkType::Ethernet,
            Self::None => NetworkType::Hybrid,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ThunderInterfaceStatus {
    interface: ThunderInterfaceType,
    mac_address: String,
    enabled: bool,
    connected: bool,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ThunderGetInterfacesResponse {
    interfaces: Vec<ThunderInterfaceStatus>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SystemVersion {
    pub stb_version: String,
    pub receiver_version: String,
    pub stb_timestamp: String,
    pub success: bool,
}

pub struct ThunderNetworkService {
    pub state: ThunderState,
}

impl ThunderNetworkService {
    async fn get_interfaces(state: ThunderState) -> ThunderGetInterfacesResponse {
        let response = state
            .get_thunder_client()
            .call(DeviceCallRequest {
                method: ThunderPlugin::Network.method("getInterfaces"),
                params: None,
            })
            .await;
        info!("{}", response.message);
        serde_json::from_str(&response.message.to_string()).unwrap()

        // let get_interfaces = Network.method("getInterfaces");
        // let get_internet_response = client
        //     .clone()
        //     .call_thunder(&get_interfaces, None, Some(span.clone()))
        //     .await;
        // serde_json::from_value(get_internet_response.message).unwrap()
    }

    async fn get_connected_interface(state: ThunderState) -> ThunderInterfaceType {
        let get_internet_response = Self::get_interfaces(state).await;
        let mut thunder_interface_type = ThunderInterfaceType::None;
        for i in get_internet_response.interfaces {
            if i.connected {
                thunder_interface_type = i.interface;
                break;
            }
        }
        thunder_interface_type
    }

    async fn has_internet(state: &ThunderState) -> bool {
        let response = state
            .get_thunder_client()
            .call(DeviceCallRequest {
                method: ThunderPlugin::LocationSync.method("location"),
                params: None,
            })
            .await;
        info!("{}", response.message);
        if let Some(ip) = response.message["publicip"].as_str() {
            return !ip.is_empty();
        };
        return false;
    }
}

#[derive(Debug)]
pub struct ThunderDeviceInfoRequestProcessor {
    state: ThunderState,
    streamer: DefaultExtnStreamer,
}

impl ThunderDeviceInfoRequestProcessor {
    pub fn new(state: ThunderState) -> ThunderDeviceInfoRequestProcessor {
        ThunderDeviceInfoRequestProcessor {
            state,
            streamer: DefaultExtnStreamer::new(),
        }
    }

    async fn mac_address(state: ThunderState, req: ExtnMessage) -> bool {
        let response = state
            .get_thunder_client()
            .call(DeviceCallRequest {
                method: ThunderPlugin::System.method("getDeviceInfo"),
                params: Some(DeviceChannelParams::Json(String::from(
                    "{\"params\": [\"estb_mac\"]}",
                ))),
            })
            .await;
        info!("{}", response.message);

        let mac_value_option = response.message["estb_mac"].as_str();
        let response: String;
        if let None = mac_value_option {
            response = "".to_string();
        } else {
            response = mac_value_option.unwrap().to_string();
        }
        Self::respond(state.get_client(), req, ExtnResponse::String(response))
            .await
            .is_ok()
    }

    async fn model(state: ThunderState, req: ExtnMessage) -> bool {
        let response = state
            .get_thunder_client()
            .call(DeviceCallRequest {
                method: ThunderPlugin::System.method("getSystemVersions"),
                params: None,
            })
            .await;
        info!("{}", response.message);

        let full_firmware_version = String::from(response.message["stbVersion"].as_str().unwrap());
        let split_string: Vec<&str> = full_firmware_version.split("_").collect();
        let response = String::from(split_string[0]);
        Self::respond(state.get_client(), req, ExtnResponse::String(response))
            .await
            .is_ok()
    }

    async fn make(state: ThunderState, req: ExtnMessage) -> bool {
        let response = state
            .get_thunder_client()
            .call(DeviceCallRequest {
                method: ThunderPlugin::System.method("getDeviceInfo"),
                params: None,
            })
            .await;
        info!("{}", response.message);
        let make_opt = response.message["make"].as_str();
        if let None = make_opt {
            ExtnResponse::Error(RippleError::InvalidOutput);
        }
        let response = make_opt.unwrap();
        Self::respond(
            state.get_client(),
            req,
            ExtnResponse::String(response.to_string()),
        )
        .await
        .is_ok()
    }

    async fn audio(state: ThunderState, req: ExtnMessage) -> bool {
        let response = state
            .get_thunder_client()
            .call(DeviceCallRequest {
                method: ThunderPlugin::DisplaySettings.method("getAudioFormat"),
                params: None,
            })
            .await;
        info!("{}", response.message);
        if response.message.get("success").is_none()
            || response.message["success"].as_bool().unwrap() == false
        {
            error!("{}", response.message);
            return false;
        }

        let mut hm: HashMap<AudioProfile, bool> = HashMap::new();
        hm.insert(AudioProfile::Stereo, false);
        hm.insert(AudioProfile::DolbyDigital5_1, false);
        hm.insert(AudioProfile::DolbyDigital5_1Plus, false);
        hm.insert(AudioProfile::DolbyDigital7_1, false);
        hm.insert(AudioProfile::DolbyDigital7_1Plus, false);
        hm.insert(AudioProfile::DolbyAtmos, false);

        if response.message.get("supportedAudioFormat").is_none() {
            error!("{}", response.message);
            return false;
        }
        let supported_profiles = response.message["supportedAudioFormat"].as_array().unwrap();
        for profile in supported_profiles {
            let profile_name = profile.as_str().unwrap();
            match profile_name {
                "PCM" => {
                    hm.insert(AudioProfile::Stereo, true);
                    hm.insert(AudioProfile::DolbyDigital5_1, true);
                }
                "DOLBY AC3" => {
                    hm.insert(AudioProfile::Stereo, true);
                    hm.insert(AudioProfile::DolbyDigital5_1, true);
                }
                "DOLBY EAC3" => {
                    hm.insert(AudioProfile::Stereo, true);
                    hm.insert(AudioProfile::DolbyDigital5_1, true);
                }
                "DOLBY AC4" => {
                    hm.insert(AudioProfile::Stereo, true);
                    hm.insert(AudioProfile::DolbyDigital5_1, true);
                    hm.insert(AudioProfile::DolbyDigital7_1, true);
                }
                "DOLBY TRUEHD" => {
                    hm.insert(AudioProfile::Stereo, true);
                    hm.insert(AudioProfile::DolbyDigital5_1, true);
                    hm.insert(AudioProfile::DolbyDigital7_1, true);
                }
                "DOLBY EAC3 ATMOS" => {
                    hm.insert(AudioProfile::Stereo, true);
                    hm.insert(AudioProfile::DolbyDigital5_1, true);
                    hm.insert(AudioProfile::DolbyDigital7_1, true);
                    hm.insert(AudioProfile::DolbyAtmos, true);
                }
                "DOLBY TRUEHD ATMOS" => {
                    hm.insert(AudioProfile::Stereo, true);
                    hm.insert(AudioProfile::DolbyDigital5_1, true);
                    hm.insert(AudioProfile::DolbyDigital7_1, true);
                    hm.insert(AudioProfile::DolbyAtmos, true);
                }
                "DOLBY AC4 ATMOS" => {
                    hm.insert(AudioProfile::Stereo, true);
                    hm.insert(AudioProfile::DolbyDigital5_1, true);
                    hm.insert(AudioProfile::DolbyDigital7_1, true);
                    hm.insert(AudioProfile::DolbyAtmos, true);
                }
                _ => (),
            }
        }
        Self::respond(
            state.get_client(),
            req,
            if let ExtnPayload::Response(r) =
                DeviceResponse::AudioProfileResponse(hm.clone()).get_extn_payload()
            {
                r
            } else {
                ExtnResponse::Error(RippleError::ProcessorError)
            },
        )
        .await
        .is_ok()
    }

    // async fn hdcp_support(state: ThunderState, req: ExtnMessage) -> bool {
    //     let response = state
    //     .get_thunder_client()
    //     .call(DeviceCallRequest {
    //         method: ThunderPlugin::Hdcp.method("getSettopHDCPSupport"),
    //         params: None,
    //     })
    //     .await;
    // info!("{}", response.message);
    //     let hdcp_version = response.message["supportedHDCPVersion"].to_string();
    //     let is_hdcp_supported: bool = response.message["isHDCPSupported"]
    //         .to_string()
    //         .parse()
    //         .unwrap();
    //     let mut hdcp_response = HashMap::new();
    //     if hdcp_version.contains("1.4") {
    //         hdcp_response.insert(HdcpProfile::Hdcp1_4, is_hdcp_supported);
    //     }
    //     if hdcp_version.contains("2.2") {
    //         hdcp_response.insert(HdcpProfile::Hdcp2_2, is_hdcp_supported);
    //     }
    //     Self::respond(
    //         state.get_client(),
    //         req,
    //         if let ExtnPayload::Response(r) =
    //             DeviceResponse::HdcpSupportResponse(hdcp_response.clone()).get_extn_payload()
    //         {
    //             r
    //         } else {
    //             ExtnResponse::Error(RippleError::ProcessorError)
    //         },
    //     )
    //     .await
    //     .is_ok()
    // }

    // async fn hdcp_status(state: ThunderState, req: ExtnMessage) -> bool {
    //     let response = state
    //     .get_thunder_client()
    //     .call(DeviceCallRequest {
    //         method: ThunderPlugin::Hdcp.method("getHDCPStatus"),
    //         params: None,
    //     })
    //     .await;
    // info!("{}", response.message);
    //     let thdcp: ThunderHDCPStatus = serde_json::from_value(response.message).unwrap();
    //     Self::respond(
    //         state.get_client(),
    //         req,
    //         if let ExtnPayload::Response(r) =
    //             DeviceResponse::HdcpStatusResponse(thdcp).get_extn_payload()
    //         {
    //             r
    //         } else {
    //             ExtnResponse::Error(RippleError::ProcessorError)
    //         },
    //     )
    //     .await
    //     .is_ok()
    // }

    async fn hdr(state: ThunderState, req: ExtnMessage) -> bool {
        let response = state
            .get_thunder_client()
            .call(DeviceCallRequest {
                method: ThunderPlugin::DisplaySettings.method("getTVHDRCapabilities"),
                params: None,
            })
            .await;
        info!("{}", response.message);
        let supported_cap: u32 = response.message["capabilities"]
            .to_string()
            .parse()
            .unwrap_or(0);
        let mut hm = HashMap::new();
        hm.insert(
            HdrProfile::Hdr10,
            0 != (supported_cap & hdr_flags::HDRSTANDARD_HDR10),
        );
        hm.insert(
            HdrProfile::Hlg,
            0 != (supported_cap & hdr_flags::HDRSTANDARD_HLG),
        );
        hm.insert(
            HdrProfile::DolbyVision,
            0 != (supported_cap & hdr_flags::HDRSTANDARD_DOLBY_VISION),
        );
        hm.insert(
            HdrProfile::Technicolor,
            0 != (supported_cap & hdr_flags::HDRSTANDARD_TECHNICOLOR_PRIME),
        );
        Self::respond(
            state.get_client(),
            req,
            if let ExtnPayload::Response(r) =
                DeviceResponse::HdrResponse(hm.clone()).get_extn_payload()
            {
                r
            } else {
                ExtnResponse::Error(RippleError::ProcessorError)
            },
        )
        .await
        .is_ok()
    }

    async fn screen_resolution(state: ThunderState, req: ExtnMessage) -> bool {
        let response = state
            .get_thunder_client()
            .call(DeviceCallRequest {
                method: ThunderPlugin::DisplaySettings.method("getCurrentResolution"),
                params: None,
            })
            .await;
        info!("{}", response.message);

        if response.message.get("success").is_none()
            || response.message["success"].as_bool().unwrap() == false
        {
            error!("{}", response.message);
            return false;
        }
        info!("{}", response.message);
        let resol = response.message["resolution"].as_str().unwrap();
        let ans = get_dimension_from_resolution(&resol);

        Self::respond(
            state.get_client(),
            req,
            if let ExtnPayload::Response(r) =
                DeviceResponse::ScreenResolutionResponse(ans).get_extn_payload()
            {
                r
            } else {
                ExtnResponse::Error(RippleError::ProcessorError)
            },
        )
        .await
        .is_ok()
    }

    async fn video_resolution(state: ThunderState, req: ExtnMessage) -> bool {
        let response = state
            .get_thunder_client()
            .call(DeviceCallRequest {
                method: ThunderPlugin::DisplaySettings.method("getDefaultResolution"),
                params: None,
            })
            .await;
        if response.message.get("success").is_none()
            || response.message["success"].as_bool().unwrap() == false
        {
            error!("{}", response.message);
            return false;
        }
        info!("{}", response.message);
        let resol = response.message["defaultResolution"].as_str().unwrap();
        let ans = get_dimension_from_resolution(&resol);

        Self::respond(
            state.get_client(),
            req,
            if let ExtnPayload::Response(r) =
                DeviceResponse::VideoResolutionResponse(ans).get_extn_payload()
            {
                r
            } else {
                ExtnResponse::Error(RippleError::ProcessorError)
            },
        )
        .await
        .is_ok()
    }

    async fn network(state: ThunderState, req: ExtnMessage) -> bool {
        let interface_response =
            ThunderNetworkService::get_connected_interface(state.clone()).await;
        let network_status = NetworkResponse {
            _type: Box::new(interface_response.clone()).to_network_type(),
            state: match interface_response.clone() {
                ThunderInterfaceType::Ethernet | ThunderInterfaceType::Wifi => {
                    NetworkState::Connected
                }
                ThunderInterfaceType::None => NetworkState::Disconnected,
            },
        };
        Self::respond(
            state.get_client(),
            req,
            ExtnResponse::NetworkResponse(network_status.clone()),
        )
        .await
        .is_ok()
    }

    async fn on_internet_connected(state: ThunderState, req: ExtnMessage, timeout: u64) -> bool {
        if ThunderNetworkService::has_internet(&state).await {
            return Self::respond(state.get_client(), req, ExtnResponse::None(()))
                .await
                .is_ok();
        }

        let (s, mut r) = mpsc::channel::<DeviceResponseMessage>(32);
        let cloned_state = state.clone();
        let client = state.get_thunder_client();

        client
            .clone()
            .subscribe(
                DeviceSubscribeRequest {
                    module: LocationSync.callsign_and_version(),
                    event_name: "locationchange".into(),
                    params: None,
                    sub_id: None,
                },
                s,
            )
            .await;
        info!("subscribed to locationchangeChanged events");

        let thread_res = tokio::spawn(async move {
            while let Some(_) = r.recv().await {
                if ThunderNetworkService::has_internet(&cloned_state).await {
                    // Internet precondition for browsers are supposed to be met
                    // when locationchange event is given, but seems to be a short period
                    // where it still is not. Wait for a small amount of time.
                    thread::sleep(time::Duration::from_millis(1000));
                    cloned_state
                        .get_thunder_client()
                        .unsubscribe(DeviceUnsubscribeRequest {
                            module: LocationSync.callsign_and_version(),
                            event_name: "locationchange".into(),
                        })
                        .await;
                    info!("Unsubscribing to locationchangeChanged events");
                }
            }
        });
        let dur = Duration::from_millis(timeout);
        if let Err(_) = tokio::time::timeout(dur, thread_res).await {
            return Self::respond(
                state.get_client(),
                req,
                ExtnResponse::Error(RippleError::NoResponse),
            )
            .await
            .is_ok();
        }
        return Self::respond(state.get_client().clone(), req, ExtnResponse::None(()))
            .await
            .is_ok();
    }

    async fn version(state: ThunderState, req: ExtnMessage) -> bool {
        let response = state
            .get_thunder_client()
            .call(DeviceCallRequest {
                method: ThunderPlugin::System.method("getSystemVersions"),
                params: None,
            })
            .await;
        info!("{}", response.message);
        let tsv: SystemVersion = serde_json::from_value(response.message).unwrap();
        let tsv_split = tsv.receiver_version.split(".");
        let tsv_vec: Vec<&str> = tsv_split.collect();

        if tsv_vec.len() >= 3 {
            let major: String = tsv_vec[0].chars().filter(|c| c.is_digit(10)).collect();
            let minor: String = tsv_vec[1].chars().filter(|c| c.is_digit(10)).collect();
            let patch: String = tsv_vec[2].chars().filter(|c| c.is_digit(10)).collect();

            Self::respond(
                state.get_client(),
                req,
                if let ExtnPayload::Response(r) =
                    DeviceResponse::FirmwareInfo(FireboltSemanticVersion {
                        major: major.parse::<u32>().unwrap(),
                        minor: minor.parse::<u32>().unwrap(),
                        patch: patch.parse::<u32>().unwrap(),
                        readable: tsv.stb_version,
                    })
                    .get_extn_payload()
                {
                    r
                } else {
                    ExtnResponse::Error(RippleError::ProcessorError)
                },
            )
            .await
            .is_ok()
        } else {
            let mut fsv = FireboltSemanticVersion::default();
            fsv.readable = tsv.stb_version;
            Self::respond(
                state.get_client(),
                req,
                if let ExtnPayload::Response(r) =
                    DeviceResponse::FirmwareInfo(fsv).get_extn_payload()
                {
                    r
                } else {
                    ExtnResponse::Error(RippleError::ProcessorError)
                },
            )
            .await
            .is_ok()
        }
    }

    async fn available_memory(state: ThunderState, req: ExtnMessage) -> bool {
        let response = state
            .get_thunder_client()
            .call(DeviceCallRequest {
                method: ThunderPlugin::RDKShell.method("getSystemMemory"),
                params: None,
            })
            .await;
        info!("{}", response.message);
        if response.message.get("success").is_some()
            && response.message["success"].as_bool().unwrap() == true
        {
            if let Some(v) = response.message["freeRam"].as_u64() {
                return Self::respond(state.get_client(), req, ExtnResponse::Value(json!(v)))
                    .await
                    .is_ok();
            }
        }
        error!("{}", response.message);
        Self::handle_error(state.get_client(), req, RippleError::ProcessorError).await
    }

    async fn on_active_input_changed(state: ThunderState, req: ExtnMessage) -> bool {
        //TODO: thunder event handler
        todo!();
    }

    async fn on_resolution_changed(state: ThunderState, req: ExtnMessage) -> bool {
        //TODO: thunder event handler
        todo!();
    }

    async fn on_network_changed(state: ThunderState, req: ExtnMessage) -> bool {
        //TODO: thunder event handler
        todo!();
    }
}

pub fn get_dimension_from_resolution(resolution: &str) -> Vec<i32> {
    match resolution {
        val if val.starts_with("480") => Resolution::Resolution480.dimension(),
        val if val.starts_with("576") => Resolution::Resolution576.dimension(),
        val if val.starts_with("540") => Resolution::Resolution540.dimension(),
        val if val.starts_with("720") => Resolution::Resolution720.dimension(),
        val if val.starts_with("1080") => Resolution::Resolution1080.dimension(),
        val if val.starts_with("2160") => Resolution::Resolution2160.dimension(),
        val if val.starts_with("4K") || val.starts_with("4k") => {
            Resolution::Resolution4k.dimension()
        }
        _ => Resolution::ResolutionDefault.dimension(),
    }
}

impl ExtnStreamProcessor for ThunderDeviceInfoRequestProcessor {
    type STATE = ThunderState;
    type VALUE = DeviceInfoRequest;

    fn get_state(&self) -> Self::STATE {
        self.state.clone()
    }

    fn receiver(&mut self) -> mpsc::Receiver<ExtnMessage> {
        self.streamer.receiver()
    }

    fn sender(&self) -> mpsc::Sender<ExtnMessage> {
        self.streamer.sender()
    }
}

#[async_trait]
impl ExtnRequestProcessor for ThunderDeviceInfoRequestProcessor {
    fn get_client(&self) -> ExtnClient {
        self.state.get_client()
    }

    async fn process_request(
        state: Self::STATE,
        msg: ExtnMessage,
        extracted_message: Self::VALUE,
    ) -> bool {
        match extracted_message {
            DeviceInfoRequest::MacAddress => Self::mac_address(state.clone(), msg).await,
            DeviceInfoRequest::Model => Self::model(state.clone(), msg).await,
            DeviceInfoRequest::Audio => Self::audio(state.clone(), msg).await,
            //            DeviceInfoRequest::HdcpSupport => Self::hdcp_support(state.clone(),msg).await,
            //            DeviceInfoRequest::HdcpStatus => Self::hdcp_status(state.clone(),msg).await,
            DeviceInfoRequest::Hdr => Self::hdr(state.clone(), msg).await,
            DeviceInfoRequest::ScreenResolution => {
                Self::screen_resolution(state.clone(), msg).await
            }
            DeviceInfoRequest::VideoResolution => Self::video_resolution(state.clone(), msg).await,
            DeviceInfoRequest::Network => Self::network(state.clone(), msg).await,
            DeviceInfoRequest::Make => Self::make(state.clone(), msg).await,
            DeviceInfoRequest::Version => Self::version(state.clone(), msg).await,
            DeviceInfoRequest::AvailableMemory => Self::available_memory(state.clone(), msg).await,
            DeviceInfoRequest::OnInternetConnected(time_out) => {
                Self::on_internet_connected(state.clone(), msg, time_out.timeout).await
            }
            DeviceInfoRequest::Sku => todo!(),
            _ => false,
        }
    }
}
