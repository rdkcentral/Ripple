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
        extn_id::{ExtnId, ExtnProviderAdjective, ExtnProviderRequest},
    },
    framework::ripple_contract::RippleContract,
    log::trace,
    processor::rpc_router::{RouterState, RpcRouter},
    tokio::sync::mpsc::{Receiver, Sender},
    utils::error::RippleError,
};
use async_trait::async_trait;
use jsonrpsee::core::server::rpc_module::Methods;
use log::info;

#[derive(Debug, Clone)]
pub struct RPCRequestState {
    client: ExtnClient,
    router_state: RouterState,
    extn_id: ExtnId,
}

/// Processor to service incoming RPC Requests used by extensions and other local rpc handlers for aliasing.
#[derive(Debug)]
pub struct RPCRequestProcessor {
    state: RPCRequestState,
    streamer: DefaultExtnStreamer,
}

impl RPCRequestProcessor {
    pub fn new(client: ExtnClient, methods: Methods, extn_id: ExtnId) -> RPCRequestProcessor {
        let router_state = RouterState::new();
        router_state.update_methods(methods.clone());
        for method in methods.method_names() {
            info!("Adding RPC method {}", method);
        }

        RPCRequestProcessor {
            state: RPCRequestState {
                client,
                router_state,
                extn_id,
            },
            streamer: DefaultExtnStreamer::new(),
        }
    }
}

impl ExtnStreamProcessor for RPCRequestProcessor {
    type STATE = RPCRequestState;
    type VALUE = ExtnProviderRequest;

    fn get_state(&self) -> Self::STATE {
        self.state.clone()
    }

    fn sender(&self) -> Sender<ExtnMessage> {
        self.streamer.sender()
    }

    fn receiver(&mut self) -> Receiver<ExtnMessage> {
        self.streamer.receiver()
    }

    fn contract(&self) -> RippleContract {
        RippleContract::ExtnProvider(ExtnProviderAdjective {
            id: self.state.extn_id.clone(),
        })
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
            let req: RpcRequest = serde_json::from_value(extracted_message.value).unwrap();
            let resp = RpcRouter::resolve_route(req.clone(), &router_state).await;

            match resp {
                Ok(response_msg) => {
                    if Self::respond(client, extn_msg, ExtnResponse::String(response_msg))
                        .await
                        .is_ok()
                    {
                        return true;
                    }
                }
                Err(_) => {
                    Self::handle_error(client, extn_msg, RippleError::ProcessorError).await;
                }
            };
            true
        });

        true
    }
}
