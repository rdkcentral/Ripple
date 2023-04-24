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

use serde_json::json;
use thunder_ripple_sdk::ripple_sdk::{
    self,
    api::device::{
        device_info_request::{DeviceInfoRequest, DeviceResponse},
        device_operator::{
            DeviceCallRequest, DeviceChannelParams, DeviceChannelRequest, DeviceOperator,
        },
        device_request::{AudioProfile, HdrProfile, Resolution},
    },
    async_trait::async_trait,
    extn::{
        client::extn_processor::{
            DefaultExtnStreamer, ExtnRequestProcessor, ExtnStreamProcessor, ExtnStreamer,
        },
        extn_client_message::{ExtnMessage, ExtnPayload, ExtnPayloadProvider, ExtnResponse},
    },
    log::{error, info},
    serde_json,
    utils::error::RippleError,
};
use thunder_ripple_sdk::{
    client::thunder_plugin::ThunderPlugin,
    ripple_sdk::{extn::client::extn_client::ExtnClient, tokio::sync::mpsc},
    thunder_state::ThunderState,
};

pub mod hdr_flags {
    pub const HDRSTANDARD_NONE: u32 = 0x00;
    pub const HDRSTANDARD_HDR10: u32 = 0x01;
    pub const HDRSTANDARD_HLG: u32 = 0x02;
    pub const HDRSTANDARD_DOLBY_VISION: u32 = 0x04;
    pub const HDRSTANDARD_TECHNICOLOR_PRIME: u32 = 0x08;
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

    // async fn hdcp_support(state: ThunderState, req: ExtnMessage) {
    //     let thunder_method = ThunderPlugin::Hdcp.method("getSettopHDCPSupport");
    //     let client = self.client.clone();
    //     let span = info_span!(parent: &parent_span, "calling thunder", %thunder_method);
    //     let response = client.call_thunder(&thunder_method, None, Some(span)).await;
    //     info!("{}", response.message);
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
    //     request.respond_and_log(Ok(DabResponsePayload::HdcpSupportResponse(
    //         hdcp_response.clone(),
    //     )));
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

    // async fn name(state: ThunderState, req: ExtnMessage) -> bool {
    //     let response = state
    //         .get_thunder_client()
    //         .call(DeviceCallRequest {
    //             method: ThunderPlugin::DeviceInfo.method("systeminfo"),
    //             params: None,
    //         })
    //         .await;
    //     info!("{}", response.message);
    //     let response = match response.message["devicename"].as_str() {
    //         Some(v) => {
    //             let split_string: Vec<&str> = v.split("_").collect();
    //             ExtnResponse::String(String::from(split_string[0]))
    //         }
    //         None => ExtnResponse::Error(RippleError::InvalidOutput),
    //     };
    //     if let Err(_e) = Self::respond(state.get_client(), req, response).await {
    //         return Err(rpc_err("parse error").into());
    //     }
    // }
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
            DeviceInfoRequest::Make => Self::make(state.clone(), msg).await,
            DeviceInfoRequest::Model => Self::model(state.clone(), msg).await,
            DeviceInfoRequest::AvailableMemory => Self::available_memory(state.clone(), msg).await,
            DeviceInfoRequest::MacAddress => Self::mac_address(state.clone(), msg).await,
            //            DeviceInfoRequest::Name => Self::name(state.clone(), msg).await,
            _ => false,
        }
    }
}
