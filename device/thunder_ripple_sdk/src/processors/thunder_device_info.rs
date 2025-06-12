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
};

use crate::{
    client::{
        device_operator::{DeviceCallRequest, DeviceChannelParams, DeviceOperator},
        thunder_client::ThunderClient,
        thunder_plugin::ThunderPlugin,
    },
    ripple_sdk::{
        api::device::device_request::AudioProfile, extn::client::extn_client::ExtnClient,
        tokio::sync::mpsc,
    },
    thunder_state::ThunderState,
    utils::check_thunder_response_success,
};
use crate::{
    ripple_sdk::{
        api::{
            device::{
                device_info_request::{DeviceInfoRequest, DeviceResponse},
                device_request::{HDCPStatus, HdcpProfile, HdrProfile, Resolution},
            },
            firebolt::fb_openrpc::FireboltSemanticVersion,
        },
        async_trait::async_trait,
        chrono::NaiveDateTime,
        extn::{
            client::extn_processor::{
                DefaultExtnStreamer, ExtnRequestProcessor, ExtnStreamProcessor, ExtnStreamer,
            },
            extn_client_message::{ExtnMessage, ExtnPayload, ExtnPayloadProvider, ExtnResponse},
        },
        log::{error, info},
        serde_json::{self},
        utils::error::RippleError,
    },
    utils::get_audio_profile_from_value,
};
use regex::{Match, Regex};
use ripple_sdk::{
    api::{
        context::RippleContextUpdateRequest,
        device::device_request::TimeZone,
        device::{
            device_info_request::{FirmwareInfo, PlatformBuildInfo},
            device_request::PowerState,
        },
    },
    serde_json::Value,
};
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Debug, Serialize, Deserialize)]
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
    pub const HDRSTANDARD_HDR10PLUS: u32 = 0x10;
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "UPPERCASE")]
#[allow(dead_code)]
enum ThunderInterfaceType {
    Wifi,
    Ethernet,
    None,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct ThunderInterfaceStatus {
    interface: ThunderInterfaceType,
    mac_address: String,
    enabled: bool,
    connected: bool,
}

#[derive(Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct ThunderGetInterfacesResponse {
    interfaces: Vec<ThunderInterfaceStatus>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemVersion {
    pub stb_version: String,
    pub receiver_version: String,
    pub stb_timestamp: String,
    pub success: bool,
}

#[derive(Debug, Clone, Default)]
pub struct CachedDeviceInfo {
    model: Option<String>,
    hdcp_support: Option<HashMap<HdcpProfile, bool>>,
    version: Option<FireboltSemanticVersion>,
}

#[derive(Debug, Clone)]
pub struct CachedState {
    pub state: ThunderState,
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

    fn get_model(&self) -> Option<String> {
        self.cached.read().unwrap().model.clone()
    }

    fn update_model(&self, model: String) {
        let mut cached = self.cached.write().unwrap();
        let _ = cached.model.insert(model);
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
    pub async fn has_internet(state: &ThunderState) -> bool {
        let response = state
            .get_thunder_client()
            .call(DeviceCallRequest {
                method: ThunderPlugin::Network.method("isConnectedToInternet"),
                params: None,
            })
            .await;
        info!("{}", response.message);
        let response = response.message.get("connectedToInternet");
        if response.is_none() {
            return false;
        }
        let v = response.unwrap().as_bool().unwrap_or(false);
        let _ = state
            .get_client()
            .request_transient(RippleContextUpdateRequest::InternetStatus(v.into()));
        v
    }
}
#[derive(Debug, Serialize, Deserialize)]
pub struct ThunderTimezoneResponse {
    #[serde(rename = "timeZone")]
    pub time_zone: String,
}

#[derive(Debug)]
pub struct ThunderDeviceInfoRequestProcessor {
    state: CachedState,
    streamer: DefaultExtnStreamer,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ThunderAvailableTimezonesResponse {
    pub zoneinfo: HashMap<String, HashMap<String, String>>,
}

#[derive(Debug, Default, Clone)]
pub struct ThunderAllTimezonesResponse {
    pub timezones: HashMap<String, String>,
}

impl ThunderAllTimezonesResponse {
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

impl ThunderDeviceInfoRequestProcessor {
    pub fn new(state: ThunderState) -> ThunderDeviceInfoRequestProcessor {
        ThunderDeviceInfoRequestProcessor {
            state: CachedState::new(state),
            streamer: DefaultExtnStreamer::new(),
        }
    }

