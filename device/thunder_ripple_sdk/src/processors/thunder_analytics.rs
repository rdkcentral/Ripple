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
    api::observability::analytics::AnalyticsRequest,
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
    tokio::sync::mpsc::{Receiver as MReceiver, Sender as MSender},
};

use crate::thunder_state::ThunderState;

#[derive(Debug)]
pub struct ThunderAnalyticsProcessor {
    state: ThunderState,
    streamer: DefaultExtnStreamer,
}

impl ThunderAnalyticsProcessor {
    pub fn new(state: ThunderState) -> ThunderAnalyticsProcessor {
        println!("*** _DEBUG: ThunderAnalyticsProcessor::new: entry");
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
        println!(
            "*** _DEBUG: 2 ThunderAnalyticsProcessor::process_request: extracted_message={:?}",
            extracted_message
        );

        match extracted_message {
            AnalyticsRequest::SendMetrics(data) => {
                println!(
                    "*** _DEBUG: ThunderAnalyticsProcessor::process_request: data={:?}",
                    data
                );
            }
        }

        Self::respond(state.get_client(), msg, ExtnResponse::None(()))
            .await
            .is_ok()
    }
}

async fn send_metrics(state: ThunderState, metrics: Value) -> ExtnResponse {
    /*
    setup operation at higher scope to allow it to time itself
    */
    let operation = Operation::new(
        AppsOperationType::Install,
        app.clone().id,
        AppData::new(app.clone().version),
    );

    let method: String = ThunderPlugin::PackageManager.method("install");
    let request = InstallAppRequest::new(app.clone());

    let metrics_timer = start_service_metrics_timer(
        &state.thunder_state.get_client(),
        ThunderMetricsTimerName::PackageManagerInstall.to_string(),
    );

    let device_response = state
        .thunder_state
        .get_thunder_client()
        .call(DeviceCallRequest {
            method,
            params: Some(DeviceChannelParams::Json(
                serde_json::to_string(&request).unwrap(),
            )),
        })
        .await;

    let thunder_resp = serde_json::from_value::<String>(device_response.message);

    let status = if thunder_resp.is_ok() {
        ThunderResponseStatus::Success
    } else {
        ThunderResponseStatus::Failure
    };

    stop_and_send_service_metrics_timer(
        state.thunder_state.get_client().clone(),
        metrics_timer,
        status.to_string(),
    )
    .await;

    match thunder_resp {
        Ok(handle) => {
            Self::add_or_remove_operation(
                state.clone(),
                handle.clone(),
                operation,
                Some(status.to_string()),
            );

            ExtnResponse::String(handle)
        }
        Err(_) => ExtnResponse::Error(RippleError::ProcessorError),
    }
}
