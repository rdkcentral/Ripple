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
use ripple_sdk::{
    api::distributor::distributor_discovery::MediaEventRequest,
    async_trait::async_trait,
    extn::client::{
        extn_client::ExtnClient,
        extn_processor::{
            DefaultExtnStreamer, ExtnRequestProcessor, ExtnStreamProcessor, ExtnStreamer,
        },
    },
};

pub struct DistributorMediaEventProcessor {
    client: ExtnClient,
    streamer: DefaultExtnStreamer,
}

impl DistributorMediaEventProcessor {
    pub fn new(client: ExtnClient) -> DistributorMediaEventProcessor {
        DistributorMediaEventProcessor {
            client,
            streamer: DefaultExtnStreamer::new(),
        }
    }
}

impl ExtnStreamProcessor for DistributorMediaEventProcessor {
    type STATE = ExtnClient;
    type VALUE = MediaEventRequest;

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
impl ExtnRequestProcessor for DistributorMediaEventProcessor {
    fn get_client(&self) -> ExtnClient {
        self.client.clone()
    }
    async fn process_request(
        state: Self::STATE,
        msg: ripple_sdk::extn::extn_client_message::ExtnMessage,
        _: Self::VALUE,
    ) -> bool {
        state
            .clone()
            .respond(
                msg,
                ripple_sdk::extn::extn_client_message::ExtnResponse::None(()),
            )
            .await
            .is_ok()
    }
}
