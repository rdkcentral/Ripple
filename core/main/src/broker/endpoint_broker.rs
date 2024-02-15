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
    api::{
        firebolt::fb_capabilities::JSON_RPC_STANDARD_ERROR_INVALID_PARAMS,
        gateway::rpc_gateway_api::{ApiMessage, CallContext, JsonRpcApiResponse, RpcRequest},
        manifest::extn_manifest::{
            PassthroughEndpoint, PassthroughProtocol, PassthroughTransformer,
        },
        session::AccountSession,
    },
    framework::RippleResponse,
    log::error,
    tokio::{
        self,
        sync::mpsc::{Receiver, Sender},
    },
    utils::error::RippleError,
    uuid::Uuid,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, RwLock,
    },
};

use crate::{
    firebolt::firebolt_gateway::JsonRpcError,
    state::platform_state::PlatformState,
    utils::rpc_utils::{get_base_method, is_wildcard_method},
};

use super::{
    http_broker::HttpBroker, thunder_broker::ThunderBroker, websocket_broker::WebsocketBroker,
};

#[derive(Clone, Debug)]
pub struct BrokerSender {
    pub sender: Sender<BrokerRequest>,
}

#[derive(Clone, Debug)]
pub struct BrokerRequest {
    pub rpc: RpcRequest,
    pub transformer: Option<PassthroughTransformer>,
}

/// BrokerCallback will be used by the communication broker to send the firebolt response
/// back to the gateway for client consumption
#[derive(Clone, Debug)]
pub struct BrokerCallback {
    pub sender: Sender<BrokerOutput>,
}

static ATOMIC_ID: AtomicU64 = AtomicU64::new(0);