    pub async fn get_serial_number(state: &ThunderState) -> String {
        let resp = state
            .get_thunder_client()
            .call(DeviceCallRequest {
                method: ThunderPlugin::System.method("getSerialNumber"),
                params: None,
            })
            .await;
        info!("{}", resp.message);

        resp.message["serialNumber"]
            .as_str()
            .map_or_else(|| "".to_string(), |serial_number| serial_number.to_string())
    }

    pub async fn model_info(state: &ThunderState) -> String {
        let resp = state
            .get_thunder_client()
            .call(DeviceCallRequest {
                method: ThunderPlugin::System.method("getSystemVersions"),
                params: None,
            })
            .await;
        info!("{}", resp.message);
        let resp = resp.message.get("stbVersion");
        if resp.is_none() {
            return "NA".to_owned();
        }
        let resp = resp.unwrap().as_str().unwrap().trim_matches('"');
        let split_string: Vec<&str> = resp.split('_').collect();
        String::from(split_string[0])
    }

    async fn get_model(state: &CachedState) -> String {
        let model = if let Some(m) = state.get_model() {
            m
        } else {
            Self::model_info(&state.state).await
        };

        state.update_model(model.clone());
        model
    }

    async fn model(state: CachedState, req: ExtnMessage) -> bool {
        let response = Self::get_model(&state).await;
        Self::respond(state.get_client(), req, ExtnResponse::String(response))
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
        if !check_thunder_response_success(&response) {
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
        let mut hdcp_response = HashMap::new();
        let resp = response.message.get("supportedHDCPVersion");

        if resp.is_none() {
            return hdcp_response;
        }
        let v = resp.unwrap();

        let hdcp_version = v.to_string();

        let is_hdcp_supported: bool = if let Some(h) = response.message.get("isHDCPSupported") {
            h.to_string().parse::<bool>().unwrap_or(false)
        } else {
            false
        };
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
        let mut response: HDCPStatus = HDCPStatus::default();
        let resp = state
            .get_thunder_client()
            .call(DeviceCallRequest {
                method: ThunderPlugin::Hdcp.method("getHDCPStatus"),
                params: None,
            })
            .await;
        info!("{}", resp.message);
        if let Ok(thdcp) = serde_json::from_value::<ThunderHDCPStatus>(resp.message) {
            response = thdcp.hdcp_status;
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
        hm.insert(
            HdrProfile::Hdr10plus,
            0 != (supported_cap & hdr_flags::HDRSTANDARD_HDR10PLUS),
        );
        hm
    }

    pub async fn get_firmware_version(state: &ThunderState) -> FireboltSemanticVersion {
        let version: FireboltSemanticVersion;
        let resp = state
            .get_thunder_client()
            .call(DeviceCallRequest {
                method: ThunderPlugin::System.method("getSystemVersions"),
                params: None,
            })
            .await;
        info!("{}", resp.message);
        if let Ok(tsv) = serde_json::from_value::<SystemVersion>(resp.message) {
            let tsv_split = tsv.receiver_version.split('.');
            let tsv_vec: Vec<&str> = tsv_split.collect();

            if tsv_vec.len() >= 3 {
                let major: String = tsv_vec[0].chars().filter(|c| c.is_ascii_digit()).collect();
                let minor: String = tsv_vec[1].chars().filter(|c| c.is_ascii_digit()).collect();
                let patch: String = tsv_vec[2].chars().filter(|c| c.is_ascii_digit()).collect();

                version = FireboltSemanticVersion {
                    major: major.parse::<u32>().unwrap(),
                    minor: minor.parse::<u32>().unwrap(),
                    patch: patch.parse::<u32>().unwrap(),
                    readable: tsv.stb_version,
                };
            } else {
                version = FireboltSemanticVersion {
                    readable: tsv.stb_version,
                    ..FireboltSemanticVersion::default()
                };
            }
        } else {
            version = FireboltSemanticVersion::default()
        }
        version
    }

    async fn get_os_info(state: &CachedState) -> FirmwareInfo {
        let version: FireboltSemanticVersion;
        // TODO: refactor this to use return syntax and not use response variable across branches
        match state.get_version() {
            Some(v) => version = v,
            None => {
                version = Self::get_firmware_version(&state.state).await;
                state.update_version(version.clone());
            }
        }
        version.into()
    }

    async fn os_info(state: CachedState, req: ExtnMessage) -> bool {
        let response = Self::get_os_info(&state).await;
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
        if check_thunder_response_success(&response) {
            if let Some(v) = response.message["freeRam"].as_u64() {
                return Self::respond(state.get_client(), req, ExtnResponse::Value(json!(v)))
                    .await
                    .is_ok();
            }
        }
        error!("{}", response.message);
        Self::handle_error(state.get_client(), req, RippleError::ProcessorError).await
    }

    async fn get_timezone_value(state: &ThunderState) -> Result<String, RippleError> {
        let response = state
            .get_thunder_client()
            .call(DeviceCallRequest {
                method: ThunderPlugin::System.method("getTimeZoneDST"),
                params: None,
            })
            .await;

        info!("getTimeZoneDST: {}", response.message);
        if check_thunder_response_success(&response) {
            if let Ok(v) = serde_json::from_value::<ThunderTimezoneResponse>(response.message) {
                return Ok(v.time_zone);
            }
        }
        Err(RippleError::ProcessorError)
    }

    pub async fn get_timezone_and_offset(state: &ThunderState) -> Option<TimeZone> {
        // Get the current timezone
        let Ok(timezone) = ThunderDeviceInfoRequestProcessor::get_timezone_value(state).await
        else {
            return None;
        };

        // Get all timezones (including the specific one we need)
        let Ok(timezones) =
            ThunderDeviceInfoRequestProcessor::get_all_timezones(state, &timezone).await
        else {
            return None;
        };

        // Create the TimeZone object
        let timezone_obj = TimeZone {
            time_zone: timezone.clone(),
            offset: timezones.get_offset(&timezone),
        };

        // Update client with timezone info
        let _ = state
            .get_client()
            .request_transient(RippleContextUpdateRequest::TimeZone(timezone_obj.clone()));

        Some(timezone_obj)
    }

    // Updated function to handle both old and new response formats
    async fn get_all_timezones(
        state: &ThunderState,
        timezone: &str,
    ) -> Result<ThunderAllTimezonesResponse, RippleError> {
        // Prepare parameters for the Thunder call
        let params = Some(DeviceChannelParams::Json(
            json!({
                "timeZones": [
                    timezone,
                    "Etc/UTC",
                ]
            })
            .to_string(),
        ));

        let response = state
            .get_thunder_client()
            .call(DeviceCallRequest {
                method: ThunderPlugin::System.method("getTimeZones"),
                params,
            })
            .await;

        if !check_thunder_response_success(&response) {
            return Err(RippleError::ProcessorError);
        }
        Self::extract_timezone_data_from_response(&response.message, timezone)
    }

    // Helper function to handle both old and new formats of the response
    fn extract_timezone_data_from_response(
        response_value: &Value,
        timezone: &str,
    ) -> Result<ThunderAllTimezonesResponse, RippleError> {
        let mut timezones = HashMap::new();

        if let Some(zoneinfo) = response_value.get("zoneinfo").and_then(|z| z.as_object()) {
            for (key, value) in zoneinfo {
                match key.as_str() {
                    "UTC" | "Etc/UTC" => {
                        if let Some(time_str) = value.as_str() {
                            timezones.insert("Etc/UTC".to_string(), time_str.to_string());
                        }
                    }
                    "America" => {
                        if let Some(obj) = value.as_object() {
                            for (city, city_val) in obj {
                                let tz_key = format!("America/{}", city);
                                if tz_key == timezone {
                                    if let Some(time_str) = city_val.as_str() {
                                        timezones.insert(tz_key, time_str.to_string());
                                    }
                                }
                            }
                        }
                    }
                    _ => {
                        // Handle flat keys that match the requested timezone
                        let tz_key = format!("America/{}", key);
                        if tz_key == timezone {
                            if let Some(time_str) = value.as_str() {
                                timezones.insert(tz_key, time_str.to_string());
                            }
                        }
                    }
                }
            }
            Ok(ThunderAllTimezonesResponse { timezones })
        } else {
            Err(RippleError::ProcessorError)
        }
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
        if check_thunder_response_success(&response) {
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
        if check_thunder_response_success(&response) {
            return Self::ack(state.get_client(), request).await.is_ok();
        }
        Self::handle_error(state.get_client(), request, RippleError::ProcessorError).await
    }

    pub async fn get_power_state(state: &ThunderState) -> Result<PowerState, RippleError> {
        let dev_response = state
            .get_thunder_client()
            .call(DeviceCallRequest {
                method: ThunderPlugin::System.method("getPowerState"),
                params: None,
            })
            .await;

        if let Some(response) = dev_response.message.get("powerState").cloned() {
            if let Ok(v) = serde_json::from_value(response) {
                return Ok(v);
            }
        }
        Err(RippleError::ProcessorError)
    }

    async fn power_state(state: CachedState, req: ExtnMessage) -> bool {
        let mut response = ExtnResponse::Error(RippleError::ProcessorError);
        if let Ok(v) = Self::get_power_state(&state.state).await {
            if let ExtnPayload::Response(r) = DeviceResponse::PowerState(v).get_extn_payload() {
                response = r;
            }
        }

        Self::respond(state.get_client(), req, response)
            .await
            .is_ok()
    }

    async fn platform_build_info(state: CachedState, msg: ExtnMessage) -> bool {
        println!("*** _DEBUG: platform_build_info: entry");
        let resp = state
            .get_thunder_client()
            .call(DeviceCallRequest {
                method: ThunderPlugin::System.method("getSystemVersions"),
                params: None,
            })
            .await;
        println!("*** _DEBUG: platform_build_info: resp= {:?}", resp);
        if let Ok(tsv) = serde_json::from_value::<SystemVersion>(resp.message) {
            let release_regex = Regex::new(r"([^_]*)_(.*)_(VBN|PROD[^_]*)_(.*)").unwrap();
            let non_release_regex =
                Regex::new(r"([^_]*)_(VBN|PROD[^_]*)_(.*)_(\d{14}(sdy|sey))(.*)").unwrap();
            let fallback_regex = Regex::new(r"([^_]*)_(.*)").unwrap();
            fn match_or_empty(m_opt: Option<Match>) -> String {
                if let Some(m) = m_opt {
                    String::from(m.as_str())
                } else {
                    String::from("")
                }
            }

            let name = tsv.stb_version.clone();

            let info_opt = if let Some(caps) = release_regex.captures(&tsv.stb_version) {
                Some(PlatformBuildInfo {
                    name,
                    device_model: match_or_empty(caps.get(1)),
                    branch: None,
                    release_version: caps.get(2).map(|s| String::from(s.as_str())),
                    debug: match_or_empty(caps.get(3)) == "VBN",
                })
            } else if let Some(caps) = non_release_regex.captures(&tsv.stb_version) {
                Some(PlatformBuildInfo {
                    name,
                    device_model: match_or_empty(caps.get(1)),
                    branch: caps.get(3).map(|s| String::from(s.as_str())),
                    release_version: None,
                    debug: match_or_empty(caps.get(2)) == "VBN",
                })
            } else if let Some(caps) = fallback_regex.captures(&tsv.stb_version) {
                Some(PlatformBuildInfo {
                    name,
                    device_model: match_or_empty(caps.get(1)),
                    branch: None,
                    release_version: None,
                    debug: match_or_empty(caps.get(2)).contains("VBN"),
                })
            } else {
                error!("Could not parse build name {}", tsv.stb_version);
                None
            };

            if let Some(info) = info_opt {
                if let ExtnPayload::Response(r) =
                    DeviceResponse::PlatformBuildInfo(info).get_extn_payload()
                {
                    Self::respond(state.get_client(), msg, r).await.is_ok()
                } else {
                    Self::handle_error(state.get_client(), msg, RippleError::ProcessorError).await
                }
            } else {
                Self::handle_error(state.get_client(), msg, RippleError::ProcessorError).await
            }
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
        println!("*** _DEBUG: process_request: {:?}", extracted_message);
        match extracted_message {
            DeviceInfoRequest::Model => Self::model(state.clone(), msg).await,
            DeviceInfoRequest::Audio => Self::audio(state.clone(), msg).await,
            DeviceInfoRequest::HdcpSupport => Self::hdcp_support(state.clone(), msg).await,
            DeviceInfoRequest::HdcpStatus => Self::hdcp_status(state.clone(), msg).await,
            DeviceInfoRequest::FirmwareInfo => Self::os_info(state.clone(), msg).await,
            DeviceInfoRequest::AvailableMemory => Self::available_memory(state.clone(), msg).await,
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
                ThunderNetworkService::has_internet(&state.state).await
            }
            DeviceInfoRequest::PowerState => Self::power_state(state.clone(), msg).await,
            DeviceInfoRequest::PlatformBuildInfo => {
                Self::platform_build_info(state.clone(), msg).await
            }
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

#[cfg(test)]
pub mod tests {
    use ripple_sdk::{
        api::device::{
            device_info_request::{DeviceInfoRequest, PlatformBuildInfo},
            device_request::DeviceRequest,
        },
        extn::{
            client::extn_processor::ExtnRequestProcessor, extn_client_message::ExtnRequest,
            mock_extension_client::MockExtnClient,
        },
        framework::ripple_contract::RippleContract,
        serde_json::json,
        tokio,
        utils::channel_utils::oneshot_send_and_log,
    };
    use serde::{Deserialize, Serialize};
    use std::sync::Arc;
    use tokio::sync::oneshot;

    use crate::client::thunder_async_client::ThunderAsyncResponse;
    use crate::{
        client::{
            device_operator::{DeviceCallRequest, DeviceResponseMessage},
            //thunder_client::ThunderCallMessage,
            thunder_plugin::ThunderPlugin,
        },
        processors::thunder_device_info::ThunderDeviceInfoRequestProcessor,
        tests::mock_thunder_controller::{CustomHandler, MockThunderController, ThunderHandlerFn},
    };
    use ripple_sdk::api::gateway::rpc_gateway_api::JsonRpcApiResponse;

    // <pca>
    // macro_rules! run_platform_info_test {
    //     ($build_name:expr) => {
    //         test_platform_build_info_with_build_name($build_name, Arc::new(|_msg: DeviceCallRequest, device_response_message_tx: Sender<DeviceResponseMessage>| {

    //             oneshot_send_and_log(
    //                 device_response_message_tx,
    //                 DeviceResponseMessage::call(json!({"success" : true, "stbVersion": $build_name, "receiverVersion": $build_name, "stbTimestamp": "".to_owned() })),
    //                 "",
    //             );
    //         })).await;
    //     };
    // }
    // macro_rules! run_platform_info_test {
    //     ($build_name:expr) => {
    //         test_platform_build_info_with_build_name($build_name, Arc::new(|_msg: DeviceCallRequest, device_response_message_tx: oneshot::Sender<DeviceResponseMessage>| {

    //             println!("*** _DEBUG: run_platform_info_test: Custom handler: build_name={}", $build_name);
    //             oneshot_send_and_log(
    //                 device_response_message_tx,
    //                 DeviceResponseMessage::call(json!({"success" : true, "stbVersion": $build_name, "receiverVersion": $build_name, "stbTimestamp": "".to_owned() })),
    //                 "",
    //             );
    //         })).await;
    //     };
    // }

    macro_rules! run_platform_info_test {
        ($build_name:expr) => {
            test_platform_build_info_with_build_name($build_name, Arc::new(|_msg: DeviceCallRequest, async_resp_message_tx: oneshot::Sender<ThunderAsyncResponse>| {

                println!("*** _DEBUG: run_platform_info_test: Custom handler: build_name={}", $build_name);
            let thunderasyncresp = ThunderAsyncResponse {
                id: None,
                result: Ok(JsonRpcApiResponse {
                    jsonrpc: "2.0".to_owned(),
                    id: None,
                    result: Some(json!({"success" : true, "stbVersion": $build_name, "receiverVersion": $build_name, "stbTimestamp": "".to_owned() })),
                    error: None,
                    method: None,
                    params: None
                }),
            };

            oneshot_send_and_log(
                    async_resp_message_tx,
                    thunderasyncresp,
                    "",
                );
            })).await;
        };
    }
    // </pca>

    #[derive(Debug, Serialize, Deserialize)]
    struct BuildInfoTest {
        build_name: String,
        info: PlatformBuildInfo,
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_platform_build_info() {
        run_platform_info_test!("SCXI11BEI_023.005.03.6.8p12s3_VBN_sdy");
        run_platform_info_test!("SCXI11BEI_23_VBN_sdy");
        run_platform_info_test!("SCXI11BEI_VBN_23_20231130001020sdy");
        run_platform_info_test!("SCXI11BEI_024.004.00.6.9p8s1_PRODLOG_sdy");
        run_platform_info_test!("SCXI11BEI_024.004.00.6.9p8s1_PROD_sdy");
        run_platform_info_test!("SCXI11BEI_024.004.00.6.9p8s1_VBN_sdy");
        run_platform_info_test!("SCXI11BEI_024.004.00.6.9p8s1_VBN_sey");
        run_platform_info_test!("SCXI11BEI_023.003.00.6.8p7s1_PRODLOG_sdy_XOE");
        run_platform_info_test!("SCXI11BEI_VBN_stable2_20231129231433sdy_XOE_NG");
        run_platform_info_test!("SCXI11AIC_PROD_6.6_p1v_20231130001020sdy_NG");
        run_platform_info_test!("SCXI11AIC_VBN_23Q4_sprint_20231129232625sdy_FG_NG");
        run_platform_info_test!("SCXI11AIC_VBN_23Q4_sprint_20231129232625sey_FG_NG");
        run_platform_info_test!("SCXI11BEI_PROD_some_branch_20231129233157sdy_FG_NG-signed");
        run_platform_info_test!("SCXI11BEI_PROD_QS024_20231129231350sdy_XOE_NG");
        run_platform_info_test!("COESST11AEI_VBN_23Q4_sprint_20231130233011sdy_DFL_FG_GRT");
        run_platform_info_test!("COESST11AEI_23.40p11d24_EXP_PROD_sdy-signed");
        run_platform_info_test!(
            "SCXI11BEI_VBN_23Q4_sprint_20231113173051sdy_FG_EDGE_DISTPDEMO-signed"
        );
        run_platform_info_test!("SCXI11BEI_somebuild");
        run_platform_info_test!("SCXI11BEI_someVBNbuild");
    }

    // <pca>
    // async fn test_platform_build_info_with_build_name(
    //     _build_name: &'static str,
    //     handler: Arc<ThunderHandlerFn>,
    // ) {
    //     let mut ch = CustomHandler::default();
    //     ch.custom_request_handler.insert(
    //         ThunderPlugin::System.unversioned_method("getSystemVersions"),
    //         handler,
    //     );

    //     let state = MockThunderController::state_with_mock(Some(ch));

    //     let msg = MockExtnClient::req(
    //         RippleContract::DeviceInfo,
    //         ExtnRequest::Device(DeviceRequest::DeviceInfo(
    //             DeviceInfoRequest::PlatformBuildInfo,
    //         )),
    //     );

    //     ThunderDeviceInfoRequestProcessor::process_request(
    //         state,
    //         msg,
    //         DeviceInfoRequest::PlatformBuildInfo,
    //     )
    //     .await;

    //     println!("*** _DEBUG: test_platform_build_info_with_build_name: Mark 1");

    //     // let msg: ExtnMessage = r.recv().await.unwrap().try_into().unwrap();
    //     // let resp_opt = msg.payload.extract::<DeviceResponse>();
    //     // if let Some(DeviceResponse::PlatformBuildInfo(info)) = resp_opt {
    //     //     let exp = tests.iter().find(|x| x.build_name == build_name).unwrap();
    //     //     assert_eq!(info, exp.info);
    //     // } else {
    //     //     panic!("Did not get the expected PlatformBuildInfo from extension call");
    //     // }
    // }
    async fn test_platform_build_info_with_build_name(
        _build_name: &'static str,
        handler: Arc<ThunderHandlerFn>,
    ) {
        println!(
            "*** _DEBUG: test_platform_build_info_with_build_name: {}",
            _build_name
        );

        let mut ch = CustomHandler::default();
        ch.custom_request_handler.insert(
            ThunderPlugin::System.unversioned_method("getSystemVersions"),
            handler,
        );

        let (state, mut device_response_message_rx) =
            MockThunderController::state_with_mock(Some(ch));

        let msg = MockExtnClient::req(
            RippleContract::DeviceInfo,
            ExtnRequest::Device(DeviceRequest::DeviceInfo(
                DeviceInfoRequest::PlatformBuildInfo,
            )),
        );

        println!(
            "*** _DEBUG: test_platform_build_info_with_build_name: Calling ThunderDeviceInfoRequestProcessor::process_request"
        );

        tokio::spawn(async move {
            while let Some(r) = device_response_message_rx.recv().await {
                println!(
                "*** _DEBUG: test_platform_build_info_with_build_name: Received DeviceResponseMessage: {:?}",
                r
            );
            }
        });

        ThunderDeviceInfoRequestProcessor::process_request(
            state,
            msg,
            DeviceInfoRequest::PlatformBuildInfo,
        )
        .await;

        println!("*** _DEBUG: test_platform_build_info_with_build_name: Mark 1");

        // let msg: ExtnMessage = r.recv().await.unwrap().try_into().unwrap();
        // let resp_opt = msg.payload.extract::<DeviceResponse>();
        // if let Some(DeviceResponse::PlatformBuildInfo(info)) = resp_opt {
        //     let exp = tests.iter().find(|x| x.build_name == build_name).unwrap();
        //     assert_eq!(info, exp.info);
        // } else {
        //     panic!("Did not get the expected PlatformBuildInfo from extension call");
        // }
    }
    // </pca>

    macro_rules! check_offset {
        ($mock_response:expr, $timezone:expr, $expected_offset:expr, $expected_rounded_offset:expr) => {{
            let resp = ThunderDeviceInfoRequestProcessor::extract_timezone_data_from_response(
                &$mock_response,
                $timezone,
            );
            match resp {
                Ok(timezones) => {
                    let offset = timezones.get_offset($timezone);
                    assert_eq!(
                        offset, $expected_offset,
                        "Expected offset for {} is {}, got {}",
                        $timezone, $expected_offset, offset
                    );
                    let rounded_offset = offset / 3600;
                    assert_eq!(
                        rounded_offset, $expected_rounded_offset,
                        "Expected rounded offset for {} is {}, got {}",
                        $timezone, $expected_rounded_offset, rounded_offset
                    );
                }
                Err(e) => {
                    panic!("Error processing mock format: {}", e.to_string());
                }
            }
        }};
    }

    #[tokio::test]
    async fn test_extract_timezone_data_from_response_and_offset() {
        let timezone = "America/New_York";
        let expected_offset = -14400; // -4 hours in seconds
        let expected_rounded_offset = -4; // -4 hours

        // Mock Thunder response in the flat format
        let mock_response = json!({
            "zoneinfo": {
                "New_York": "Tue May 20 16:34:30 2025 EDT",
                "UTC": "Tue May 20 20:34:30 2025 UTC",
            }
        });
        check_offset!(
            mock_response,
            timezone,
            expected_offset,
            expected_rounded_offset
        );

        // Mock Thunder response in the nested format
        let mock_nested_response = json!({
            "zoneinfo": {
                "America": {"New_York": "Tue May 20 16:34:30 2025 EDT"},
                "UTC": "Tue May 20 20:34:30 2025 UTC",
                "Etc/UTC": "Tue May 20 20:34:30 2025 UTC",
            }
        });
        check_offset!(
            mock_nested_response,
            timezone,
            expected_offset,
            expected_rounded_offset
        );
    }
}
