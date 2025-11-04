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
        firebolt::fb_capabilities::{
            FireboltPermission, CAPABILITY_NOT_AVAILABLE, JSON_RPC_STANDARD_ERROR_INVALID_PARAMS,
        },
        gateway::rpc_gateway_api::{
            ApiMessage, ApiProtocol, CallContext, JsonRpcApiRequest, JsonRpcApiResponse,
            RpcRequest, RPC_V2,
        },
        observability::log_signal::LogSignal,
        session::AccountSession,
    },
    extn::extn_client_message::{ExtnEvent, ExtnMessage},
    framework::RippleResponse,
    log::{debug, error, info, trace},
    service::service_message::{
        Id as ServiceMessageId, JsonRpcMessage as ServiceJsonRpcMessage,
        JsonRpcSuccess as ServiceJsonRpcSuccess, ServiceMessage,
    },
    tokio::{
        self,
        sync::mpsc::{self, Receiver, Sender},
        time::{timeout, Duration},
    },
    tokio_tungstenite::tungstenite::Message,
    utils::error::RippleError,
};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, RwLock,
    },
};

use crate::{
    broker::broker_utils::BrokerUtils,
    firebolt::firebolt_gateway::JsonRpcError,
    service::extn::ripple_client::RippleClient,
    state::{
        ops_metrics_state::OpMetricState, platform_state::PlatformState, session_state::Session,
    },
    utils::router_utils::{
        add_telemetry_status_code, capture_stage, get_rpc_header, return_extn_response,
    },
};

use super::{
    event_management_utility::EventManagementUtility,
    extn_broker::ExtnBroker,
    http_broker::HttpBroker,
    provider_broker_state::{ProvideBrokerState, ProviderResult},
    rules::rules_engine::{
        jq_compile, EventHandler, Rule, RuleEndpoint, RuleEndpointProtocol, RuleEngine,
        RuleRetrievalError, RuleRetrieved, RuleType,
    },
    service_broker::ServiceBroker,
    thunder_broker::ThunderBroker,
    websocket_broker::WebsocketBroker,
    workflow_broker::WorkflowBroker,
};

#[derive(Clone, Debug)]
pub struct BrokerSender {
    pub sender: Sender<BrokerRequest>,
}

#[derive(Clone, Debug, Default)]
pub struct BrokerCleaner {
    pub cleaner: Option<Sender<String>>,
}

impl BrokerCleaner {
    async fn cleanup_session(&self, appid: &str) -> Result<String, RippleError> {
        if let Some(cleaner) = self.cleaner.clone() {
            if let Err(e) = cleaner.try_send(appid.to_owned()) {
                error!("Couldnt cleanup {} {:?}", appid, e);
                return Err(RippleError::SendFailure);
            }
            return Ok(appid.to_owned());
        }
        Err(RippleError::NotAvailable)
    }
}

// Default Broker mpsc channel buffer size
pub const BROKER_CHANNEL_BUFFER_SIZE: usize = 32;

#[derive(Clone, Debug, Default)]
pub struct BrokerRequest {
    pub rpc: RpcRequest,
    pub rule: Rule,
    pub subscription_processed: Option<bool>,
    pub workflow_callback: Option<BrokerCallback>,
    pub telemetry_response_listeners: Vec<Sender<BrokerOutput>>,
}
impl ripple_sdk::api::observability::log_signal::ContextAsJson for BrokerRequest {
    fn as_json(&self) -> serde_json::Value {
        let mut map = serde_json::Map::new();
        map.insert(
            "session_id".to_string(),
            serde_json::Value::String(self.rpc.ctx.session_id.clone()),
        );
        map.insert(
            "request_id".to_string(),
            serde_json::Value::String(self.rpc.ctx.request_id.clone()),
        );
        map.insert(
            "app_id".to_string(),
            serde_json::Value::String(self.rpc.ctx.app_id.clone()),
        );
        map.insert(
            "call_id".to_string(),
            serde_json::Value::Number(serde_json::Number::from(self.rpc.ctx.call_id)),
        );
        map.insert(
            "method".to_string(),
            serde_json::Value::String(self.rpc.method.clone()),
        );

        serde_json::Value::Object(map)
    }
}
impl std::fmt::Display for BrokerRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "BrokerRequest {{ rpc: {:?}, rule: {:?}, subscription_processed: {:?}, workflow_callback: {:?} }}",
            self.rpc, self.rule, self.subscription_processed, self.workflow_callback
        )
    }
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
impl Default for BrokerConnectRequest {
    fn default() -> Self {
        Self {
            key: "".to_owned(),
            endpoint: RuleEndpoint::default(),
            sub_map: HashMap::new(),
            session: None,
            reconnector: mpsc::channel(2).0,
        }
    }
}
impl From<BrokerRequest> for JsonRpcApiRequest {
    fn from(value: BrokerRequest) -> Self {
        Self {
            jsonrpc: "2.0".to_owned(),
            id: Some(value.rpc.ctx.call_id),
            method: value.rpc.ctx.method,
            params: serde_json::from_str(&value.rpc.params_json).unwrap_or(None),
        }
    }
}
impl From<BrokerRequest> for JsonRpcApiResponse {
    fn from(value: BrokerRequest) -> Self {
        Self {
            jsonrpc: "2.0".to_owned(),
            id: Some(value.rpc.ctx.call_id),
            result: None,
            error: None,
            method: None,
            params: None,
        }
    }
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
    pub fn new(
        rpc_request: &RpcRequest,
        rule: Rule,
        workflow_callback: Option<BrokerCallback>,
        telemetry_response_listeners: Vec<Sender<BrokerOutput>>,
    ) -> BrokerRequest {
        BrokerRequest {
            rpc: rpc_request.clone(),
            rule,
            subscription_processed: None,
            workflow_callback,
            telemetry_response_listeners,
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

impl Default for BrokerCallback {
    fn default() -> Self {
        Self {
            sender: mpsc::channel(2).0,
        }
    }
}

pub(crate) static ATOMIC_ID: AtomicU64 = AtomicU64::new(0);

impl BrokerCallback {
    pub async fn send_json_rpc_api_response(&self, response: JsonRpcApiResponse) {
        let output = BrokerOutput::new(response);
        if let Err(e) = self.sender.try_send(output) {
            error!("couldnt send response for {:?}", e);
        }
    }
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
        self.send_json_rpc_api_response(data).await;
    }
}

#[derive(Debug)]
pub struct BrokerContext {
    pub app_id: String,
}

#[derive(Debug, Clone, Default)]
pub struct BrokerOutput {
    pub data: JsonRpcApiResponse,
}

impl BrokerOutput {
    pub fn new(data: JsonRpcApiResponse) -> Self {
        Self { data }
    }
    pub fn with_jsonrpc_response(&mut self, data: JsonRpcApiResponse) -> &mut Self {
        self.data = data;
        self
    }
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
    pub fn is_error(&self) -> bool {
        self.data.error.is_some()
    }
    pub fn is_success(&self) -> bool {
        self.data.result.is_some()
    }
    pub fn get_result(&self) -> Option<Value> {
        self.data.result.clone()
    }
    pub fn get_error(&self) -> Option<Value> {
        self.data.error.clone()
    }
    pub fn get_error_string(&self) -> String {
        if let Some(e) = self.data.error.clone() {
            if let Ok(v) = serde_json::to_string(&e) {
                return v;
            }
        }
        "unknown".to_string()
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
        if let Err(e) = self.sender.try_send(request) {
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
    rule_engine: Arc<RwLock<RuleEngine>>,
    cleaner_list: Arc<RwLock<Vec<BrokerCleaner>>>,
    reconnect_tx: Sender<BrokerConnectRequest>,
    provider_broker_state: ProvideBrokerState,
    metrics_state: OpMetricState,
}

#[derive(Debug)]
pub enum HandleBrokerageError {
    RuleNotFound(String),
    BrokerNotFound(String),
    BrokerSendError,
    Broker,
}
impl From<RuleRetrievalError> for HandleBrokerageError {
    fn from(value: RuleRetrievalError) -> Self {
        match value {
            RuleRetrievalError::RuleNotFound(e) => HandleBrokerageError::RuleNotFound(e),
            RuleRetrievalError::RuleNotFoundAsWildcard => {
                HandleBrokerageError::RuleNotFound("Rule Not found as wildcard".to_string())
            }
            RuleRetrievalError::TooManyWildcardMatches => {
                HandleBrokerageError::RuleNotFound("Too many wildcard matches".to_string())
            }
        }
    }
}
#[derive(Debug, Clone)]
pub enum RenderedRequest {
    JsonRpc(JsonRpcApiResponse),
    ApiMessage(ApiMessage),
    Unlisten(BrokerRequest),
    BrokerRequest(BrokerRequest),
    ProviderJsonRpc(JsonRpcApiResponse),
    ProviderApiMessage(ApiMessage),
}
pub enum RenderedResponse {
    StaticJsonRpcApiResponse(JsonRpcApiResponse),
}
// impl From<RenderedRequest> for BrokerRequest {
//     fn from(value: RenderedRequest) -> Self {
//         match value {
//             RenderedRequest::JsonRpc(v, _) => BrokerRequest::from(v),
//             RenderedRequest::ApiMessage(v, _) => BrokerRequest::from(v),
//             RenderedRequest::Unlisten(v) => BrokerRequest::from(v),
//             RenderedRequest::BrokerRequest(v) => v,
//             RenderedRequest::ProviderJsonRpc(v, _) => BrokerRequest::from(v),
//             RenderedRequest::ProviderApiMessage(v, _) => BrokerRequest::from(v),
//         }
//     }
// }

#[derive(Debug)]
pub enum BrokerEndpoint {
    BrokerSender(BrokerSender),
    /*
    marker to indicate that something else should be used, such as a BrokerCallBack
    */
    Provider(BrokerCallback),
    //  Workflow(BrokerCallback, BrokerSender),
}
impl std::fmt::Display for BrokerEndpoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BrokerEndpoint::BrokerSender(_) => write!(f, "BrokerSender"),
            BrokerEndpoint::Provider(_) => write!(f, "Provider"),
        }
    }
}
impl BrokerEndpoint {
    pub async fn send_request(self, request: BrokerRequest) -> RippleResponse {
        match self {
            BrokerEndpoint::BrokerSender(broker_sender) => broker_sender
                .sender
                .try_send(request)
                .map_err(|_| RippleError::SendFailure),
            _ => {
                error!("BrokerEndpoint::send: BrokerSender not supported");
                Err(RippleError::SendFailure)
            }
        }
    }
}

impl Default for EndpointBrokerState {
    fn default() -> Self {
        Self {
            endpoint_map: Arc::new(RwLock::new(HashMap::new())),
            callback: BrokerCallback::default(),
            request_map: Arc::new(RwLock::new(HashMap::new())),
            extension_request_map: Arc::new(RwLock::new(HashMap::new())),
            rule_engine: Arc::new(RwLock::new(RuleEngine::default())),
            cleaner_list: Arc::new(RwLock::new(Vec::new())),
            reconnect_tx: mpsc::channel(2).0,
            provider_broker_state: ProvideBrokerState::default(),
            metrics_state: OpMetricState::default(),
        }
    }
}

impl EndpointBrokerState {
    pub fn new(
        metrics_state: OpMetricState,
        tx: Sender<BrokerOutput>,
        rule_engine: RuleEngine,
        _ripple_client: RippleClient,
    ) -> Self {
        let (reconnect_tx, _rec_tr) = mpsc::channel(2);
        let state = Self {
            endpoint_map: Arc::new(RwLock::new(HashMap::new())),
            callback: BrokerCallback { sender: tx },
            request_map: Arc::new(RwLock::new(HashMap::new())),
            extension_request_map: Arc::new(RwLock::new(HashMap::new())),
            rule_engine: Arc::new(RwLock::new(rule_engine)),
            cleaner_list: Arc::new(RwLock::new(Vec::new())),
            reconnect_tx,
            provider_broker_state: ProvideBrokerState::default(),
            metrics_state,
        };
        /*bobra: configuring this out for unit tests */
        #[cfg(not(test))]
        state.reconnect_thread(_rec_tr, _ripple_client);
        state
    }
    pub fn with_rules_engine(mut self, rule_engine: Arc<RwLock<RuleEngine>>) -> Self {
        self.rule_engine = rule_engine;
        self
    }
    pub fn add_rule(self, rule: Rule) -> Self {
        self.rule_engine.write().unwrap().add_rule(rule);
        self
    }
    pub fn has_rule(&self, rule: &str) -> bool {
        self.rule_engine.read().unwrap().has_rule(rule)
    }
    #[cfg(not(test))]
    fn reconnect_thread(&self, mut rx: Receiver<BrokerConnectRequest>, client: RippleClient) {
        use crate::firebolt::firebolt_gateway::FireboltGatewayCommand;
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
                    state.build_endpoint(None, v)
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
        //https://en.cppreference.com/w/cpp/atomic/memory_order#Sequentially-consistent_ordering
        /*
        Switching to SeqCst for now, as the there could be consistency issues using Relaxed
        note that fetch add returns the previous value after the add . This is
        fine because the requirement is to have a unique id for each request
        */
        ATOMIC_ID.fetch_add(1, Ordering::SeqCst)
    }
    /// Generic method which takes the given parameters from RPC request and adds rules using rule engine
    //TODO: decide fate of this function
    #[allow(dead_code)]
    fn apply_request_rule(rpc_request: &BrokerRequest) -> Result<Value, RippleError> {
        if let Ok(mut params) = serde_json::from_str::<Vec<Value>>(&rpc_request.rpc.params_json) {
            let last = params.pop().unwrap_or(Value::Null);

            if let Some(filter) = rpc_request
                .rule
                .transform
                .get_transform_data(super::rules::rules_engine::RuleTransformType::Request)
            {
                let transformed_request_res = jq_compile(
                    last,
                    &filter,
                    format!("{}_request", rpc_request.rpc.ctx.method),
                );

                LogSignal::new(
                    "endpoint_broker".to_string(),
                    "apply_request_rule".to_string(),
                    rpc_request.rpc.ctx.clone(),
                )
                .with_diagnostic_context_item("success", "true")
                .with_diagnostic_context_item("result", &format!("{:?}", transformed_request_res))
                .emit_debug();

                return transformed_request_res;
            }
            LogSignal::new(
                "endpoint_broker".to_string(),
                "apply_request_rule".to_string(),
                rpc_request.rpc.ctx.clone(),
            )
            .with_diagnostic_context_item("success", "true")
            .with_diagnostic_context_item("result", &last.to_string())
            .emit_debug();
            return Ok(serde_json::to_value(&last).unwrap());
        }
        LogSignal::new(
            "endpoint_broker".to_string(),
            "apply_request_rule: parse error".to_string(),
            rpc_request.rpc.ctx.clone(),
        )
        .emit_error();
        Err(RippleError::ParseError)
    }

