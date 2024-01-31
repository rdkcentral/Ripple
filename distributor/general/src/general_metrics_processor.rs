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
    api::firebolt::fb_metrics::{BehavioralMetricPayload, BehavioralMetricRequest},
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
        mut state: Self::STATE,
        msg: ripple_sdk::extn::extn_client_message::ExtnMessage,
        extracted_message: Self::VALUE,
    ) -> bool {
        match extracted_message.clone().payload {
            BehavioralMetricPayload::Ready(_) => {
                if let Err(e) = state
                    .respond(
                        msg,
                        ripple_sdk::extn::extn_client_message::ExtnResponse::Boolean(true),
                    )
                    .await
                {
                    error!("Error sending back response {:?}", e);
                    return false;
                }
                true
            }
            BehavioralMetricPayload::SignIn(_) => {
                mock_metrics_response(state, msg, extracted_message).await
            }
            BehavioralMetricPayload::SignOut(_) => {
                mock_metrics_response(state, msg, extracted_message).await
            }
            BehavioralMetricPayload::StartContent(_) => {
                mock_metrics_response(state, msg, extracted_message).await
            }
            BehavioralMetricPayload::StopContent(_) => {
                mock_metrics_response(state, msg, extracted_message).await
            }
            BehavioralMetricPayload::Page(_) => {
                mock_metrics_response(state, msg, extracted_message).await
            }
            BehavioralMetricPayload::Action(_) => {
                mock_metrics_response(state, msg, extracted_message).await
            }
            BehavioralMetricPayload::Error(_) => {
                mock_metrics_response(state, msg, extracted_message).await
            }
            BehavioralMetricPayload::MediaLoadStart(_) => {
                mock_metrics_response(state, msg, extracted_message).await
            }
            BehavioralMetricPayload::MediaPlay(_) => {
                mock_metrics_response(state, msg, extracted_message).await
            }
            BehavioralMetricPayload::MediaPlaying(_) => {
                mock_metrics_response(state, msg, extracted_message).await
            }
            BehavioralMetricPayload::MediaPause(_) => {
                mock_metrics_response(state, msg, extracted_message).await
            }
            BehavioralMetricPayload::MediaWaiting(_) => {
                mock_metrics_response(state, msg, extracted_message).await
            }
            BehavioralMetricPayload::MediaProgress(_) => {
                mock_metrics_response(state, msg, extracted_message).await
            }
            BehavioralMetricPayload::MediaSeeking(_) => {
                mock_metrics_response(state, msg, extracted_message).await
            }
            BehavioralMetricPayload::MediaSeeked(_) => {
                mock_metrics_response(state, msg, extracted_message).await
            }
            BehavioralMetricPayload::MediaRateChanged(_) => {
                mock_metrics_response(state, msg, extracted_message).await
            }
            BehavioralMetricPayload::MediaRenditionChanged(_) => {
                mock_metrics_response(state, msg, extracted_message).await
            }
            BehavioralMetricPayload::MediaEnded(_) => {
                mock_metrics_response(state, msg, extracted_message).await
            }
            BehavioralMetricPayload::AppStateChange(_) => {
                mock_metrics_response(state, msg, extracted_message).await
            }
            BehavioralMetricPayload::Raw(_) => {
                mock_metrics_response(state, msg, extracted_message).await
            }
        }
    }
}

/// listener for events any events.
pub async fn mock_metrics_response(
    mut state: ExtnClient,
    msg: ripple_sdk::extn::extn_client_message::ExtnMessage,
    _extracted_message: BehavioralMetricRequest,
) -> bool {
    if let Err(e) = state
        .respond(
            msg,
            ripple_sdk::extn::extn_client_message::ExtnResponse::Boolean(true),
        )
        .await
    {
        error!("Error sending back response {:?}", e);
        return false;
    }
    true
}
