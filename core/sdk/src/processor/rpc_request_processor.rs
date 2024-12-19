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
    api::gateway::rpc_gateway_api::{ApiProtocol, RpcRequest},
    async_trait::async_trait,
    extn::{
        client::{
            extn_client::ExtnClient,
            extn_processor::{
                DefaultExtnStreamer, ExtnRequestProcessor, ExtnStreamProcessor, ExtnStreamer,
            },
            extn_sender::ExtnSender,
        },
        extn_client_message::ExtnMessage,
        extn_id::{ExtnClassId, ExtnId, ExtnProviderAdjective, ExtnProviderRequest},
    },
    framework::ripple_contract::RippleContract,
    tokio::sync::mpsc::{Receiver, Sender},
    // processor::rpc_router::RPCRouter,
};
use jsonrpsee::{
    core::server::rpc_module::{MethodKind, Methods},
    types::{error::ErrorCode, Id, Params},
};

const EXTN_NAME: &'static str = "badger";

/// Processor to service incoming RPC Requests used by extensions and other local rpc handlers for aliasing.
#[derive(Debug)]
pub struct RPCRequestProcessor {
    client: ExtnClient,
    methods: Methods,
    streamer: DefaultExtnStreamer,
}

impl RPCRequestProcessor {
    pub fn new(client: ExtnClient, methods: Methods) -> RPCRequestProcessor {
        RPCRequestProcessor {
            client,
            methods,
            streamer: DefaultExtnStreamer::new(),
        }
    }
}

impl ExtnStreamProcessor for RPCRequestProcessor {
    type STATE = ExtnClient;
    // <pca>
    //type VALUE = ExtnProviderRequest;
    type VALUE = RpcRequest;
    // </pca>
    fn get_state(&self) -> Self::STATE {
        self.client.clone()
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
        self.client.clone()
    }

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

        // let methods = state.get_methods();
        // let resources = Resources::default();

        tokio::spawn(async move {
            // if let Ok(msg) = RpcRouter::resolve_route(Self::methods, resources, req).await {
            //     return_extn_response(msg, extn_msg);
            // }
            println!(
                "**** rpc_request_processor: process_request: msg - extnMessage:: {:?}",
                msg.clone()
            );
        });

        true
    }
}