    pub fn update_request(
        &self,
        rpc_request: &RpcRequest,
        rule: &Rule,
        extn_message: Option<ExtnMessage>,
        workflow_callback: Option<BrokerCallback>,
        telemetry_response_listeners: Vec<Sender<BrokerOutput>>,
    ) -> BrokerRequest {
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
                    workflow_callback: workflow_callback.clone(),
                    telemetry_response_listeners: telemetry_response_listeners.clone(),
                },
            );
        }

        if extn_message.is_some() {
            let mut extn_map = self.extension_request_map.write().unwrap();
            let _ = extn_map.insert(id, extn_message.unwrap());
        }

        rpc_request_c.ctx.call_id = id;

        BrokerRequest::new(
            &rpc_request_c,
            rule.clone(),
            workflow_callback,
            telemetry_response_listeners,
        )
    }
    pub fn build_thunder_endpoint(&mut self, ps: Option<PlatformState>) {
        let endpoint = {
            self.rule_engine
                .write()
                .unwrap()
                .rules
                .endpoints
                .get("thunder")
                .cloned()
        };
        if let Some(endpoint) = endpoint {
            let request = BrokerConnectRequest::new(
                "thunder".to_owned(),
                endpoint.clone(),
                self.reconnect_tx.clone(),
            );
            self.build_endpoint(ps, request);
        }
    }

    pub fn build_other_endpoints(&mut self, ps: PlatformState, session: Option<AccountSession>) {
        let endpoints = self.rule_engine.read().unwrap().rules.endpoints.clone();
        for (key, endpoint) in endpoints {
            // skip thunder endpoint as it is already built using build_thunder_endpoint
            if let RuleEndpointProtocol::Thunder = endpoint.protocol {
                continue;
            }
            let request = BrokerConnectRequest::new_with_sesssion(
                key,
                endpoint.clone(),
                self.reconnect_tx.clone(),
                session.clone(),
            );
            self.build_endpoint(Some(ps.clone()), request);
        }
    }

    fn add_endpoint(&mut self, key: String, endpoint: BrokerSender) -> &mut Self {
        {
            let mut endpoint_map = self.endpoint_map.write().unwrap();
            endpoint_map.insert(key, endpoint);
        }
        self
    }
    pub fn get_endpoints(&self) -> HashMap<String, BrokerSender> {
        self.endpoint_map.read().unwrap().clone()
    }

    fn build_endpoint(&mut self, ps: Option<PlatformState>, request: BrokerConnectRequest) {
        let endpoint = request.endpoint.clone();
        let key = request.key.clone();
        let (broker, cleaner) = match endpoint.protocol {
            RuleEndpointProtocol::Http => (
                HttpBroker::get_broker(None, request, self.callback.clone(), self).get_sender(),
                None,
            ),
            RuleEndpointProtocol::Websocket => {
                let ws_broker =
                    WebsocketBroker::get_broker(None, request, self.callback.clone(), self);
                (ws_broker.get_sender(), Some(ws_broker.get_cleaner()))
            }
            RuleEndpointProtocol::Thunder => {
                let thunder_broker =
                    ThunderBroker::get_broker(ps, request, self.callback.clone(), self);
                (
                    thunder_broker.get_sender(),
                    Some(thunder_broker.get_cleaner()),
                )
            }
            RuleEndpointProtocol::Workflow => (
                WorkflowBroker::get_broker(None, request, self.callback.clone(), self).get_sender(),
                None,
            ),
            RuleEndpointProtocol::Extn => (
                ExtnBroker::get_broker(ps, request, self.callback.clone(), self).get_sender(),
                None,
            ),
            RuleEndpointProtocol::Service => (
                ServiceBroker::get_broker(ps, request, self.callback.clone(), self).get_sender(),
                None,
            ),
        };
        self.add_endpoint(key, broker);

        if let Some(cleaner) = cleaner {
            let mut cleaner_list = self.cleaner_list.write().unwrap();
            cleaner_list.push(cleaner);
        }
    }

    fn handle_static_request(&self, rpc_request: RpcRequest) -> JsonRpcApiResponse {
        let mut data = JsonRpcApiResponse::default();
        // return empty result and handle the rest with jq rule
        let jv: Value = "".into();
        data.result = Some(jv);
        data.id = Some(rpc_request.ctx.call_id);
        //let output = BrokerOutput::new(data);

        capture_stage(&self.metrics_state, &rpc_request, "static_rule_request");
        data
    }

    fn handle_provided_request(
        &self,
        rpc_request: &RpcRequest,
        id: u64,
        permission: Vec<FireboltPermission>,
        session: Option<Session>,
    ) -> RenderedRequest {
        // let (id, request) =
        //     self.update_request(rpc_request, rule, None, None, telemetry_response_listeners);
        match self.provider_broker_state.check_provider_request(
            rpc_request,
            &permission,
            session.clone(),
        ) {
            Some(ProviderResult::Registered) => {
                // return empty result and handle the rest with jq rule
                let data = JsonRpcApiResponse {
                    id: Some(id),
                    jsonrpc: "2.0".to_string(),
                    result: Some(Value::Null),
                    error: None,
                    method: None,
                    params: None,
                };
                RenderedRequest::ProviderJsonRpc(data)
            }
            Some(ProviderResult::Session(_s)) => RenderedRequest::ProviderApiMessage(
                ProvideBrokerState::format_provider_message(rpc_request, id),
            ),
            Some(ProviderResult::NotAvailable(p)) => {
                // Not Available
                let data = JsonRpcApiResponse::new(
                    Some(id),
                    Some(json!({
                        "error": CAPABILITY_NOT_AVAILABLE,
                        "messsage": format!("{} not available", p)
                    })),
                );
                RenderedRequest::ProviderJsonRpc(data)
            }
            None => {
                // Not Available
                let data = JsonRpcApiResponse::new(
                    Some(id),
                    Some(json!({
                        "error": CAPABILITY_NOT_AVAILABLE,
                        "messsage": "capability not available".to_string()
                    })),
                );
                RenderedRequest::ProviderJsonRpc(data)
            }
        }
    }

    fn get_sender(&self, hash: &str) -> Option<BrokerSender> {
        self.endpoint_map.read().unwrap().get(hash).cloned()
    }
    fn get_broker_rule(
        &self,
        rpc_request: &RpcRequest,
    ) -> Result<RuleRetrieved, RuleRetrievalError> {
        self.rule_engine.read().unwrap().get_rule(rpc_request)
    }
    /// Main handler method which checks for brokerage and then sends the request for
    /// asynchronous processing
    pub fn handle_brokerage(
        &self,
        rpc_request: RpcRequest,
        extn_message: Option<ExtnMessage>,
        custom_callback: Option<BrokerCallback>,
        permissions: Vec<FireboltPermission>,
        session: Option<Session>,
        telemetry_response_listeners: Vec<Sender<BrokerOutput>>,
    ) -> bool {
        LogSignal::new(
            "handle_brokerage".to_string(),
            "starting brokerage".to_string(),
            rpc_request.ctx.clone(),
        )
        .with_diagnostic_context_item("workflow", &custom_callback.is_some().to_string())
        .emit_debug();

        let resp = self.handle_brokerage_workflow(
            rpc_request.clone(),
            extn_message,
            custom_callback,
            permissions,
            session,
            telemetry_response_listeners,
        );

        if resp.is_err() {
            let err = resp.unwrap_err();
            LogSignal::new(
                "handle_brokerage".to_string(),
                "Rule error".to_string(),
                rpc_request.ctx.clone(),
            )
            .with_diagnostic_context_item("error", &format!("{:?}", err))
            .emit_error();
            false
        } else {
            true
        }
    }

    fn get_endpoint(
        &self,
        rule: &Rule,
        broker_callback: BrokerCallback,
    ) -> Result<BrokerEndpoint, HandleBrokerageError> {
        /*
        if endpoint is defined, try to get it
        else if static rule, get thunder broker
        else fail
        */
        if let Some(endpoint) = rule.endpoint.clone() {
            if let Some(sender) = self.get_sender(&endpoint) {
                return Ok(BrokerEndpoint::BrokerSender(sender));
            } else {
                return Err(HandleBrokerageError::BrokerNotFound(endpoint));
            }
        };

        match rule.rule_type() {
            RuleType::Static | RuleType::Endpoint => match self.get_sender("thunder") {
                Some(sender) => Ok(BrokerEndpoint::BrokerSender(sender)),
                None => Err(HandleBrokerageError::BrokerNotFound("thunder".to_string())),
            },
            RuleType::Provider => Ok(BrokerEndpoint::Provider(broker_callback)),
        }
    }

    /*
    Render correct output based on request type
    */
    pub fn render_brokered_request(
        &self,
        rule: &Rule,
        broker_request: &BrokerRequest,
        permissions: Vec<FireboltPermission>,
        session: Option<Session>,
    ) -> Result<RenderedRequest, HandleBrokerageError> {
        LogSignal::new(
            "render_brokered_request".to_string(),
            "starting render".to_string(),
            broker_request.clone(),
        )
        .with_diagnostic_context_item(
            "endpoint",
            rule.clone().endpoint.unwrap_or_default().as_str(),
        )
        .with_diagnostic_context_item("rule", rule.alias.as_str())
        .emit_debug();

        let rpc_request = broker_request.rpc.clone();
        match rule.rule_type() {
            super::rules::rules_engine::RuleType::Static => {
                let response =
                    RenderedRequest::JsonRpc(self.handle_static_request(rpc_request.clone()));
                Ok(response)
            }
            super::rules::rules_engine::RuleType::Provider => {
                let response = self.handle_provided_request(
                    &rpc_request,
                    rpc_request.ctx.call_id,
                    permissions,
                    session,
                );
                Ok(response)
            }
            super::rules::rules_engine::RuleType::Endpoint => {
                if rpc_request.is_unlisten() {
                    Ok(RenderedRequest::Unlisten(broker_request.clone()))
                } else {
                    Ok(RenderedRequest::BrokerRequest(broker_request.clone()))
                }
            }
        }
    }

    pub fn handle_brokerage_workflow(
        &self,
        rpc_request: RpcRequest,
        extn_message: Option<ExtnMessage>,
        workflow_callback: Option<BrokerCallback>,
        permissions: Vec<FireboltPermission>,
        session: Option<Session>,
        telemetry_response_listeners: Vec<Sender<BrokerOutput>>,
    ) -> Result<RenderedRequest, HandleBrokerageError> {
        /*if rule not found, "unhandled https://github.com/rdkcentral/Ripple/blob/ae3fcd78b055cf70022959bf827de9ed569762aa/core/main/src/broker/endpoint_broker.rs#L719" */
        let rule: Rule = match self.get_broker_rule(&rpc_request)? {
            RuleRetrieved::ExactMatch(rule) | RuleRetrieved::WildcardMatch(rule) => rule,
        };
        /*
         attempt to get the endpoint from the rule
        https://github.com/rdkcentral/Ripple/blob/ae3fcd78b055cf70022959bf827de9ed569762aa/core/main/src/broker/endpoint_broker.rs#L722
        */
        let endpoint = match self.get_endpoint(&rule, self.callback.clone()) {
            Ok(endpoint) => {
                LogSignal::new(
                    "handle_brokerage_workflow".to_string(),
                    "rule found".to_string(),
                    rpc_request.ctx.clone(),
                )
                .with_diagnostic_context_item("rule", &format!("{}", rule))
                .with_diagnostic_context_item("endpoint", &format!("{}", endpoint))
                .emit_debug();
                endpoint
            }
            Err(e) => return Err(e),
        };

        LogSignal::new(
            "handle_brokerage_workflow".to_string(),
            "starting brokerage workflow".to_string(),
            rpc_request.ctx.clone(),
        )
        .with_diagnostic_context_item("rule", &format!("{}", rule))
        .with_diagnostic_context_item("endpoint", &format!("{}", endpoint))
        // this is printing non debuggable data
        .with_diagnostic_context_item("workflow", &workflow_callback.is_some().to_string())
        .emit_debug();
        /*
        broker_callback is used to send the response back to the caller
        */
        let broker_callback = self.callback.clone();

        match self.render_brokered_request(
            &rule,
            &self.update_request(
                &rpc_request,
                &rule,
                extn_message,
                workflow_callback,
                telemetry_response_listeners,
            ),
            permissions,
            session.clone(),
        ) {
            Ok(response) => match response.clone() {
                RenderedRequest::JsonRpc(data) => {
                    tokio::spawn(async move {
                        if let Err(err) = broker_callback.sender.try_send(BrokerOutput::new(data)) {
                            error!(
                                "Error sending RenderedRequest json rpc response to broker {:?}",
                                err
                            );
                        }
                    });
                    Ok(response)
                }
                RenderedRequest::ApiMessage(api_message) => {
                    if let Some(sesh) = session {
                        info!("Sending apimessage response to endpoint {:?}", api_message);
                        tokio::spawn(async move { sesh.send_json_rpc(api_message).await });
                    }
                    Ok(response)
                }
                RenderedRequest::Unlisten(data) => {
                    info!("Sending unlisten json rpc response to endpoint {:?}", data);

                    if let Some(thunder) = self.get_sender("thunder") {
                        tokio::spawn(async move {
                            match thunder.send(data.clone()).await {
                                Ok(_) => {
                                    broker_callback
                                        .send_json_rpc_api_response(data.clone().rpc.into())
                                        .await
                                }
                                Err(e) => broker_callback.send_error(data.clone(), e).await,
                            }
                        });
                    }
                    Ok(response)
                }
                RenderedRequest::BrokerRequest(request) => {
                    info!(
                        "Sending broker_request json rpc response to endpoint {:?}",
                        request
                    );

                    let data = JsonRpcApiResponse {
                        id: Some(request.rpc.ctx.call_id),
                        jsonrpc: "2.0".to_string(),
                        result: Some(Value::Null),
                        error: None,
                        method: Some(request.rpc.method.clone()),
                        params: request.rpc.get_params(),
                    };
                    let request_for_spawn = request.clone();
                    tokio::spawn(async move { endpoint.send_request(request_for_spawn).await });

                    Ok(RenderedRequest::ProviderJsonRpc(data))
                }

                RenderedRequest::ProviderJsonRpc(json_rpc_api_response) => {
                    tokio::spawn(async move {
                        if let Err(err) = broker_callback
                            .sender
                            .try_send(BrokerOutput::new(json_rpc_api_response))
                        {
                            error!("Error sending RenderedRequest provider json rpc response to broker {:?}", err);
                        }
                    });
                    Ok(response)
                }
                RenderedRequest::ProviderApiMessage(api_message) => {
                    tokio::spawn(async move {
                        if let Some(sesh) = session {
                            let _ = sesh.send_json_rpc(api_message).await;
                        }
                    });
                    Ok(response)
                }
            },
            Err(e) => {
                error!("Error handling brokerage {:?}", e);
                Err(e)
            }
        }
    }

    pub fn handle_broker_response(&self, data: JsonRpcApiResponse) {
        if let Err(e) = self.callback.sender.try_send(BrokerOutput { data }) {
            error!("Cannot forward broker response {:?}", e)
        }
    }

    // Method to cleanup all subscription on App termination
    pub async fn cleanup_for_app(&self, app_id: &str) {
        let cleaners = { self.cleaner_list.read().unwrap().clone() };

        for cleaner in cleaners {
            /*
            for now, just eat the error - the return type was mainly added to prepate for future refactoring/testability
            */
            let _ = cleaner.cleanup_session(app_id).await;
        }
    }
    /// Send a request through the broker and wait for response with a oneshot channel and custom timeout
    pub async fn send_with_response_timeout(
        &self,
        rpc_request: RpcRequest,
        response_tx: ripple_sdk::tokio::sync::oneshot::Sender<Result<Value, Value>>,
        timeout_secs: u64,
    ) -> Result<(), RippleError> {
        // Get the appropriate broker rule for this request
        let rule = match self.get_broker_rule(&rpc_request) {
            Ok(RuleRetrieved::ExactMatch(rule) | RuleRetrieved::WildcardMatch(rule)) => rule,
            Err(_) => return Err(RippleError::NotAvailable),
        };

        // Create a custom callback that will send the response through the oneshot channel
        let (callback_tx, mut callback_rx) = mpsc::channel(1);
        let custom_callback = BrokerCallback {
            sender: callback_tx,
        };

        // Create broker request with the custom callback
        let broker_request =
            self.update_request(&rpc_request, &rule, None, Some(custom_callback), vec![]);

        // Get the appropriate endpoint for this rule
        let endpoint = self
            .get_endpoint(&rule, self.callback.clone())
            .map_err(|_| RippleError::NotAvailable)?;

        // Send the request through the endpoint
        match endpoint {
            BrokerEndpoint::BrokerSender(sender) => {
                sender.send(broker_request).await?;
            }
            _ => {
                return Err(RippleError::BrokerError(
                    "Failed to send broker request".to_owned(),
                ))
            }
        }

        // Wait for the response and forward it through the oneshot channel with timeout
        tokio::spawn(async move {
            // Use the provided timeout value
            let timeout_duration = Duration::from_secs(timeout_secs);

            match timeout(timeout_duration, callback_rx.recv()).await {
                Ok(Some(broker_output)) => {
                    // Received a valid broker response
                    // Success case: data contains a result field
                    if let Some(result) = broker_output.data.result {
                        let _ = response_tx.send(Ok(result));
                    }
                    // Error case: data contains an error field
                    else if let Some(error) = broker_output.data.error {
                        let _ = response_tx.send(Err(error));
                    }
                    // Edge case: neither result nor error
                    else {
                        // treat this as success with null result
                        let _ = response_tx.send(Ok(Value::Null));
                    }
                }
                Ok(None) => {
                    // Channel was closed without sending a response
                    let error_response = serde_json::json!({
                        "code": -32000,
                        "message": "Broker channel closed unexpectedly"
                    });
                    let _ = response_tx.send(Err(error_response));
                }
                Err(_) => {
                    // Timeout occurred
                    let error_response = serde_json::json!({
                        "code": -32001,
                        "message": "Request timeout"
                    });
                    let _ = response_tx.send(Err(error_response));
                }
            }
        });

        Ok(())
    }
}

