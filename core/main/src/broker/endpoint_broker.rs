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

use std::collections::HashMap;

use jsonrpsee::types::TwoPointZero;
use ripple_sdk::{
    api::{
        gateway::rpc_gateway_api::{CallContext, JsonRpcApiResponse, RpcRequest, ApiMessage},
        manifest::extn_manifest::{PassthroughEndpoint, PassthroughProtocol}, firebolt::fb_capabilities::JSON_RPC_STANDARD_ERROR_INVALID_PARAMS,
    },
    framework::RippleResponse,
    log::error,
    tokio::{
        self,
        sync::mpsc::{Sender, Receiver},
    },
    utils::error::RippleError,
    uuid::Uuid,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

use crate::{utils::rpc_utils::{get_base_method, is_wildcard_method}, state::platform_state::PlatformState, firebolt::firebolt_gateway::{JsonRpcMessage, JsonRpcError}};

use super::websocket_broker::WebsocketBroker;

#[derive(Clone, Debug)]
pub struct BrokerSender {
    pub sender: Sender<RpcRequest>,
}

/// BrokerCallback will be used by the communication broker to send the firebolt response
/// back to the gateway for client consumption
#[derive(Clone, Debug)]
pub struct BrokerCallback {
    sender: Sender<BrokerOutput>,
}

impl BrokerCallback {

    /// Default method used for sending errors via the BrokerCallback
    async fn send_error(&self, request: RpcRequest, error: RippleError) {
        let err = JsonRpcMessage {
            jsonrpc: TwoPointZero {},
            id: request.ctx.call_id,
            error: Some(JsonRpcError {
                code: JSON_RPC_STANDARD_ERROR_INVALID_PARAMS,
                message: format!("Error with {:?}",error) ,
                data: None,
            }),
        };
        let msg = serde_json::to_value(&err).unwrap();
        let id = Some(request.ctx.call_id);
        if let Some(cid) = request.ctx.cid {
            let output = BrokerOutput {
                context: BrokerContext {
                    id,
                    cid
                },
                data: msg
            };
            if let Err(e) = self.sender.send(output).await {
                error!("couldnt send error for {:?}", e);
            }
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BrokerContext {
    pub id: Option<u64>,
    pub cid: String,
}

#[derive(Clone, Debug)]
pub struct BrokerOutput {
    pub context: BrokerContext,
    pub data: Value,
}

impl From<CallContext> for BrokerContext {
    fn from(value: CallContext) -> Self {
        Self {
            id: Some(value.call_id),
            cid: value.get_id(),
        }
    }
}

impl BrokerSender {
    // Method to send the request to the underlying broker for handling.
    pub async fn send(&self, request: RpcRequest) -> RippleResponse {
        if let Err(e) = self.sender.send(request).await {
            error!("Error sending to broker {:?}", e);
            Err(RippleError::SendFailure)
        } else {
            Ok(())
        }
    }
}

#[derive(Debug, Clone)]
pub struct EndpointBrokerState {
    endpoint_map: HashMap<String, BrokerSender>,
    rpc_hash: HashMap<String, String>,
    callback: BrokerCallback,
}

impl EndpointBrokerState {
    pub fn get(tx: Sender<BrokerOutput>) -> Self {
        Self {
            endpoint_map: HashMap::new(),
            rpc_hash: HashMap::new(),
            callback: BrokerCallback { sender: tx },
        }
    }

    /// Method which sets up the broker from the manifests
    pub fn add_endpoint_broker(&mut self, endpoint: &PassthroughEndpoint) {
        match &endpoint.protocol {
            PassthroughProtocol::Websocket => {
                let uuid = Uuid::new_v4().to_string();
                for rpc in &endpoint.rpcs {
                    if let Some(base_method) = is_wildcard_method(&rpc) {
                        self.rpc_hash.insert(base_method, uuid.clone());
                    } else {
                        self.rpc_hash.insert(rpc.clone(), uuid.clone());
                    }
                }
                self.endpoint_map.insert(
                    uuid,
                    WebsocketBroker::get_broker(endpoint.clone(), self.callback.clone()).get_sender(),
                );
            }
            _ => {}
        }
    }

    fn get_sender(&self, hash: &str) -> Option<BrokerSender> {
        self.endpoint_map.get(hash).cloned()
    }

    /// Critical method which checks if the given method is brokered or
    /// provided by Ripple Implementation
    fn brokered_method(&self, method: &str) -> Option<BrokerSender> {
        if let Some(hash) = self.rpc_hash.get(&get_base_method(method)) {
            self.get_sender(hash)
        } else if let Some(hash) = self.rpc_hash.get(method) {
            self.get_sender(hash)
        } else {
            None
        }
    }

    /// Main handler method whcih checks for brokerage and then sends the request for
    /// asynchronous processing
    pub fn handle_brokerage(&self, rpc_request: RpcRequest) -> bool {
        let callback = self.callback.clone();
        if let Some(broker) = self.brokered_method(&rpc_request.method) {
            tokio::spawn(async move {
                if let Err(e) = broker.send(rpc_request.clone()).await {
                    // send some rpc error
                    callback.send_error(rpc_request, e).await
                }
            });
            true
        } else {
            false
        }
    }
}

/// Trait which contains all the abstract methods for a Endpoint Broker
/// There could be Websocket or HTTP protocol implementations of the given trait
pub trait EndpointBroker {
    fn get_broker(endpoint: PassthroughEndpoint, callback: BrokerCallback) -> Self;
    fn get_sender(&self) -> BrokerSender;

    /// Adds BrokerContext to a given request used by the Broker Implementations
    /// just before sending the data through the protocol
    fn update_request(rpc_request: &RpcRequest) -> Result<String, RippleError> {
        if let Ok(v) = Self::add_context(&rpc_request) {
            let id = rpc_request.ctx.request_id.parse::<u64>().unwrap();
            let method = rpc_request.ctx.method.clone();
            return Ok(json!({
                "jsonrpc": 2.0,
                "id": id,
                "method": method,
                "params": v
            })
            .to_string());
        }
        Err(RippleError::MissingInput)
    }

    /// Generic method which takes the given parameters from RPC request and adds context
    fn add_context(rpc_request: &RpcRequest) -> Result<Value, RippleError> {
        if let Ok(mut params) = serde_json::from_str::<Map<String, Value>>(&rpc_request.params_json) {
            let context: BrokerContext = rpc_request.clone().ctx.into();
            params.insert("_ctx".into(), serde_json::to_value(context).unwrap());
            return Ok(serde_json::to_value(&params).unwrap());
        }
        Err(RippleError::ParseError)
    }

    /// Removes the context from the broker and sends the filtered response back to the client for 
    /// consumption
    fn strip_context(data: &Value) -> Result<BrokerOutput, RippleError> {
        if let Ok(mut v) = serde_json::from_value::<Map<String, Value>>(data.clone()) {
            if let Some((_, context)) = v.remove_entry("_ctx") {
                if let Ok(context) = serde_json::from_value::<BrokerContext>(context) {
                    return Ok(BrokerOutput {
                        context,
                        data: serde_json::to_value(v).unwrap(),
                    });
                }
            }
        }
        Err(RippleError::ParseError)
    }

    /// Default handler method for the broker to remove the context and send it back to the
    /// client for consumption
    fn handle_response(result: &str, callback: BrokerCallback) {
        let mut final_result = Err(RippleError::ParseError);
        if let Ok(response) = serde_json::from_str::<JsonRpcApiResponse>(result) {
            if let Some(r) = &response.result {
                final_result = Self::strip_context(r)
            } else if let Some(e) = &response.error {
                final_result = Self::strip_context(e)
            }
        }
        if let Ok(output) = final_result {
            tokio::spawn(async move { callback.sender.send(output).await });
        } else {
            error!("Bad broker response {}", result)
        }
    }
}


/// Forwarder gets the BrokerOutput and forwards the response to the gateway.
pub struct BrokerOutputForwarder;

impl BrokerOutputForwarder {
    pub fn start_forwarder(platform_state:PlatformState, mut rx: Receiver<BrokerOutput>) {
        tokio::spawn(async move {
            while let Some(v) = rx.recv().await {
                let context = v.context;
                let session_id = context.cid;
                if let Some(session) = platform_state.session_state.get_session_for_connection_id(&session_id) {
                    if let Some(request_id) = context.id {
                        let message = ApiMessage {
                            request_id: request_id.to_string(),
                            // by default it only supports JsonRpc
                            protocol: ripple_sdk::api::gateway::rpc_gateway_api::ApiProtocol::JsonRpc,
                            jsonrpc_msg: v.data.to_string()
                        };
                        if let Err(e) = session.send_json_rpc(message).await {
                            error!("Error while responding back message {:?}", e)
                        }
                    }
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    mod endpoint_broker{
        use ripple_sdk::api::gateway::rpc_gateway_api::{RpcRequest, CallContext};

        use crate::broker::{endpoint_broker::EndpointBroker, websocket_broker::WebsocketBroker};


        #[test]
        fn test_update_context() {
            let request = RpcRequest{
                method: "module.method".to_owned(),
                params_json: "{}".to_owned(),
                ctx: CallContext {
                    session_id: "session_id".to_owned(),
                    request_id: "1".to_owned(),
                    app_id: "some_app_id".to_owned(),
                    call_id: 1,
                    protocol: ripple_sdk::api::gateway::rpc_gateway_api::ApiProtocol::JsonRpc,
                    method: "module.method".to_owned(),
                    cid: Some("cid".to_owned()),
                    gateway_secure: true
                }
            };

            if let Ok(v) = WebsocketBroker::add_context(&request) {
                println!("_ctx {}", v.to_string());
                //assert!(v.get("_ctx").unwrap().as_u64().unwrap().eq(&1));
            }
        }
    }
}