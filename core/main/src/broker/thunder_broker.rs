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
use super::{
    endpoint_broker::{
        BrokerCallback, BrokerCleaner, BrokerConnectRequest, BrokerOutput, BrokerRequest,
        BrokerSender, BrokerSubMap, EndpointBroker, EndpointBrokerState,
        BROKER_CHANNEL_BUFFER_SIZE,
    },
    thunder::thunder_plugins_status_mgr::StatusManager,
    thunder::user_data_migrator::UserDataMigrator,
};
use crate::{broker::broker_utils::BrokerUtils, state::platform_state::PlatformState};
use futures_util::{SinkExt, StreamExt};
use ripple_sdk::{
    api::{
        gateway::rpc_gateway_api::{JsonRpcApiResponse, RpcRequest},
        observability::log_signal::LogSignal,
    },
    log::{debug, error, info, trace},
    tokio::{
        self,
        sync::{mpsc, Mutex},
        time,
    },
    utils::error::RippleError,
};
use serde_json::json;
use serde_json::Value;
use std::time::SystemTime;
use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
    time::Duration,
    vec,
};

pub const COMPOSITE_REQUEST_TIME_OUT: u64 = 8;

#[derive(Clone)]
pub struct ThunderBroker {
    sender: BrokerSender,
    subscription_map: Arc<RwLock<BrokerSubMap>>,
    cleaner: BrokerCleaner,
    status_manager: StatusManager,
    default_callback: BrokerCallback,
    data_migrator: Option<UserDataMigrator>,
    custom_callback_list: Arc<Mutex<HashMap<u64, BrokerCallback>>>,
    composite_request_list: Arc<Mutex<HashMap<u64, CompositeRequest>>>,
    composite_request_purge_started: Arc<Mutex<bool>>,
}

#[derive(Clone)]
pub struct CompositeRequest {
    pub time_stamp: SystemTime,
    pub rpc_request: RpcRequest,
}

impl CompositeRequest {
    pub fn new(time_stamp: SystemTime, rpc_request: RpcRequest) -> CompositeRequest {
        CompositeRequest {
            time_stamp,
            rpc_request,
        }
    }
}

impl ThunderBroker {
    fn new(
        sender: BrokerSender,
        subscription_map: Arc<RwLock<BrokerSubMap>>,
        cleaner: BrokerCleaner,
        default_callback: BrokerCallback,
    ) -> Self {
        Self {
            sender,
            subscription_map,
            cleaner,
            status_manager: StatusManager::new(),
            default_callback,
            data_migrator: None,
            custom_callback_list: Arc::new(Mutex::new(HashMap::new())),
            composite_request_list: Arc::new(Mutex::new(HashMap::new())),
            composite_request_purge_started: Arc::new(Mutex::new(false)),
        }
    }

    fn with_data_migtator(mut self) -> Self {
        self.data_migrator = UserDataMigrator::create();
        self
    }

    pub fn get_default_callback(&self) -> BrokerCallback {
        self.default_callback.clone()
    }

    pub async fn register_composite_request(&self, id: u64, request: RpcRequest) {
        let mut composite_request_list = self.composite_request_list.lock().await;
        let composite_req = CompositeRequest::new(SystemTime::now(), request);
        composite_request_list.insert(id, composite_req);
        let purge_thread_started = self.composite_request_purge_started.lock().await;
        if *purge_thread_started {
            self.start_purge_composite_request_timer();
        }
    }

    pub async fn unregister_composite_request(&self, id: u64) {
        let mut composite_request_list = self.composite_request_list.lock().await;
        composite_request_list.remove(&id);
    }

    async fn get_composite_request(&self, id: Option<u64>) -> Option<RpcRequest> {
        let rid = id?;
        let composite_request_list = self.composite_request_list.lock().await;
        composite_request_list
            .get(&rid)
            .map(|req| req.rpc_request.clone())
    }

    pub async fn register_custom_callback(&self, id: u64, callback: BrokerCallback) {
        let mut custom_callback_list = self.custom_callback_list.lock().await;
        custom_callback_list.insert(id, callback);
    }

    pub async fn unregister_custom_callback(&self, id: u64) {
        let mut custom_callback_list = self.custom_callback_list.lock().await;
        custom_callback_list.remove(&id);
    }

    async fn get_broker_callback(&self, id: Option<u64>) -> BrokerCallback {
        if id.is_none() {
            return self.default_callback.clone();
        }
        let custom_callback_list = self.custom_callback_list.lock().await;
        if let Some(callback) = custom_callback_list.get(&id.unwrap()) {
            return callback.clone();
        }
        self.default_callback.clone()
    }

    // Start a timer to purge individual composite request that are older than 8 seconds
    fn start_purge_composite_request_timer(&self) {
        let composite_request_list = self.composite_request_list.clone();
        let mut interval = time::interval(Duration::from_millis(3000));
        let purge_thread_started = self.composite_request_purge_started.clone();
        tokio::spawn(async move {
            *purge_thread_started.lock().await = true;
            debug!("Starting composite request purge timer");
            // iterate each individual composite request and check if timestamp is greater than 8 seconds
            loop {
                interval.tick().await;
                let mut composite_request_list = composite_request_list.lock().await;
                let mut keys_to_remove = Vec::new();
                for (key, value) in composite_request_list.iter() {
                    match value.time_stamp.elapsed() {
                        Ok(elapsed) => {
                            if elapsed.as_secs() > COMPOSITE_REQUEST_TIME_OUT {
                                keys_to_remove.push(*key);
                            }
                        }
                        Err(e) => {
                            error!("Error while calculating elapsed time {:?}", e);
                        }
                    }
                }
                // remove request from the list
                for key in keys_to_remove {
                    composite_request_list.remove(&key);
                    debug!("Removed composite request with id {}", key);
                }
                if composite_request_list.is_empty() {
                    *purge_thread_started.lock().await = false;
                    debug!("Composite request list is empty, stop timer");
                    break;
                }
            }
        });
    }

