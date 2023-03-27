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
    api::gateway::rpc_gateway_api::RpcRequest,
    async_trait::async_trait,
    extn::{
        client::extn_processor::{
            DefaultExtnStreamer, ExtnRequestProcessor, ExtnStreamProcessor, ExtnStreamer,
        },
        extn_client_message::ExtnMessage,
    },
    tokio::sync::mpsc::Sender,
};

use crate::{
    firebolt::firebolt_gateway::FireboltGatewayCommand, service::extn::ripple_client::RippleClient,
};

/// Processor to service incoming RPC Requests used by extensions and other local rpc handlers for aliasing.
#[derive(Debug)]
pub struct RpcGatewayProcessor {
    client: RippleClient,
    streamer: DefaultExtnStreamer,
}

impl RpcGatewayProcessor {
    pub fn new(client: RippleClient) -> RpcGatewayProcessor {
        RpcGatewayProcessor {
            client,
            streamer: DefaultExtnStreamer::new(),
        }
    }
}

impl ExtnStreamProcessor for RpcGatewayProcessor {
    type STATE = RippleClient;
    type VALUE = RpcRequest;
    fn get_state(&self) -> Self::STATE {
        self.client.clone()
    }

    fn sender(&self) -> Sender<ExtnMessage> {
        self.streamer.sender()
    }

    fn receiver(&mut self) -> ripple_sdk::tokio::sync::mpsc::Receiver<ExtnMessage> {
        self.streamer.receiver()
    }
}

#[async_trait]
impl ExtnRequestProcessor for RpcGatewayProcessor {
    fn get_client(&self) -> ripple_sdk::extn::client::extn_client::ExtnClient {
        self.client.get_extn_client()
    }

    async fn process_request(state: Self::STATE, msg: ExtnMessage, _request: Self::VALUE) -> bool {
        // Notice how this processor is different from others where it doesnt respond to
        // Self::respond this processor delegates the request down
        // to the gateway which does more complex inter connected operations. The design for
        // Extn Processor is built in such a way to support transient processors which do not
        // necessarily need to provide response
        if let Err(e) = state
            .send_gateway_command(FireboltGatewayCommand::HandleRpcForExtn { msg: msg.clone() })
        {
            return Self::handle_error(state.get_extn_client(), msg, e).await;
        }
        true
    }
}