/// Trait which contains all the abstract methods for a Endpoint Broker
/// There could be Websocket or HTTP protocol implementations of the given trait
pub trait EndpointBroker {
    fn get_broker(
        ps: Option<PlatformState>,
        request: BrokerConnectRequest,
        callback: BrokerCallback,
        endpoint_broker: &mut EndpointBrokerState,
    ) -> Self;

    fn get_sender(&self) -> BrokerSender;

    fn prepare_request(&self, rpc_request: &BrokerRequest) -> Result<Vec<String>, RippleError> {
        let response = Self::update_request(rpc_request)?;
        Ok(vec![response])
    }

    /// Adds BrokerContext to a given request used by the Broker Implementations
    /// just before sending the data through the protocol
    fn update_request(broker_request: &BrokerRequest) -> Result<String, RippleError> {
        let v = Self::apply_request_rule(broker_request)?;
        trace!("transformed request {:?}", v);
        let id = broker_request.rpc.ctx.call_id;
        let method = broker_request.rule.alias.clone();
        let rpc_request_str = if let Value::Null = v {
            json!({
                "jsonrpc": "2.0",
                "id": id,
                "method": method
            })
            .to_string()
        } else {
            json!({
                "jsonrpc": "2.0",
                "id": id,
                "method": method,
                "params": v
            })
            .to_string()
        };

        Ok(rpc_request_str)
    }

    /// Generic method which takes the given parameters from RPC request and adds rules using rule engine
    fn apply_request_rule(rpc_request: &BrokerRequest) -> Result<Value, RippleError> {
        if let Ok(mut params) = serde_json::from_str::<Vec<Value>>(&rpc_request.rpc.params_json) {
            let last = params.pop().unwrap_or(Value::Null);

            if let Some(filter) = rpc_request
                .rule
                .transform
                .get_transform_data(super::rules::rules_engine::RuleTransformType::Request)
            {
                let transformed_request_res = jq_compile(
                    last,
                    &filter,
                    format!("{}_request", rpc_request.rpc.ctx.method),
                );

                LogSignal::new(
                    "endpoint_broker".to_string(),
                    "apply_request_rule".to_string(),
                    rpc_request.rpc.ctx.clone(),
                )
                .with_diagnostic_context_item("success", "true")
                .with_diagnostic_context_item("result", &format!("{:?}", transformed_request_res))
                .emit_debug();

                return transformed_request_res;
            }
            LogSignal::new(
                "endpoint_broker".to_string(),
                "apply_request_rule".to_string(),
                rpc_request.rpc.ctx.clone(),
            )
            .with_diagnostic_context_item("success", "true")
            .with_diagnostic_context_item("result", &last.to_string())
            .emit_debug();
            return Ok(serde_json::to_value(&last).unwrap());
        }
        LogSignal::new(
            "endpoint_broker".to_string(),
            "apply_request_rule: parse error".to_string(),
            rpc_request.rpc.ctx.clone(),
        )
        .emit_error();
        Err(RippleError::ParseError)
    }

    /// Default handler method for the broker to remove the context and send it back to the
    /// client for consumption
    fn handle_jsonrpc_response(
        result: &[u8],
        callback: BrokerCallback,
        _params: Option<Value>,
    ) -> Result<BrokerOutput, RippleError> {
        let mut final_result = Err(RippleError::ParseError);
        if let Ok(data) = serde_json::from_slice::<JsonRpcApiResponse>(result) {
            final_result = Ok(BrokerOutput::new(data));
        }
        if let Ok(output) = final_result.clone() {
            tokio::spawn(async move { callback.sender.try_send(output) });
        } else {
            error!("Bad broker response {}", String::from_utf8_lossy(result));
        }
        final_result
    }

    fn get_cleaner(&self) -> BrokerCleaner;

    fn send_broker_success_response(
        callback: &BrokerCallback,
        success_message: JsonRpcApiResponse,
    ) {
        BrokerOutputForwarder::send_json_rpc_response_to_broker(success_message, callback.clone());
    }
    fn send_broker_failure_response(callback: &BrokerCallback, error_message: JsonRpcApiResponse) {
        BrokerOutputForwarder::send_json_rpc_response_to_broker(error_message, callback.clone());
    }
}