impl BrokerCallback {
    /// Default method used for sending errors via the BrokerCallback
    async fn send_error(&self, request: BrokerRequest, error: RippleError) {
        let value = serde_json::to_value(JsonRpcError {
            code: JSON_RPC_STANDARD_ERROR_INVALID_PARAMS,
            message: format!("Error with {:?}", error),
            data: None,
        })
        .unwrap();
        let data = JsonRpcApiResponse {
            jsonrpc: "2.0".to_owned(),
            id: Some(request.rpc.ctx.call_id),
            error: Some(value),
            result: None,
            method: None,
            params: None,
        };
        let output = BrokerOutput { data };
        if let Err(e) = self.sender.send(output).await {
            error!("couldnt send error for {:?}", e);
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BrokerContext {
    pub app_id: String,
}

#[derive(Debug, Clone)]
pub struct BrokerOutput {
    pub data: JsonRpcApiResponse,
}

impl BrokerOutput {
    pub fn is_result(&self) -> bool {
        self.data.result.is_some()
    }

    pub fn get_event(&self) -> Option<u64> {
        if let Some(e) = &self.data.method {
            let event: Vec<&str> = e.split('.').collect();
            if let Some(v) = event.first() {
                if let Ok(r) = v.parse::<u64>() {
                    return Some(r);
                }
            }
        }
        None
    }
}

impl From<CallContext> for BrokerContext {
    fn from(value: CallContext) -> Self {
        Self {
            app_id: value.app_id,
        }
    }
}

impl BrokerSender {
    // Method to send the request to the underlying broker for handling.
    pub async fn send(&self, request: BrokerRequest) -> RippleResponse {
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
    endpoint_map: Arc<RwLock<HashMap<String, BrokerSender>>>,
    rpc_hash: Arc<RwLock<HashMap<String, String>>>,
    callback: BrokerCallback,
    request_map: Arc<RwLock<HashMap<u64, RpcRequest>>>,
    transformer_map: Arc<RwLock<HashMap<String, PassthroughTransformer>>>,
}

impl EndpointBrokerState {
    pub fn get(tx: Sender<BrokerOutput>) -> Self {
        Self {
            endpoint_map: Arc::new(RwLock::new(HashMap::new())),
            rpc_hash: Arc::new(RwLock::new(HashMap::new())),
            callback: BrokerCallback { sender: tx },
            request_map: Arc::new(RwLock::new(HashMap::new())),
            transformer_map: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    fn get_request(&self, id: u64) -> Result<RpcRequest, RippleError> {
        if let Some(v) = self.request_map.read().unwrap().get(&id).cloned() {
            // cleanup if the request is not a subscription
            Ok(v)
        } else {
            Err(RippleError::InvalidInput)
        }
    }

    fn update_request(&self, rpc_request: &RpcRequest) -> BrokerRequest {
        ATOMIC_ID.fetch_add(1, Ordering::Relaxed);
        let id = ATOMIC_ID.load(Ordering::Relaxed);
        let mut rpc_request_c = rpc_request.clone();
        {
            let mut request_map = self.request_map.write().unwrap();
            let _ = request_map.insert(id, rpc_request.clone());
        }

        rpc_request_c.ctx.call_id = id;
        self.get_broker_request(&rpc_request_c)
    }

    /// Method which sets up the broker from the manifests
    pub fn add_endpoint_broker(
        &mut self,
        endpoint: &PassthroughEndpoint,
        session: Option<AccountSession>,
    ) {
        let uuid = Uuid::new_v4().to_string();
        for rpc in &endpoint.rpcs {
            if let Some(base_method) = is_wildcard_method(rpc) {
                {
                    let mut rpc_hash = self.rpc_hash.write().unwrap();
                    rpc_hash.insert(base_method, uuid.clone());
                }
            } else {
                {
                    let mut rpc_hash = self.rpc_hash.write().unwrap();
                    rpc_hash.insert(rpc.clone().matcher.to_lowercase(), uuid.clone());
                }
            }

            if let Some(transformer) = &rpc.transformer {
                let updated_key_transformer: HashMap<String, PassthroughTransformer> = transformer
                    .iter()
                    .map(|(k, v)| (k.to_lowercase(), v.clone()))
                    .collect();
                {
                    self.transformer_map
                        .write()
                        .unwrap()
                        .extend(updated_key_transformer);
                }
            }
        }
        match &endpoint.protocol {
            PassthroughProtocol::Websocket => {
                let mut endpoint_map = self.endpoint_map.write().unwrap();
                endpoint_map.insert(
                    uuid,
                    WebsocketBroker::get_broker(session, endpoint.clone(), self.callback.clone())
                        .get_sender(),
                );
            }
            PassthroughProtocol::Http => {
                let mut endpoint_map = self.endpoint_map.write().unwrap();
                endpoint_map.insert(
                    uuid,
                    HttpBroker::get_broker(session, endpoint.clone(), self.callback.clone())
                        .get_sender(),
                );
            }
            PassthroughProtocol::Thunder => {
                let mut endpoint_map = self.endpoint_map.write().unwrap();
                endpoint_map.insert(
                    uuid,
                    ThunderBroker::get_broker(session, endpoint.clone(), self.callback.clone())
                        .get_sender(),
                );
            }
        }
    }

    fn get_sender(&self, hash: &str) -> Option<BrokerSender> {
        {
            self.endpoint_map.read().unwrap().get(hash).cloned()
        }
    }

    fn get_hash(&self, hash: &str) -> Option<String> {
        {
            self.rpc_hash.read().unwrap().get(hash).cloned()
        }
    }

    /// Critical method which checks if the given method is brokered or
    /// provided by Ripple Implementation
    fn brokered_method(&self, method: &str) -> Option<BrokerSender> {
        let method_lower_case = method.to_lowercase();
        if let Some(hash) = self.get_hash(&get_base_method(&method_lower_case)) {
            self.get_sender(&hash)
        } else if let Some(hash) = self.get_hash(&method_lower_case) {
            self.get_sender(&hash)
        } else {
            None
        }
    }

    /// Main handler method whcih checks for brokerage and then sends the request for
    /// asynchronous processing
    pub fn handle_brokerage(&self, rpc_request: RpcRequest) -> bool {
        let callback = self.callback.clone();
        if let Some(broker) = self.brokered_method(&rpc_request.method) {
            let updated_request = self.update_request(&rpc_request);
            tokio::spawn(async move {
                if let Err(e) = broker.send(updated_request.clone()).await {
                    // send some rpc error
                    callback.send_error(updated_request, e).await
                }
            });
            true
        } else {
            false
        }
    }

    // Get the transformer(if any) for a given method
    pub fn get_transformer(&self, rpc_request: &RpcRequest) -> Option<PassthroughTransformer> {
        self.transformer_map
            .read()
            .unwrap()
            .get(&rpc_request.method.to_lowercase())
            .cloned()
    }

    // Get Broker Request from rpc_request
    pub fn get_broker_request(&self, rpc_request: &RpcRequest) -> BrokerRequest {
        BrokerRequest {
            rpc: rpc_request.clone(),
            transformer: self.get_transformer(rpc_request),
        }
    }
}

/// Trait which contains all the abstract methods for a Endpoint Broker
/// There could be Websocket or HTTP protocol implementations of the given trait
pub trait EndpointBroker {
    fn get_broker(
        session: Option<AccountSession>,
        endpoint: PassthroughEndpoint,
        callback: BrokerCallback,
    ) -> Self;
    fn get_sender(&self) -> BrokerSender;

    fn prepare_request(&self, rpc_request: &BrokerRequest) -> Result<Vec<String>, RippleError> {
        let response = Self::update_request(rpc_request)?;
        Ok(vec![response])
    }

    /// Adds BrokerContext to a given request used by the Broker Implementations
    /// just before sending the data through the protocol
    fn update_request(rpc_request: &BrokerRequest) -> Result<String, RippleError> {
        if let Ok(v) = Self::add_context(&rpc_request.rpc) {
            let id = rpc_request.rpc.ctx.call_id;
            let method = rpc_request.rpc.ctx.method.clone();
            return Ok(json!({
                "jsonrpc": "2.0",
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
        if let Ok(params) =
            serde_json::from_str::<Vec<HashMap<String, Value>>>(&rpc_request.params_json)
        {
            if let Some(mut last) = params.last().cloned() {
                let context: BrokerContext = rpc_request.clone().ctx.into();
                let _ = last.insert("_ctx".into(), serde_json::to_value(context).unwrap());
                return Ok(serde_json::to_value(&last).unwrap());
            }
        }
        Err(RippleError::ParseError)
    }

    /// Default handler method for the broker to remove the context and send it back to the
    /// client for consumption
    fn handle_response(result: &[u8], callback: BrokerCallback) {
        let mut final_result = Err(RippleError::ParseError);
        if let Ok(data) = serde_json::from_slice::<JsonRpcApiResponse>(result) {
            final_result = Ok(BrokerOutput { data });
        }
        if let Ok(output) = final_result {
            tokio::spawn(async move { callback.sender.send(output).await });
        } else {
            error!("Bad broker response {}", String::from_utf8_lossy(result));
        }
    }
}

/// Forwarder gets the BrokerOutput and forwards the response to the gateway.
pub struct BrokerOutputForwarder;

impl BrokerOutputForwarder {
    pub fn start_forwarder(platform_state: PlatformState, mut rx: Receiver<BrokerOutput>) {
        tokio::spawn(async move {
            while let Some(mut v) = rx.recv().await {
                let id = if let Some(e) = v.get_event() {
                    Some(e)
                } else {
                    v.data.id
                };
                if let Some(id) = id {
                    if let Ok(rpc_request) = platform_state.endpoint_state.get_request(id) {
                        let session_id = rpc_request.ctx.get_id();
                        if let Some(session) = platform_state
                            .session_state
                            .get_session_for_connection_id(&session_id)
                        {
                            let request_id = rpc_request.ctx.call_id;
                            v.data.id = Some(request_id);
                            let message = ApiMessage {
                                request_id: request_id.to_string(),
                                protocol: rpc_request.ctx.protocol,
                                jsonrpc_msg: serde_json::to_string(&v.data).unwrap(),
                            };
                            if let Err(e) = session.send_json_rpc(message).await {
                                error!("Error while responding back message {:?}", e)
                            }
                        }
                    }
                } else {
                    error!("Error couldnt broker")
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use ripple_sdk::{tokio::sync::mpsc::channel, Mockable};

    use super::*;
    mod endpoint_broker {
        use ripple_sdk::{api::gateway::rpc_gateway_api::RpcRequest, Mockable};

        use crate::broker::{endpoint_broker::EndpointBroker, websocket_broker::WebsocketBroker};

        #[test]
        fn test_update_context() {
            let request = RpcRequest::mock();

            if let Ok(v) = WebsocketBroker::add_context(&request) {
                println!("_ctx {}", v);
                //assert!(v.get("_ctx").unwrap().as_u64().unwrap().eq(&1));
            }
        }
    }

    #[tokio::test]
    async fn test_send_error() {
        let (tx, mut tr) = channel(2);
        let callback = BrokerCallback { sender: tx };

        callback
            .send_error(
                BrokerRequest {
                    rpc: RpcRequest::mock(),
                    transformer: None,
                },
                RippleError::InvalidInput,
            )
            .await;
        let value = tr.recv().await.unwrap();
        assert!(value.data.error.is_some())
    }

    mod broker_output {
        use ripple_sdk::{api::gateway::rpc_gateway_api::JsonRpcApiResponse, Mockable};

        use crate::broker::endpoint_broker::BrokerOutput;

        #[test]
        fn test_result() {
            let mut data = JsonRpcApiResponse::mock();
            let output = BrokerOutput { data: data.clone() };
            assert!(!output.is_result());
            data.result = Some(serde_json::Value::Null);
            let output = BrokerOutput { data };
            assert!(output.is_result());
        }

        #[test]
        fn test_get_event() {
            let mut data = JsonRpcApiResponse::mock();
            data.method = Some("20.events".to_owned());
            let output = BrokerOutput { data };
            assert_eq!(20, output.get_event().unwrap())
        }
    }

    mod endpoint_broker_state {
        use ripple_sdk::{
            api::gateway::rpc_gateway_api::RpcRequest, tokio::sync::mpsc::channel, Mockable,
        };

        use super::EndpointBrokerState;

        #[test]
        fn get_request() {
            let (tx, _) = channel(2);
            let state = EndpointBrokerState::get(tx);
            let mut request = RpcRequest::mock();
            state.update_request(&request);
            request.ctx.call_id = 2;
            state.update_request(&request);
            assert!(state.get_request(2).is_ok());
            assert!(state.get_request(1).is_ok());
        }
    }
}
