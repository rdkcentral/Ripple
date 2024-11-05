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
    api::{
        device::device_operator::{
            DeviceCallRequest, DeviceChannelParams, DeviceOperator, DeviceResponseMessage,
        },
        firebolt::fb_metrics::BehavioralMetricsEvent,
        observability::analytics::AnalyticsRequest,
    },
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
    serde_json,
    tokio::sync::mpsc::{Receiver as MReceiver, Sender as MSender},
    utils::error::RippleError,
};

use crate::{client::thunder_plugin::ThunderPlugin, thunder_state::ThunderState};

#[derive(Debug)]
pub struct ThunderAnalyticsProcessor {
    state: ThunderState,
    streamer: DefaultExtnStreamer,
}

impl ThunderAnalyticsProcessor {
    pub fn new(state: ThunderState) -> ThunderAnalyticsProcessor {
        ThunderAnalyticsProcessor {
            state,
            streamer: DefaultExtnStreamer::new(),
        }
    }
}

impl ExtnStreamProcessor for ThunderAnalyticsProcessor {
    type VALUE = AnalyticsRequest;
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
impl ExtnRequestProcessor for ThunderAnalyticsProcessor {
    fn get_client(&self) -> ExtnClient {
        self.state.get_client()
    }

    async fn process_request(
        state: Self::STATE,
        msg: ExtnMessage,
        extracted_message: Self::VALUE,
    ) -> bool {
        let response_message = match extracted_message {
            AnalyticsRequest::SendMetrics(event) => send_metrics(state.clone(), event).await,
        };

        let extn_response = match response_message.message["success"].as_bool() {
            Some(success) => {
                if success {
                    ExtnResponse::None(())
                } else {
                    ExtnResponse::Error(RippleError::ExtnError)
                }
            }
            None => ExtnResponse::Error(RippleError::ExtnError),
        };

        Self::respond(state.get_client(), msg, extn_response)
            .await
            .is_ok()
    }
}

async fn send_metrics(
    thunder_state: ThunderState,
    metrics_event: BehavioralMetricsEvent,
) -> DeviceResponseMessage {
    let method: String = ThunderPlugin::Analytics.method("sendEvent");

    thunder_state
        .get_thunder_client()
        .call(DeviceCallRequest {
            method,
            params: Some(DeviceChannelParams::Json(
                serde_json::to_string(&metrics_event).unwrap(),
            )),
        })
        .await
}
