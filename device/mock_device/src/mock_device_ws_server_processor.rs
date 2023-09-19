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
    api::mock_websocket_server::{
        AddRequestResponseResponse, MockWebsocketServerRequest, MockWebsocketServerResponse,
        RemoveRequestResponse,
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
    log::{debug, error},
    tokio::sync::mpsc::{Receiver, Sender},
};
use std::sync::Arc;

use crate::mock_ws_server::MockWebsocketServer;

#[derive(Debug, Clone)]
pub struct MockDeviceMockWebsocketServerState {
    client: ExtnClient,
    server: Arc<MockWebsocketServer>,
}

impl MockDeviceMockWebsocketServerState {
    fn new(client: ExtnClient, server: Arc<MockWebsocketServer>) -> Self {
        Self { client, server }
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
    ) -> MockDeviceMockWebsocketServerProcessor {
        MockDeviceMockWebsocketServerProcessor {
            state: MockDeviceMockWebsocketServerState::new(client, server),
            streamer: DefaultExtnStreamer::new(),
        }
    }

    async fn respond(
        client: ExtnClient,
        req: ExtnMessage,
        resp: MockWebsocketServerResponse,
    ) -> bool {
        let resp = client
            .clone()
            .respond(req, ExtnResponse::MockWebsocketServer(resp))
            .await;

        match resp {
            Ok(_) => true,
            Err(err) => {
                error!("{err:?}");
                false
            }
        }
    }
}

impl ExtnStreamProcessor for MockDeviceMockWebsocketServerProcessor {
    type STATE = MockDeviceMockWebsocketServerState;
    type VALUE = MockWebsocketServerRequest;

    fn get_state(&self) -> Self::STATE {
        self.state.clone()
    }

    fn receiver(&mut self) -> Receiver<ExtnMessage> {
        self.streamer.receiver()
    }

    fn sender(&self) -> Sender<ExtnMessage> {
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
        extn_request: ExtnMessage,
        extracted_message: Self::VALUE,
    ) -> bool {
        debug!("extn_request={extn_request:?}, extracted_message={extracted_message:?}");
        match extracted_message {
            MockWebsocketServerRequest::AddRequestResponse(params) => {
                state
                    .server
                    .add_request_response(&params.request, params.responses.clone())
                    .await;

                Self::respond(
                    state.client.clone(),
                    extn_request,
                    MockWebsocketServerResponse::AddRequestResponse(AddRequestResponseResponse {
                        success: true,
                    }),
                )
                .await
            }
            MockWebsocketServerRequest::RemoveRequest(params) => {
                state.server.remove_request(&params.request).await;

                Self::respond(
                    state.client.clone(),
                    extn_request,
                    MockWebsocketServerResponse::RemoveRequestResponse(RemoveRequestResponse {
                        success: true,
                    }),
                )
                .await
            }
        }
    }
}