/// Forwarder gets the BrokerOutput and forwards the response to the gateway.
pub struct BrokerOutputForwarder;

impl BrokerOutputForwarder {
    pub fn start_forwarder(mut platform_state: PlatformState, mut rx: Receiver<BrokerOutput>) {
        // set up the event utility
        let event_utility = Arc::new(EventManagementUtility::new());
        event_utility.register_custom_functions();
        let event_utility_clone = event_utility.clone();

        tokio::spawn(async move {
            while let Some(output) = rx.recv().await {
                let output_c = output.clone();
                let mut response = output.data.clone();
                let mut is_event = false;
                let id = if let Some(e) = output_c.get_event() {
                    is_event = true;
                    Some(e)
                } else {
                    response.id
                };

                if let Some(id) = id {
                    if let Ok(broker_request) = platform_state.endpoint_state.get_request(id) {
                        LogSignal::new(
                            "start_forwarder".to_string(),
                            "broker request found".to_string(),
                            broker_request.clone(),
                        )
                        .emit_debug();

                        let rule_context_name = broker_request.rpc.method.clone();
                        let workflow_callback = broker_request.workflow_callback.clone();
                        let telemetry_response_listeners =
                            broker_request.telemetry_response_listeners.clone();
                        let sub_processed = broker_request.is_subscription_processed();
                        let rpc_request = broker_request.rpc.clone();
                        let is_subscription = rpc_request.is_subscription();

                        let apply_response_needed = if let Some(result) = response.result.clone() {
                            if is_event {
                                LogSignal::new(
                                    "handle_event_output".to_string(),
                                    "processing event".to_string(),
                                    broker_request.clone(),
                                )
                                .emit_debug();
                                if Self::handle_event_output(
                                    &broker_request,
                                    &rpc_request,
                                    &mut response,
                                    &platform_state,
                                    &event_utility_clone,
                                    result.clone(),
                                )
                                .await
                                {
                                    continue;
                                }
                            } else if is_subscription
                                && Self::handle_subscription_response(
                                    &broker_request,
                                    &rpc_request,
                                    &mut response,
                                    sub_processed,
                                    &platform_state,
                                    id,
                                )
                            {
                                continue;
                            }

                            !is_event && !is_subscription
                        } else {
                            LogSignal::new(
                                "start_forwarder".to_string(),
                                "no result".to_string(),
                                rpc_request.ctx.clone(),
                            )
                            .with_diagnostic_context_item("response", response.to_string().as_str())
                            .emit_debug();
                            true
                        };

                        if apply_response_needed {
                            Self::apply_response_transform(
                                &broker_request,
                                &output_c,
                                &mut response,
                                &rule_context_name,
                            );
                        }

                        response.id = Some(rpc_request.ctx.call_id);

                        Self::forward_response(
                            response,
                            &rpc_request,
                            &mut platform_state,
                            is_event,
                            id,
                            workflow_callback,
                            telemetry_response_listeners,
                        )
                        .await;
                    } else {
                        error!(
                            "start_forwarder:{} request not found for {:?}",
                            line!(),
                            response
                        );
                    }
                } else {
                    error!(
                        "Error couldnt broker the event {:?} due to a missing request id",
                        output_c
                    )
                }
            }
        });
    }

    async fn forward_response(
        response: JsonRpcApiResponse,
        rpc_request: &RpcRequest,
        platform_state: &mut PlatformState,
        is_event: bool,
        id: u64,
        workflow_callback: Option<BrokerCallback>,
        telemetry_response_listeners: Vec<Sender<BrokerOutput>>,
    ) {
        LogSignal::new(
            "forward_response".to_string(),
            "entered".to_string(),
            rpc_request.ctx.clone(),
        )
        .emit_debug();
        let session_id = rpc_request.ctx.get_id();
        if let Some(workflow_callback) = workflow_callback {
            debug!("sending to workflow callback {:?}", response);
            LogSignal::new(
                "forward_response".to_string(),
                "sending to workflow callback".to_string(),
                rpc_request.ctx.clone(),
            )
            .emit_debug();
            let _ = workflow_callback
                .sender
                .try_send(BrokerOutput::new(response.clone()));
        } else {
            let tm_str = get_rpc_header(rpc_request);
            let mut response = response.clone();
            if is_event {
                response.update_event_message(rpc_request);
            }
            let mut message = ApiMessage::new(
                rpc_request.ctx.protocol.clone(),
                serde_json::to_string(&response).unwrap(),
                rpc_request.ctx.request_id.clone(),
            );
            let mut status_code: i64 = 1;
            if let Some(e) = &response.error {
                if let Some(Value::Number(n)) = e.get("code") {
                    if let Some(v) = n.as_i64() {
                        status_code = v;
                    }
                }
            }
            platform_state.metrics.update_api_stats_ref(
                &rpc_request.ctx.request_id,
                add_telemetry_status_code(&tm_str, status_code.to_string().as_str()),
            );
            if let Some(api_stats) = platform_state
                .metrics
                .get_api_stats(&rpc_request.ctx.request_id)
            {
                message.stats = Some(api_stats);
                if rpc_request.ctx.app_id.eq_ignore_ascii_case("internal") {
                    platform_state
                        .metrics
                        .remove_api_stats(&rpc_request.ctx.request_id);
                }
            }
            if matches!(rpc_request.ctx.protocol, ApiProtocol::Extn) {
                if let Ok(extn_message) =
                    platform_state.endpoint_state.get_extn_message(id, is_event)
                {
                    let client = platform_state.get_client().get_extn_client();
                    if is_event {
                        forward_extn_event(&extn_message, response.clone(), platform_state).await;
                    } else {
                        return_extn_response(message, extn_message, client)
                    }
                }
            } else if matches!(rpc_request.ctx.protocol, ApiProtocol::Service) {
                Self::handle_service_message(rpc_request, &message, platform_state).await;
            } else if let Some(session) = platform_state
                .session_state
                .get_session_for_connection_id(&session_id)
            {
                let _ = session.send_json_rpc(message).await;
            }
        }
        for listener in telemetry_response_listeners {
            let _ = listener.try_send(BrokerOutput::new(response.clone()));
        }
    }

    async fn handle_event_output(
        broker_request: &BrokerRequest,
        rpc_request: &RpcRequest,
        response: &mut JsonRpcApiResponse,
        platform_state: &PlatformState,
        event_utility: &Arc<EventManagementUtility>,
        result: Value,
    ) -> bool {
        if let Some(event_handler) = broker_request.rule.event_handler.clone() {
            let platform_state_c = platform_state.clone();
            let rpc_request_c = rpc_request.clone();
            let response_c = response.clone();
            let broker_request_c = broker_request.clone();

            LogSignal::new(
                "handle_event_output".to_string(),
                "spawning event handler".to_string(),
                broker_request.clone(),
            )
            .emit_debug();

            tokio::spawn(Self::handle_event(
                platform_state_c,
                event_handler,
                broker_request_c,
                rpc_request_c,
                response_c,
            ));
            return true;
        }

        if let Some(filter) = broker_request.rule.transform.get_transform_data(
            super::rules::rules_engine::RuleTransformType::Event(
                rpc_request.ctx.context.contains(&RPC_V2.into()),
            ),
        ) {
            apply_rule_for_event(broker_request, &result, rpc_request, &filter, response);
        }

        if !apply_filter(broker_request, &result, rpc_request) {
            LogSignal::new(
                "handle_event_output".to_string(),
                "event filtered out".to_string(),
                broker_request.clone(),
            )
            .emit_debug();
            return true;
        }

        if let Some(decorator_method) = broker_request.rule.transform.event_decorator_method.clone()
        {
            if let Some(func) = event_utility.get_function(&decorator_method) {
                LogSignal::new(
                    "handle_event_output".to_string(),
                    "event decorator method found".to_string(),
                    rpc_request.ctx.clone(),
                )
                .emit_debug();
                let session_id = rpc_request.ctx.get_id();
                let request_id = rpc_request.ctx.call_id;
                let protocol = rpc_request.ctx.protocol.clone();
                let platform_state_c = platform_state.clone();
                let ctx = rpc_request.ctx.clone();
                let mut response_c = response.clone();
                tokio::spawn(async move {
                    if let Ok(value) =
                        func(platform_state_c.clone(), ctx.clone(), Some(result.clone())).await
                    {
                        response_c.result = Some(value.expect("REASON"));
                    }
                    response_c.id = Some(request_id);
                    let message = ApiMessage::new(
                        protocol,
                        serde_json::to_string(&response_c).unwrap(),
                        ctx.request_id.clone(),
                    );
                    if let Some(session) = platform_state_c
                        .session_state
                        .get_session_for_connection_id(&session_id)
                    {
                        let _ = session.send_json_rpc(message).await;
                    }
                });
                return true;
            } else {
                LogSignal::new(
                    "handle_event_output".to_string(),
                    "event decorator method not found".to_string(),
                    rpc_request.ctx.clone(),
                )
                .emit_debug();
                error!("Failed to invoke decorator method {:?}", decorator_method);
            }
        }
        false
    }

    fn handle_subscription_response(
        broker_request: &BrokerRequest,
        rpc_request: &RpcRequest,
        response: &mut JsonRpcApiResponse,
        sub_processed: bool,
        platform_state: &PlatformState,
        id: u64,
    ) -> bool {
        LogSignal::new(
            "handle_subscription_response".to_string(),
            "entered".to_string(),
            broker_request.clone(),
        )
        .emit_debug();

        if sub_processed {
            LogSignal::new(
                "handle_subscription_response".to_string(),
                "subscription already processed".to_string(),
                broker_request.clone(),
            )
            .emit_debug();
            return true;
        }
        response.result = Some(json!({
            "listening" : rpc_request.is_listening(),
            "event" : rpc_request.ctx.method
        }));
        platform_state.endpoint_state.update_unsubscribe_request(id);

        LogSignal::new(
            "handle_subscription_response".to_string(),
            "subscription response set".to_string(),
            broker_request.clone(),
        )
        .emit_debug();
        false
    }

    fn apply_response_transform(
        broker_request: &BrokerRequest,
        output: &BrokerOutput,
        response: &mut JsonRpcApiResponse,
        rule_context_name: &str,
    ) {
        LogSignal::new(
            "apply_response_transform".to_string(),
            "entered".to_string(),
            broker_request.clone(),
        )
        .emit_debug();

        let mut apply_response_using_main_req_needed = true;
        if let Some(params) = output.data.params.clone() {
            if let Some(param) = params.as_object() {
                for (key, value) in param {
                    if key == "response" {
                        if let Some(filter) = value.as_str() {
                            apply_response_using_main_req_needed = false;
                            apply_response(
                                filter.to_string(),
                                &broker_request.rpc.ctx.method,
                                response,
                            );
                        }
                    }
                }
            }
        }
        if apply_response_using_main_req_needed {
            if let Some(filter) = broker_request
                .rule
                .transform
                .get_transform_data(super::rules::rules_engine::RuleTransformType::Response)
            {
                apply_response(filter, rule_context_name, response);
            } else if response.result.is_none() && response.error.is_none() {
                response.result = Some(Value::Null);
            }
        }
    }

    async fn handle_service_message(
        rpc_request: &RpcRequest,
        message: &ApiMessage,
        platform_state: &PlatformState,
    ) {
        let context = rpc_request.ctx.clone().context;
        if context.len() < 2 {
            error!("Context does not contain a valid service id");
            return;
        }
        let service_id = context[1].to_string();
        let service_sender = platform_state
            .service_controller_state
            .get_sender(&service_id)
            .await;
        if let Some(sender) = service_sender {
            let json_rpc_response =
                serde_json::from_str::<serde_json::Value>(message.jsonrpc_msg.clone().as_str())
                    .unwrap();

            let result = json_rpc_response.get("result").cloned().unwrap_or_default();
            let jsonrpc = serde_json::to_string(
                &json_rpc_response
                    .get("jsonrpc")
                    .cloned()
                    .unwrap_or_default(),
            )
            .unwrap();
            let id = ServiceMessageId::String(message.request_id.clone());

            let service_message = ServiceMessage {
                message: ServiceJsonRpcMessage::Success(ServiceJsonRpcSuccess {
                    result,
                    jsonrpc,
                    id,
                }),
                context: Some(serde_json::to_value(rpc_request.ctx.clone()).unwrap_or_default()),
            };
            let msg_str = serde_json::to_string(&service_message).unwrap();
            let mes = Message::Text(msg_str.clone());
            debug!(
                "Sending response to service {} message: {:?}",
                service_id, mes
            );
            if let Err(err) = sender.try_send(mes) {
                error!(
                    "Failed to send request to service {}: {:?}",
                    service_id, err
                );
            } else {
                debug!("Successfully sent request to service: {}", service_id);
            }
        }
    }