    fn start(
        request: BrokerConnectRequest,
        callback: BrokerCallback,
        platform_state: Option<PlatformState>,
    ) -> Self {
        let endpoint = request.endpoint.clone();
        let (broker_request_tx, mut broker_request_rx) = mpsc::channel(BROKER_CHANNEL_BUFFER_SIZE);
        let (c_tx, mut c_tr) = mpsc::channel(2);
        let broker_sender = BrokerSender {
            sender: broker_request_tx,
        };
        let subscription_map = Arc::new(RwLock::new(request.sub_map.clone()));
        let cleaner = BrokerCleaner {
            cleaner: Some(c_tx.clone()),
        };
        let thunder_broker =
            Self::new(broker_sender, subscription_map, cleaner, callback).with_data_migtator();
        let broker_c = thunder_broker.clone();
        let broker_for_cleanup = thunder_broker.clone();
        let broker_for_reconnect = thunder_broker.clone();
        tokio::spawn(async move {
            let (ws_tx, mut ws_rx) = BrokerUtils::get_ws_broker(&endpoint.get_url(), None).await;

            let ws_tx_wrap = Arc::new(Mutex::new(ws_tx));
            // send the first request to the broker. This is the controller statechange subscription request
            let status_request = broker_c
                .status_manager
                .generate_state_change_subscribe_request();
            {
                let mut ws_tx = ws_tx_wrap.lock().await;

                let _feed = ws_tx
                    .feed(tokio_tungstenite::tungstenite::Message::Text(
                        status_request.to_string(),
                    ))
                    .await;
                let _flush = ws_tx.flush().await;
            }
            if let Some(ps) = platform_state {
                if ps
                    .get_device_manifest()
                    .get_features()
                    .thunder_plugin_status_check_at_broker_start_up
                {
                    debug!("thunder plugin status check at broker startup");
                    //send thunder plugin status check request for all plugin
                    let status_check_request =
                        broker_c.status_manager.generate_plugin_status_request(None);
                    {
                        let mut ws_tx = ws_tx_wrap.lock().await;

                        let _feed = ws_tx
                            .feed(tokio_tungstenite::tungstenite::Message::Text(
                                status_check_request.to_string(),
                            ))
                            .await;
                        let _flush = ws_tx.flush().await;
                    }
                } else {
                    debug!("thunder plugin status check at broker startup is disabled");
                }
            }
            tokio::pin! {
                let read = ws_rx.next();
            }
            let diagnostic_context: Arc<Mutex<Option<BrokerRequest>>> = Arc::new(Mutex::new(None));
            loop {
                tokio::select! {

                    Some(value) = &mut read => {
                        /* receive response here */
                        match value {
                            Ok(v) => {

                                if let tokio_tungstenite::tungstenite::Message::Text(t) = v {
                                    debug!("Broker Websocket message {:?}", t);

                                    if broker_c.status_manager.is_controller_response(broker_c.get_sender(), broker_c.get_default_callback(), t.as_bytes()).await {
                                        broker_c.status_manager.handle_controller_response(broker_c.get_sender(), broker_c.get_default_callback(), t.as_bytes()).await;
                                    }
                                    else {
                                        // send the incoming text without context back to the sender
                                        let id = Self::get_id_from_result(t.as_bytes());
                                        let composite_resp_params = Self::get_composite_response_params_by_id(broker_c.clone(), id).await;
                                        let _ = Self::handle_jsonrpc_response(t.as_bytes(),broker_c.get_broker_callback(id).await, composite_resp_params);
                                    };
                                }
                            },
                            Err(e) => {
                                error!("Broker Websocket error on read {:?}", e);
                                // Time to reconnect Thunder with existing subscription
                                break;
                            }
                        }

                    },
                    Some(mut request) = broker_request_rx.recv() => {
                        debug!("Got request from receiver for broker {:?}", request);
                        diagnostic_context.lock().await.replace(request.clone());

                        match broker_c.check_and_generate_plugin_activation_request(&request) {
                            Ok(requests) => {
                                if !requests.is_empty() {
                                    LogSignal::new("thunder_broker".to_string(),"sending message to thunder".to_string(), request.rpc.ctx.clone())
                                    .with_diagnostic_context_item("updated_request", &format!("{:?}", requests))
                                    .emit_debug();

                                    let mut ws_tx = ws_tx_wrap.lock().await;
                                    for r in requests {
                                        let _feed = ws_tx.feed(tokio_tungstenite::tungstenite::Message::Text(r)).await;
                                        let _flush = ws_tx.flush().await;
                                    }
                                }
                                else {
                                    // empty request means plugin is activated and ready to process the request
                                    // Intercept the request for data migration
                                    let mut request_consumed = false;
                                    if let Some(user_data_migrator) = broker_c.data_migrator.clone() {
                                        request_consumed = user_data_migrator.intercept_broker_request(&broker_c, ws_tx_wrap.clone(), &mut request).await;
                                    }

                                    // If the request is not consumed by the data migrator, continue with the request
                                    if !request_consumed {

                                        match broker_c.prepare_request(&request) {
                                            Ok(updated_request) => {

                                                LogSignal::new("thunder_broker".to_string(),"sending message to thunder".to_string(), request.rpc.ctx.clone())
                                                    .with_diagnostic_context_item("updated_request", &format!("{:?}", updated_request))
                                                    .emit_debug();

                                                // Add composite request to thunder broker; this is for later params_json referencing when response is received
                                                // response key in params_json is used for response rule transformation.
                                                if !request.rpc.params_json.is_empty() {
                                                    let pp: &str = request.rpc.params_json.as_str();
                                                    let pp_json = &serde_json::from_str::<Value>(pp).unwrap();
                                                    for pp in pp_json.as_array().unwrap() {
                                                        for (key, _value) in pp.as_object().unwrap() {
                                                            if key == "response" {
                                                                broker_c.register_composite_request(request.rpc.ctx.call_id, request.rpc.clone()).await;
                                                            }
                                                        }
                                                    }
                                                }
                                                let binding = ws_tx_wrap.clone();
                                                let mut ws_tx = binding.lock().await;
                                                for r in updated_request {
                                                    let _ = ws_tx.feed(tokio_tungstenite::tungstenite::Message::Text(r)).await;

                                                    let _ = ws_tx.flush().await;
                                                }
                                            }
                                            Err(e) => {
                                                LogSignal::new("thunder_broker".to_string(), "Prepare request failed".to_string(), request.rpc.ctx.clone())
                                                    .with_diagnostic_context_item("error", &format!("{:?}", e))
                                                    .emit_error();
                                                broker_c.get_default_callback().send_error(request,e).await
                                            }
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                match e {
                                    RippleError::ServiceNotReady => {
                                        info!("Thunder Service not ready, request is now in pending list {:?}", request);
                                    },
                                    _ =>
                                    broker_c.get_default_callback().send_error(request,e).await
                                }
                            }
                        }

                },
                    Some(cleanup_request) = c_tr.recv() => {
                        let value = {
                            broker_for_cleanup.subscription_map.write().unwrap().remove(&cleanup_request)
                        };
                        if let Some(mut cleanup) = value {
                            let sender = broker_for_cleanup.get_sender();
                            while let Some(mut v) = cleanup.pop() {
                                v.rpc = v.rpc.get_unsubscribe();
                                if (sender.send(v).await).is_err() {
                                    error!("Cleanup Error for {}",&cleanup_request);
                                }
                            }

                        }

                    }
                    }
            }

            let mut reconnect_request = request.clone();
            // Thunder Disconnected try reconnecting.
            {
                let mut subs = broker_for_reconnect.subscription_map.write().unwrap();
                for (k, v) in subs.drain().take(1) {
                    let _ = reconnect_request.sub_map.insert(k, v);
                }
            }
            if request.reconnector.send(reconnect_request).await.is_err() {
                error!("Error reconnecting to thunder");
            }
        });
        thunder_broker
    }

    fn update_response(response: &JsonRpcApiResponse, params: Option<Value>) -> JsonRpcApiResponse {
        let mut new_response = response.clone();
        if response.params.is_some() {
            new_response.result = response.params.clone();
        }
        if let Some(p) = params {
            let _ = new_response.params.insert(p);
        }
        new_response
    }

    async fn get_composite_response_params_by_id(
        broker: ThunderBroker,
        id: Option<u64>,
    ) -> Option<Value> {
        /* Get composite req params by call_id*/
        let rpc_req = broker.get_composite_request(id).await;
        let mut new_param: Option<Value> = None;
        if let Some(request) = rpc_req {
            let pp: &str = request.params_json.as_str();
            let pp_json = &serde_json::from_str::<Value>(pp).unwrap();

            // iterate pp_json array and extract object with response key
            for pp in pp_json.as_array().unwrap() {
                for (key, value) in pp.as_object().unwrap() {
                    if key == "response" {
                        new_param = Some(json!({"response": value}));
                    }
                }
            }
        }
        // remove composite request from list
        if let Some(id) = id {
            broker.unregister_composite_request(id).await;
        }
        new_param
    }

    fn get_id_from_result(result: &[u8]) -> Option<u64> {
        serde_json::from_slice::<JsonRpcApiResponse>(result)
            .ok()
            .and_then(|data| data.id)
    }

    fn get_callsign_and_method_from_alias(alias: &str) -> (String, Option<&str>) {
        let mut collection: Vec<&str> = alias.split('.').collect();
        let method = collection.pop();

        // Check if the second-to-last element is a digit (version number)
        if let Some(&version) = collection.last() {
            if version.chars().all(char::is_numeric) {
                collection.pop(); // Remove the version number
            }
        }

        let callsign = collection.join(".");
        (callsign, method)
    }
    fn unsubscribe(&self, request: &BrokerRequest) -> Option<BrokerRequest> {
        let mut sub_map = self.subscription_map.write().unwrap();
        trace!(
            "Unsubscribing a listen request for session id: {:?}",
            request.rpc.ctx.session_id
        );
        let app_id = &request.rpc.ctx.session_id;
        let method = &request.rpc.ctx.method;
        let mut existing_request = None;
        if let Some(mut existing_requests) = sub_map.remove(app_id) {
            if let Some(i) = existing_requests
                .iter()
                .position(|x| x.rpc.ctx.method.eq_ignore_ascii_case(method))
            {
                existing_request = Some(existing_requests.remove(i));
            }
            let _ = sub_map.insert(app_id.clone(), existing_requests);
        }
        existing_request
    }

    fn subscribe(&self, request: &BrokerRequest) -> Option<BrokerRequest> {
        let mut sub_map = self.subscription_map.write().unwrap();
        let app_id = &request.rpc.ctx.session_id;
        let method = &request.rpc.ctx.method;
        let listen = request.rpc.is_listening();
        let mut response = None;
        debug!(
            "Initial subscription map of {:?} app_id {:?}",
            sub_map, app_id
        );

        if let Some(mut v) = sub_map.remove(app_id) {
            debug!("Subscription map after removing app {:?}", v);
            if let Some(i) = v
                .iter()
                .position(|x| x.rpc.ctx.method.eq_ignore_ascii_case(method))
            {
                debug!(
                    "Removing subscription for method {} for app {}",
                    method, app_id
                );
                response = Some(v.remove(i));
            }
            if listen {
                v.push(request.clone());
            }
            let _ = sub_map.insert(app_id.clone(), v);
        } else {
            let _ = sub_map.insert(app_id.clone(), vec![request.clone()]);
        }
        response
    }

    fn check_and_generate_plugin_activation_request(
        &self,
        rpc_request: &super::endpoint_broker::BrokerRequest,
    ) -> Result<Vec<String>, RippleError> {
        let mut requests = Vec::new();
        let (callsign, method) = Self::get_callsign_and_method_from_alias(&rpc_request.rule.alias);

        if method.is_none() {
            return Err(RippleError::InvalidInput);
        }
        // check if the plugin is activated.
        let status = match self.status_manager.get_status(callsign.clone()) {
            Some(v) => v.clone(),
            None => {
                self.status_manager
                    .add_broker_request_to_pending_list(callsign.clone(), rpc_request.clone());
                // PluginState is not available with StateManager,  create an internal thunder request to activate the plugin
                let request = self
                    .status_manager
                    .generate_plugin_status_request(Some(callsign.clone()));
                requests.push(request.to_string());
                return Ok(requests);
            }
        };

        if status.state.is_missing() {
            error!("Plugin {} is missing", callsign);
            return Err(RippleError::ServiceError);
        }

        if status.state.is_activating() {
            info!(
                "Plugin {} is activating Adding broker request to pending list",
                callsign
            );
            self.status_manager
                .add_broker_request_to_pending_list(callsign.clone(), rpc_request.clone());
            return Err(RippleError::ServiceNotReady);
        }

        if !status.state.is_activated() {
            // add the broker request to pending list
            self.status_manager
                .add_broker_request_to_pending_list(callsign.clone(), rpc_request.clone());
            // create an internal thunder request to activate the plugin
            let request = self
                .status_manager
                .generate_plugin_activation_request(callsign.clone());
            requests.push(request.to_string());
            return Ok(requests);
        }
        Ok(requests)
    }
}

impl EndpointBroker for ThunderBroker {
    fn get_broker(
        ps: Option<PlatformState>,
        request: BrokerConnectRequest,
        callback: BrokerCallback,
        _broker_state: &mut EndpointBrokerState,
    ) -> Self {
        Self::start(request, callback, ps)
    }

    fn get_sender(&self) -> BrokerSender {
        self.sender.clone()
    }

    fn get_cleaner(&self) -> BrokerCleaner {
        self.cleaner.clone()
    }

    fn prepare_request(
        &self,
        rpc_request: &super::endpoint_broker::BrokerRequest,
    ) -> Result<Vec<String>, RippleError> {
        let rpc = rpc_request.clone().rpc;
        let id = rpc.ctx.call_id;
        let (callsign, method) = Self::get_callsign_and_method_from_alias(&rpc_request.rule.alias);
        let mut requests = Vec::new();

        let method = method.unwrap();
        // Below chunk of code is basically for subscription where thunder needs some special care based on
        // the JsonRpc specification
        if rpc_request.rpc.is_subscription() && !rpc_request.rpc.is_unlisten() {
            let listen = rpc_request.rpc.is_listening();
            // If there was an existing app and method combo for the same subscription just unregister that
            if let Some(cleanup) = self.subscribe(rpc_request) {
                requests.push(
                    json!({
                        "jsonrpc": "2.0",
                        "id": cleanup.rpc.ctx.call_id,
                        "method": format!("{}.unregister", callsign),
                        "params": {
                            "event": method,
                            "id": format!("{}", cleanup.rpc.ctx.call_id)
                        }
                    })
                    .to_string(),
                )
            }

            // Given unregistration is already performed by previous step just do registration
            if listen {
                requests.push(
                    json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "method": format!("{}.register", callsign),
                        "params": json!({
                            "event": method,
                            "id": format!("{}", id)
                        })
                    })
                    .to_string(),
                )
            }
        } else if rpc_request.rpc.is_unlisten() {
            if let Some(cleanup) = self.unsubscribe(rpc_request) {
                trace!(
                    "Unregistering thunder listener for call_id {} and method {}",
                    cleanup.rpc.ctx.call_id,
                    method
                );
                requests.push(
                    json!({
                        "jsonrpc": "2.0",
                        "id": cleanup.rpc.ctx.call_id,
                        "method": format!("{}.unregister", callsign),
                        "params": {
                            "event": method,
                            "id": format!("{}", cleanup.rpc.ctx.call_id)
                        }
                    })
                    .to_string(),
                )
            }
        } else {
            // Simple request and response handling
            requests.push(Self::update_request(rpc_request)?)
        }

        Ok(requests)
    }

    /// Default handler method for the broker to remove the context and send it back to the
    /// client for consumption
    fn handle_jsonrpc_response(
        result: &[u8],
        callback: BrokerCallback,
        params: Option<Value>,
    ) -> Result<BrokerOutput, RippleError> {
        let mut final_result = Err(RippleError::ParseError);
        if let Ok(data) = serde_json::from_slice::<JsonRpcApiResponse>(result) {
            let updated_data = Self::update_response(&data, params);
            final_result = Ok(BrokerOutput::new(updated_data));
        }
        if let Ok(output) = final_result.clone() {
            tokio::spawn(async move { callback.sender.send(output).await });
        } else {
            error!("Bad broker response {}", String::from_utf8_lossy(result));
        }
        final_result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        broker::{
            endpoint_broker::{
                apply_response, apply_rule_for_event, BrokerCallback, BrokerConnectRequest,
                BrokerOutput, BrokerRequest, EndpointBroker,
            },
            rules_engine::{self, Rule, RuleEndpoint, RuleEndpointProtocol, RuleTransform},
            test::mock_thunder_lite_server::MockThunderLiteServer,
        },
        create_and_send_broker_request, create_and_send_broker_request_with_jq_transform,
        process_broker_output, process_broker_output_event_resposne, read_broker_responses,
        setup_and_start_mock_thunder_lite_server,
        utils::test_utils::{MockWebsocket, WSMockData},
    };
    use ripple_sdk::{
        api::gateway::rpc_gateway_api::{ApiProtocol, CallContext, RpcRequest},
        uuid::Uuid,
    };
    use serde_json::json;
    use std::time::Duration;
    use tokio::sync::mpsc;

    #[macro_export]
    macro_rules! setup_thunder_broker {
        ($server_handle:expr) => {{
            let endpoint = RuleEndpoint {
                protocol: RuleEndpointProtocol::Thunder,
                url: $server_handle.get_address(),
                jsonrpc: true,
            };
            let (reconnect_tx, _rec_rx) = mpsc::channel(2);

            let request = BrokerConnectRequest::new("thunder".to_owned(), endpoint, reconnect_tx);
            let (tx, rx) = mpsc::channel(16);
            let callback = BrokerCallback { sender: tx };
            let mut endpoint_state = EndpointBrokerState::default();
            let thunder_broker =
                ThunderBroker::get_broker(None, request, callback, &mut endpoint_state);

            (thunder_broker, rx)
        }};
    }

    pub fn get_test_new_internal(method: String, params: Option<Value>) -> RpcRequest {
        let ctx = CallContext::new(
            Uuid::new_v4().to_string(),
            Uuid::new_v4().to_string(),
            "internal".into(),
            1,
            ApiProtocol::Extn,
            method.clone(),
            None,
            false,
        );
        RpcRequest {
            params_json: RpcRequest::prepend_ctx(params, &ctx),
            ctx,
            method,
        }
    }

    fn test_create_broker_request_with_jq_transform_fn(
        method: &str,
        alias: &str,
        call_id: u64,
        params: Option<Value>,
        transform: Option<RuleTransform>,
        event_filter: Option<String>,
        event_handler_fn: Option<String>,
    ) -> BrokerRequest {
        let mut broker_request = create_mock_broker_request(
            method,
            alias,
            params,
            transform,
            event_filter,
            event_handler_fn,
        );
        broker_request.rpc.ctx.call_id = call_id;
        broker_request
    }
    async fn test_send_broker_request_fn(
        thunder_broker: &ThunderBroker,
        request: &BrokerRequest,
    ) -> Result<(), RippleError> {
        let sender = thunder_broker.get_sender();
        sender.send(request.clone()).await
    }

    async fn test_read_single_response(
        rx: &mut mpsc::Receiver<BrokerOutput>,
    ) -> Result<BrokerOutput, String> {
        // wait 2 seconds for the response
        match tokio::time::timeout(Duration::from_secs(2), rx.recv()).await {
            Ok(Some(broker_output)) => Ok(broker_output),
            Ok(None) => Err("Received None instead of BrokerOutput".to_string()),
            Err(_) => Err("Timeout while waiting for response".to_string()),
        }
    }

    async fn get_thunderbroker(
        tx: mpsc::Sender<bool>,
        send_data: Vec<WSMockData>,
        sender: mpsc::Sender<BrokerOutput>,
        on_close: bool,
    ) -> ThunderBroker {
        // setup mock websocket server
        let port = MockWebsocket::start(send_data, Vec::new(), tx, on_close).await;

        let endpoint = RuleEndpoint {
            url: format!("ws://127.0.0.1:{}", port),
            protocol: crate::broker::rules_engine::RuleEndpointProtocol::Websocket,
            jsonrpc: false,
        };
        let (tx, _) = mpsc::channel(1);
        let request = BrokerConnectRequest::new("somekey".to_owned(), endpoint, tx);
        let callback = BrokerCallback { sender };
        ThunderBroker::get_broker(None, request, callback, &mut EndpointBrokerState::default())
    }

    //function to create a BrokerRequest
    fn create_mock_broker_request(
        method: &str,
        alias: &str,
        params: Option<Value>,
        transform: Option<RuleTransform>,
        event_filter: Option<String>,
        event_handler_fn: Option<String>,
    ) -> BrokerRequest {
        BrokerRequest {
            rpc: get_test_new_internal(method.to_owned(), params),
            rule: Rule {
                alias: alias.to_owned(),
                // if transform is not provided, use default
                transform: transform.unwrap_or_default(),
                endpoint: None,
                filter: event_filter,
                event_handler: event_handler_fn,
                sources: None,
            },
            subscription_processed: None,
            workflow_callback: None,
            telemetry_response_listeners: vec![],
        }
    }

    #[tokio::test]
    async fn test_thunder_brokerage() {
        // Set up and start the mock thunder lite server
        let server_handle = setup_and_start_mock_thunder_lite_server!(
            // entry for getter
            "org.rdk.mock_plugin.getter",
            Some(serde_json::json!({"value": "unknown"})),
            None,
            None,
            // entry for setter with event response
            "org.rdk.mock_plugin.setter",
            Some(serde_json::json!({"value": "check-event"})),
            None,
            Some((
                JsonRpcApiResponse {
                    jsonrpc: "2.0".to_string(),
                    result: Some(serde_json::Value::Null),
                    error: None,
                    id: Some(1000),
                    method: Some("org.rdk.mock_plugin.onValueChanged".to_string()),
                    params: Some(json!({"value": "ripple"})),
                },
                500 // event response generated after 500 milliseconds of setter response
            ))
        );

        let (thunder_broker, mut rx) = setup_thunder_broker!(server_handle);

        // Create and send the getter Broker request
        println!("[Tester] Calling FireboltModuleName.testGetter");
        create_and_send_broker_request!(
            thunder_broker,
            "FireboltModuleName.testGetter",
            "org.rdk.mock_plugin.getter",
            2000,
            None
        );

        // Create and send the setter Broker request
        println!("[Tester] Calling FireboltModuleName.testSetter");
        create_and_send_broker_request!(
            thunder_broker,
            "FireboltModuleName.testSetter",
            "org.rdk.mock_plugin.setter",
            3000,
            None
        );

        // Read the responses and assert that 3 responses are received
        read_broker_responses!(rx, 3);
    }

    #[tokio::test]
    async fn test_end_to_end_skip_restriction_set_get_eventing_with_jq_rules() {
        let server_handle = setup_and_start_mock_thunder_lite_server!(
            // entry for getter
            "org.rdk.PersistentStore.getValue",
            Some(
                serde_json::json!({"value":"{\"update_time\":\"2020-09-05T01:10:31.068338209+00:00\",\"value\":\"none\"}", "success": true})
            ),
            None,
            None,
            // entry for setter with event response
            "org.rdk.PersistentStore.setValue",
            Some(serde_json::json!({"success":true})),
            None,
            Some((
                JsonRpcApiResponse {
                    jsonrpc: "2.0".to_string(),
                    result: Some(serde_json::Value::Null),
                    error: None,
                    id: Some(1000),
                    method: Some("org.rdk.mock_plugin.onSkipRestrictionChanged".to_string()),
                    params: Some(
                        json!({"namespace":"Advertising","key":"skipRestriction","value":"{\"update_time\": \"2020-02-20T22:37:52.452943Z\",\"value\": \"all\"}"})
                    ),
                },
                500 // event response generated after 500 milliseconds of setter response
            ))
        );

        let (thunder_broker, mut rx) = setup_thunder_broker!(server_handle);

        // Create and send "advertising.skipRestriction" Broker request
        let transform = RuleTransform{
            request:Some("{ namespace: \"Advertising\", key: \"skipRestriction\"}".to_string()),
            response:Some("if .result and .result.success then (.result.value | fromjson | .value) else \"none\" end".to_string()), 
            event: Some("(.value | fromjson | .value)".to_string()),
            rpcv2_event: None,
            event_decorator_method: None
        };

        let broker_request = test_create_broker_request_with_jq_transform_fn(
            "advertising.skipRestriction",
            "org.rdk.PersistentStore.getValue",
            4000,
            None, // params
            Some(transform.clone()),
            None,
            None,
        );

        let result = test_send_broker_request_fn(&thunder_broker, &broker_request).await;
        assert!(result.is_ok());

        let broker_output = test_read_single_response(&mut rx).await;
        assert!(broker_output.is_ok());

        process_broker_output!(broker_request, broker_output);

        // Create and send "advertising.setSkipRestriction with event generation option"
        let transform = RuleTransform{
            request:Some("{ value: {update_time: now | todateiso8601, value: .value}, namespace: \"Advertising\", key: \"skipRestriction\"}".to_string()),
            response:Some("if .result and .result.success then null else { error: { code: -32100, message: \"couldn't set skip restriction\" }} end".to_string()), 
            event: None,
            rpcv2_event: None,
            event_decorator_method: None
        };

        create_and_send_broker_request_with_jq_transform!(
            thunder_broker,
           "advertising.setSkipRestriction",
            "org.rdk.PersistentStore.setValue",
            5000,
            Some(json!({"value": "adsUnwatched"})), // input params for advertising.setSkipRestriction 
            Some(transform.clone()),
            Some(json!("if .namespace == \"Advertising\" and .key == \"skipRestriction\" then true else false end").to_string()),
            None
        );
        //read the response of setter function
        let broker_output = test_read_single_response(&mut rx).await;
        assert!(broker_output.is_ok());

        //read the response of setter event
        let broker_output = test_read_single_response(&mut rx).await;
        assert!(broker_output.is_ok());

        process_broker_output_event_resposne!(broker_request, broker_output, Some(json!("all")));
    }

    #[tokio::test]
    async fn test_thunderbroker_get_cleaner() {
        let (tx, mut _rx) = mpsc::channel(1);
        let (sender, _rec) = mpsc::channel(1);
        let send_data = vec![WSMockData::get(json!({"key":"value"}).to_string(), None)];

        let thndr_broker = get_thunderbroker(tx, send_data, sender, false).await;
        assert!(thndr_broker.get_cleaner().cleaner.is_some());
    }

    #[tokio::test]
    async fn test_thunderbroker_handle_jsonrpc_response() {
        let (tx, mut _rx) = mpsc::channel(1);
        let (sender, mut rec) = mpsc::channel(1);
        let send_data = vec![WSMockData::get(json!({"key":"value"}).to_string(), None)];

        let _thndr_broker = get_thunderbroker(tx, send_data, sender.clone(), false).await;

        let response = json!({
            "jsonrpc": "2.0",
            "result": {
                "key": "value"
            }
        });

        assert!(ThunderBroker::handle_jsonrpc_response(
            response.to_string().as_bytes(),
            BrokerCallback {
                sender: sender.clone(),
            },
            None,
        )
        .is_ok());

        let v = tokio::time::timeout(Duration::from_secs(2), rec.recv())
            .await
            .expect("Timeout while waiting for response");

        assert!(v.is_some(), "Expected some value, but got None");
        let broker_output = v.unwrap();
        let data = broker_output
            .data
            .result
            .expect("No result in response data");
        let key_value = data.get("key").expect("Key not found");
        let key_str = key_value.as_str().expect("Value is not a string");
        assert_eq!(key_str, "value");
    }
    /*
        #[ignore]
        #[tokio::test]
        async fn test_thunderbroker_subscribe_unsubscribe() {
            let (tx, mut _rx) = mpsc::channel(1);
            let (sender, mut rec) = mpsc::channel(1);
            let send_data = vec![WSMockData::get(
                json!({
                    "jsonrpc": "2.0",
                    "method": "AcknowledgeChallenge.onRequestChallenge",
                    "params": {
                        "listen": true
                    }
                })
                .to_string(),
            )];

            let thndr_broker = get_thunderbroker(tx, send_data, sender.clone(), false).await;

            // Subscribe to an event
            let subscribe_request = BrokerRequest {
                rpc: RpcRequest::get_new_internal(
                    "AcknowledgeChallenge.onRequestChallenge".to_owned(),
                    Some(json!({"listen": true})),
                ),
                rule: Rule {
                    alias: "AcknowledgeChallenge.onRequestChallenge".to_owned(),
                    transform: RuleTransform::default(),
                    endpoint: None,
                    filter: None,
                    event_handler: None,
                    sources: None,
                },
                subscription_processed: Some(false),
                workflow_callback: None,
            };

            thndr_broker.subscribe(&subscribe_request);

            // Simulate receiving an event
            let response = json!({
                "jsonrpc": "2.0",
                "method": "AcknowledgeChallenge.onRequestChallenge",
                "params": {
                    "challenge": "The request to challenge the user"
                }
            });

            assert!(ThunderBroker::handle_jsonrpc_response(
                response.to_string().as_bytes(),
                BrokerCallback {
                    sender: sender.clone(),
                },
                None,
            )
            .is_ok());

            let v = tokio::time::timeout(Duration::from_secs(2), rec.recv())
                .await
                .expect("Timeout while waiting for response");

            if let Some(broker_output) = v {
                let data = broker_output
                    .data
                    .result
                    .expect("No result in response data");
                let challenge_value = data
                    .get("challenge")
                    .expect("Challenge not found in response data");
                let challenge_str = challenge_value.as_str().expect("Value is not a string");
                assert_eq!(challenge_str, "The request to challenge the user");
                assert_eq!(
                    broker_output.data.method.unwrap(),
                    "AcknowledgeChallenge.onRequestChallenge"
                );
            } else {
                panic!("Received None instead of a valid response");
            }

            // Unsubscribe from the event
            let unsubscribe_request = BrokerRequest {
                rpc: RpcRequest::get_unsubscribe(&subscribe_request.rpc),
                rule: Rule {
                    alias: "AcknowledgeChallenge.onRequestChallenge".to_owned(),
                    transform: RuleTransform::default(),
                    endpoint: None,
                    filter: None,
                    event_handler: None,
                    sources: None,
                },
                subscription_processed: Some(true),
                workflow_callback: None,
            };
            thndr_broker.subscribe(&unsubscribe_request);

            // Simulate receiving an event
            let response = json!({
                "jsonrpc": "2.0",
                "method": "AcknowledgeChallenge.onRequestChallenge",
                "params": {
                    "challenge": "The request to challenge the user"
                }
            });

            assert!(ThunderBroker::handle_jsonrpc_response(
                response.to_string().as_bytes(),
                BrokerCallback {
                    sender: sender.clone(),
                },
                None,
            )
            .is_ok());

            let v = tokio::time::timeout(Duration::from_secs(2), rec.recv())
                .await
                .expect("Timeout while waiting for response");

            if let Some(broker_output) = v {
                let _data = broker_output
                    .data
                    .result
                    .expect("No result in response data");
                assert_eq!(
                    broker_output.data.method.unwrap(),
                    "AcknowledgeChallenge.onRequestChallenge"
                );
            } else {
                panic!("Received None instead of a valid response");
            }
        }
    */
    #[tokio::test]
    async fn test_thunderbroker_update_response() {
        let response = JsonRpcApiResponse {
            jsonrpc: "2.0".to_string(),
            result: Some(json!({"key": "value"})),
            error: None,
            id: Some(1),
            params: Some(json!({"param_key": "param_value"})),
            method: None,
        };

        let updated_response = ThunderBroker::update_response(&response, None);
        assert_eq!(updated_response.result, response.params);
    }

    #[tokio::test]
    async fn test_thunderbroker_get_callsign_and_method_from_alias() {
        let alias = "plugin.method";
        let (callsign, method) = ThunderBroker::get_callsign_and_method_from_alias(alias);
        assert_eq!(callsign, "plugin");
        assert_eq!(method, Some("method"));

        let alias = "plugin2.";
        let (callsign, method) = ThunderBroker::get_callsign_and_method_from_alias(alias);
        assert_eq!(callsign, "plugin2");
        assert!(method.is_some());
    }

    #[tokio::test]
    async fn test_register_composite_request() {
        let server_handle = setup_and_start_mock_thunder_lite_server!();
        let (thunder_broker, _) = setup_thunder_broker!(server_handle);
        let broker_request = create_mock_broker_request(
            "FireboltModuleName.testGetter",
            "org.rdk.mock_plugin.getter",
            None,
            None,
            None,
            None,
        );

        thunder_broker
            .register_composite_request(1, broker_request.rpc.clone())
            .await;
        let composite_request_list = thunder_broker.composite_request_list.lock().await;
        assert_eq!(composite_request_list.len(), 1);
        assert_eq!(
            composite_request_list.get(&1).unwrap().rpc_request,
            broker_request.rpc
        );
    }

    #[tokio::test]
    async fn test_unregister_composite_request() {
        let server_handle = setup_and_start_mock_thunder_lite_server!();
        let (thunder_broker, _) = setup_thunder_broker!(server_handle);
        let broker_request = create_mock_broker_request(
            "FireboltModuleName.testGetter",
            "org.rdk.mock_plugin.getter",
            None,
            None,
            None,
            None,
        );

        thunder_broker
            .register_composite_request(1, broker_request.rpc.clone())
            .await;
        thunder_broker.unregister_composite_request(1).await;
        let composite_request_list = thunder_broker.composite_request_list.lock().await;
        assert_eq!(composite_request_list.len(), 0);
    }

    #[tokio::test]
    async fn test_start_purge_composite_request_timer() {
        let server_handle = setup_and_start_mock_thunder_lite_server!();
        let (thunder_broker, _) = setup_thunder_broker!(server_handle);
        let broker_request = create_mock_broker_request(
            "FireboltModuleName.testGetter",
            "org.rdk.mock_plugin.getter",
            None,
            None,
            None,
            None,
        );

        thunder_broker
            .register_composite_request(1, broker_request.rpc.clone())
            .await;
        thunder_broker.start_purge_composite_request_timer();
        let composite_request_list = thunder_broker.composite_request_list.lock().await;
        // Not waiting for the timer to expire, so the list should not be empty
        assert_eq!(composite_request_list.len(), 1);
    }

    #[tokio::test]
    async fn test_register_custom_callback() {
        let server_handle = setup_and_start_mock_thunder_lite_server!();
        let (thunder_broker, _) = setup_thunder_broker!(server_handle);

        let (tx, _rx) = mpsc::channel(1);
        let callback = BrokerCallback { sender: tx };

        thunder_broker.register_custom_callback(1, callback).await;
        let custom_callback_list = thunder_broker.custom_callback_list.lock().await;
        assert_eq!(custom_callback_list.len(), 1);
    }

    #[tokio::test]
    async fn test_unregister_custom_callback() {
        let server_handle = setup_and_start_mock_thunder_lite_server!();
        let (thunder_broker, _) = setup_thunder_broker!(server_handle);

        let (tx, _rx) = mpsc::channel(1);
        let callback = BrokerCallback { sender: tx };

        thunder_broker.register_custom_callback(1, callback).await;
        thunder_broker.unregister_custom_callback(1).await;
        let custom_callback_list = thunder_broker.custom_callback_list.lock().await;
        assert_eq!(custom_callback_list.len(), 0);
    }

    #[tokio::test]
    async fn test_subscribe() {
        let server_handle = setup_and_start_mock_thunder_lite_server!();
        let (thunder_broker, _) = setup_thunder_broker!(server_handle);
        let broker_request = create_mock_broker_request(
            "FireboltModuleName.onEvent",
            "org.rdk.mock_plugin.onValueChanged",
            Some(json!({"listen": true})),
            None,
            None,
            None,
        );

        thunder_broker.subscribe(&broker_request);
        let subscription_map = thunder_broker.subscription_map.write().unwrap();
        assert_eq!(subscription_map.len(), 1);
    }

    // Add test for unsubscribe
    #[tokio::test]
    async fn test_unsubscribe() {
        let server_handle = setup_and_start_mock_thunder_lite_server!();
        let (thunder_broker, _) = setup_thunder_broker!(server_handle);
        let subscribe_request = create_mock_broker_request(
            "FireboltModuleName.onEvent",
            "org.rdk.mock_plugin.onValueChanged",
            Some(json!({"listen": true})),
            None,
            None,
            None,
        );

        thunder_broker.subscribe(&subscribe_request);
        {
            let subscription_map = thunder_broker.subscription_map.write().unwrap();
            assert_eq!(subscription_map.len(), 1);
        }

        let mut unsubscribe_request = create_mock_broker_request(
            "FireboltModuleName.onEvent",
            "org.rdk.mock_plugin.onValueChanged",
            Some(json!({"listen": false})),
            None,
            None,
            None,
        );

        unsubscribe_request.rpc.ctx.session_id = subscribe_request.rpc.ctx.session_id.clone();
        unsubscribe_request.rpc.ctx.method = subscribe_request.rpc.ctx.method.clone();

        thunder_broker.unsubscribe(&unsubscribe_request);
        let subscription_map = thunder_broker.subscription_map.write().unwrap();
        // TBD : Check. Unsubscribe is re-inserting the subscription back to the map
        // let _ = sub_map.insert(app_id.clone(), existing_requests);
        assert_eq!(subscription_map.len(), 1);
    }
}
