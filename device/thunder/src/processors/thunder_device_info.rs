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

use serde::{Deserialize, Serialize};
use serde_json::json;
use thunder_ripple_sdk::ripple_sdk::{
    api::device::{
        device_info_request::DeviceInfoRequest,
        device_operator::{DeviceCallRequest, DeviceChannelParams, DeviceOperator},
    },
    async_trait::async_trait,
    extn::{
        client::extn_processor::{
            DefaultExtnStreamer, ExtnRequestProcessor, ExtnStreamProcessor, ExtnStreamer,
        },
        extn_client_message::{ExtnMessage, ExtnResponse},
    },
    log::{error, info},
    serde_json::{self},
    utils::error::RippleError,
};
use thunder_ripple_sdk::{
    client::thunder_plugin::ThunderPlugin,
    ripple_sdk::{extn::client::extn_client::ExtnClient, tokio::sync::mpsc},
    thunder_state::ThunderState,
};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ThunderTimezoneResponse {
    #[serde(rename = "timeZone")]
    pub time_zone: String,
}

#[derive(Debug)]
pub struct ThunderDeviceInfoRequestProcessor {
    state: ThunderState,
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
            state,
            streamer: DefaultExtnStreamer::new(),
        }
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
        let response = match response.message["make"].as_str() {
            Some(v) => ExtnResponse::String(v.to_string()),
            None => ExtnResponse::Error(RippleError::InvalidOutput),
        };

        Self::respond(state.get_client(), req, response)
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
        let response = match response.message["stbVersion"].as_str() {
            Some(v) => {
                let split_string: Vec<&str> = v.split("_").collect();
                ExtnResponse::String(String::from(split_string[0]))
            }
            None => ExtnResponse::Error(RippleError::InvalidOutput),
        };
        Self::respond(state.get_client(), req, response)
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

    async fn set_timezone(state: ThunderState, timezone: String, request: ExtnMessage) -> bool {
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

    async fn get_timezone(state: ThunderState, req: ExtnMessage) -> bool {
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
                return Self::respond(
                    state.get_client(),
                    req,
                    ExtnResponse::String(v.time_zone.to_owned()),
                )
                .await
                .is_ok();
            }
        }
        Self::handle_error(state.get_client(), req, RippleError::ProcessorError).await
    }

    async fn get_available_timezones(state: ThunderState, req: ExtnMessage) -> bool {
        let response = state
            .get_thunder_client()
            .call(DeviceCallRequest {
                method: ThunderPlugin::System.method("getTimeZones"),
                params: None,
            })
            .await;
        info!("{}", response.message);
        if response.message.get("success").is_none()
            || response.message["success"].as_bool().unwrap() == false
        {
            return Self::handle_error(state.get_client(), req, RippleError::ProcessorError).await;
        }
        let timezones =
            serde_json::from_value::<ThunderAvailableTimezonesResponse>(response.message).unwrap();
        return Self::respond(
            state.get_client(),
            req,
            ExtnResponse::AvailableTimezones(timezones.as_array()),
        )
        .await
        .is_ok();
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
            DeviceInfoRequest::GetTimezone => Self::get_timezone(state.clone(), msg).await,
            DeviceInfoRequest::GetAvailableTimezones => {
                Self::get_available_timezones(state.clone(), msg).await
            }
            DeviceInfoRequest::SetTimezone(timezone_params) => {
                Self::set_timezone(state.clone(), timezone_params, msg).await
            }
            _ => false,
        }
    }
}
