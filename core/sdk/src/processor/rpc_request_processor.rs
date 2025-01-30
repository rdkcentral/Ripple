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

use crate::{
    api::gateway::rpc_gateway_api::RpcRequest,
    extn::{
        client::{
            extn_client::ExtnClient,
            extn_processor::{
                DefaultExtnStreamer, ExtnRequestProcessor, ExtnStreamProcessor, ExtnStreamer,
            },
        },
        extn_client_message::{ExtnMessage, ExtnResponse},
    },
    framework::ripple_contract::RippleContract,
    log::trace,
    processor::rpc_router::{RouterState, RpcRouter},
    tokio::sync::mpsc::{Receiver, Sender},
    utils::error::RippleError,
};
use async_trait::async_trait;
use jsonrpsee::core::server::rpc_module::Methods;

#[derive(Debug, Clone)]
pub struct RPCRequestState {
    client: ExtnClient,
    router_state: RouterState,
}

/// Processor to service incoming RPC Requests used by extensions and other local rpc handlers for aliasing.
#[derive(Debug)]
pub struct RPCRequestProcessor {
    state: RPCRequestState,
    streamer: DefaultExtnStreamer,
}

impl RPCRequestProcessor {
    pub fn new(client: ExtnClient, methods: Methods) -> RPCRequestProcessor {
        let router_state = RouterState::new();
        router_state.update_methods(methods.clone());

        RPCRequestProcessor {
            state: RPCRequestState {
                client,
                router_state,
            },
            streamer: DefaultExtnStreamer::new(),
        }
    }
}

impl ExtnStreamProcessor for RPCRequestProcessor {
    type STATE = RPCRequestState;
    type VALUE = RpcRequest;

    fn get_state(&self) -> Self::STATE {
        self.state.clone()
    }

    fn sender(&self) -> Sender<ExtnMessage> {
        self.streamer.sender()
    }

    fn receiver(&mut self) -> Receiver<ExtnMessage> {
        self.streamer.receiver()
    }
}

#[async_trait]
impl ExtnRequestProcessor for RPCRequestProcessor {
    fn get_client(&self) -> ExtnClient {
        self.state.client.clone()
    }

    async fn process_request(
        state: Self::STATE,
        extn_msg: ExtnMessage,
        extracted_message: Self::VALUE,
    ) -> bool {
        trace!("SDK: Processing RPC Request");
        let client = state.client.clone();
        tokio::spawn(async move {
            let router_state = state.router_state.clone();
            let req = extracted_message.clone();
            let resp = RpcRouter::resolve_route(req.clone(), &router_state).await;
            let mut resp_extn_msg = extn_msg.clone();
            resp_extn_msg.target = RippleContract::Rpc;

            match resp {
                Ok(response_msg) => {
                    if response_msg.is_error() {
                        Self::handle_error(client, resp_extn_msg, RippleError::ProcessorError)
                            .await;
                    } else if let Ok(json_value) = serde_json::from_str(&response_msg.jsonrpc_msg) {
                        if Self::respond(client, resp_extn_msg, ExtnResponse::Value(json_value))
                            .await
                            .is_ok()
                        {
                            return true;
                        }
                    } else {
                        Self::handle_error(client, resp_extn_msg, RippleError::ProcessorError)
                            .await;
                    }
                }
                Err(_) => {
                    Self::handle_error(client, resp_extn_msg, RippleError::ProcessorError).await;
                }
            };

            true
        });

        true
    }
}
