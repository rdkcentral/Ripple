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

use std::collections::HashMap;

use ripple_sdk::{
    api::config::RfcRequest,
    async_trait::async_trait,
    extn::{
        client::{
            extn_client::ExtnClient,
            extn_processor::{
                DefaultExtnStreamer, ExtnRequestProcessor, ExtnStreamProcessor, ExtnStreamer,
            },
        },
        extn_client_message::{ExtnMessage, ExtnResponse},
    },
    log::error,
    serde_json::{self, json},
    tokio::sync::{mpsc::Receiver as MReceiver, mpsc::Sender as MSender},
    utils::error::RippleError,
};
use serde::Deserialize;

use crate::{
    client::{
        device_operator::{DeviceCallRequest, DeviceChannelParams, DeviceOperator},
        thunder_plugin::ThunderPlugin,
    },
    thunder_state::ThunderState,
};

#[derive(Debug, Deserialize, Clone)]
pub struct ThunderRFCResponse {
    #[serde(rename = "RFCConfig")]
    pub config: HashMap<String, String>,
}

impl ThunderRFCResponse {
    fn get_extn_response(&self, request: &RfcRequest) -> ExtnResponse {
        self.config
            .get(&request.flag)
            .map_or(ExtnResponse::Value(serde_json::Value::Null), |v| {
                if v.contains("Empty response received") {
                    ExtnResponse::Value(serde_json::Value::Null)
                } else {
                    ExtnResponse::Value(serde_json::Value::String(v.to_owned()))
                }
            })
    }
}

#[derive(Debug)]
pub struct ThunderRFCProcessor {
    state: ThunderState,
    streamer: DefaultExtnStreamer,
}

/// Event processor used for cases where a certain Extension Capability is required to be ready.
/// Bootstrap uses the [WaitForStatusReadyEventProcessor] to await during Device Connnection before starting the gateway.
impl ThunderRFCProcessor {
    pub fn new(state: ThunderState) -> ThunderRFCProcessor {
        ThunderRFCProcessor {
            state,
            streamer: DefaultExtnStreamer::new(),
        }
    }
}

impl ExtnStreamProcessor for ThunderRFCProcessor {
    type VALUE = RfcRequest;
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
impl ExtnRequestProcessor for ThunderRFCProcessor {
    fn get_client(&self) -> ExtnClient {
        self.state.get_client()
    }

    async fn process_request(
        state: Self::STATE,
        msg: ExtnMessage,
        extracted_message: Self::VALUE,
    ) -> bool {
        let flag = extracted_message.flag.clone();
        let rfc_request = json!({ "rfcList": vec![flag] }).to_string();
        let resp = state
            .get_thunder_client()
            .call(DeviceCallRequest {
                method: ThunderPlugin::System.method("getRFCConfig"),
                params: Some(DeviceChannelParams::Json(rfc_request)),
            })
            .await;

        let response = match resp {
            Ok(res) => res,
            Err(e) => {
                error!("Thunder call failed: {}", e);
                return false;
            }
        };

        Self::respond(
            state.get_client(),
            msg,
            match serde_json::from_value::<ThunderRFCResponse>(response.message) {
                Ok(rfc_response) => rfc_response.get_extn_response(&extracted_message),
                Err(e) => {
                    error!("rfc serialization error {:?}", e);
                    ExtnResponse::Error(RippleError::InvalidOutput)
                }
            },
        )
        .await
        .is_ok()
    }
}
