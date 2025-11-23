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

use ripple_sdk::{
    api::firebolt::fb_telemetry::TelemetryPayload,
    async_trait::async_trait,
    extn::{
        client::extn_processor::{
            DefaultExtnStreamer, ExtnEventProcessor, ExtnStreamProcessor, ExtnStreamer,
        },
        extn_client_message::ExtnMessage,
    },
    log::info,
    serde_json::json,
    tokio::sync::{mpsc::Receiver as MReceiver, mpsc::Sender as MSender},
    utils::error::RippleError,
};
use serde::Serialize;

use crate::{
    client::{
        device_operator::{DeviceCallRequest, DeviceChannelParams, DeviceOperator},
        thunder_plugin::ThunderPlugin,
    },
    thunder_state::ThunderState,
};

#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ThunderTelemetryEvent {
    event_name: String,
    event_value: String,
}
impl From<ThunderTelemetryEvent> for DeviceChannelParams {
    fn from(event: ThunderTelemetryEvent) -> Self {
        DeviceChannelParams::Json(json!(event).to_string())
    }
}

fn strip_crlf(bytes: Vec<u8>) -> Result<String, RippleError> {
    if let Ok(v) = String::from_utf8(bytes) {
        return Ok(v.trim().to_owned());
    }
    Err(RippleError::ParseError)
}

fn writer() -> csv::Writer<Vec<u8>> {
    csv::WriterBuilder::new()
        .has_headers(false)
        .from_writer(vec![])
}
fn serialize<T: Serialize>(event: &T) -> Result<String, RippleError> {
    let mut wri = writer();
    if wri.serialize(event).is_err() {
        return Err(RippleError::ParseError);
    }
    if let Ok(v) = wri.into_inner() {
        return strip_crlf(v);
    }
    Err(RippleError::ParseError)
}
fn render_event_data(event: &TelemetryPayload) -> Result<String, RippleError> {
    serialize(event)
}

fn telemetry_event(event_name: &str, event_payload: String) -> DeviceChannelParams {
    ThunderTelemetryEvent {
        event_name: String::from(event_name),
        event_value: event_payload,
    }
    .into()
}

fn get_event_name(event: &TelemetryPayload) -> &'static str {
    match event {
        TelemetryPayload::AppLoadStart(_) => "app_load_start_split",
        TelemetryPayload::AppLoadStop(_) => "app_load_stop_split",
        TelemetryPayload::AppSDKLoaded(_) => "app_sdk_loaded_split",
        TelemetryPayload::AppError(_) => "app_error_split",
        TelemetryPayload::SystemError(_) => "ripple_system_error_split",
        TelemetryPayload::SignIn(_) => "app_sign_in_split",
        TelemetryPayload::SignOut(_) => "app_sign_out_split",
        TelemetryPayload::InternalInitialize(_) => "app_internal_initialize_split",
        TelemetryPayload::FireboltInteraction(_) => "app_firebolt_split",
        TelemetryPayload::FireboltEvent(_) => "app_firebolt_event_split",
    }
}

pub enum ThunderResponseStatus {
    Success,
    Failure,
}

impl std::fmt::Display for ThunderResponseStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ThunderResponseStatus::Success => write!(f, "success"),
            ThunderResponseStatus::Failure => write!(f, "failure"),
        }
    }
}

#[derive(Debug)]
pub struct ThunderTelemetryProcessor {
    state: ThunderState,
    streamer: DefaultExtnStreamer,
}

/// Event processor used for cases where a certain Extension Capability is required to be ready.
/// Bootstrap uses the [WaitForStatusReadyEventProcessor] to await during Device Connnection before starting the gateway.
impl ThunderTelemetryProcessor {
    pub fn new(state: ThunderState) -> ThunderTelemetryProcessor {
        ThunderTelemetryProcessor {
            state,
            streamer: DefaultExtnStreamer::new(),
        }
    }
}

impl ExtnStreamProcessor for ThunderTelemetryProcessor {
    type VALUE = TelemetryPayload;
    type STATE = ThunderState;

    fn get_state(&self) -> Self::STATE {
        self.state.clone()
    }

    fn sender(&self) -> MSender<ExtnMessage> {
        self.streamer.sender()
    }

    fn receiver(&mut self) -> MReceiver<ExtnMessage> {
        self.streamer.receiver()
    }
}

#[async_trait]
impl ExtnEventProcessor for ThunderTelemetryProcessor {
    async fn process_event(
        state: Self::STATE,
        _msg: ExtnMessage,
        extracted_message: Self::VALUE,
    ) -> Option<bool> {
        if let TelemetryPayload::FireboltEvent(_) = extracted_message {
            return None;
        }

        if let Ok(data) = render_event_data(&extracted_message) {
            info!("Sending telemetry event: {}", data);
            let _ = state
                .get_thunder_client()
                .call(DeviceCallRequest {
                    method: ThunderPlugin::Telemetry.unversioned_method("logApplicationEvent"),
                    params: Some(telemetry_event(get_event_name(&extracted_message), data)),
                })
                .await;
        }
        None
    }
}
