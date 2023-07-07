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
    api::firebolt::fb_metrics::BehavioralMetricRequest,
    async_trait::async_trait,
    extn::client::{
        extn_client::ExtnClient,
        extn_processor::{
            DefaultExtnStreamer, ExtnRequestProcessor, ExtnStreamProcessor, ExtnStreamer,
        },
    },
    log::error,
};

pub struct DistributorMetricsProcessor {
    client: ExtnClient,
    streamer: DefaultExtnStreamer,
}

impl DistributorMetricsProcessor {
    pub fn new(client: ExtnClient) -> DistributorMetricsProcessor {
        DistributorMetricsProcessor {
            client,
            streamer: DefaultExtnStreamer::new(),
        }
    }
}

impl ExtnStreamProcessor for DistributorMetricsProcessor {
    type STATE = ExtnClient;
    type VALUE = BehavioralMetricRequest;

    fn get_state(&self) -> Self::STATE {
        self.client.clone()
    }

    fn receiver(
        &mut self,
    ) -> ripple_sdk::tokio::sync::mpsc::Receiver<ripple_sdk::extn::extn_client_message::ExtnMessage>
    {
        self.streamer.receiver()
    }

    fn sender(
        &self,
    ) -> ripple_sdk::tokio::sync::mpsc::Sender<ripple_sdk::extn::extn_client_message::ExtnMessage>
    {
        self.streamer.sender()
    }
}

#[async_trait]
impl ExtnRequestProcessor for DistributorMetricsProcessor {
    fn get_client(&self) -> ExtnClient {
        self.client.clone()
    }
    async fn process_request(
        state: Self::STATE,
        msg: ripple_sdk::extn::extn_client_message::ExtnMessage,
        extracted_message: Self::VALUE,
    ) -> bool {
        match extracted_message {
            BehavioralMetricRequest::Ready(_) => {
                if let Err(e) = state
                    .clone()
                    .respond(
                        msg,
                        ripple_sdk::extn::extn_client_message::ExtnResponse::Boolean(false),
                    )
                    .await
                {
                    error!("Error sending back response {:?}", e);
                    return false;
                }
                true
            }
            BehavioralMetricRequest::SignIn(_) => {
                mock_metrics_response(state, msg, extracted_message).await
            }
            BehavioralMetricRequest::SignOut(_) => {
                mock_metrics_response(state, msg, extracted_message).await
            }
            BehavioralMetricRequest::StartContent(_) => {
                mock_metrics_response(state, msg, extracted_message).await
            }
            BehavioralMetricRequest::StopContent(_) => {
                mock_metrics_response(state, msg, extracted_message).await
            }
            BehavioralMetricRequest::Page(_) => {
                mock_metrics_response(state, msg, extracted_message).await
            }
            BehavioralMetricRequest::Action(_) => {
                mock_metrics_response(state, msg, extracted_message).await
            }
            BehavioralMetricRequest::Error(_) => {
                mock_metrics_response(state, msg, extracted_message).await
            }
            BehavioralMetricRequest::MediaLoadStart(_) => {
                mock_metrics_response(state, msg, extracted_message).await
            }
            BehavioralMetricRequest::MediaPlay(_) => {
                mock_metrics_response(state, msg, extracted_message).await
            }
            BehavioralMetricRequest::MediaPlaying(_) => {
                mock_metrics_response(state, msg, extracted_message).await
            }
            BehavioralMetricRequest::MediaPause(_) => {
                mock_metrics_response(state, msg, extracted_message).await
            }
            BehavioralMetricRequest::MediaWaiting(_) => {
                mock_metrics_response(state, msg, extracted_message).await
            }
            BehavioralMetricRequest::MediaProgress(_) => {
                mock_metrics_response(state, msg, extracted_message).await
            }
            BehavioralMetricRequest::MediaSeeking(_) => {
                mock_metrics_response(state, msg, extracted_message).await
            }
            BehavioralMetricRequest::MediaSeeked(_) => {
                mock_metrics_response(state, msg, extracted_message).await
            }
            BehavioralMetricRequest::MediaRateChanged(_) => {
                mock_metrics_response(state, msg, extracted_message).await
            }
            BehavioralMetricRequest::MediaRenditionChanged(_) => {
                mock_metrics_response(state, msg, extracted_message).await
            }
            BehavioralMetricRequest::MediaEnded(_) => {
                mock_metrics_response(state, msg, extracted_message).await
            }
            BehavioralMetricRequest::TelemetrySignIn(_) => {
                mock_metrics_response(state, msg, extracted_message).await
            }
            BehavioralMetricRequest::TelemetrySignOut(_) => {
                mock_metrics_response(state, msg, extracted_message).await
            }
            BehavioralMetricRequest::TelemetryInternalInitialize(_) => {
                mock_metrics_response(state, msg, extracted_message).await
            }
            BehavioralMetricRequest::Raw(_) => {
                mock_metrics_response(state, msg, extracted_message).await
            }
        }
    }
}

/// listener for events any events.
pub async fn mock_metrics_response(
    state: ExtnClient,
    msg: ripple_sdk::extn::extn_client_message::ExtnMessage,
    _extracted_message: BehavioralMetricRequest,
) -> bool {
    if let Err(e) = state
        .clone()
        .respond(
            msg,
            ripple_sdk::extn::extn_client_message::ExtnResponse::Boolean(false),
        )
        .await
    {
        error!("Error sending back response {:?}", e);
        return false;
    }
    true
}
