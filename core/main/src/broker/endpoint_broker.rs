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
        gateway::rpc_gateway_api::{
            ApiMessage, ApiProtocol, CallContext, JsonRpcApiResponse, RpcRequest,
        },
        session::AccountSession,
    },
    extn::extn_client_message::{ExtnEvent, ExtnMessage},
    framework::RippleResponse,
    log::{error, trace},
    tokio::{
        self,
        sync::mpsc::{self, Receiver, Sender},
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

use crate::{
    firebolt::firebolt_gateway::{FireboltGatewayCommand, JsonRpcError},
    service::extn::ripple_client::RippleClient,
    state::platform_state::PlatformState,
    utils::router_utils::{return_api_message_for_transport, return_extn_response},
};

use super::{
    event_management_utility::EventManagementUtility,
    http_broker::HttpBroker,
    rules_engine::{jq_compile, Rule, RuleEndpoint, RuleEndpointProtocol, RuleEngine},
    thunder_broker::ThunderBroker,
    websocket_broker::WebsocketBroker,
};

#[derive(Clone, Debug)]
pub struct BrokerSender {
    pub sender: Sender<BrokerRequest>,
}

#[derive(Clone, Debug)]
pub struct BrokerCleaner {
    pub cleaner: Option<Sender<String>>,
}

impl BrokerCleaner {
    async fn cleanup_session(&self, appid: &str) {
        if let Some(cleaner) = self.cleaner.clone() {
            if let Err(e) = cleaner.send(appid.to_owned()).await {
                error!("Couldnt cleanup {} {:?}", appid, e)
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct BrokerRequest {
    pub rpc: RpcRequest,
    pub rule: Rule,
    pub subscription_processed: Option<bool>,
}

pub type BrokerSubMap = HashMap<String, Vec<BrokerRequest>>;

#[derive(Clone, Debug)]
pub struct BrokerConnectRequest {
    pub key: String,
    pub endpoint: RuleEndpoint,
    pub sub_map: BrokerSubMap,
    pub session: Option<AccountSession>,
    pub reconnector: Sender<BrokerConnectRequest>,
}

impl BrokerConnectRequest {
    pub fn new(
        key: String,
        endpoint: RuleEndpoint,
        reconnector: Sender<BrokerConnectRequest>,
    ) -> Self {
        Self {
            key,
            endpoint,
            sub_map: HashMap::new(),
            session: None,
            reconnector,
        }
    }

    pub fn new_with_sesssion(
        key: String,
        endpoint: RuleEndpoint,
        reconnector: Sender<BrokerConnectRequest>,
        session: Option<AccountSession>,
    ) -> Self {
        Self {
            key,
            endpoint,
            sub_map: HashMap::new(),
            session,
            reconnector,
        }
    }
}

impl BrokerRequest {
    pub fn is_subscription_processed(&self) -> bool {
        self.subscription_processed.is_some()
    }
}

impl BrokerRequest {
    pub fn new(rpc_request: &RpcRequest, rule: Rule) -> BrokerRequest {
        BrokerRequest {
            rpc: rpc_request.clone(),
            rule,
            subscription_processed: None,
        }
    }

    pub fn get_id(&self) -> String {
        self.rpc.ctx.session_id.clone()
    }
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
    pub async fn send_error(&self, request: BrokerRequest, error: RippleError) {
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
    extension_request_map: Arc<RwLock<HashMap<u64, ExtnMessage>>>,
    rule_engine: RuleEngine,
    cleaner_list: Arc<RwLock<Vec<BrokerCleaner>>>,
    reconnect_tx: Sender<BrokerConnectRequest>,
}

impl EndpointBrokerState {
    pub fn new(
        tx: Sender<BrokerOutput>,
        rule_engine: RuleEngine,
        ripple_client: RippleClient,
    ) -> Self {
        let (reconnect_tx, rec_tr) = mpsc::channel(2);
        let state = Self {
            endpoint_map: Arc::new(RwLock::new(HashMap::new())),
            callback: BrokerCallback { sender: tx },
            request_map: Arc::new(RwLock::new(HashMap::new())),
            extension_request_map: Arc::new(RwLock::new(HashMap::new())),
            rule_engine,
            cleaner_list: Arc::new(RwLock::new(Vec::new())),
            reconnect_tx,
        };
        state.reconnect_thread(rec_tr, ripple_client);
        state
    }

    fn reconnect_thread(&self, mut rx: Receiver<BrokerConnectRequest>, client: RippleClient) {
        let mut state = self.clone();
        tokio::spawn(async move {
            while let Some(v) = rx.recv().await {
                if matches!(v.endpoint.protocol, RuleEndpointProtocol::Thunder) {
                    if client
                        .send_gateway_command(FireboltGatewayCommand::StopServer)
                        .is_err()
                    {
                        error!("Stopping server")
                    }
                    break;
                } else {
                    state.build_endpoint(v)
                }
            }
        });
    }

    fn get_request(&self, id: u64) -> Result<BrokerRequest, RippleError> {
        let result = { self.request_map.read().unwrap().get(&id).cloned() };
        if result.is_none() {
            return Err(RippleError::InvalidInput);
        }

        let result = result.unwrap();
        if !result.rpc.is_subscription() {
            let _ = self.request_map.write().unwrap().remove(&id);
        }
        Ok(result)
    }

    fn update_unsubscribe_request(&self, id: u64) {
        let mut result = self.request_map.write().unwrap();
        if let Some(mut value) = result.remove(&id) {
            value.subscription_processed = Some(true);
            let _ = result.insert(id, value);
        }
    }

    fn get_extn_message(&self, id: u64, is_event: bool) -> Result<ExtnMessage, RippleError> {
        if is_event {
            let v = { self.extension_request_map.read().unwrap().get(&id).cloned() };
            if let Some(v1) = v {
                Ok(v1)
            } else {
                Err(RippleError::NotAvailable)
            }
        } else {
            let result = { self.extension_request_map.write().unwrap().remove(&id) };
            match result {
                Some(v) => Ok(v),
                None => Err(RippleError::NotAvailable),
            }
        }
    }

    pub fn get_next_id() -> u64 {
        ATOMIC_ID.fetch_add(1, Ordering::Relaxed);
        ATOMIC_ID.load(Ordering::Relaxed)
    }

    fn update_request(
        &self,
        rpc_request: &RpcRequest,
        rule: Rule,
        extn_message: Option<ExtnMessage>,
    ) -> (u64, BrokerRequest) {
        let id = Self::get_next_id();
        let mut rpc_request_c = rpc_request.clone();
        {
            let mut request_map = self.request_map.write().unwrap();
            let _ = request_map.insert(
                id,
                BrokerRequest {
                    rpc: rpc_request.clone(),
                    rule: rule.clone(),
                    subscription_processed: None,
                },
            );
        }

        if extn_message.is_some() {
            let mut extn_map = self.extension_request_map.write().unwrap();
            let _ = extn_map.insert(id, extn_message.unwrap());
        }

        rpc_request_c.ctx.call_id = id;
        (id, BrokerRequest::new(&rpc_request_c, rule))
    }

    pub fn build_thunder_endpoint(&mut self) {
        if let Some(endpoint) = self.rule_engine.rules.endpoints.get("thunder").cloned() {
            let request = BrokerConnectRequest::new(
                "thunder".to_owned(),
                endpoint.clone(),
                self.reconnect_tx.clone(),
            );
            self.build_endpoint(request);
        }
    }

    pub fn build_other_endpoints(&mut self, session: Option<AccountSession>) {
        for (key, endpoint) in self.rule_engine.rules.endpoints.clone() {
            let request = BrokerConnectRequest::new_with_sesssion(
                key,
                endpoint.clone(),
                self.reconnect_tx.clone(),
                session.clone(),
            );
            self.build_endpoint(request);
        }
    }

    fn build_endpoint(&mut self, request: BrokerConnectRequest) {
        let endpoint = request.endpoint.clone();
        let key = request.key.clone();
        let (broker, cleaner) = match endpoint.protocol {
            RuleEndpointProtocol::Http => (
                HttpBroker::get_broker(request, self.callback.clone()).get_sender(),
                None,
            ),
            RuleEndpointProtocol::Websocket => {
                let ws_broker = WebsocketBroker::get_broker(request, self.callback.clone());
                (ws_broker.get_sender(), Some(ws_broker.get_cleaner()))
            }
            RuleEndpointProtocol::Thunder => {
                let thunder_broker = ThunderBroker::get_broker(request, self.callback.clone());
                (
                    thunder_broker.get_sender(),
                    Some(thunder_broker.get_cleaner()),
                )
            }
        };

        {
            let mut endpoint_map = self.endpoint_map.write().unwrap();
            endpoint_map.insert(key, broker);
        }

        if let Some(cleaner) = cleaner {
            let mut cleaner_list = self.cleaner_list.write().unwrap();
            cleaner_list.push(cleaner);
        }
    }

    fn handle_static_request(
        &self,
        rpc_request: RpcRequest,
        extn_message: Option<ExtnMessage>,
        rule: Rule,
        callback: BrokerCallback,
    ) {
        let (id, _updated_request) = self.update_request(&rpc_request, rule.clone(), extn_message);
        let mut data = JsonRpcApiResponse::default();
        // return empty result and handle the rest with jq rule
        let jv: Value = "".into();
        data.result = Some(jv);
        data.id = Some(id);
        let output = BrokerOutput { data };
        tokio::spawn(async move { callback.sender.send(output).await });
    }

    fn get_sender(&self, hash: &str) -> Option<BrokerSender> {
        self.endpoint_map.read().unwrap().get(hash).cloned()
    }

    /// Main handler method whcih checks for brokerage and then sends the request for
    /// asynchronous processing
    pub fn handle_brokerage(
        &self,
        rpc_request: RpcRequest,
        extn_message: Option<ExtnMessage>,
    ) -> bool {
        let mut handled: bool = true;
        let callback = self.callback.clone();
        let mut broker_sender = None;
        let mut found_rule = None;
        if let Some(rule) = self.rule_engine.get_rule(&rpc_request) {
            let _ = found_rule.insert(rule.clone());
            if let Some(endpoint) = rule.endpoint {
                if let Some(endpoint) = self.get_sender(&endpoint) {
                    let _ = broker_sender.insert(endpoint);
                }
            } else if rule.alias != "static" {
                if let Some(endpoint) = self.get_sender("thunder") {
                    let _ = broker_sender.insert(endpoint);
                }
            }
        }

        if found_rule.is_some() {
            let rule = found_rule.unwrap();

            if rule.alias == "static" {
                self.handle_static_request(rpc_request, extn_message, rule, callback);
            } else if broker_sender.is_some() {
                let broker = broker_sender.unwrap();
                let (_, updated_request) = self.update_request(&rpc_request, rule, extn_message);
                tokio::spawn(async move {
                    if let Err(e) = broker.send(updated_request.clone()).await {
                        callback.send_error(updated_request, e).await
                    }
                });
            } else {
                handled = false;
            }
        } else {
            handled = false;
        }

        handled
    }

    pub fn get_rule(&self, rpc_request: &RpcRequest) -> Option<Rule> {
        self.rule_engine.get_rule(rpc_request)
    }

    // Method to cleanup all subscription on App termination
    pub async fn cleanup_for_app(&self, app_id: &str) {
        let cleaners = { self.cleaner_list.read().unwrap().clone() };
        for cleaner in cleaners {
            cleaner.cleanup_session(app_id).await
        }
    }

    // Get Broker Request from rpc_request
    pub fn get_broker_request(&self, rpc_request: &RpcRequest, rule: Rule) -> BrokerRequest {
        BrokerRequest {
            rpc: rpc_request.clone(),
            rule,
            subscription_processed: None,
        }
    }
}

/// Trait which contains all the abstract methods for a Endpoint Broker
/// There could be Websocket or HTTP protocol implementations of the given trait
pub trait EndpointBroker {
    fn get_broker(request: BrokerConnectRequest, callback: BrokerCallback) -> Self;

    fn get_sender(&self) -> BrokerSender;

    fn prepare_request(&self, rpc_request: &BrokerRequest) -> Result<Vec<String>, RippleError> {
        let response = Self::update_request(rpc_request)?;
        Ok(vec![response])
    }

    /// Adds BrokerContext to a given request used by the Broker Implementations
    /// just before sending the data through the protocol
    fn update_request(rpc_request: &BrokerRequest) -> Result<String, RippleError> {
        let v = Self::apply_request_rule(rpc_request)?;
        trace!("transformed request {:?}", v);
        let id = rpc_request.rpc.ctx.call_id;
        let method = rpc_request.rule.alias.clone();
        if let Value::Null = v {
            Ok(json!({
                "jsonrpc": "2.0",
                "id": id,
                "method": method
            })
            .to_string())
        } else {
            Ok(json!({
                "jsonrpc": "2.0",
                "id": id,
                "method": method,
                "params": v
            })
            .to_string())
        }
    }

    /// Generic method which takes the given parameters from RPC request and adds rules using rule engine
    fn apply_request_rule(rpc_request: &BrokerRequest) -> Result<Value, RippleError> {
        if let Ok(mut params) = serde_json::from_str::<Vec<Value>>(&rpc_request.rpc.params_json) {
            if params.len() > 1 {
                if let Some(last) = params.pop() {
                    if let Some(filter) = rpc_request
                        .rule
                        .transform
                        .get_transform_data(super::rules_engine::RuleTransformType::Request)
                    {
                        return jq_compile(
                            last,
                            &filter,
                            format!("{}_request", rpc_request.rpc.ctx.method),
                        );
                    }
                    return Ok(serde_json::to_value(&last).unwrap());
                }
            } else {
                return Ok(Value::Null);
            }
        }
        Err(RippleError::ParseError)
    }

    /// Default handler method for the broker to remove the context and send it back to the
    /// client for consumption
    fn handle_jsonrpc_response(result: &[u8], callback: BrokerCallback) {
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

    fn get_cleaner(&self) -> BrokerCleaner;
}

/// Forwarder gets the BrokerOutput and forwards the response to the gateway.
pub struct BrokerOutputForwarder;

impl BrokerOutputForwarder {
    pub fn start_forwarder(platform_state: PlatformState, mut rx: Receiver<BrokerOutput>) {
        // set up the event utility
        let event_utility = Arc::new(EventManagementUtility::new());
        event_utility.register_custom_functions();
        let event_utility_clone = event_utility.clone();

        tokio::spawn(async move {
            while let Some(output) = rx.recv().await {
                let output_c = output.clone();
                let mut response = output.data;
                let mut is_event = false;
                // First validate the id check if it could be an event
                let id = if let Some(e) = output_c.get_event() {
                    is_event = true;
                    Some(e)
                } else {
                    response.id
                };

                if let Some(id) = id {
                    if let Ok(broker_request) = platform_state.endpoint_state.get_request(id) {
                        let sub_processed = broker_request.is_subscription_processed();
                        let rpc_request = broker_request.rpc.clone();
                        let session_id = rpc_request.ctx.get_id();
                        let is_subscription = rpc_request.is_subscription();
                        let mut apply_response_needed = false;

                        // Step 1: Create the data
                        if let Some(result) = response.result.clone() {
                            if is_event {
                                apply_rule_for_event(
                                    &broker_request,
                                    &result,
                                    &rpc_request,
                                    &mut response,
                                );
                                if !apply_filter(&broker_request, &result, &rpc_request) {
                                    continue;
                                }
                                // check if the request transform has event_decorator_method
                                if let Some(decorator_method) =
                                    broker_request.rule.transform.event_decorator_method.clone()
                                {
                                    if let Some(func) =
                                        event_utility_clone.get_function(&decorator_method)
                                    {
                                        // spawn a tokio thread to run the function and continue the main thread.
                                        let session_id = rpc_request.ctx.get_id();
                                        let request_id = rpc_request.ctx.call_id;
                                        let protocol = rpc_request.ctx.protocol.clone();
                                        let platform_state_c = platform_state.clone();
                                        let ctx = rpc_request.ctx.clone();
                                        tokio::spawn(async move {
                                            if let Ok(value) = func(
                                                platform_state_c.clone(),
                                                ctx.clone(),
                                                Some(result.clone()),
                                            )
                                            .await
                                            {
                                                response.result = Some(value.expect("REASON"));
                                            }
                                            response.id = Some(request_id);

                                            let message = ApiMessage {
                                                request_id: request_id.to_string(),
                                                protocol,
                                                jsonrpc_msg: serde_json::to_string(&response)
                                                    .unwrap(),
                                            };

                                            if let Some(session) = platform_state_c
                                                .session_state
                                                .get_session_for_connection_id(&session_id)
                                            {
                                                return_api_message_for_transport(
                                                    session,
                                                    message,
                                                    platform_state_c,
                                                )
                                                .await
                                            }
                                        });
                                        continue;
                                    } else {
                                        error!(
                                            "Failed to invoke decorator method {:?}",
                                            decorator_method
                                        );
                                    }
                                }
                            } else if is_subscription {
                                if sub_processed {
                                    continue;
                                }
                                response.result = Some(json!({
                                    "listening" : rpc_request.is_listening(),
                                    "event" : rpc_request.ctx.method
                                }));
                                platform_state.endpoint_state.update_unsubscribe_request(id);
                            } else {
                                apply_response_needed = true;
                            }
                        } else {
                            trace!("start_forwarder: no result {:?}", response);
                            apply_response_needed = true;
                        }
                        if apply_response_needed {
                            if let Some(filter) = broker_request.rule.transform.get_transform_data(
                                super::rules_engine::RuleTransformType::Response,
                            ) {
                                apply_response(filter, &rpc_request, &mut response);
                            } else {
                                if response.result.is_none() && response.error.is_none() {
                                    response.result = Some(Value::Null);
                                }
                            }
                        }

                        let request_id = rpc_request.ctx.call_id;
                        response.id = Some(request_id);

                        // Step 2: Create the message
                        let message = ApiMessage {
                            request_id: request_id.to_string(),
                            protocol: rpc_request.ctx.protocol.clone(),
                            jsonrpc_msg: serde_json::to_string(&response).unwrap(),
                        };

                        // Step 3: Handle Non Extension
                        if matches!(rpc_request.ctx.protocol, ApiProtocol::Extn) {
                            if let Ok(extn_message) =
                                platform_state.endpoint_state.get_extn_message(id, is_event)
                            {
                                if is_event {
                                    forward_extn_event(&extn_message, response, &platform_state)
                                        .await;
                                } else {
                                    return_extn_response(message, extn_message)
                                }
                            }
                        } else if let Some(session) = platform_state
                            .session_state
                            .get_session_for_connection_id(&session_id)
                        {
                            return_api_message_for_transport(
                                session,
                                message,
                                platform_state.clone(),
                            )
                            .await
                        }
                    } else {
                        error!(
                            "start_forwarder:{} request not found for {:?}",
                            line!(),
                            response
                        );
                    }
                } else {
                    error!("Error couldnt broker the event {:?}", output_c)
                }
            }
        });
    }

    pub fn handle_non_jsonrpc_response(
        data: &[u8],
        callback: BrokerCallback,
        request: BrokerRequest,
    ) -> RippleResponse {
        // find if its event
        let method = if request.rpc.is_subscription() {
            Some(format!(
                "{}.{}",
                request.rpc.ctx.call_id, request.rpc.ctx.method
            ))
        } else {
            None
        };
        let parse_result = serde_json::from_slice::<Value>(data);
        if parse_result.is_err() {
            return Err(RippleError::ParseError);
        }
        let result = Some(parse_result.unwrap());
        // build JsonRpcApiResponse
        let data = JsonRpcApiResponse {
            jsonrpc: "2.0".to_owned(),
            id: Some(request.rpc.ctx.call_id),
            method,
            result,
            error: None,
            params: None,
        };
        let output = BrokerOutput { data };
        tokio::spawn(async move { callback.sender.send(output).await });
        Ok(())
    }
}

async fn forward_extn_event(
    extn_message: &ExtnMessage,
    v: JsonRpcApiResponse,
    platform_state: &PlatformState,
) {
    if let Ok(event) = extn_message.get_event(ExtnEvent::Value(serde_json::to_value(v).unwrap())) {
        if let Err(e) = platform_state
            .get_client()
            .get_extn_client()
            .send_message(event)
            .await
        {
            error!("couldnt send back event {:?}", e)
        }
    }
}

fn apply_response(
    result_response_filter: String,
    rpc_request: &RpcRequest,
    response: &mut JsonRpcApiResponse,
) {
    match serde_json::to_value(response.clone()) {
        Ok(input) => {
            match jq_compile(
                input,
                &result_response_filter,
                format!("{}_response", rpc_request.ctx.method),
            ) {
                Ok(jq_out) => {
                    trace!(
                        "jq rendered output {:?} original input {:?} for filter {}",
                        jq_out,
                        response,
                        result_response_filter
                    );

                    if jq_out.is_object() && jq_out.get("error").is_some() {
                        response.error = Some(jq_out.get("error").unwrap().clone());
                        response.result = None;
                    } else {
                        response.result = Some(jq_out);
                        response.error = None;
                    }
                    trace!("mutated response {:?}", response);
                }
                Err(e) => {
                    response.error = Some(json!(e.to_string()));
                    error!("jq_compile error {:?}", e);
                }
            }
        }
        Err(e) => {
            response.error = Some(json!(e.to_string()));
            error!("json rpc response error {:?}", e);
        }
    }
}

fn apply_rule_for_event(
    broker_request: &BrokerRequest,
    result: &Value,
    rpc_request: &RpcRequest,
    response: &mut JsonRpcApiResponse,
) {
    if let Some(filter) = broker_request
        .rule
        .transform
        .get_transform_data(super::rules_engine::RuleTransformType::Event)
    {
        if let Ok(r) = jq_compile(
            result.clone(),
            &filter,
            format!("{}_event", rpc_request.ctx.method),
        ) {
            response.result = Some(r);
        }
    }
}

fn apply_filter(broker_request: &BrokerRequest, result: &Value, rpc_request: &RpcRequest) -> bool {
    if let Some(filter) = broker_request.rule.filter.clone() {
        if let Ok(r) = jq_compile(
            result.clone(),
            &filter,
            format!("{}_event filter", rpc_request.ctx.method),
        ) {
            println!("apply_filter: {:?}", r);
            if r.is_null() {
                return false;
            } else {
                // get bool value for r and return
                return r.as_bool().unwrap();
            }
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::broker::rules_engine::RuleTransform;
    use ripple_sdk::{tokio::sync::mpsc::channel, Mockable};

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
                        filter: None,
                    },
                    subscription_processed: None,
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
            api::gateway::rpc_gateway_api::RpcRequest, tokio, tokio::sync::mpsc::channel, Mockable,
        };

        use crate::{
            broker::{
                endpoint_broker::tests::RippleClient,
                rules_engine::{Rule, RuleEngine, RuleSet, RuleTransform},
            },
            state::bootstrap_state::ChannelsState,
        };

        use super::EndpointBrokerState;

        #[tokio::test]
        async fn get_request() {
            let (tx, _) = channel(2);
            let client = RippleClient::new(ChannelsState::new());
            let state = EndpointBrokerState::new(
                tx,
                RuleEngine {
                    rules: RuleSet::default(),
                },
                client,
            );
            let mut request = RpcRequest::mock();
            state.update_request(
                &request,
                Rule {
                    alias: "somecallsign.method".to_owned(),
                    transform: RuleTransform::default(),
                    endpoint: None,
                    filter: None,
                },
                None,
            );
            request.ctx.call_id = 2;
            state.update_request(
                &request,
                Rule {
                    alias: "somecallsign.method".to_owned(),
                    transform: RuleTransform::default(),
                    endpoint: None,
                    filter: None,
                },
                None,
            );

            // Hardcoding the id here will be a problem as multiple tests uses the atomic id and there is no guarantee
            // that this test case would always be the first one to run
            // Revisit this test case, to make it more robust
            // assert!(state.get_request(2).is_ok());
            // assert!(state.get_request(1).is_ok());
        }
    }

    #[tokio::test]
    async fn test_apply_response_contains_error() {
        let error = json!({"code":-32601,"message":"The service is in an illegal state!!!."});
        let ctx = CallContext::new(
            "session_id".to_string(),
            "request_id".to_string(),
            "app_id".to_string(),
            1,
            ApiProtocol::Bridge,
            "method".to_string(),
            Some("cid".to_string()),
            true,
        );
        let rpc_request = RpcRequest::new("new_method".to_string(), "params".to_string(), ctx);
        let mut data = JsonRpcApiResponse::mock();
        data.error = Some(error);
        let mut output: BrokerOutput = BrokerOutput { data: data.clone() };
        let filter = "if .result and .result.success then (.result.stbVersion | split(\"_\") [0]) elif .error then if .error.code == -32601 then {error: { code: -1, message: \"Unknown method.\" }} else \"Error occurred with a different code\" end else \"No result or recognizable error\" end".to_string();
        //let mut response = JsonRpcApiResponse::mock();
        //response.error = Some(error);
        apply_response(filter, &rpc_request, &mut output.data);
        //let msg = output.data.error.unwrap().get("message").unwrap().clone();
        assert_eq!(
            output.data.error.unwrap().get("message").unwrap().clone(),
            json!("Unknown method.".to_string())
        );

        // securestorage.get code 22 in error response
        let error = json!({"code":22,"message":"test error code 22"});
        let mut data = JsonRpcApiResponse::mock();
        data.error = Some(error);
        let mut output: BrokerOutput = BrokerOutput { data: data.clone() };
        let filter = "if .result and .result.success then .result.value elif .error.code==22 or .error.code==43 then null else .error end".to_string();
        //let mut response = JsonRpcApiResponse::mock();
        //response.error = Some(error);
        apply_response(filter, &rpc_request, &mut output.data);
        assert_eq!(output.data.error, None);
        assert_eq!(output.data.result.unwrap(), serde_json::Value::Null);

        // securestorage.get code other than 22 or 43 in error response
        let error = json!({"code":300,"message":"test error code 300"});
        let mut data = JsonRpcApiResponse::mock();
        data.error = Some(error.clone());
        let mut output: BrokerOutput = BrokerOutput { data: data.clone() };
        let filter = "if .result and .result.success then .result.value elif .error.code==22 or .error.code==43 then null else { error: .error } end".to_string();
        //let mut response = JsonRpcApiResponse::mock();
        //response.error = Some(error.clone());
        apply_response(filter, &rpc_request, &mut output.data);
        assert_eq!(output.data.error, Some(error));
    }

    #[tokio::test]
    async fn test_apply_response_contains_result() {
        // mock test
        let ctx = CallContext::new(
            "session_id".to_string(),
            "request_id".to_string(),
            "app_id".to_string(),
            1,
            ApiProtocol::Bridge,
            "method".to_string(),
            Some("cid".to_string()),
            true,
        );
        let rpc_request = RpcRequest::new("new_method".to_string(), "params".to_string(), ctx);

        // device.sku
        let filter = "if .result and .result.success then (.result.stbVersion | split(\"_\") [0]) elif .error then if .error.code == -32601 then {\"error\":\"Unknown method.\"} else \"Error occurred with a different code\" end else \"No result or recognizable error\" end".to_string();
        //let mut response = JsonRpcApiResponse::mock();
        let result = json!({"stbVersion":"SCXI11BEI_VBN_24Q3_sprint_20240717150752sdy_FG","receiverVersion":"7.6.0.0","stbTimestamp":"Wed 17 Jul 2024 15:07:52 UTC","success":true});
        //response.result = Some(result);
        let mut data = JsonRpcApiResponse::mock();
        data.result = Some(result);
        let mut output: BrokerOutput = BrokerOutput { data: data.clone() };
        apply_response(filter, &rpc_request, &mut output.data);
        assert_eq!(output.data.result.unwrap(), "SCXI11BEI".to_string());

        // device.videoResolution
        let result = json!("Resolution1080P");
        let filter = "if .result then if .result | contains(\"480\") then ( [640, 480] ) elif .result | contains(\"576\") then ( [720, 576] ) elif .result | contains(\"1080\") then ( [1920, 1080] ) elif .result | contains(\"2160\") then ( [2160, 1440] ) end elif .error then if .error.code == -32601 then \"Unknown method.\" else \"Error occurred with a different code\" end else \"No result or recognizable error\" end".to_string();
        let mut response = JsonRpcApiResponse::mock();
        response.result = Some(result);
        //let data = JsonRpcApiResponse::mock();
        //let mut output: BrokerOutput = BrokerOutput { data: data.clone() };
        apply_response(filter, &rpc_request, &mut response);
        assert_eq!(response.result.unwrap(), json!([1920, 1080]));

        // device.audio
        let result = json!({"currentAudioFormat":"DOLBY AC3","supportedAudioFormat":["NONE","PCM","AAC","VORBIS","WMA","DOLBY AC3","DOLBY AC4","DOLBY MAT","DOLBY TRUEHD","DOLBY EAC3 ATMOS","DOLBY TRUEHD ATMOS","DOLBY MAT ATMOS","DOLBY AC4 ATMOS","UNKNOWN"],"success":true});
        //let data = JsonRpcApiResponse::mock();
        //let mut output: BrokerOutput = BrokerOutput { data: data.clone() };
        let filter = "if .result and .result.success then .result | {\"stereo\": (.supportedAudioFormat |  index(\"PCM\") > 0),\"dolbyDigital5.1\": (.supportedAudioFormat |  index(\"DOLBY AC3\") > 0),\"dolbyDigital5.1plus\": (.supportedAudioFormat |  index(\"DOLBY EAC3\") > 0),\"dolbyAtmos\": (.supportedAudioFormat |  index(\"DOLBY EAC3 ATMOS\") > 0)} elif .error then if .error.code == -32601 then \"Unknown method.\" else \"Error occurred with a different code\" end else \"No result or recognizable error\" end".to_string();
        let mut response = JsonRpcApiResponse::mock();
        response.result = Some(result);
        apply_response(filter, &rpc_request, &mut response);
        assert_eq!(
            response.result.unwrap(),
            json!({"dolbyAtmos": true, "dolbyDigital5.1": true, "dolbyDigital5.1plus": false, "stereo": true})
        );

        // device.network
        let result = json!({"interfaces":[{"interface":"ETHERNET","macAddress":
        "f0:46:3b:5b:eb:14","enabled":true,"connected":false},{"interface":"WIFI","macAddress
        ":"f0:46:3b:5b:eb:15","enabled":true,"connected":true}],"success":true});
        //let data = JsonRpcApiResponse::mock();
        //let mut output: BrokerOutput = BrokerOutput { data: data.clone() };
        let filter = "if .result and .result.success then (.result.interfaces | .[] | select(.connected) | {\"state\": \"connected\",\"type\": .interface | ascii_downcase }) elif .error then if .error.code == -32601 then \"Unknown method.\" else \"Error occurred with a different code\" end else \"No result or recognizable error\" end".to_string();
        let mut response = JsonRpcApiResponse::mock();
        response.result = Some(result);
        apply_response(filter, &rpc_request, &mut response);
        assert_eq!(
            response.result.unwrap(),
            json!({"state":"connected", "type":"wifi"})
        );

        // device.name
        let result = json!({"friendlyName": "my_device","success":true});
        //let data = JsonRpcApiResponse::mock();
        //let mut output: BrokerOutput = BrokerOutput { data: data.clone() };
        let filter = "if .result.success then (if .result.friendlyName | length == 0 then \"Living Room\" else .result.friendlyName end) else \"Living Room\" end".to_string();
        let mut response = JsonRpcApiResponse::mock();
        response.result = Some(result);
        apply_response(filter, &rpc_request, &mut response);
        assert_eq!(response.result.unwrap(), json!("my_device"));

        // localization.language
        let result = json!({"success": true, "value": "{\"update_time\":\"2024-07-29T20:23:29.539132160Z\",\"value\":\"FR\"}"});
        //let data = JsonRpcApiResponse::mock();
        //let mut output: BrokerOutput = BrokerOutput { data: data.clone() };
        let filter = "if .result.success then (.result.value | fromjson | .value) else \"en\" end"
            .to_string();
        let mut response = JsonRpcApiResponse::mock();
        response.result = Some(result);
        apply_response(filter, &rpc_request, &mut response);

        assert_eq!(response.result.unwrap(), json!("FR"));

        // secondscreen.friendlyName
        let result = json!({"friendlyName": "my_device","success":true});
        //let data = JsonRpcApiResponse::mock();
        //let mut output: BrokerOutput = BrokerOutput { data: data.clone() };
        let filter = "if .result.success then (if .result.friendlyName | length == 0 then \"Living Room\" else .result.friendlyName end) else \"Living Room\" end".to_string();
        let mut response = JsonRpcApiResponse::mock();
        response.result = Some(result);
        apply_response(filter, &rpc_request, &mut response);

        assert_eq!(response.result.unwrap(), json!("my_device"));

        // advertising.setSkipRestriction
        let result = json!({"success":true});
        //let data = JsonRpcApiResponse::mock();
        //let mut output: BrokerOutput = BrokerOutput { data: data.clone() };
        let filter = "if .result.success then null else { code: -32100, message: \"couldn't set skip restriction\" } end".to_string();
        let mut response = JsonRpcApiResponse::mock();
        response.result = Some(result);
        apply_response(filter, &rpc_request, &mut response);

        assert_eq!(response.result.unwrap(), serde_json::Value::Null);

        // securestorage.get
        let result = json!({"value": "some_value","success": true,"ttl": 100});
        //let data = JsonRpcApiResponse::mock();
        //let mut output: BrokerOutput = BrokerOutput { data: data.clone() };
        let filter = "if .result.success then .result.value elif .error.code==22 or .error.code==43 then \"null\" else .error end".to_string();
        let mut response = JsonRpcApiResponse::mock();
        response.result = Some(result);
        apply_response(filter, &rpc_request, &mut response);
        assert_eq!(response.result.unwrap(), "some_value");

        // localization.countryCode
        let result = json!({"territory": "USA","success": true});
        //let data = JsonRpcApiResponse::mock();
        //let mut output: BrokerOutput = BrokerOutput { data: data.clone() };
        let filter = "if .result.success then if .result.territory == \"ITA\" then \"IT\" elif .result.territory == \"GBR\" then \"GB\" elif .result.territory == \"IRL\" then \"IE\" elif .result.territory == \"DEU\" then \"DE\" elif .result.territory == \"AUS\" then \"AU\" else \"GB\" end end".to_string();
        let mut response = JsonRpcApiResponse::mock();
        response.result = Some(result);
        apply_response(filter, &rpc_request, &mut response);
        assert_eq!(response.result.unwrap(), "GB");
    }
}
