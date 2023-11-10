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
    time::Duration,
};

use crate::{
    client::{thunder_client::ThunderClient, thunder_plugin::ThunderPlugin},
    ripple_sdk::{
        api::device::{device_info_request::DeviceCapabilities, device_request::AudioProfile},
        chrono::NaiveDateTime,
        extn::client::extn_client::ExtnClient,
        tokio::sync::mpsc,
    },
    thunder_state::ThunderState,
};
use crate::{
    ripple_sdk::{
        api::{
            device::{
                device_info_request::{DeviceInfoRequest, DeviceResponse},
                device_operator::{DeviceCallRequest, DeviceChannelParams, DeviceOperator},
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
use ripple_sdk::{
    api::{
        context::RippleContextUpdateRequest,
        device::{
            device_info_request::{
                DEVICE_INFO_AUTHORIZED, DEVICE_MAKE_MODEL_AUTHORIZED,
                DEVICE_NETWORK_STATUS_AUTHORIZED, DEVICE_SKU_AUTHORIZED,
            },
            device_request::{InternetConnectionStatus, PowerState, TimeZone},
        },
    },
    log::trace,
    serde_json::{Map, Value},
    tokio::join,
};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::json;

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
    fn to_network_type(&self) -> NetworkType {
        match self {
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
    serial_number: Option<String>,
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
    pub fn new(state: ThunderState) -> Self {
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

    fn get_serial_number(&self) -> Option<String> {
        self.cached.read().unwrap().serial_number.clone()
    }

    fn update_serial_number(&self, serial_number: String) {
        let mut cached = self.cached.write().unwrap();
        let _ = cached.serial_number.insert(serial_number);
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
                method: ThunderPlugin::Network.method("isConnectedToInternet"),
                params: None,
            })
            .await;
        info!("{}", response.message);
        if let Some(ip) = response.message["connectedToInternet"].as_bool() {
            return ip;
        };
        false
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

#[derive(Debug, Default, Clone)]
pub struct ThunderAllTimezonesResponse {
    pub timezones: HashMap<String, String>,
}

impl ThunderAllTimezonesResponse {
    fn recurse_timezones(
        timezones: &mut HashMap<String, String>,
        prefix: String,
        source: &Map<String, Value>,
        filter: Vec<&str>,
    ) {
        for (key, value) in source {
            if filter.is_empty() || filter.contains(&key.as_str()) {
                let new_prefix = if !prefix.is_empty() {
                    format!("{}/{}", prefix, key)
                } else {
                    key.clone()
                };
                match value {
                    Value::String(s) => {
                        timezones.insert(new_prefix.clone(), s.clone());
                    }
                    Value::Object(map) => {
                        Self::recurse_timezones(timezones, new_prefix, map, Vec::new())
                    }
                    _ => {}
                }
            }
        }
    }

    fn as_array(&self) -> Vec<String> {
        self.timezones.keys().cloned().collect()
    }

    fn get_offset(&self, key: &str) -> i64 {
        if let Some(tz) = self.timezones.get(key) {
            if let Some(utc_tz) = self.timezones.get("Etc/UTC").cloned() {
                if let Ok(ntz) = NaiveDateTime::parse_from_str(tz, "%a %b %d %H:%M:%S %Y %Z") {
                    if let Ok(nutz) =
                        NaiveDateTime::parse_from_str(&utc_tz, "%a %b %d %H:%M:%S %Y %Z")
                    {
                        let delta = (ntz - nutz).num_seconds();
                        return round_to_nearest_quarter_hour(delta);
                    }
                }
            }
        }
        0
    }
}
impl<'de> Deserialize<'de> for ThunderAllTimezonesResponse {
    fn deserialize<D>(deserializer: D) -> Result<ThunderAllTimezonesResponse, D::Error>
    where
        D: Deserializer<'de>,
    {
        let tz_value: Map<String, Value> = Map::deserialize(deserializer)?;
        let mut timezones = HashMap::new();
        if let Some(Value::Object(zones)) = tz_value.get("zoneinfo") {
            Self::recurse_timezones(
                &mut timezones,
                String::from(""),
                zones,
                vec![
                    "Etc",
                    "Utc",
                    "America",
                    "Australia",
                    "Africa",
                    "Europe",
                    "Pacific",
                ],
            );
        }
        Ok(ThunderAllTimezonesResponse { timezones })
    }
}

impl ThunderAvailableTimezonesResponse {
    pub fn as_array(&self) -> Vec<String> {
        let mut timezones = Vec::default();
        for (area, locations) in &self.zoneinfo {
            let mut found_location = false;
            for location in locations.keys() {
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
                if let Some(value) = mac_value_option {
                    response = value.to_string();
                    state.update_mac_address(response.clone())
                } else {
                    response = "".to_string();
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

    async fn get_serial_number(state: &CachedState) -> String {
        let response: String;
        match state.get_serial_number() {
            Some(value) => response = value,
            None => {
                let resp = state
                    .get_thunder_client()
                    .call(DeviceCallRequest {
                        method: ThunderPlugin::System.method("getSerialNumber"),
                        params: None,
                    })
                    .await;
                info!("{}", resp.message);

                let serial_number_option = resp.message["serialNumber"].as_str();
                if serial_number_option.is_none() {
                    response = "".to_string();
                } else {
                    response = serial_number_option.unwrap().to_string();
                    state.update_serial_number(response.clone())
                }
            }
        }
        response
    }

    async fn serial_number(state: CachedState, req: ExtnMessage) -> bool {
        let response: String = Self::get_serial_number(&state).await;

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
                let split_string: Vec<&str> = full_firmware_version.split('_').collect();
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

    async fn get_internet_connection_status(
        state: &CachedState,
    ) -> Option<InternetConnectionStatus> {
        let dev_response = state
            .get_thunder_client()
            .call(DeviceCallRequest {
                method: ThunderPlugin::Network.method("getInternetConnectionState"),
                params: None,
            })
            .await;
        let value = dev_response.message.get("state").cloned();
        value.as_ref()?;
        if let Ok(internet_status) = serde_json::from_value::<u32>(value.unwrap()) {
            match internet_status {
                0 => Some(InternetConnectionStatus::NoInternet),
                1 => Some(InternetConnectionStatus::LimitedInternet),
                2 => Some(InternetConnectionStatus::CaptivePortal),
                3 => Some(InternetConnectionStatus::FullyConnected),
                _ => None,
            }
        } else {
            None
        }
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

    async fn internet_connection_status(state: CachedState, req: ExtnMessage) -> bool {
        if let Some(response) = Self::get_internet_connection_status(&state).await {
            trace!(
                "Successfully got internetConnection status from thunder: {:?}",
                response
            );
            Self::respond(
                state.get_client(),
                req,
                if let ExtnPayload::Response(resp) =
                    DeviceResponse::InternetConnectionStatus(response).get_extn_payload()
                {
                    resp
                } else {
                    ExtnResponse::Error(RippleError::ProcessorError)
                },
            )
            .await
            .is_ok()
        } else {
            error!("Unable to get internet connection status from thunder");
            Self::handle_error(state.get_client(), req, RippleError::ProcessorError).await
        }
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
            || !response.message["success"].as_bool().unwrap_or_default()
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
            || !response.message["success"].as_bool().unwrap_or_default()
        {
            error!("{}", response.message);
            return Vec::new();
        }
        info!("{}", response.message);
        let resol = response.message["resolution"].as_str().unwrap();
        get_dimension_from_resolution(resol)
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
            || !response.message["success"].as_bool().unwrap_or_default()
        {
            error!("{}", response.message);
            return Vec::new();
        }
        info!("{}", response.message);
        let resol = response.message["defaultResolution"].as_str().unwrap();
        get_dimension_from_resolution(resol)
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
            _type: interface_response.to_network_type(),
            state: match interface_response {
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
        if tokio::time::timeout(
            Duration::from_millis(timeout),
            Self::respond(state.get_client(), req.clone(), {
                let value = ThunderNetworkService::has_internet(&state).await;

                if let ExtnPayload::Response(r) =
                    DeviceResponse::InternetConnectionStatus(match value {
                        true => InternetConnectionStatus::FullyConnected,
                        false => InternetConnectionStatus::NoInternet,
                    })
                    .get_extn_payload()
                {
                    r
                } else {
                    ExtnResponse::Error(RippleError::ProcessorError)
                }
            }),
        )
        .await
        .is_err()
        {
            Self::handle_error(state.get_client(), req, RippleError::ProcessorError).await
        } else {
            true
        }
    }

    async fn get_version(state: &CachedState) -> FireboltSemanticVersion {
        let response: FireboltSemanticVersion;
        // TODO: refactor this to use return syntax and not use response variable across branches
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
                let tsv_split = tsv.receiver_version.split('.');
                let tsv_vec: Vec<&str> = tsv_split.collect();

                if tsv_vec.len() >= 3 {
                    let major: String = tsv_vec[0].chars().filter(|c| c.is_ascii_digit()).collect();
                    let minor: String = tsv_vec[1].chars().filter(|c| c.is_ascii_digit()).collect();
                    let patch: String = tsv_vec[2].chars().filter(|c| c.is_ascii_digit()).collect();

                    response = FireboltSemanticVersion {
                        major: major.parse::<u32>().unwrap(),
                        minor: minor.parse::<u32>().unwrap(),
                        patch: patch.parse::<u32>().unwrap(),
                        readable: tsv.stb_version,
                    };
                    state.update_version(response.clone());
                } else {
                    response = FireboltSemanticVersion {
                        readable: tsv.stb_version,
                        ..FireboltSemanticVersion::default()
                    };
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
            && response.message["success"].as_bool().unwrap_or_default()
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

        info!("getTimeZoneDST: {}", response.message);
        if response.message.get("success").is_none()
            || response.message["success"].as_bool().unwrap_or_default()
        {
            if let Ok(v) = serde_json::from_value::<ThunderTimezoneResponse>(response.message) {
                return Ok(v.time_zone);
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

    pub async fn get_timezone_with_offset(state: CachedState, req: ExtnMessage) -> bool {
        if let Some(TimeZone { time_zone, offset }) = state.get_client().get_timezone() {
            if !time_zone.is_empty() {
                return Self::respond(
                    state.get_client(),
                    req,
                    ExtnResponse::TimezoneWithOffset(time_zone, offset),
                )
                .await
                .is_ok();
            } else if let Some(tz) = Self::get_timezone_and_offset(&state).await {
                let cloned_state = state.clone();
                let cloned_tz = tz.clone();
                cloned_state
                    .get_client()
                    .context_update(RippleContextUpdateRequest::TimeZone(TimeZone {
                        time_zone: cloned_tz.time_zone,
                        offset: cloned_tz.offset,
                    }));
                return Self::respond(
                    state.get_client(),
                    req,
                    ExtnResponse::TimezoneWithOffset(tz.time_zone, tz.offset),
                )
                .await
                .is_ok();
            }
        }
        error!("get_timezone_offset: Unsupported timezone");
        Self::handle_error(state.get_client(), req, RippleError::ProcessorError).await
    }

    pub async fn get_timezone_and_offset(state: &CachedState) -> Option<TimeZone> {
        let timezone_result = ThunderDeviceInfoRequestProcessor::get_timezone_value(state).await;
        let timezones_result = ThunderDeviceInfoRequestProcessor::get_all_timezones(state).await;

        if let (Ok(timezone), Ok(timezones)) = (timezone_result, timezones_result) {
            Some(TimeZone {
                time_zone: timezone.clone(),
                offset: timezones.get_offset(&timezone),
            })
        } else {
            None
        }
    }

    async fn get_all_timezones(
        state: &CachedState,
    ) -> Result<ThunderAllTimezonesResponse, RippleError> {
        let response = state
            .get_thunder_client()
            .call(DeviceCallRequest {
                method: ThunderPlugin::System.method("getTimeZones"),
                params: None,
            })
            .await;
        if response.message.get("success").is_some()
            && response.message["success"].as_bool().unwrap_or_default()
        {
            match serde_json::from_value::<ThunderAllTimezonesResponse>(response.message) {
                Ok(timezones) => Ok(timezones),
                Err(e) => {
                    error!("{}", e.to_string());
                    Err(RippleError::ProcessorError)
                }
            }
        } else {
            Err(RippleError::ProcessorError)
        }
    }

    async fn get_available_timezones(state: CachedState, req: ExtnMessage) -> bool {
        if let Ok(v) = Self::get_all_timezones(&state).await {
            Self::respond(
                state.get_client(),
                req,
                ExtnResponse::AvailableTimezones(v.as_array()),
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
                params,
            })
            .await;
        info!("{}", response.message);

        if response.message.get("success").is_none()
            || response.message["success"].as_bool().unwrap_or_default()
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
        if response.message.get("success").is_some()
            && response.message["success"].as_bool().is_some()
        {
            return Self::ack(state.get_client(), request).await.is_ok();
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

    pub async fn get_voice_guidance_speed(state: ThunderState) -> Result<f32, ()> {
        let response = state
            .get_thunder_client()
            .call(DeviceCallRequest {
                method: ThunderPlugin::TextToSpeech.method("getttsconfiguration"),
                params: None,
            })
            .await;

        if let Some(rate) = response.message["rate"].as_f64() {
            return Ok(scale_voice_speed_from_thunder_to_firebolt(rate as f32));
        }

        Err(())
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
        if response.message.get("success").is_some()
            && response.message["success"].as_bool().is_some()
        {
            return Self::ack(state.get_client(), request).await.is_ok();
        }
        Self::handle_error(state.get_client(), request, RippleError::ProcessorError).await
    }

    async fn power_state(state: CachedState, req: ExtnMessage) -> bool {
        let dev_response = state
            .get_thunder_client()
            .call(DeviceCallRequest {
                method: ThunderPlugin::System.method("getPowerState"),
                params: None,
            })
            .await;
        if let Some(value) = dev_response.message.get("powerState").cloned() {
            if let Ok(power_state) = serde_json::from_value::<PowerState>(value) {
                return Self::respond(
                    state.get_client(),
                    req,
                    if let ExtnPayload::Response(r) =
                        DeviceResponse::PowerState(power_state).get_extn_payload()
                    {
                        r
                    } else {
                        ExtnResponse::Error(RippleError::ProcessorError)
                    },
                )
                .await
                .is_ok();
            }
        }

        error!("Unable to get power state from thunder");
        Self::handle_error(state.get_client(), req, RippleError::ProcessorError).await
    }

    async fn get_device_capabilities(state: CachedState, keys: &[&str], msg: ExtnMessage) -> bool {
        let device_info_authorized = keys.contains(&DEVICE_INFO_AUTHORIZED);
        let (
            video_dimensions,
            native_dimensions,
            firmware_info_result,
            hdr_info,
            hdcp_result,
            audio_result,
            model_result,
            make_result,
            network_status,
        ) = join!(
            async {
                if device_info_authorized {
                    Some(Self::get_video_resolution(&state).await)
                } else {
                    None
                }
            },
            async {
                if device_info_authorized {
                    Some(Self::get_screen_resolution(&state).await)
                } else {
                    None
                }
            },
            async {
                if device_info_authorized {
                    Some(Self::get_version(&state).await)
                } else {
                    None
                }
            },
            async {
                if device_info_authorized {
                    Some(Self::get_cached_hdr(&state).await)
                } else {
                    None
                }
            },
            async {
                if device_info_authorized {
                    Some(Self::get_hdcp_status(&state).await)
                } else {
                    None
                }
            },
            async {
                if device_info_authorized {
                    Some(Self::get_audio(&state).await)
                } else {
                    None
                }
            },
            async {
                if keys.contains(&DEVICE_SKU_AUTHORIZED) {
                    Some(Self::get_model(&state).await)
                } else {
                    None
                }
            },
            async {
                if keys.contains(&DEVICE_MAKE_MODEL_AUTHORIZED) {
                    Some(Self::get_make(&state).await)
                } else {
                    None
                }
            },
            async {
                if keys.contains(&DEVICE_NETWORK_STATUS_AUTHORIZED) {
                    Some(matches!(
                        Self::get_network(&state).await._type,
                        NetworkType::Wifi
                    ))
                } else {
                    None
                }
            }
        );

        let device_capabilities = DeviceCapabilities {
            audio: audio_result,
            firmware_info: firmware_info_result,
            hdcp: hdcp_result,
            hdr: hdr_info,
            is_wifi: network_status,
            make: make_result,
            model: model_result,
            video_resolution: video_dimensions,
            screen_resolution: native_dimensions,
        };
        if let ExtnPayload::Response(r) =
            DeviceResponse::FullCapabilities(Box::new(device_capabilities)).get_extn_payload()
        {
            Self::respond(state.get_client(), msg, r).await.is_ok()
        } else {
            Self::handle_error(state.get_client(), msg, RippleError::ProcessorError).await
        }
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
Firebolt spec range for this value is 0.25 >= value <= 2.0
but...
Thunder is 1..100
for

*/
fn scale_voice_speed_from_thunder_to_firebolt(thunder_voice_speed: f32) -> f32 {
    if thunder_voice_speed >= 25.0 {
        thunder_voice_speed / 50.0
    } else {
        25.0 / 50.0
    }
}
/*
Note that this returns an f32 (when it should seemingly return i32), which is to make it compatible with
DeviceRequest::VoiceGuidanceSetSpeed(speed).
*/
fn scale_voice_speed_from_firebolt_to_thunder(firebolt_voice_speed: f32) -> f32 {
    if (0.50..=2.0).contains(&firebolt_voice_speed) {
        firebolt_voice_speed * 50.0
    } else {
        0.50 * 50.0
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
            DeviceInfoRequest::SerialNumber => Self::serial_number(state.clone(), msg).await,
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
            DeviceInfoRequest::InternetConnectionStatus => {
                Self::internet_connection_status(state.clone(), msg).await
            }
            DeviceInfoRequest::FullCapabilities(keys) => {
                let keys_as_str: Vec<&str> = keys.iter().map(String::as_str).collect();
                Self::get_device_capabilities(state.clone(), &keys_as_str, msg).await
            }
            DeviceInfoRequest::PowerState => Self::power_state(state.clone(), msg).await,
            _ => false,
        }
    }
}

fn round_to_nearest_quarter_hour(offset_seconds: i64) -> i64 {
    // Convert minutes to quarter hours
    let quarter_hours = (offset_seconds as f64 / 900.0).round() as i64;

    // Convert back to minutes
    let rounded_minutes = quarter_hours * 15;

    // Convert minutes back to seconds
    rounded_minutes * 60
}
