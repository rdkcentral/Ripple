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
    api::mock_websocket_server::{AddRequestResponseParams, MockWebsocketServerRequest},
    async_trait::async_trait,
    extn::client::{
        extn_client::ExtnClient,
        extn_processor::{
            DefaultExtnStreamer, ExtnRequestProcessor, ExtnStreamProcessor, ExtnStreamer,
        },
    },
    tokio::sync::Mutex,
};
use std::sync::Arc;

use crate::{mock_ws_server::MockWebsocketServer, utils::MockData};

#[derive(Debug, Clone)]
pub struct MockDeviceMockWebsocketServerState {
    client: ExtnClient,
    server: Arc<MockWebsocketServer>,
    mock_data: Arc<Mutex<MockData>>,
}

impl MockDeviceMockWebsocketServerState {
    fn new(
        client: ExtnClient,
        server: Arc<MockWebsocketServer>,
        mock_data: Arc<Mutex<MockData>>,
    ) -> Self {
        Self {
            client,
            server,
            mock_data,
        }
    }
}

pub struct MockDeviceMockWebsocketServerProcessor {
    state: MockDeviceMockWebsocketServerState,
    streamer: DefaultExtnStreamer,
}

impl MockDeviceMockWebsocketServerProcessor {
    pub fn new(
        client: ExtnClient,
        server: Arc<MockWebsocketServer>,
        mock_data: Arc<Mutex<MockData>>,
    ) -> MockDeviceMockWebsocketServerProcessor {
        MockDeviceMockWebsocketServerProcessor {
            state: MockDeviceMockWebsocketServerState::new(client, server, mock_data),
            streamer: DefaultExtnStreamer::new(),
        }
    }
}

impl ExtnStreamProcessor for MockDeviceMockWebsocketServerProcessor {
    type STATE = MockDeviceMockWebsocketServerState;
    type VALUE = MockWebsocketServerRequest;

    fn get_state(&self) -> Self::STATE {
        self.state.clone()
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
impl ExtnRequestProcessor for MockDeviceMockWebsocketServerProcessor {
    fn get_client(&self) -> ExtnClient {
        self.state.client.clone()
    }

    async fn process_request(
        state: Self::STATE,
        msg: ripple_sdk::extn::extn_client_message::ExtnMessage,
        extracted_message: Self::VALUE,
    ) -> bool {
        // TODO: call the get and remove for the requests
        match extracted_message {
            MockWebsocketServerRequest::AddRequestResponse(params) => {
                // state.server.
                state.server.add_request_response(&params.request, params.responses.clone()).await
            }
            // AccountSessionRequest::Provision(p) => Self::provision(state.clone(), msg, p).await,
            // AccountSessionRequest::SetAccessToken(s) => Self::set_token(state, msg, s).await,
        }
        true
    }
}
