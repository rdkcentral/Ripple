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

use jaq_interpret::{Ctx, FilterT, ParseCtx, RcIter, Val};
use ripple_sdk::{
    api::{
        firebolt::fb_capabilities::JSON_RPC_STANDARD_ERROR_INVALID_PARAMS,
        gateway::rpc_gateway_api::{ApiMessage, CallContext, JsonRpcApiResponse, RpcRequest},
        session::AccountSession,
    },
    framework::RippleResponse,
    log::error,
    tokio::{
        self,
        sync::mpsc::{Receiver, Sender},
    },
    utils::error::RippleError,
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

use crate::{firebolt::firebolt_gateway::JsonRpcError, state::platform_state::PlatformState};

use super::{
    http_broker::HttpBroker,
    rules_engine::{Rule, RuleEndpoint, RuleEndpointProtocol, RuleEngine},
    thunder_broker::ThunderBroker,
    websocket_broker::WebsocketBroker,
};

#[derive(Clone, Debug)]
pub struct BrokerSender {
    pub sender: Sender<BrokerRequest>,
}

#[derive(Clone, Debug)]
pub struct BrokerRequest {
    pub rpc: RpcRequest,
    pub rule: Rule,
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
    callback: BrokerCallback,
    request_map: Arc<RwLock<HashMap<u64, BrokerRequest>>>,
    rule_engine: RuleEngine,
}

impl EndpointBrokerState {
    pub fn get(tx: Sender<BrokerOutput>, rule_engine: RuleEngine) -> Self {
        Self {
            endpoint_map: Arc::new(RwLock::new(HashMap::new())),
            callback: BrokerCallback { sender: tx },
            request_map: Arc::new(RwLock::new(HashMap::new())),
            rule_engine,
        }
    }

    fn get_request(&self, id: u64) -> Result<BrokerRequest, RippleError> {
        if let Some(v) = self.request_map.read().unwrap().get(&id).cloned() {
            // cleanup if the request is not a subscription
            Ok(v)
        } else {
            Err(RippleError::InvalidInput)
        }
    }

    fn update_request(&self, rpc_request: &RpcRequest, rule: Rule) -> BrokerRequest {
        ATOMIC_ID.fetch_add(1, Ordering::Relaxed);
        let id = ATOMIC_ID.load(Ordering::Relaxed);
        let mut rpc_request_c = rpc_request.clone();
        {
            let mut request_map = self.request_map.write().unwrap();
            let _ = request_map.insert(
                id,
                BrokerRequest {
                    rpc: rpc_request.clone(),
                    rule: rule.clone(),
                },
            );
        }

        rpc_request_c.ctx.call_id = id;
        self.get_broker_request(&rpc_request_c, rule)
    }

    pub fn build_thunder_endpoint(&mut self) {
        if let Some(endpoint) = self.rule_engine.rules.endpoints.get("thunder").cloned() {
            let mut endpoint_map = self.endpoint_map.write().unwrap();
            endpoint_map.insert(
                "thunder".to_owned(),
                ThunderBroker::get_broker(endpoint.clone(), self.callback.clone()).get_sender(),
            );
        }
    }

    pub fn build_other_endpoints(&mut self, session: Option<AccountSession>) {
        for (key, endpoint) in self.rule_engine.rules.endpoints.clone() {
            match endpoint.protocol {
                RuleEndpointProtocol::Http => {
                    let mut endpoint_map = self.endpoint_map.write().unwrap();
                    endpoint_map.insert(
                        key,
                        HttpBroker::get_broker_with_session(
                            endpoint.clone(),
                            self.callback.clone(),
                            session.clone(),
                        )
                        .get_sender(),
                    );
                }
                RuleEndpointProtocol::Websocket => {
                    let mut endpoint_map = self.endpoint_map.write().unwrap();
                    endpoint_map.insert(
                        key,
                        WebsocketBroker::get_broker(endpoint.clone(), self.callback.clone())
                            .get_sender(),
                    );
                }
                _ => {}
            }
        }
    }

    fn get_sender(&self, hash: &str) -> Option<BrokerSender> {
        {
            self.endpoint_map.read().unwrap().get(hash).cloned()
        }
    }

    /// Main handler method whcih checks for brokerage and then sends the request for
    /// asynchronous processing
    pub fn handle_brokerage(&self, rpc_request: RpcRequest) -> bool {
        let callback = self.callback.clone();
        let mut broker_sender = None;
        let mut found_rule = None;
        if let Some(rule) = self.rule_engine.get_rule(&rpc_request) {
            let _ = found_rule.insert(rule.clone());
            if let Some(endpoint) = rule.endpoint {
                if let Some(endpoint) = self.get_sender(&endpoint) {
                    let _ = broker_sender.insert(endpoint);
                }
            } else if let Some(endpoint) = self.get_sender("thunder") {
                let _ = broker_sender.insert(endpoint);
            }
        }

        if broker_sender.is_none() || found_rule.is_none() {
            return false;
        }
        let rule = found_rule.unwrap();
        let broker = broker_sender.unwrap();
        let updated_request = self.update_request(&rpc_request, rule);
        tokio::spawn(async move {
            if let Err(e) = broker.send(updated_request.clone()).await {
                // send some rpc error
                callback.send_error(updated_request, e).await
            }
        });
        true
    }

    pub fn get_rule(&self, rpc_request: &RpcRequest) -> Option<Rule> {
        self.rule_engine.get_rule(rpc_request)
    }

    // Get Broker Request from rpc_request
    pub fn get_broker_request(&self, rpc_request: &RpcRequest, rule: Rule) -> BrokerRequest {
        BrokerRequest {
            rpc: rpc_request.clone(),
            rule,
        }
    }
}

/// Trait which contains all the abstract methods for a Endpoint Broker
/// There could be Websocket or HTTP protocol implementations of the given trait
pub trait EndpointBroker {
    fn get_broker_with_session(
        endpoint: RuleEndpoint,
        callback: BrokerCallback,
        _: Option<AccountSession>,
    ) -> Self
    where
        Self: Sized,
    {
        Self::get_broker(endpoint, callback)
    }

    fn get_broker(endpoint: RuleEndpoint, callback: BrokerCallback) -> Self;
    fn get_sender(&self) -> BrokerSender;

    fn prepare_request(&self, rpc_request: &BrokerRequest) -> Result<Vec<String>, RippleError> {
        let response = Self::update_request(rpc_request)?;
        Ok(vec![response])
    }

    /// Adds BrokerContext to a given request used by the Broker Implementations
    /// just before sending the data through the protocol
    fn update_request(rpc_request: &BrokerRequest) -> Result<String, RippleError> {
        if let Ok(v) = Self::apply_rule(rpc_request) {
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
    fn apply_rule(rpc_request: &BrokerRequest) -> Result<Value, RippleError> {
        if let Ok(mut params) = serde_json::from_str::<Vec<Value>>(&rpc_request.rpc.params_json) {
            if let Some(last) = params.pop() {
                if let Some(filter) = rpc_request
                    .rule
                    .transform
                    .get_filter(super::rules_engine::RuleTransformType::Request)
                {
                    return jq_compile(last, &filter);
                }
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
                let mut is_event = false;
                let id = if let Some(e) = v.get_event() {
                    is_event = true;
                    Some(e)
                } else {
                    v.data.id
                };
                if let Some(id) = id {
                    if let Ok(broker_request) = platform_state.endpoint_state.get_request(id) {
                        let rpc_request = broker_request.rpc;
                        let session_id = rpc_request.ctx.get_id();
                        if let Some(session) = platform_state
                            .session_state
                            .get_session_for_connection_id(&session_id)
                        {
                            let request_id = rpc_request.ctx.call_id;
                            v.data.id = Some(request_id);
                            if let Some(result) = v.data.result.clone() {
                                if is_event {
                                    if let Some(filter) = broker_request
                                        .rule
                                        .transform
                                        .get_filter(super::rules_engine::RuleTransformType::Event)
                                    {
                                        if let Ok(r) = jq_compile(result, &filter) {
                                            v.data.result = Some(r);
                                        }
                                    }
                                } else if let Some(filter) = broker_request
                                    .rule
                                    .transform
                                    .get_filter(super::rules_engine::RuleTransformType::Response)
                                {
                                    if let Ok(r) = jq_compile(result, &filter) {
                                        v.data.result = Some(r);
                                    }
                                }
                            }
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
                    error!("Error couldnt broker the event {:?}", v)
                }
            }
        });
    }
}

fn jq_compile(input: Value, filter: &str) -> Result<Value, RippleError> {
    println!("Jq rule {}  input {:?}", filter, input);
    // start out only from core filters,
    // which do not include filters in the standard library
    // such as `map`, `select` etc.
    let mut defs = ParseCtx::new(Vec::new());

    // parse the filter
    let (f, errs) = jaq_parse::parse(filter, jaq_parse::main());
    assert_eq!(errs, Vec::new());

    // compile the filter in the context of the given definitions
    let f = defs.compile(f.unwrap());
    assert!(defs.errs.is_empty());

    let inputs = RcIter::new(core::iter::empty());

    // iterator over the output values
    let mut out = f.run((Ctx::new([], &inputs), Val::from(input)));

    if let Some(Ok(v)) = out.next() {
        return Ok(Value::from(v));
    }
    Err(RippleError::ParseError)
}

#[cfg(test)]
mod tests {
    use ripple_sdk::{tokio::sync::mpsc::channel, Mockable};

    use crate::broker::rules_engine::RuleTransform;

    use super::*;
    mod endpoint_broker {
        use ripple_sdk::{api::gateway::rpc_gateway_api::RpcRequest, Mockable};

        #[test]
        fn test_update_context() {
            let _request = RpcRequest::mock();

            // if let Ok(v) = WebsocketBroker::add_context(&request) {
            //     println!("_ctx {}", v);
            //     //assert!(v.get("_ctx").unwrap().as_u64().unwrap().eq(&1));
            // }
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
                    rule: Rule {
                        alias: "somecallsign.method".to_owned(),
                        transform: RuleTransform::default(),
                        endpoint: None,
                    },
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

        use crate::broker::rules_engine::{Rule, RuleEngine, RuleSet, RuleTransform};

        use super::EndpointBrokerState;

        #[test]
        fn get_request() {
            let (tx, _) = channel(2);
            let state = EndpointBrokerState::get(
                tx,
                RuleEngine {
                    rules: RuleSet::default(),
                },
            );
            let mut request = RpcRequest::mock();
            state.update_request(
                &request,
                Rule {
                    alias: "somecallsign.method".to_owned(),
                    transform: RuleTransform::default(),
                    endpoint: None,
                },
            );
            request.ctx.call_id = 2;
            state.update_request(
                &request,
                Rule {
                    alias: "somecallsign.method".to_owned(),
                    transform: RuleTransform::default(),
                    endpoint: None,
                },
            );
            assert!(state.get_request(2).is_ok());
            assert!(state.get_request(1).is_ok());
        }
    }
}