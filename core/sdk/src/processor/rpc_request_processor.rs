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
    api::{gateway::rpc_gateway_api::RpcRequest, manifest::extn_manifest::ExtnManifest},
    async_trait::async_trait,
    extn::{
        client::{
            extn_client::ExtnClient,
            extn_processor::{
                DefaultExtnStreamer, ExtnRequestProcessor, ExtnStreamProcessor, ExtnStreamer,
            },
        },
        extn_client_message::ExtnMessage,
    },
    framework::ripple_contract::RippleContract,
    processor::rpc_router::RouterState,
    processor::rpc_router::RpcRouter,
    tokio::sync::mpsc::{Receiver, Sender},
    utils::router_utils::return_extn_response,
};
use chrono::Utc;
use futures::future::ok;
use jsonrpsee::core::server::rpc_module::Methods;
use std::sync::{Arc, Mutex};

const EXTN_NAME: &'static str = "badger";

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
    // TBD - manifest - do we need it for badger methods/ do we need to check if the has overriden the methods for badger especially?
    // can even create a new struct to put in methods, resources, manifest if manifest is needed for badger methods

    pub fn new(
        client: ExtnClient,
        methods: Methods,
        manifest: ExtnManifest,
    ) -> RPCRequestProcessor {
        // TBD: should create a new state badger_state, in sdk for router_state???
        // TBD: BootstrapState creates the extn_manifest - set that in the new badger_state in sdk like how we set it in PlatformState??

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
    // <pca>
    //type VALUE = ExtnProviderRequest;
    type VALUE = RpcRequest;
    // </pca>
    fn get_state(&self) -> Self::STATE {
        self.state.clone()
    }

    fn sender(&self) -> Sender<ExtnMessage> {
        self.streamer.sender()
    }

    fn receiver(&mut self) -> Receiver<ExtnMessage> {
        self.streamer.receiver()
    }

    // <pca>
    fn contract(&self) -> RippleContract {
        RippleContract::Rpc
    }
    // </pca>

    // TBD = need to define - contract or fulfills_mutiple ?
}

#[async_trait]
impl ExtnRequestProcessor for RPCRequestProcessor {
    fn get_client(&self) -> ExtnClient {
        self.state.client.clone()
    }

    // i dont know whether this is right, but how can I get the router_state from the RouterState????????
    // fn get_router_state(&self) -> Option<RouterState> {
    //     Some(self.router_state.clone())
    // }

    // copied from RpcGatewayProcessor - ThunderDeviceInfoRequestProcessor wondering if this needs to be similar to match request
    // i.e let request = ExtnProviderRequest { value: serde_json::to_value(request).unwrap(), id: self.id.clone(), };

    // referring to the
    async fn process_request(
        state: Self::STATE,
        msg: ExtnMessage,
        extracted_message: Self::VALUE,
    ) -> bool {
        println!(
            "**** rpc_request_processor: process_request: extracted_message: {:?}",
            extracted_message.clone()
        );
        println!(
            "**** rpc_request_processor: process_request: msg - extnMessage:: {:?}",
            msg.clone()
        );

        // <pca>
        // let request = extracted_message.value;
        // let id = extracted_message.id;
        // </pca>

        tokio::spawn(async move {
            // ok - reiterate - in main =>  the router-state is initialized in PlatformState & update methods is called in the inititialization of the FireboltGateway
            // so how can I make it happen in sdk?

            // TBD on how to get the instance of the RouterState here ?
            // self.router_state.lock().unwrap().update_methods(self.methods.clone()); = need to add &self in process_request method signature

            let router_state = state.router_state.clone();
            println!(
                "**** rpc_request_processor: process_request: router_state: {:?}",
                router_state
            );
            let req = RpcRequest::from(extracted_message.clone());
            println!(
                "**** rpc_request_processor: process_request:  routing req method: {}",
                req.method
            );
            let start = Utc::now().timestamp_millis();
            let resp = RpcRouter::resolve_route(req.clone(), &router_state).await;

            let status = match resp.clone() {
                Ok(msg) => {
                    println!(
                        "**** rpc_request_processor: process_request: routing req method: {} msg: {:?}",
                        req.method,
                        msg
                    );
                    if msg.is_error() {
                        msg.jsonrpc_msg
                    } else {
                        "0".into()
                    }
                }
                Err(e) => {
                    println!(
                        "**** rpc_request_processor: process_request: failed to resolve route for msg - extnMessage:: {:?}",
                        msg.clone()
                    );
                    format!("{}", e)
                }
            };

            // badger_info_rpc return RpcResult<BadgerDeviceInfo> as response
            // **** rpc_request_processor: process_request: routing req method: badger.info 
            // msg: ApiMessage { protocol: JsonRpc, jsonrpc_msg: "{\"jsonrpc\":\"2.0\",
            // \"result\":{\"privacySettings\":{}},\"id\":2}", 
            // request_id: "169d72d6-561f-4c58-b593-7f59d0a88467", stats: None }

            // if Self::respond(
            //     state.get_client().get_extn_client(),
            //     msg.clone(),
            //     ExtnResponse::JsonRpc(status.),
            // )
            // .await
            // .is_ok()
            // {
            //     return true;
            // }
            
            // Self::handle_error(state.get_client(), req, RippleError::ProcessorError).await

            // TBD: what to return? - need to return the response to the extn broker
            return true;
        });

        true
    }
}