    async fn handle_event(
        platform_state: PlatformState,
        event_handler: EventHandler,
        broker_request: BrokerRequest,
        rpc_request: RpcRequest,
        mut response: JsonRpcApiResponse,
    ) {
        let session_id = rpc_request.ctx.get_id();
        let request_id = rpc_request.ctx.call_id;
        let protocol = rpc_request.ctx.protocol.clone();
        let platform_state_c = platform_state.clone();

        let params = if let Some(request) = event_handler.params {
            if let Ok(map) = serde_json::from_str::<serde_json::Map<String, Value>>(&request) {
                Some(Value::Object(map))
            } else {
                None
            }
        } else {
            None
        };

        if let Ok(event_handler_response) = BrokerUtils::process_internal_main_request(
            &platform_state_c,
            event_handler.method.as_str(),
            params,
        )
        .await
        {
            if let Ok(event_handler_response_string) =
                serde_json::to_string(&event_handler_response)
            {
                if let Some(mut event_filter) = broker_request.rule.transform.get_transform_data(
                    super::rules::rules_engine::RuleTransformType::Event(
                        rpc_request.ctx.context.contains(&RPC_V2.into()),
                    ),
                ) {
                    event_filter = event_filter
                        .replace("$event_handler_response", &event_handler_response_string);

                    apply_rule_for_event(
                        &broker_request,
                        &event_handler_response,
                        &rpc_request,
                        &event_filter,
                        &mut response,
                    );
                } else {
                    response.result = Some(event_handler_response);
                }
            } else {
                error!("handle_event: Could not deserialize event handler response");
                response.result = Some(event_handler_response);
            }
        }

        response.id = Some(request_id);
        response.update_event_message(&rpc_request);

        let message = ApiMessage::new(
            protocol,
            serde_json::to_string(&response).unwrap(),
            rpc_request.ctx.request_id.clone(),
        );

        if let Some(session) = platform_state_c
            .session_state
            .get_session_for_connection_id(&session_id)
        {
            let _ = session.send_json_rpc(message).await;
        }
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

        let result = if !data.is_empty() {
            match serde_json::from_slice::<Value>(data) {
                Ok(v) => Some(v),
                Err(e) => {
                    error!("handle_non_jsonrpc_response: Error parsing data: e={:?}", e);
                    return Err(RippleError::ParseError);
                }
            }
        } else {
            None
        };

        debug!("result {:?}", result);
        // build JsonRpcApiResponse
        let data = JsonRpcApiResponse {
            jsonrpc: "2.0".to_owned(),
            id: Some(request.rpc.ctx.call_id),
            method,
            result,
            error: None,
            params: None,
        };
        BrokerOutputForwarder::send_json_rpc_response_to_broker(data, callback.clone());
        Ok(())
    }
    pub fn send_json_rpc_response_to_broker(
        json_rpc_api_response: JsonRpcApiResponse,
        callback: BrokerCallback,
    ) {
        tokio::spawn(async move {
            if let Err(e) = callback
                .sender
                .try_send(BrokerOutput::new(json_rpc_api_response))
            {
                error!("Error sending json rpc response to broker {:?}", e)
            }
        });
    }
    pub fn send_json_rpc_success_response_to_broker(
        json_rpc_api_success_response: JsonRpcApiResponse,
        callback: BrokerCallback,
    ) {
        tokio::spawn(async move {
            if let Err(err) = callback
                .sender
                .try_send(BrokerOutput::new(json_rpc_api_success_response))
            {
                error!(
                    "Error sending json rpc success response to broker {:?}",
                    err
                )
            }
        });
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

pub fn apply_response(
    result_response_filter: String,
    method: &str,
    response: &mut JsonRpcApiResponse,
) {
    match serde_json::to_value(response.clone()) {
        Ok(input) => {
            match jq_compile(
                input,
                &result_response_filter,
                format!("{}_response", method),
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
                    error!(
                        "jq compile error: e={:?}, filter={}, response={:?}",
                        e, result_response_filter, response
                    );
                }
            }
        }
        Err(e) => {
            response.error = Some(json!(e.to_string()));
            error!(
                "json rpc response error: e={:?}, filter={}, response={:?}",
                e, result_response_filter, response
            );
        }
    }
}

pub fn apply_rule_for_event(
    broker_request: &BrokerRequest,
    result: &Value,
    rpc_request: &RpcRequest,
    filter: &str,
    response: &mut JsonRpcApiResponse,
) {
    if let Ok(r) = jq_compile(
        result.clone(),
        filter,
        format!("{}_event", rpc_request.ctx.method),
    ) {
        LogSignal::new(
            "apply_rule_for_event".to_string(),
            "broker request found".to_string(),
            broker_request.clone(),
        )
        .with_diagnostic_context_item("success", "true")
        .with_diagnostic_context_item("result", r.to_string().as_str())
        .emit_debug();
        response.result = Some(r);
    } else {
        LogSignal::new(
            "apply_rule_for_event".to_string(),
            "broker request found".to_string(),
            broker_request.clone(),
        )
        .with_diagnostic_context_item("success", "false")
        .emit_debug();
    }
}

fn apply_filter(broker_request: &BrokerRequest, result: &Value, rpc_request: &RpcRequest) -> bool {
    if let Some(filter) = broker_request.rule.filter.clone() {
        if let Ok(r) = jq_compile(
            result.clone(),
            &filter,
            format!("{}_event filter", rpc_request.ctx.method),
        ) {
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
mod endpoint_broker_tests {
    use super::*;
    use crate::broker::rules::rules_engine::RuleTransform;
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
                        event_handler: None,
                        sources: None,
                    },
                    subscription_processed: None,
                    workflow_callback: None,
                    telemetry_response_listeners: vec![],
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
            let mut output = BrokerOutput::default();
            let output = output.with_jsonrpc_response(data.clone());
            assert!(!output.is_result());
            data.result = Some(serde_json::Value::Null);
            let mut output = BrokerOutput::default();
            let output = output.with_jsonrpc_response(data);
            assert!(output.is_result());
        }

        #[test]
        fn test_get_event() {
            let mut data = JsonRpcApiResponse::mock();
            data.method = Some("20.events".to_owned());
            let mut output = BrokerOutput::default();
            let output = output.with_jsonrpc_response(data);
            assert_eq!(20, output.get_event().unwrap())
        }
    }

    mod endpoint_broker_state {
        use ripple_sdk::{tokio, tokio::sync::mpsc::channel};

        use crate::{
            broker::rules::rules_engine::{RuleEngine, RuleSet},
            service::extn::ripple_client::RippleClient,
            state::{bootstrap_state::ChannelsState, ops_metrics_state::OpMetricState},
        };

        use super::EndpointBrokerState;
        use crate::broker::endpoint_broker::BrokerConnectRequest;
        use crate::broker::endpoint_broker::ATOMIC_ID;
        use crate::broker::rules::rules_engine::RuleEndpoint;
        use crate::broker::rules::rules_engine::RuleEndpointProtocol;
        use ripple_sdk::api::session::AccountSession;
        use std::{collections::HashMap, sync::atomic::Ordering};

        fn reset_counter(value: u64) {
            ATOMIC_ID.store(value, Ordering::SeqCst);
        }
        #[cfg(test)]
        mod get_next_id_tests {

            use serial_test::serial;

            use super::*;

            #[test]
            #[serial]
            fn test_get_next_id_initial_value() {
                // Reset the ATOMIC_ID to a known state for testing
                reset_counter(0);

                assert_eq!(
                    EndpointBrokerState::get_next_id(),
                    0,
                    "Expected initial ID to be 0"
                );

                assert_eq!(
                    EndpointBrokerState::get_next_id(),
                    1,
                    "Expected next ID to be 1"
                );
            }

            #[test]
            #[serial]
            fn test_get_next_id_increment() {
                // Reset the ATOMIC_ID to a known state for testing
                reset_counter(0);

                assert_eq!(
                    EndpointBrokerState::get_next_id(),
                    0,
                    "Expected first ID to be 0"
                );
                assert_eq!(
                    EndpointBrokerState::get_next_id(),
                    1,
                    "Expected second ID to be 1"
                );
                assert_eq!(
                    EndpointBrokerState::get_next_id(),
                    2,
                    "Expected third ID to be 2"
                );
            }

            #[test]
            #[serial]
            fn test_get_next_id_large_values() {
                // Set ATOMIC_ID to a large value
                reset_counter(u64::MAX);

                assert_eq!(
                    EndpointBrokerState::get_next_id(),
                    u64::MAX,
                    "Expected first ID to be u64::MAX - 1"
                );
                assert_eq!(
                    EndpointBrokerState::get_next_id(),
                    0,
                    "Expected second ID to be 0 after wrapping around"
                );
            }

            #[test]
            #[serial]
            fn test_get_next_id_wraparound_behavior() {
                // Set ATOMIC_ID to the maximum value
                reset_counter(u64::MAX);
                let _ = EndpointBrokerState::get_next_id();

                // In a real-world scenario, this would likely panic or wrap around.
                // For this test, we assume wrapping behavior.
                assert_eq!(
                    EndpointBrokerState::get_next_id(),
                    0,
                    "Expected ID to wrap around to 0"
                );
            }

            #[test]
            #[serial]
            fn test_get_next_id_thread_safety() {
                // Reset the ATOMIC_ID to a known state for testing
                reset_counter(0);

                let num_threads = 10;
                let num_iterations = 1000;
                let mut handles = vec![];

                for _ in 0..num_threads {
                    handles.push(std::thread::spawn(move || {
                        for _ in 0..num_iterations {
                            EndpointBrokerState::get_next_id();
                        }
                    }));
                }

                for handle in handles {
                    handle.join().unwrap();
                }

                assert!(
                    /*
                    this inequality is a "compromise" to deal with singleton counter being
                    non deterministically incremented in other tests, causing this one to fail
                    */
                    ATOMIC_ID.load(Ordering::SeqCst) >= (num_threads * num_iterations) as u64,
                    "Expected final ID to match the total number of increments"
                );
            }
        }

        #[tokio::test]
        async fn test_build_endpoint_http() {
            let (tx, _) = channel(2);
            let client = RippleClient::new(ChannelsState::new());
            let mut state = EndpointBrokerState::new(
                OpMetricState::default(),
                tx,
                RuleEngine {
                    rules: RuleSet::default(),
                    functions: HashMap::default(),
                },
                client,
            );

            let endpoint = RuleEndpoint {
                protocol: RuleEndpointProtocol::Http,
                ..Default::default()
            };

            let request = BrokerConnectRequest::new(
                "http_endpoint".to_string(),
                endpoint.clone(),
                state.reconnect_tx.clone(),
            );

            state.build_endpoint(None, request);

            let endpoints = state.get_endpoints();
            assert!(endpoints.contains_key("http_endpoint"));
        }

        #[tokio::test]
        async fn test_build_endpoint_websocket() {
            let (tx, _) = channel(2);
            let client = RippleClient::new(ChannelsState::new());
            let mut state = EndpointBrokerState::new(
                OpMetricState::default(),
                tx,
                RuleEngine {
                    rules: RuleSet::default(),
                    functions: HashMap::default(),
                },
                client,
            );

            let endpoint = RuleEndpoint {
                protocol: RuleEndpointProtocol::Websocket,
                ..Default::default()
            };

            let request = BrokerConnectRequest::new(
                "websocket_endpoint".to_string(),
                endpoint.clone(),
                state.reconnect_tx.clone(),
            );

            state.build_endpoint(None, request);

            let endpoints = state.get_endpoints();
            assert!(endpoints.contains_key("websocket_endpoint"));
        }

        #[tokio::test]
        async fn test_build_endpoint_thunder() {
            let (tx, _) = channel(2);
            let client = RippleClient::new(ChannelsState::new());
            let mut state = EndpointBrokerState::new(
                OpMetricState::default(),
                tx,
                RuleEngine {
                    rules: RuleSet::default(),
                    functions: HashMap::default(),
                },
                client,
            );

            let endpoint = RuleEndpoint {
                protocol: RuleEndpointProtocol::Thunder,
                ..Default::default()
            };

            let request = BrokerConnectRequest::new(
                "thunder_endpoint".to_string(),
                endpoint.clone(),
                state.reconnect_tx.clone(),
            );

            state.build_endpoint(None, request);

            let endpoints = state.get_endpoints();
            assert!(endpoints.contains_key("thunder_endpoint"));
        }

        #[tokio::test]
        async fn test_build_endpoint_workflow() {
            let (tx, _) = channel(2);
            let client = RippleClient::new(ChannelsState::new());
            let mut state = EndpointBrokerState::new(
                OpMetricState::default(),
                tx,
                RuleEngine {
                    rules: RuleSet::default(),
                    functions: HashMap::default(),
                },
                client,
            );

            let endpoint = RuleEndpoint {
                protocol: RuleEndpointProtocol::Workflow,
                ..Default::default()
            };

            let request = BrokerConnectRequest::new(
                "workflow_endpoint".to_string(),
                endpoint.clone(),
                state.reconnect_tx.clone(),
            );

            state.build_endpoint(None, request);

            let endpoints = state.get_endpoints();
            assert!(endpoints.contains_key("workflow_endpoint"));
        }

        #[tokio::test]
        async fn test_build_endpoint_extn() {
            let (tx, _) = channel(2);
            let client = RippleClient::new(ChannelsState::new());
            let mut state = EndpointBrokerState::new(
                OpMetricState::default(),
                tx,
                RuleEngine {
                    rules: RuleSet::default(),
                    functions: HashMap::default(),
                },
                client,
            );

            let endpoint = RuleEndpoint {
                protocol: RuleEndpointProtocol::Extn,
                ..Default::default()
            };

            let request = BrokerConnectRequest::new(
                "extn_endpoint".to_string(),
                endpoint.clone(),
                state.reconnect_tx.clone(),
            );

            state.build_endpoint(None, request);

            let endpoints = state.get_endpoints();
            assert!(endpoints.contains_key("extn_endpoint"));
        }

        #[tokio::test]
        async fn test_build_endpoint_with_cleaner() {
            let (tx, _) = channel(2);
            let client = RippleClient::new(ChannelsState::new());
            let mut state = EndpointBrokerState::new(
                OpMetricState::default(),
                tx,
                RuleEngine {
                    rules: RuleSet::default(),
                    functions: HashMap::default(),
                },
                client,
            );

            let endpoint = RuleEndpoint {
                protocol: RuleEndpointProtocol::Websocket,
                ..Default::default()
            };

            let request = BrokerConnectRequest::new(
                "websocket_with_cleaner".to_string(),
                endpoint.clone(),
                state.reconnect_tx.clone(),
            );

            state.build_endpoint(None, request);

            let cleaners = state.cleaner_list.read().unwrap();
            assert!(!cleaners.is_empty());
        }

        #[tokio::test]
        async fn test_build_endpoint_duplicate_key() {
            let (tx, _) = channel(2);
            let client = RippleClient::new(ChannelsState::new());
            let mut state = EndpointBrokerState::new(
                OpMetricState::default(),
                tx,
                RuleEngine {
                    rules: RuleSet::default(),
                    functions: HashMap::default(),
                },
                client,
            );

            let endpoint = RuleEndpoint {
                protocol: RuleEndpointProtocol::Http,
                ..Default::default()
            };

            let request = BrokerConnectRequest::new(
                "duplicate_key".to_string(),
                endpoint.clone(),
                state.reconnect_tx.clone(),
            );

            state.build_endpoint(None, request.clone());
            state.build_endpoint(None, request);

            let endpoints = state.get_endpoints();
            assert_eq!(endpoints.len(), 1);
            assert!(endpoints.contains_key("duplicate_key"));
        }

        #[tokio::test]
        async fn test_build_endpoint_with_session() {
            let (tx, _) = channel(2);
            let client = RippleClient::new(ChannelsState::new());
            let mut state = EndpointBrokerState::new(
                OpMetricState::default(),
                tx,
                RuleEngine {
                    rules: RuleSet::default(),
                    functions: HashMap::default(),
                },
                client,
            );

            let endpoint = RuleEndpoint {
                protocol: RuleEndpointProtocol::Extn,
                ..Default::default()
            };

            let session = Some(AccountSession::default());
            let request = BrokerConnectRequest::new_with_sesssion(
                "extn_with_session".to_string(),
                endpoint.clone(),
                state.reconnect_tx.clone(),
                session.clone(),
            );

            state.build_endpoint(None, request);

            let endpoints = state.get_endpoints();
            assert!(endpoints.contains_key("extn_with_session"));
        }

        // #[tokio::test]
        // async fn get_request() {
        //     let (tx, _) = channel(2);
        //     let client = RippleClient::new(ChannelsState::new());
        //     let state = EndpointBrokerState::new(
        //         MetricsState::default(),
        //         tx,
        //         RuleEngine {
        //             rules: RuleSet::default(),
        //         },
        //         client,
        //     );
        //     let mut request = RpcRequest::mock();
        //     state.update_request(
        //         &request,
        //         Rule {
        //             alias: "somecallsign.method".to_owned(),
        //             transform: RuleTransform::default(),
        //             endpoint: None,
        //             filter: None,
        //             event_handler: None,
        //             sources: None,
        //         },
        //         None,
        //         None,
        //         vec![],
        //     );
        //     request.ctx.call_id = 2;
        //     state.update_request(
        //         &request,
        //         Rule {
        //             alias: "somecallsign.method".to_owned(),
        //             transform: RuleTransform::default(),
        //             endpoint: None,
        //             filter: None,
        //             event_handler: None,
        //             sources: None,
        //         },
        //         None,
        //         None,
        //         vec![],
        //     );

        //     // Hardcoding the id here will be a problem as multiple tests uses the atomic id and there is no guarantee
        //     // that this test case would always be the first one to run
        //     // Revisit this test case, to make it more robust
        //     // assert!(state.get_request(2).is_ok());
        //     // assert!(state.get_request(1).is_ok());
        // }
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
        let mut output: BrokerOutput = BrokerOutput::new(data.clone());
        let filter = "if .result and .result.success then (.result.stbVersion | split(\"_\") [0]) elif .error then if .error.code == -32601 then {error: { code: -1, message: \"Unknown method.\" }} else \"Error occurred with a different code\" end else \"No result or recognizable error\" end".to_string();
        //let mut response = JsonRpcApiResponse::mock();
        //response.error = Some(error);
        apply_response(filter, &rpc_request.ctx.method, &mut output.data);
        //let msg = output.data.error.unwrap().get("message").unwrap().clone();
        assert_eq!(
            output.data.error.unwrap().get("message").unwrap().clone(),
            json!("Unknown method.".to_string())
        );

        // securestorage.get code 22 in error response
        let error = json!({"code":22,"message":"test error code 22"});
        let mut data = JsonRpcApiResponse::mock();
        data.error = Some(error);
        let mut output: BrokerOutput = BrokerOutput::new(data);
        let filter = "if .result and .result.success then .result.value elif .error.code==22 or .error.code==43 then null else .error end".to_string();

        apply_response(filter, &rpc_request.ctx.method, &mut output.data);
        assert_eq!(output.data.error, None);
        assert_eq!(output.data.result.unwrap(), serde_json::Value::Null);

        // securestorage.get code other than 22 or 43 in error response
        let error = json!({"code":300,"message":"test error code 300"});
        let mut data = JsonRpcApiResponse::mock();
        data.error = Some(error.clone());
        let mut output: BrokerOutput = BrokerOutput::new(data);
        let filter = "if .result and .result.success then .result.value elif .error.code==22 or .error.code==43 then null else { error: .error } end".to_string();
        apply_response(filter, &rpc_request.ctx.method, &mut output.data);
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
        let mut output: BrokerOutput = BrokerOutput::new(data.clone());
        apply_response(filter, &rpc_request.ctx.method, &mut output.data);
        assert_eq!(output.data.result.unwrap(), "SCXI11BEI".to_string());

        // device.videoResolution
        let result = json!("Resolution1080P");
        let filter = "if .result then if .result | contains(\"480\") then ( [640, 480] ) elif .result | contains(\"576\") then ( [720, 576] ) elif .result | contains(\"1080\") then ( [1920, 1080] ) elif .result | contains(\"2160\") then ( [2160, 1440] ) end elif .error then if .error.code == -32601 then \"Unknown method.\" else \"Error occurred with a different code\" end else \"No result or recognizable error\" end".to_string();
        let mut response = JsonRpcApiResponse::mock();
        response.result = Some(result);
        apply_response(filter, &rpc_request.ctx.method, &mut response);
        assert_eq!(response.result.unwrap(), json!([1920, 1080]));

        // device.audio
        let result = json!({"currentAudioFormat":"DOLBY AC3","supportedAudioFormat":["NONE","PCM","AAC","VORBIS","WMA","DOLBY AC3","DOLBY AC4","DOLBY MAT","DOLBY TRUEHD","DOLBY EAC3 ATMOS","DOLBY TRUEHD ATMOS","DOLBY MAT ATMOS","DOLBY AC4 ATMOS","UNKNOWN"],"success":true});
        let filter = "if .result and .result.success then .result | {\"stereo\": (.supportedAudioFormat |  index(\"PCM\") > 0),\"dolbyDigital5.1\": (.supportedAudioFormat |  index(\"DOLBY AC3\") > 0),\"dolbyDigital5.1plus\": (.supportedAudioFormat |  index(\"DOLBY EAC3\") > 0),\"dolbyAtmos\": (.supportedAudioFormat |  index(\"DOLBY EAC3 ATMOS\") > 0)} elif .error then if .error.code == -32601 then \"Unknown method.\" else \"Error occurred with a different code\" end else \"No result or recognizable error\" end".to_string();
        let mut response = JsonRpcApiResponse::mock();
        response.result = Some(result);
        apply_response(filter, &rpc_request.ctx.method, &mut response);
        assert_eq!(
            response.result.unwrap(),
            json!({"dolbyAtmos": true, "dolbyDigital5.1": true, "dolbyDigital5.1plus": false, "stereo": true})
        );

        // device.network
        let result = json!({"interfaces":[{"interface":"ETHERNET","macAddress":
        "f0:46:3b:5b:eb:14","enabled":true,"connected":false},{"interface":"WIFI","macAddress
        ":"f0:46:3b:5b:eb:15","enabled":true,"connected":true}],"success":true});

        let filter = "if .result and .result.success then (.result.interfaces | .[] | select(.connected) | {\"state\": \"connected\",\"type\": .interface | ascii_downcase }) elif .error then if .error.code == -32601 then \"Unknown method.\" else \"Error occurred with a different code\" end else \"No result or recognizable error\" end".to_string();
        let mut response = JsonRpcApiResponse::mock();
        response.result = Some(result);
        apply_response(filter, &rpc_request.ctx.method, &mut response);
        assert_eq!(
            response.result.unwrap(),
            json!({"state":"connected", "type":"wifi"})
        );

        // device.name
        let result = json!({"friendlyName": "my_device","success":true});
        let filter = "if .result.success then (if .result.friendlyName | length == 0 then \"Living Room\" else .result.friendlyName end) else \"Living Room\" end".to_string();
        let mut response = JsonRpcApiResponse::mock();
        response.result = Some(result);
        apply_response(filter, &rpc_request.ctx.method, &mut response);
        assert_eq!(response.result.unwrap(), json!("my_device"));

        // localization.language
        let result = json!({"success": true, "value": "{\"update_time\":\"2024-07-29T20:23:29.539132160Z\",\"value\":\"FR\"}"});
        let filter = "if .result.success then (.result.value | fromjson | .value) else \"en\" end"
            .to_string();
        let mut response = JsonRpcApiResponse::mock();
        response.result = Some(result);
        apply_response(filter, &rpc_request.ctx.method, &mut response);

        assert_eq!(response.result.unwrap(), json!("FR"));

        // secondscreen.friendlyName
        let result = json!({"friendlyName": "my_device","success":true});
        let filter = "if .result.success then (if .result.friendlyName | length == 0 then \"Living Room\" else .result.friendlyName end) else \"Living Room\" end".to_string();
        let mut response = JsonRpcApiResponse::mock();
        response.result = Some(result);
        apply_response(filter, &rpc_request.ctx.method, &mut response);

        assert_eq!(response.result.unwrap(), json!("my_device"));

        // advertising.setSkipRestriction
        let result = json!({"success":true});
        let filter = "if .result.success then null else { code: -32100, message: \"couldn't set skip restriction\" } end".to_string();
        let mut response = JsonRpcApiResponse::mock();
        response.result = Some(result);
        apply_response(filter, &rpc_request.ctx.method, &mut response);

        assert_eq!(response.result.unwrap(), serde_json::Value::Null);

        // securestorage.get
        let result = json!({"value": "some_value","success": true,"ttl": 100});
        let filter = "if .result.success then .result.value elif .error.code==22 or .error.code==43 then \"null\" else .error end".to_string();
        let mut response = JsonRpcApiResponse::mock();
        response.result = Some(result);
        apply_response(filter, &rpc_request.ctx.method, &mut response);
        assert_eq!(response.result.unwrap(), "some_value");

        // localization.countryCode
        let result = json!({"territory": "USA","success": true});
        let filter = "if .result.success then if .result.territory == \"ITA\" then \"IT\" elif .result.territory == \"GBR\" then \"GB\" elif .result.territory == \"IRL\" then \"IE\" elif .result.territory == \"DEU\" then \"DE\" elif .result.territory == \"AUS\" then \"AU\" else \"GB\" end end".to_string();
        let mut response = JsonRpcApiResponse::mock();
        response.result = Some(result);
        apply_response(filter, &rpc_request.ctx.method, &mut response);
        assert_eq!(response.result.unwrap(), "GB");
    }
    #[cfg(test)]
    mod static_rules {
        use std::collections::HashMap;

        use crate::broker::endpoint_broker::apply_response;
        use crate::broker::endpoint_broker::BrokerConnectRequest;
        use crate::broker::endpoint_broker::BrokerOutput;
        use crate::broker::endpoint_broker::EndpointBrokerState;
        use crate::broker::rules::rules_engine::RuleEndpoint;
        use crate::broker::rules::rules_engine::RuleEndpointProtocol;
        use crate::broker::rules::rules_engine::RuleEngine;
        use crate::broker::rules::rules_engine::RuleSet;
        use crate::broker::rules::rules_engine::{Rule, RuleTransform};
        use crate::service::extn::ripple_client::RippleClient;
        use crate::state::bootstrap_state::ChannelsState;
        use crate::state::ops_metrics_state::OpMetricState;
        use ripple_sdk::api::gateway::rpc_gateway_api::JsonRpcApiResponse;
        use ripple_sdk::api::gateway::rpc_gateway_api::RpcRequest;

        use ripple_sdk::tokio;
        use ripple_sdk::tokio::sync::mpsc::channel;
        use ripple_sdk::Mockable;
        use serde_json::json;
        use serde_json::Value;
        use serial_test::serial;

        #[serial]
        #[tokio::test]
        pub async fn test_static_rule_happy_path() {
            //write this test case to test static rule happy path
            let (tx, _) = channel(2);
            let client = RippleClient::new(ChannelsState::new());
            let mut state = EndpointBrokerState::new(
                OpMetricState::default(),
                tx,
                RuleEngine {
                    rules: RuleSet::default(),
                    functions: HashMap::default(),
                },
                client,
            );
            let endpoint = RuleEndpoint {
                protocol: RuleEndpointProtocol::Http,
                ..Default::default()
            };
            let request = BrokerConnectRequest::new(
                "http_endpoint".to_string(),
                endpoint.clone(),
                state.reconnect_tx.clone(),
            );
            state.build_endpoint(None, request);
            let mut data = JsonRpcApiResponse::mock();
            data.result = Some(
                json!( { "success" : true, "stbVersion":"SCXI11BEI_VBN_24Q3_sprint_20240717150752sdy_FG","receiverVersion":""} ),
            );
            let mut output: BrokerOutput = BrokerOutput::new(data.clone());
            let filter = "if .result and .result.success then (.result.stbVersion | split(\"_\") [0]) elif .error then if .error.code == -32601 then {error: { code: -1, message: \"Unknown method.\" }} else \"Error occurred with a different code\" end else \"No result or recognizable error\" end".to_string();
            let mut response = JsonRpcApiResponse::mock();
            response.result = data.result;
            let mut rpc_request = RpcRequest::mock();
            rpc_request.ctx.call_id = 2;
            let rule = Rule {
                alias: "somecallsign.method".to_owned(),
                transform: RuleTransform::default(),
                endpoint: None,
                filter: None,
                event_handler: None,
                sources: None,
            };
            state.update_request(&rpc_request, &rule, None, None, vec![]);
            apply_response(filter, &rpc_request.ctx.method, &mut output.data);
            assert_eq!(output.data.result.unwrap(), "SCXI11BEI".to_string());
        }
        #[serial]
        #[tokio::test]
        pub async fn test_static_rule_no_success_field() {
            let (tx, _) = channel(2);
            let client = RippleClient::new(ChannelsState::new());
            let mut state = EndpointBrokerState::new(
                OpMetricState::default(),
                tx,
                RuleEngine {
                    rules: RuleSet::default(),
                    functions: HashMap::default(),
                },
                client,
            );
            let endpoint = RuleEndpoint {
                protocol: RuleEndpointProtocol::Http,
                ..Default::default()
            };
            let request = BrokerConnectRequest::new(
                "http_endpoint".to_string(),
                endpoint.clone(),
                state.reconnect_tx.clone(),
            );
            state.build_endpoint(None, request);
            let mut data = JsonRpcApiResponse::mock();
            data.result =
                Some(json!({ "stbVersion": "SCXI11BEI_VBN_24Q3_sprint_20240717150752sdy_FG" }));
            let mut output: BrokerOutput = BrokerOutput::new(data.clone());
            let filter = "if .result and .result.success then (.result.stbVersion | split(\"_\") [0]) elif .error then if .error.code == -32601 then {error: { code: -1, message: \"Unknown method.\" }} else \"Error occurred with a different code\" end else \"No result or recognizable error\" end".to_string();
            let mut response = JsonRpcApiResponse::mock();
            response.result = data.result;
            let mut rpc_request = RpcRequest::mock();
            rpc_request.ctx.call_id = 2;
            let rule = Rule {
                alias: "somecallsign.method".to_owned(),
                transform: RuleTransform::default(),
                endpoint: None,
                filter: None,
                event_handler: None,
                sources: None,
            };
            state.update_request(&rpc_request, &rule, None, None, vec![]);
            apply_response(filter, &rpc_request.ctx.method, &mut output.data);
            assert_eq!(
                output.data.result.unwrap(),
                "No result or recognizable error"
            );
        }
        #[serial]
        #[tokio::test]
        pub async fn test_static_rule_error_code_handling() {
            let (tx, _) = channel(2);
            let client = RippleClient::new(ChannelsState::new());
            let mut state = EndpointBrokerState::new(
                OpMetricState::default(),
                tx,
                RuleEngine {
                    rules: RuleSet::default(),
                    functions: HashMap::default(),
                },
                client,
            );
            let endpoint = RuleEndpoint {
                protocol: RuleEndpointProtocol::Http,
                ..Default::default()
            };
            let request = BrokerConnectRequest::new(
                "http_endpoint".to_string(),
                endpoint.clone(),
                state.reconnect_tx.clone(),
            );
            state.build_endpoint(None, request);
            let mut data = JsonRpcApiResponse::mock();
            data.error = Some(json!({ "code": -32601, "message": "Method not found" }));
            let mut output: BrokerOutput = BrokerOutput::new(data.clone());
            let filter = "if .result and .result.success then (.result.stbVersion | split(\"_\") [0]) elif .error then if .error.code == -32601 then {error: { code: -1, message: \"Unknown method.\" }} else \"Error occurred with a different code\" end else \"No result or recognizable error\" end".to_string();
            let mut response = JsonRpcApiResponse::mock();
            response.error = data.error;
            let mut rpc_request = RpcRequest::mock();
            rpc_request.ctx.call_id = 2;
            let rule = Rule {
                alias: "somecallsign.method".to_owned(),
                transform: RuleTransform::default(),
                endpoint: None,
                filter: None,
                event_handler: None,
                sources: None,
            };
            state.update_request(&rpc_request, &rule, None, None, vec![]);
            apply_response(filter, &rpc_request.ctx.method, &mut output.data);
            assert_eq!(
                output.data.error.unwrap(),
                json!({ "code": -1, "message": "Unknown method." })
            );
        }
        #[serial]
        #[tokio::test]
        pub async fn test_static_rule_no_result_or_error() {
            let (tx, _) = channel(2);
            let client = RippleClient::new(ChannelsState::new());
            let mut state = EndpointBrokerState::new(
                OpMetricState::default(),
                tx,
                RuleEngine {
                    rules: RuleSet::default(),
                    functions: HashMap::default(),
                },
                client,
            );
            let endpoint = RuleEndpoint {
                protocol: RuleEndpointProtocol::Http,
                ..Default::default()
            };
            let request = BrokerConnectRequest::new(
                "http_endpoint".to_string(),
                endpoint.clone(),
                state.reconnect_tx.clone(),
            );
            state.build_endpoint(None, request);
            let data = JsonRpcApiResponse::mock();
            let mut output: BrokerOutput = BrokerOutput::new(data.clone());
            let filter = "if .result and .result.success then (.result.stbVersion | split(\"_\") [0]) elif .error then if .error.code == -32601 then {error: { code: -1, message: \"Unknown method.\" }} else \"Error occurred with a different code\" end else \"No result or recognizable error\" end".to_string();
            let mut response = JsonRpcApiResponse::mock();
            response.result = data.result;
            let mut rpc_request = RpcRequest::mock();
            rpc_request.ctx.call_id = 2;
            let rule = Rule {
                alias: "somecallsign.method".to_owned(),
                transform: RuleTransform::default(),
                endpoint: None,
                filter: None,
                event_handler: None,
                sources: None,
            };
            state.update_request(&rpc_request, &rule, None, None, vec![]);
            apply_response(filter, &rpc_request.ctx.method, &mut output.data);
            assert_eq!(
                output.data.result.unwrap(),
                "No result or recognizable error"
            );
        }
        #[serial]
        #[tokio::test]
        pub async fn test_static_rule_success_with_empty_stb_version() {
            let (tx, _) = channel(2);
            let client = RippleClient::new(ChannelsState::new());
            let mut state = EndpointBrokerState::new(
                OpMetricState::default(),
                tx,
                RuleEngine {
                    rules: RuleSet::default(),
                    functions: HashMap::default(),
                },
                client,
            );
            let endpoint = RuleEndpoint {
                protocol: RuleEndpointProtocol::Http,
                ..Default::default()
            };
            let request = BrokerConnectRequest::new(
                "http_endpoint".to_string(),
                endpoint.clone(),
                state.reconnect_tx.clone(),
            );
            state.build_endpoint(None, request);
            let mut data = JsonRpcApiResponse::mock();
            data.result = Some(json!({ "success": true, "stbVersion": "" }));
            let mut output: BrokerOutput = BrokerOutput::new(data.clone());
            let filter = "if .result and .result.success then (.result.stbVersion | split(\"_\") [0]) elif .error then if .error.code == -32601 then {error: { code: -1, message: \"Unknown method.\" }} else \"Error occurred with a different code\" end else \"No result or recognizable error\" end".to_string();
            let mut response = JsonRpcApiResponse::mock();
            response.result = data.result;
            let mut rpc_request = RpcRequest::mock();
            rpc_request.ctx.call_id = 2;
            let rule = Rule {
                alias: "somecallsign.method".to_owned(),
                transform: RuleTransform::default(),
                endpoint: None,
                filter: None,
                event_handler: None,
                sources: None,
            };
            state.update_request(&rpc_request, &rule, None, None, vec![]);
            apply_response(filter, &rpc_request.ctx.method, &mut output.data);
            assert_eq!(output.data.result.unwrap(), Value::Null);
        }
    }
    #[cfg(test)]
    mod provided_request {

        use std::collections::HashMap;

        use crate::{
            broker::{
                endpoint_broker::{EndpointBrokerState, RenderedRequest},
                rules::rules_engine::{Rule, RuleEngine, RuleSet},
            },
            service::extn::ripple_client::RippleClient,
            state::{bootstrap_state::ChannelsState, ops_metrics_state::OpMetricState},
        };
        use ripple_sdk::{
            api::gateway::rpc_gateway_api::RpcRequest, tokio::sync::mpsc::channel, Mockable,
        };
        #[test]
        fn test_basic() {
            let (tx, _) = channel(2);

            let client = RippleClient::new(ChannelsState::new());
            let mut engine = RuleEngine {
                rules: RuleSet::default(),
                functions: HashMap::default(),
            };
            let r = Rule {
                alias: "provided".to_owned(),
                transform: Default::default(),
                endpoint: Some("thunder".to_string()),
                filter: None,
                event_handler: None,
                sources: None,
            };
            engine.add_rule(r);

            let under_test = EndpointBrokerState::new(OpMetricState::default(), tx, engine, client);

            let f = under_test.handle_provided_request(
                &RpcRequest::mock(),
                EndpointBrokerState::get_next_id(),
                vec![],
                None,
            );
            match f {
                RenderedRequest::ProviderJsonRpc(jsonrpc) => {
                    assert!(jsonrpc.is_error());
                }
                e => panic!("invalid response={:?}", e),
            }
        }
    }

    #[cfg(test)]
    mod dispatch_brokerage {
        fn endpoint_broker_state_under_test(endpoints: Vec<String>) -> EndpointBrokerState {
            let (tx, _) = channel(2);
            let client = RippleClient::new(ChannelsState::new());
            let mut endpoint_broker = EndpointBrokerState::new(
                OpMetricState::default(),
                tx,
                RuleEngine {
                    rules: RuleSet::default(),
                    functions: HashMap::default(),
                },
                client,
            );
            for endpoint in endpoints {
                let (tx, _) = mpsc::channel::<BrokerRequest>(10);
                endpoint_broker.add_endpoint(endpoint, BrokerSender { sender: tx });
            }

            endpoint_broker
        }
        use std::collections::HashMap;

        use crate::{
            broker::{
                endpoint_broker::{
                    BrokerRequest, BrokerSender, EndpointBrokerState, HandleBrokerageError,
                },
                rules::rules_engine::{Rule, RuleEngine, RuleSet},
            },
            service::extn::ripple_client::RippleClient,
            state::{bootstrap_state::ChannelsState, ops_metrics_state::OpMetricState},
        };
        use ripple_sdk::{
            api::gateway::rpc_gateway_api::RpcRequest,
            extn::extn_client_message::ExtnMessage,
            tokio::{
                self,
                sync::mpsc::{self, channel},
            },
            Mockable,
        };

        #[tokio::test]
        async fn test_dispatch_brokerage_static_rule() {
            let under_test = endpoint_broker_state_under_test(vec!["thunder".to_string()])
                .add_rule(
                    Rule::default()
                        .with_alias("static".to_string())
                        .with_endpoint("thunder".to_string())
                        .to_owned(),
                );
            let broker_request = BrokerRequest::default();
            assert!(
                under_test
                    .render_brokered_request(
                        &Rule::default()
                            .with_alias("static".to_string())
                            .with_endpoint("thunder".to_string())
                            .to_owned(),
                        &broker_request,
                        vec![],
                        None
                    )
                    .is_ok(),
                "expected Ok"
            );
        }

        #[tokio::test]
        async fn test_dispatch_brokerage_provided_rule() {
            let (bs, _) = channel(2);
            let mut under_test = endpoint_broker_state_under_test(vec![]).add_rule(
                Rule::default()
                    .with_alias("provided".to_string())
                    .to_owned(),
            );
            let under_test =
                under_test.add_endpoint("thunder".to_string(), BrokerSender { sender: bs });
            let broker_request = BrokerRequest::default();

            assert!(
                under_test
                    .render_brokered_request(
                        &Rule::default()
                            .with_alias("provided".to_string())
                            .to_owned(),
                        &broker_request,
                        vec![],
                        None,
                    )
                    .is_ok(),
                "expected Ok but got Err"
            );
            assert!(
                under_test
                    .render_brokered_request(
                        &Rule::default()
                            .with_alias("provided".to_string())
                            .to_owned(),
                        &broker_request,
                        vec![],
                        None,
                    )
                    .is_ok(),
                "expected Ok but got Err"
            );
        }

        #[tokio::test]
        async fn test_dispatch_brokerage_endpoint_rule() {
            let (tx, _) = channel(2);
            let client = RippleClient::new(ChannelsState::new());
            let mut engine = RuleEngine {
                rules: RuleSet::default(),
                functions: HashMap::default(),
            };
            let rule = Rule {
                alias: "endpoint".to_owned(),
                transform: Default::default(),
                endpoint: Some("thunder".to_string()),
                filter: None,
                event_handler: None,
                sources: None,
            };
            engine.add_rule(rule);
            let mut under_test =
                EndpointBrokerState::new(OpMetricState::default(), tx, engine, client);

            let (tx, _) = mpsc::channel::<BrokerRequest>(10);
            under_test.add_endpoint("thunder".to_string(), BrokerSender { sender: tx });

            let mut request = RpcRequest::mock();
            request.method = "endpoint".to_string();

            let result =
                under_test.handle_brokerage_workflow(request, None, None, vec![], None, vec![]);
            assert!(result.is_ok(), "Expected Ok but got: {:?}", result);
        }

        #[tokio::test]
        async fn test_dispatch_brokerage_rule_not_found() {
            let (tx, _) = channel(2);
            let client = RippleClient::new(ChannelsState::new());
            let engine = RuleEngine {
                rules: RuleSet::default(),
                functions: HashMap::default(),
            };
            let under_test = EndpointBrokerState::new(OpMetricState::default(), tx, engine, client);

            let mut request = RpcRequest::mock();
            request.method = "nonexistent".to_string();

            let result =
                under_test.handle_brokerage_workflow(request, None, None, vec![], None, vec![]);
            assert!(
                matches!(result, Err(HandleBrokerageError::RuleNotFound(_))),
                "Expected RuleNotFound error but got: {:?}",
                result
            );
        }

        #[tokio::test]
        async fn test_dispatch_brokerage_broker_not_found() {
            let (tx, _) = channel(2);
            let client = RippleClient::new(ChannelsState::new());
            let mut engine = RuleEngine {
                rules: RuleSet::default(),
                functions: HashMap::default(),
            };
            let rule = Rule {
                alias: "endpoint".to_owned(),
                transform: Default::default(),
                endpoint: Some("nonexistent_broker".to_string()),
                filter: None,
                event_handler: None,
                sources: None,
            };
            engine.add_rule(rule);
            let under_test = EndpointBrokerState::new(OpMetricState::default(), tx, engine, client);

            let mut request = RpcRequest::mock();
            request.method = "endpoint".to_string();

            let result =
                under_test.handle_brokerage_workflow(request, None, None, vec![], None, vec![]);
            assert!(
                matches!(result, Err(HandleBrokerageError::BrokerNotFound(_))),
                "Expected BrokerNotFound error but got: {:?}",
                result
            );
        }
        #[cfg(test)]
        mod update_request {
            use ripple_sdk::{api::gateway::rpc_gateway_api::RpcRequest, tokio};

            use crate::{
                broker::{
                    endpoint_broker::BrokerCallback,
                    rules::rules_engine::{Rule, RuleSet, RuleTransform},
                },
                state::ops_metrics_state::OpMetricState,
            };

            use super::*;
            use ripple_sdk::tokio::sync::mpsc::channel;

            #[tokio::test]
            async fn test_update_request_basic() {
                let (tx, _) = channel(2);
                let client = RippleClient::new(ChannelsState::new());
                let state = EndpointBrokerState::new(
                    OpMetricState::default(),
                    tx,
                    RuleEngine {
                        rules: RuleSet::default(),
                        functions: HashMap::default(),
                    },
                    client,
                );

                let rpc_request = RpcRequest::mock();
                let rule = Rule {
                    alias: "test.method".to_owned(),
                    transform: RuleTransform::default(),
                    endpoint: None,
                    filter: None,
                    event_handler: None,
                    sources: None,
                };

                let broker_request = state.update_request(&rpc_request, &rule, None, None, vec![]);

                assert_eq!(broker_request.rule.alias, rule.alias);
            }

            #[tokio::test]
            async fn test_update_request_with_extn_message() {
                let (tx, _) = channel(2);
                let client = RippleClient::new(ChannelsState::new());
                let state = EndpointBrokerState::new(
                    OpMetricState::default(),
                    tx,
                    RuleEngine {
                        rules: RuleSet::default(),
                        functions: HashMap::default(),
                    },
                    client,
                );

                let rpc_request = RpcRequest::mock();
                let rule = Rule {
                    alias: "test.method".to_owned(),
                    transform: RuleTransform::default(),
                    endpoint: None,
                    filter: None,
                    event_handler: None,
                    sources: None,
                };
                let extn_message = Some(ExtnMessage::default());

                let broker_request =
                    state.update_request(&rpc_request, &rule, extn_message.clone(), None, vec![]);

                assert_eq!(broker_request.rule.alias, rule.alias);
            }

            #[tokio::test]
            async fn test_update_request_with_workflow_callback() {
                let (tx, _) = channel(2);
                let client = RippleClient::new(ChannelsState::new());
                let state = EndpointBrokerState::new(
                    OpMetricState::default(),
                    tx,
                    RuleEngine {
                        rules: RuleSet::default(),
                        functions: HashMap::default(),
                    },
                    client,
                );

                let rpc_request = RpcRequest::mock();
                let rule = Rule {
                    alias: "test.method".to_owned(),
                    transform: RuleTransform::default(),
                    endpoint: None,
                    filter: None,
                    event_handler: None,
                    sources: None,
                };
                let workflow_callback = Some(BrokerCallback::default());

                let broker_request = state.update_request(
                    &rpc_request,
                    &rule,
                    None,
                    workflow_callback.clone(),
                    vec![],
                );

                assert_eq!(broker_request.rule.alias, rule.alias);
                assert!(broker_request.workflow_callback.is_some());
            }

            #[tokio::test]
            async fn test_update_request_with_telemetry_response_listeners() {
                let (tx, _) = channel(2);
                let client = RippleClient::new(ChannelsState::new());
                let state = EndpointBrokerState::new(
                    OpMetricState::default(),
                    tx,
                    RuleEngine {
                        rules: RuleSet::default(),
                        functions: HashMap::default(),
                    },
                    client,
                );

                let rpc_request = RpcRequest::mock();
                let rule = Rule {
                    alias: "test.method".to_owned(),
                    transform: RuleTransform::default(),
                    endpoint: None,
                    filter: None,
                    event_handler: None,
                    sources: None,
                };
                let telemetry_response_listeners = vec![channel(2).0];

                let broker_request = state.update_request(
                    &rpc_request,
                    &rule,
                    None,
                    None,
                    telemetry_response_listeners.clone(),
                );

                assert_eq!(broker_request.rule.alias, rule.alias);
                assert_eq!(
                    broker_request.telemetry_response_listeners.len(),
                    telemetry_response_listeners.len()
                );
            }
        }
    }
    #[cfg(test)]
    mod cleaner {
        use ripple_sdk::tokio::{self, sync::mpsc};

        use crate::broker::endpoint_broker::BrokerCleaner;

        #[tokio::test]
        async fn test_cleanup_session_with_cleaner() {
            let (tx, mut rx) = mpsc::channel(1);
            let cleaner = BrokerCleaner { cleaner: Some(tx) };

            assert!(cleaner.cleanup_session("test_app").await.is_ok());
            let received = rx.recv().await;
            assert_eq!(received, Some("test_app".to_string()));
        }

        #[tokio::test]
        async fn test_cleanup_session_without_cleaner() {
            let cleaner = BrokerCleaner { cleaner: None };

            // Should not panic or send anything
            assert!(cleaner.cleanup_session("test_app").await.is_err());
        }
    }
    #[cfg(test)]
    mod workflow {
        // fn test_workflow() {
        //     let (tx, _) = channel(2);
        //     let client = RippleClient::new(ChannelsState::new());
        //     let mut state = EndpointBrokerState::new(
        //         MetricsState::default(),
        //         tx,
        //         RuleEngine {
        //             rules: RuleSet::default(),
        //         },
        //         client,
        //     );
        //     let endpoint = RuleEndpoint {
        //         protocol: RuleEndpointProtocol::Http,
        //         ..Default::default()
        //     };
        //     let request = BrokerConnectRequest::new(
        //         "http_endpoint".to_string(),
        //         endpoint.clone(),
        //         state.reconnect_tx.clone(),
        //     );
        //     state.build_endpoint(None, request);
        // }
    }
}
