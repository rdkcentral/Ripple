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
    api::firebolt::fb_telemetry::OperationalMetricRequest,
    async_trait::async_trait,
    extn::{
        client::extn_processor::{
            DefaultExtnStreamer, ExtnRequestProcessor, ExtnStreamProcessor, ExtnStreamer,
        },
        extn_client_message::ExtnMessage,
    },
    tokio::sync::mpsc::{Receiver as MReceiver, Sender as MSender},
};

use crate::state::platform_state::PlatformState;
/// Supports processing of Metrics request from extensions and forwards the metrics accordingly.

/// Supports processing of Metrics request from extensions and forwards the metrics accordingly.
#[derive(Debug)]
pub struct OpMetricsProcessor {
    state: PlatformState,
    streamer: DefaultExtnStreamer,
}

impl OpMetricsProcessor {
    pub fn new(state: PlatformState) -> OpMetricsProcessor {
        OpMetricsProcessor {
            state,
            streamer: DefaultExtnStreamer::new(),
        }
    }
}

impl ExtnStreamProcessor for OpMetricsProcessor {
    type STATE = PlatformState;
    type VALUE = OperationalMetricRequest;
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
impl ExtnRequestProcessor for OpMetricsProcessor {
    fn get_client(&self) -> ripple_sdk::extn::client::extn_client::ExtnClient {
        self.state.get_client().get_extn_client()
    }

    async fn process_request(
        state: Self::STATE,
        msg: ExtnMessage,
        extracted_message: Self::VALUE,
    ) -> bool {
        let requestor = msg.requestor.to_string();
        match extracted_message {
            OperationalMetricRequest::Subscribe => {
                state.operational_telemetry_listener(&requestor, true).await
            }
            OperationalMetricRequest::UnSubscribe => {
                state
                    .operational_telemetry_listener(&requestor, false)
                    .await
            }
        }
        Self::ack(state.get_client().get_extn_client(), msg)
            .await
            .is_ok()
    }
}
