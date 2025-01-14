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
    api::{gateway::rpc_gateway_api::{RpcRequest, ApiProtocol}, manifest::extn_manifest::ExtnManifest},
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
};
use jsonrpsee::core::server::rpc_module::Methods;
use async_trait::async_trait;

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
        RippleContract::JsonRpsee
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
        extn_msg: ExtnMessage,
        extracted_message: Self::VALUE,
    ) -> bool {
        println!(
            "**** rpc_request_processor: process_request: extracted_message: {:?}",
            extracted_message.clone()
        );
        println!(
            "**** rpc_request_processor: process_request: msg - extnMessage:: {:?}",
            extn_msg.clone()
        );

        // <pca>
        // let request = extracted_message.value;
        // let id = extracted_message.id;
        // </pca>

        let client = state.client.clone();
        tokio::spawn(async move {
            // ok - reiterate - in main =>  the router-state is initialized in PlatformState & update methods is called in the inititialization of the FireboltGateway
            // so how can I make it happen in sdk?

            let router_state = state.router_state.clone();
            println!(
                "**** rpc_request_processor: process_request: router_state: {:?}",
                router_state
            );
            let req = RpcRequest::from(extracted_message.clone());
            println!(
                "**** rpc_request_processor: process_request: req: {:?}",
                req.clone()
            );

            println!(
                "**** rpc_request_processor: process_request:  routing req method: {}",
                req.method
            );

            // RpcRouter::resolve_route(req.clone(), &router_state).await;

            // Route
            match req.clone().ctx.protocol {
                ApiProtocol::Extn => {
                    println!("**** rpc_request_processor: process_request: Routing Extn Protocol");
                    RpcRouter::route_extn_protocol(
                        &router_state,
                        req.clone(),
                        extn_msg,
                    )
                    .await;
                }
                _ => {
                    println!("**** rpc_request_processor: process_request: Routing others");
                    // if the websocket disconnects before the session is received this leads to an error
                    let res = RpcRouter::resolve_route(req.clone(), &router_state).await;
                    println!(
                        "**** rpc_request_processor: process_request: routing req method: {} res: {:?}",
                        req.method,
                        res
                    );
                    // TBD - how to return the response to the extn_broker ? 
                    // do we need session to send the response back to the extn_broker thru return_api_message_for_transport via bridge?
                }
            }

            // match resp {
            //     Ok(response_msg) => {
            //         println!(
            //             "**** rpc_request_processor: process_request: routing req method: {} msg: {:?}",
            //             req.method,
            //             response_msg
            //         );
            //         if response_msg.is_error() {
            //             Self::handle_error(client, extn_msg, RippleError::ProcessorError).await;
            //         } else {
            //             if let Ok(json_value) = serde_json::from_str(&response_msg.jsonrpc_msg) {
            //                 if Self::respond(client, extn_msg, ExtnResponse::Value(json_value))
            //                     .await
            //                     .is_ok()
            //                 {
            //                     return true;
            //                 }
            //             } else {
            //                 Self::handle_error(client, extn_msg, RippleError::ProcessorError).await;
            //             }
            //         }
            //     }
            //     Err(e) => {
            //         println!(
            //             "**** rpc_request_processor: process_request: failed to resolve route for msg - extnMessage:: {:?}",
            //             extn_msg
            //         );
            //         Self::handle_error(client, extn_msg, RippleError::ProcessorError).await;
            //     }
            // };

            // badger_info_rpc return RpcResult<BadgerDeviceInfo> as response
            // **** rpc_request_processor: process_request: routing req method: badger.info
            // msg: ApiMessage { protocol: JsonRpc, jsonrpc_msg: "{\"jsonrpc\":\"2.0\",
            // \"result\":{\"privacySettings\":{}},\"id\":2}",
            // request_id: "169d72d6-561f-4c58-b593-7f59d0a88467", stats: None }

            true
        });

        true
    }
}
