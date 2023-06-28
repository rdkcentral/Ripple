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

use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
    thread,
    time::{self, Duration},
};

use crate::processors::thunder_device_info::ThunderPlugin::LocationSync;

use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::json;
use thunder_ripple_sdk::{
    client::{thunder_client::ThunderClient, thunder_plugin::ThunderPlugin},
    ripple_sdk::{
        api::device::{device_info_request::DeviceCapabilities, device_request::AudioProfile},
        chrono::NaiveDateTime,
        extn::client::extn_client::ExtnClient,
        tokio::sync::mpsc,
    },
    thunder_state::ThunderState,
};
use thunder_ripple_sdk::{
    ripple_sdk::{
        api::{
            device::{
                device_info_request::{DeviceInfoRequest, DeviceResponse},
                device_operator::{
                    DeviceCallRequest, DeviceChannelParams, DeviceOperator, DeviceResponseMessage,
                    DeviceSubscribeRequest, DeviceUnsubscribeRequest,
                },
                device_request::{
                    HDCPStatus, HdcpProfile, HdrProfile, NetworkResponse, NetworkState,
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
        serde_json::{self},
        tokio,
        utils::error::RippleError,
    },
    utils::get_audio_profile_from_value,
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

#[derive(Debug, Clone, Default)]
pub struct CachedDeviceInfo {
    mac_address: Option<String>,
    model: Option<String>,
    make: Option<String>,
    hdcp_support: Option<HashMap<HdcpProfile, bool>>,
    hdcp_status: Option<HDCPStatus>,
    hdr_profile: Option<HashMap<HdrProfile, bool>>,
    version: Option<FireboltSemanticVersion>,
}

#[derive(Debug, Clone)]
pub struct CachedState {
    state: ThunderState,
    cached: Arc<RwLock<CachedDeviceInfo>>,
}

impl CachedState {
    fn new(state: ThunderState) -> Self {
        Self {
            state,
            cached: Arc::new(RwLock::new(CachedDeviceInfo::default())),
        }
    }

    fn get_client(&self) -> ExtnClient {
        self.state.get_client()
    }

    fn get_thunder_client(&self) -> ThunderClient {
        self.state.get_thunder_client()
    }

    fn get_hdcp_support(&self) -> Option<HashMap<HdcpProfile, bool>> {
        self.cached.read().unwrap().hdcp_support.clone()
    }

    fn update_hdcp_support(&self, value: HashMap<HdcpProfile, bool>) {
        let mut hdcp = self.cached.write().unwrap();
        let _ = hdcp.hdcp_support.insert(value);
    }

    fn get_hdcp_status(&self) -> Option<HDCPStatus> {
        self.cached.read().unwrap().hdcp_status.clone()
    }

    fn update_hdcp_status(&self, value: HDCPStatus) {
        let mut hdcp = self.cached.write().unwrap();
        let _ = hdcp.hdcp_status.insert(value);
    }

    fn get_hdr(&self) -> Option<HashMap<HdrProfile, bool>> {
        self.cached.read().unwrap().hdr_profile.clone()
    }

    fn update_hdr_support(&self, value: HashMap<HdrProfile, bool>) {
        let mut hdr = self.cached.write().unwrap();
        let _ = hdr.hdr_profile.insert(value);
    }

    fn get_mac_address(&self) -> Option<String> {
        self.cached.read().unwrap().mac_address.clone()
    }

    fn update_mac_address(&self, mac: String) {
        let mut cached = self.cached.write().unwrap();
        let _ = cached.mac_address.insert(mac);
    }

    fn get_model(&self) -> Option<String> {
        self.cached.read().unwrap().model.clone()
    }

    fn update_model(&self, model: String) {
        let mut cached = self.cached.write().unwrap();
        let _ = cached.model.insert(model);
    }

    fn get_make(&self) -> Option<String> {
        self.cached.read().unwrap().make.clone()
    }

    fn update_make(&self, make: String) {
        let mut cached = self.cached.write().unwrap();
        let _ = cached.make.insert(make);
    }

    fn get_version(&self) -> Option<FireboltSemanticVersion> {
        self.cached.read().unwrap().version.clone()
    }

    fn update_version(&self, version: FireboltSemanticVersion) {
        let mut cached = self.cached.write().unwrap();
        let _ = cached.version.insert(version);
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct ThunderTimezonesResponse {
    pub zoneinfo: HashMap<String, MapOrString>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(untagged)]
pub enum MapOrString {
    Map(HashMap<String, MapOrString>),
    String(String),
}

fn get_date_time_str(map: &HashMap<String, MapOrString>, keys: Vec<String>) -> Option<String> {
    if keys.is_empty() {
        return None;
    }

    let first_key = &keys[0];

    if let Some(value) = map.get(first_key) {
        if keys.len() == 1 {
            if let MapOrString::String(date_time_str) = value {
                return Some(date_time_str.clone());
            }
        } else if let MapOrString::Map(nested_map) = value {
            let remaining_keys = keys[1..].to_vec();
            return get_date_time_str(nested_map, remaining_keys);
        }
    }

    None
}

pub struct ThunderNetworkService;

impl ThunderNetworkService {
    async fn get_interfaces(state: CachedState) -> ThunderGetInterfacesResponse {
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

    async fn get_connected_interface(state: CachedState) -> ThunderInterfaceType {
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

    async fn has_internet(state: &CachedState) -> bool {
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
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ThunderTimezoneResponse {
    #[serde(rename = "timeZone")]
    pub time_zone: String,
}

#[derive(Debug)]
pub struct ThunderDeviceInfoRequestProcessor {
    state: CachedState,
    streamer: DefaultExtnStreamer,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ThunderAvailableTimezonesResponse {
    pub zoneinfo: HashMap<String, HashMap<String, String>>,
}

impl ThunderAvailableTimezonesResponse {
    pub fn as_array(&self) -> Vec<String> {
        let mut timezones = Vec::default();
        for (area, locations) in &self.zoneinfo {
            let mut found_location = false;
            for (location, _local_time) in locations {
                timezones.push(format!("{}/{}", area, location));
                found_location = true;
            }
            if !found_location {
                // If there weren't any specific locations within the area, just add the area itself as a timezone.
                timezones.push(area.to_string());
            }
        }
        timezones
    }
}

impl ThunderDeviceInfoRequestProcessor {
    pub fn new(state: ThunderState) -> ThunderDeviceInfoRequestProcessor {
        ThunderDeviceInfoRequestProcessor {
            state: CachedState::new(state),
            streamer: DefaultExtnStreamer::new(),
        }
    }

    async fn get_mac_address(state: &CachedState) -> String {
        let response: String;
        match state.get_mac_address() {
            Some(value) => response = value,
            None => {
                let resp = state
                    .get_thunder_client()
                    .call(DeviceCallRequest {
                        method: ThunderPlugin::System.method("getDeviceInfo"),
                        params: Some(DeviceChannelParams::Json(String::from(
                            "{\"params\": [\"estb_mac\"]}",
                        ))),
                    })
                    .await;
                info!("{}", resp.message);

                let mac_value_option = resp.message["estb_mac"].as_str();
                if let None = mac_value_option {
                    response = "".to_string();
                } else {
                    response = mac_value_option.unwrap().to_string();
                    state.update_mac_address(response.clone())
                }
            }
        }
        response
    }

    async fn mac_address(state: CachedState, req: ExtnMessage) -> bool {
        let response: String = Self::get_mac_address(&state).await;

        Self::respond(state.get_client(), req, ExtnResponse::String(response))
            .await
            .is_ok()
    }

    async fn get_model(state: &CachedState) -> String {
        let response: String;
        match state.get_model() {
            Some(value) => response = value,
            None => {
                let resp = state
                    .get_thunder_client()
                    .call(DeviceCallRequest {
                        method: ThunderPlugin::System.method("getSystemVersions"),
                        params: None,
                    })
                    .await;
                info!("{}", resp.message);

                let full_firmware_version =
                    String::from(resp.message["stbVersion"].as_str().unwrap());
                let split_string: Vec<&str> = full_firmware_version.split("_").collect();
                response = String::from(split_string[0]);
                state.update_model(response.clone());
            }
        }
        response
    }

    async fn model(state: CachedState, req: ExtnMessage) -> bool {
        let response = Self::get_model(&state).await;
        Self::respond(state.get_client(), req, ExtnResponse::String(response))
            .await
            .is_ok()
    }

    async fn get_make(state: &CachedState) -> String {
        let response: String;
        match state.get_make() {
            Some(value) => response = value,
            None => {
                let resp = state
                    .get_thunder_client()
                    .call(DeviceCallRequest {
                        method: ThunderPlugin::System.method("getDeviceInfo"),
                        params: None,
                    })
                    .await;
                info!("{}", resp.message);
                if let Some(make) = resp.message["make"].as_str() {
                    response = make.into();
                    state.update_make(response.clone());
                } else {
                    response = "".into();
                }
            }
        }
        response
    }

    async fn make(state: CachedState, req: ExtnMessage) -> bool {
        let response: String = Self::get_make(&state).await;

        Self::respond(
            state.get_client(),
            req,
            ExtnResponse::String(response.to_string()),
        )
        .await
        .is_ok()
    }

    async fn get_audio(state: &CachedState) -> HashMap<AudioProfile, bool> {
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
            return HashMap::new();
        }

        get_audio_profile_from_value(response.message)
    }

    async fn audio(state: CachedState, req: ExtnMessage) -> bool {
        let hm = Self::get_audio(&state).await;
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

    pub async fn get_hdcp_support(state: ThunderState) -> HashMap<HdcpProfile, bool> {
        let response = state
            .get_thunder_client()
            .call(DeviceCallRequest {
                method: ThunderPlugin::Hdcp.method("getSettopHDCPSupport"),
                params: None,
            })
            .await;
        info!("{}", response.message);
        let hdcp_version = response.message["supportedHDCPVersion"].to_string();
        let is_hdcp_supported: bool = response.message["isHDCPSupported"]
            .to_string()
            .parse()
            .unwrap();
        let mut hdcp_response = HashMap::new();
        if hdcp_version.contains("1.4") {
            hdcp_response.insert(HdcpProfile::Hdcp1_4, is_hdcp_supported);
        }
        if hdcp_version.contains("2.2") {
            hdcp_response.insert(HdcpProfile::Hdcp2_2, is_hdcp_supported);
        }
        hdcp_response
    }

    async fn get_cached_hdcp_support(state: CachedState) -> HashMap<HdcpProfile, bool> {
        if let Some(v) = state.get_hdcp_support() {
            v
        } else {
            let v = Self::get_hdcp_support(state.clone().state).await;
            state.update_hdcp_support(v.clone());
            v
        }
    }

    async fn hdcp_support(state: CachedState, req: ExtnMessage) -> bool {
        let hdcp_response = Self::get_cached_hdcp_support(state.clone()).await;
        Self::respond(
            state.get_client(),
            req,
            if let ExtnPayload::Response(r) =
                DeviceResponse::HdcpSupportResponse(hdcp_response.clone()).get_extn_payload()
            {
                r
            } else {
                ExtnResponse::Error(RippleError::ProcessorError)
            },
        )
        .await
        .is_ok()
    }

    async fn get_hdcp_status(state: &CachedState) -> HDCPStatus {
        let response: HDCPStatus;
        match state.get_hdcp_status() {
            Some(status) => response = status,
            None => {
                let resp = state
                    .get_thunder_client()
                    .call(DeviceCallRequest {
                        method: ThunderPlugin::Hdcp.method("getHDCPStatus"),
                        params: None,
                    })
                    .await;
                info!("{}", resp.message);
                let thdcp: ThunderHDCPStatus = serde_json::from_value(resp.message).unwrap();
                response = thdcp.hdcp_status;
                state.update_hdcp_status(response.clone());
            }
        }
        response
    }

    async fn hdcp_status(state: CachedState, req: ExtnMessage) -> bool {
        let response = Self::get_hdcp_status(&state).await;

        Self::respond(
            state.get_client(),
            req,
            if let ExtnPayload::Response(r) =
                DeviceResponse::HdcpStatusResponse(response).get_extn_payload()
            {
                r
            } else {
                ExtnResponse::Error(RippleError::ProcessorError)
            },
        )
        .await
        .is_ok()
    }

    async fn get_cached_hdr(state: &CachedState) -> HashMap<HdrProfile, bool> {
        if let Some(v) = state.get_hdr() {
            v
        } else {
            let v = Self::get_hdr(state.clone().state).await;
            state.update_hdr_support(v.clone());
            v
        }
    }

    pub async fn get_hdr(state: ThunderState) -> HashMap<HdrProfile, bool> {
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
        hm
    }

    async fn hdr(state: CachedState, req: ExtnMessage) -> bool {
        let hm = Self::get_cached_hdr(&state).await;
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

    async fn get_screen_resolution(state: &CachedState) -> Vec<i32> {
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
            return Vec::new();
        }
        info!("{}", response.message);
        let resol = response.message["resolution"].as_str().unwrap();
        get_dimension_from_resolution(&resol)
    }

    async fn screen_resolution(state: CachedState, req: ExtnMessage) -> bool {
        let ans = Self::get_screen_resolution(&state).await;

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

    async fn get_video_resolution(state: &CachedState) -> Vec<i32> {
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
            return Vec::new();
        }
        info!("{}", response.message);
        let resol = response.message["defaultResolution"].as_str().unwrap();
        get_dimension_from_resolution(&resol)
    }

    async fn video_resolution(state: CachedState, req: ExtnMessage) -> bool {
        let ans = Self::get_video_resolution(&state).await;

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

    async fn get_network(state: &CachedState) -> NetworkResponse {
        let interface_response =
            ThunderNetworkService::get_connected_interface(state.clone()).await;
        NetworkResponse {
            _type: Box::new(interface_response.clone()).to_network_type(),
            state: match interface_response.clone() {
                ThunderInterfaceType::Ethernet | ThunderInterfaceType::Wifi => {
                    NetworkState::Connected
                }
                ThunderInterfaceType::None => NetworkState::Disconnected,
            },
        }
    }

    async fn network(state: CachedState, req: ExtnMessage) -> bool {
        let network_status = Self::get_network(&state).await;
        Self::respond(
            state.get_client(),
            req,
            ExtnResponse::NetworkResponse(network_status.clone()),
        )
        .await
        .is_ok()
    }

    async fn on_internet_connected(state: CachedState, req: ExtnMessage, timeout: u64) -> bool {
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

    async fn get_version(state: &CachedState) -> FireboltSemanticVersion {
        let response: FireboltSemanticVersion;
        match state.get_version() {
            Some(v) => response = v,
            None => {
                let resp = state
                    .get_thunder_client()
                    .call(DeviceCallRequest {
                        method: ThunderPlugin::System.method("getSystemVersions"),
                        params: None,
                    })
                    .await;
                info!("{}", resp.message);
                let tsv: SystemVersion = serde_json::from_value(resp.message).unwrap();
                let tsv_split = tsv.receiver_version.split(".");
                let tsv_vec: Vec<&str> = tsv_split.collect();

                if tsv_vec.len() >= 3 {
                    let major: String = tsv_vec[0].chars().filter(|c| c.is_digit(10)).collect();
                    let minor: String = tsv_vec[1].chars().filter(|c| c.is_digit(10)).collect();
                    let patch: String = tsv_vec[2].chars().filter(|c| c.is_digit(10)).collect();

                    response = FireboltSemanticVersion {
                        major: major.parse::<u32>().unwrap(),
                        minor: minor.parse::<u32>().unwrap(),
                        patch: patch.parse::<u32>().unwrap(),
                        readable: tsv.stb_version,
                    };
                    state.update_version(response.clone());
                } else {
                    let mut fsv = FireboltSemanticVersion::default();
                    fsv.readable = tsv.stb_version;
                    response = fsv;
                    state.update_version(response.clone())
                }
            }
        }
        response
    }

    async fn version(state: CachedState, req: ExtnMessage) -> bool {
        let response = Self::get_version(&state).await;
        Self::respond(
            state.get_client(),
            req,
            if let ExtnPayload::Response(r) =
                DeviceResponse::FirmwareInfo(response).get_extn_payload()
            {
                r
            } else {
                ExtnResponse::Error(RippleError::ProcessorError)
            },
        )
        .await
        .is_ok()
    }

    async fn available_memory(state: CachedState, req: ExtnMessage) -> bool {
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

    async fn _on_active_input_changed(_state: CachedState, _req: ExtnMessage) -> bool {
        //TODO: thunder event handler
        todo!();
    }

    async fn _on_resolution_changed(_state: CachedState, _req: ExtnMessage) -> bool {
        //TODO: thunder event handler
        todo!();
    }

    async fn _on_network_changed(_state: CachedState, _req: ExtnMessage) -> bool {
        //TODO: thunder event handler
        todo!();
    }

    async fn get_timezone_value(state: &CachedState) -> Result<String, RippleError> {
        let response = state
            .get_thunder_client()
            .call(DeviceCallRequest {
                method: ThunderPlugin::System.method("getTimeZoneDST"),
                params: None,
            })
            .await;

        info!("{}", response.message);
        if response.message.get("success").is_none()
            || response.message["success"].as_bool().unwrap() == true
        {
            if let Ok(v) = serde_json::from_value::<ThunderTimezoneResponse>(response.message) {
                return Ok(v.time_zone.to_owned());
            }
        }
        Err(RippleError::ProcessorError)
    }

    async fn get_timezone(state: CachedState, req: ExtnMessage) -> bool {
        if let Ok(v) = Self::get_timezone_value(&state).await {
            Self::respond(state.get_client(), req, ExtnResponse::String(v))
                .await
                .is_ok()
        } else {
            Self::handle_error(state.get_client(), req, RippleError::ProcessorError).await
        }
    }

    async fn get_timezone_with_offset(state: CachedState, req: ExtnMessage) -> bool {
        if let Ok(timezone) = Self::get_timezone_value(&state).await {
            if timezone.find("/").is_some() {
                if let Ok(timezones) = Self::get_all_timezones(&state).await {
                    let timezones: &ThunderTimezonesResponse = &timezones;
                    let keys: Vec<String> = timezone.split('/').map(|s| s.to_string()).collect();
                    let parsed_utc_datetime =
                        get_date_time_str(&timezones.zoneinfo, vec!["UTC".to_owned()])
                            .or_else(|| {
                                get_date_time_str(
                                    &timezones.zoneinfo,
                                    vec!["Etc".to_owned(), "UTC".to_owned()],
                                )
                            })
                            .and_then(|date_time_str| {
                                NaiveDateTime::parse_from_str(
                                    &date_time_str,
                                    "%a %b %d %H:%M:%S %Y %Z",
                                )
                                .ok()
                            });

                    if parsed_utc_datetime.is_some() {
                        let parsed_datetime = get_date_time_str(&timezones.zoneinfo, keys.clone())
                            .and_then(|date_time_str| {
                                NaiveDateTime::parse_from_str(
                                    &date_time_str,
                                    "%a %b %d %H:%M:%S %Y %Z",
                                )
                                .ok()
                            });

                        let offset_seconds = parsed_datetime.map_or_else(
                            || {
                                error!(
                                    "get_timezone_offset: Unsupported timezone: timezone={}",
                                    timezone
                                );
                                Err(RippleError::ProcessorError)
                            },
                            |_| {
                                let delta = parsed_datetime.unwrap() - parsed_utc_datetime.unwrap();
                                Ok(delta.num_seconds())
                            },
                        );

                        if let Ok(offset) = offset_seconds {
                            return Self::respond(
                                state.get_client(),
                                req,
                                ExtnResponse::TimezoneWithOffset(timezone, offset),
                            )
                            .await
                            .is_ok();
                        }
                    }
                }
            }
        }
        error!("get_timezone_offset: Unsupported timezone");
        Self::handle_error(state.get_client(), req, RippleError::ProcessorError).await
    }

    async fn get_all_timezones<T: DeserializeOwned>(state: &CachedState) -> Result<T, RippleError> {
        let response = state
            .get_thunder_client()
            .call(DeviceCallRequest {
                method: ThunderPlugin::System.method("getTimeZones"),
                params: None,
            })
            .await;
        info!("{}", response.message);
        if response.message.get("success").is_some()
            && response.message["success"].as_bool().unwrap()
        {
            if let Ok(timezones) = serde_json::from_value::<T>(response.message) {
                return Ok(timezones);
            }
        }
        Err(RippleError::ProcessorError)
    }

    async fn get_available_timezones(state: CachedState, req: ExtnMessage) -> bool {
        if let Ok(v) = Self::get_all_timezones(&state).await {
            let timezones: ThunderAvailableTimezonesResponse = v;
            Self::respond(
                state.get_client(),
                req,
                ExtnResponse::AvailableTimezones(timezones.as_array()),
            )
            .await
            .is_ok()
        } else {
            Self::handle_error(state.get_client(), req, RippleError::ProcessorError).await
        }
    }

    async fn set_timezone(state: CachedState, timezone: String, request: ExtnMessage) -> bool {
        let params = Some(DeviceChannelParams::Json(
            json!({
                "timeZone": timezone,
            })
            .to_string(),
        ));

        let response = state
            .get_thunder_client()
            .call(DeviceCallRequest {
                method: ThunderPlugin::System.method("setTimeZoneDST"),
                params: params,
            })
            .await;
        info!("{}", response.message);

        if response.message.get("success").is_none()
            || response.message["success"].as_bool().unwrap() == true
        {
            return Self::respond(state.get_client(), request, ExtnResponse::None(()))
                .await
                .is_ok();
        }
        Self::handle_error(state.get_client(), request, RippleError::ProcessorError).await
    }

    async fn voice_guidance_enabled(state: CachedState, request: ExtnMessage) -> bool {
        let response = state
            .get_thunder_client()
            .call(DeviceCallRequest {
                method: ThunderPlugin::TextToSpeech.method("isttsenabled"),
                params: None,
            })
            .await;
        if let Some(v) = response.message["isenabled"].as_bool() {
            return Self::respond(state.get_client(), request, ExtnResponse::Boolean(v))
                .await
                .is_ok();
        }
        Self::handle_error(state.get_client(), request, RippleError::ProcessorError).await
    }

    async fn voice_guidance_set_enabled(
        state: CachedState,
        request: ExtnMessage,
        enabled: bool,
    ) -> bool {
        let params = Some(DeviceChannelParams::Json(
            json!({
                "enabletts": enabled,
            })
            .to_string(),
        ));
        let response = state
            .get_thunder_client()
            .call(DeviceCallRequest {
                method: ThunderPlugin::TextToSpeech.method("enabletts"),
                params,
            })
            .await;
        if response.message.get("success").is_some() {
            if let Some(_) = response.message["success"].as_bool() {
                return Self::ack(state.get_client(), request).await.is_ok();
            }
        }
        Self::handle_error(state.get_client(), request, RippleError::ProcessorError).await
    }

    async fn voice_guidance_speed(state: CachedState, request: ExtnMessage) -> bool {
        let response = state
            .get_thunder_client()
            .call(DeviceCallRequest {
                method: ThunderPlugin::TextToSpeech.method("getttsconfiguration"),
                params: None,
            })
            .await;
        if let Some(rate) = response.message["rate"].as_f64() {
            return Self::respond(
                state.get_client(),
                request,
                ExtnResponse::Float(scale_voice_speed_from_thunder_to_firebolt(rate as f32)),
            )
            .await
            .is_ok();
        }
        Self::handle_error(state.get_client(), request, RippleError::ProcessorError).await
    }

    async fn voice_guidance_set_speed(
        state: CachedState,
        request: ExtnMessage,
        speed: f32,
    ) -> bool {
        let params = Some(DeviceChannelParams::Json(
            json!({
                "rate": scale_voice_speed_from_firebolt_to_thunder(speed),
            })
            .to_string(),
        ));
        let response = state
            .get_thunder_client()
            .call(DeviceCallRequest {
                method: ThunderPlugin::TextToSpeech.method("setttsconfiguration"),
                params,
            })
            .await;
        if response.message.get("success").is_some() {
            if let Some(_) = response.message["success"].as_bool() {
                return Self::ack(state.get_client(), request).await.is_ok();
            }
        }
        Self::handle_error(state.get_client(), request, RippleError::ProcessorError).await
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

/*
per https://ccp.sys.comcast.net/browse/RPPL-283
Firebolt spec range for this value is 0 >= value <= 10
but...
Thunder is 1..100
for

*/
fn scale_voice_speed_from_thunder_to_firebolt(thunder_voice_speed: f32) -> f32 {
    thunder_voice_speed / 50.0
}
/*
Note that this returns an f32 (when it should seemingly return i32), which is to make it compatible with
DeviceRequest::VoiceGuidanceSetSpeed(speed).
*/
fn scale_voice_speed_from_firebolt_to_thunder(firebolt_voice_speed: f32) -> f32 {
    if firebolt_voice_speed <= 1.0 {
        firebolt_voice_speed * 50.0
    } else if firebolt_voice_speed < 2.0 {
        (50.0 * (firebolt_voice_speed / 10.0)) + 50.0
    } else {
        100.0
    }
}

impl ExtnStreamProcessor for ThunderDeviceInfoRequestProcessor {
    type STATE = CachedState;
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
            DeviceInfoRequest::HdcpSupport => Self::hdcp_support(state.clone(), msg).await,
            DeviceInfoRequest::HdcpStatus => Self::hdcp_status(state.clone(), msg).await,
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
            DeviceInfoRequest::GetTimezone => Self::get_timezone(state.clone(), msg).await,
            DeviceInfoRequest::GetTimezoneWithOffset => {
                Self::get_timezone_with_offset(state.clone(), msg).await
            }
            DeviceInfoRequest::GetAvailableTimezones => {
                Self::get_available_timezones(state.clone(), msg).await
            }
            DeviceInfoRequest::SetTimezone(timezone_params) => {
                Self::set_timezone(state.clone(), timezone_params, msg).await
            }
            DeviceInfoRequest::SetVoiceGuidanceEnabled(v) => {
                Self::voice_guidance_set_enabled(state.clone(), msg, v).await
            }
            DeviceInfoRequest::VoiceGuidanceEnabled => {
                Self::voice_guidance_enabled(state.clone(), msg).await
            }
            DeviceInfoRequest::SetVoiceGuidanceSpeed(s) => {
                Self::voice_guidance_set_speed(state.clone(), msg, s).await
            }
            DeviceInfoRequest::VoiceGuidanceSpeed => {
                Self::voice_guidance_speed(state.clone(), msg).await
            }
            DeviceInfoRequest::FullCapabilities => {
                let device_capabilities = DeviceCapabilities {
                    audio: Self::get_audio(&state).await,
                    firmware_info: Self::get_version(&state).await,
                    hdcp: Self::get_hdcp_status(&state).await,
                    hdr: Self::get_cached_hdr(&state).await,
                    is_wifi: match Self::get_network(&state).await._type {
                        NetworkType::Wifi => true,
                        _ => false,
                    },
                    make: Self::get_make(&state).await,
                    model: Self::get_model(&state).await,
                    video_resolution: Self::get_video_resolution(&state).await,
                    screen_resolution: Self::get_screen_resolution(&state).await,
                };
                if let ExtnPayload::Response(r) =
                    DeviceResponse::FullCapabilities(device_capabilities).get_extn_payload()
                {
                    Self::respond(state.get_client(), msg, r).await.is_ok()
                } else {
                    Self::handle_error(state.get_client(), msg, RippleError::ProcessorError).await
                }
            }
            _ => false,
        }
    }
}
