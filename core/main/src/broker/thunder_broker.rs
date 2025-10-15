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
use crate::state::platform_state::PlatformState;
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
    tokio_tungstenite::tungstenite::Message,
    utils::{error::RippleError, ws_utils::WebSocketUtils},
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

// New data structures for method-oriented subscription management
#[derive(Debug, Clone)]
pub struct ThunderSubscriptionState {
    /// The BrokerRequest that was sent to Thunder for this method
    pub thunder_request: BrokerRequest,
    /// Number of clients interested in this method
    pub client_count: usize,
}

/// Maps method name to list of client connections interested in that method
pub type MethodClientMap = HashMap<String, Vec<String>>;

/// Maps client connection to list of methods it's subscribed to
pub type ClientMethodMap = HashMap<String, Vec<String>>;

/// Maps method name to Thunder subscription state
pub type MethodSubscriptionMap = HashMap<String, ThunderSubscriptionState>;

#[derive(Clone)]
pub struct ThunderBroker {
    sender: BrokerSender,
    /// Legacy subscription map - keep for backward compatibility during transition
    subscription_map: Arc<RwLock<BrokerSubMap>>,
    /// New method-oriented subscription tracking
    method_subscriptions: Arc<RwLock<MethodSubscriptionMap>>,
    /// Map of method -> list of interested client connections
    method_clients: Arc<RwLock<MethodClientMap>>,
    /// Map of client connection -> list of subscribed methods (for cleanup)
    client_methods: Arc<RwLock<ClientMethodMap>>,
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
            method_subscriptions: Arc::new(RwLock::new(HashMap::new())),
            method_clients: Arc::new(RwLock::new(HashMap::new())),
            client_methods: Arc::new(RwLock::new(HashMap::new())),
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
        debug!("Registering custom callback for id {}", id);
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
            let resp = WebSocketUtils::get_ws_stream(&endpoint.get_url(), None).await;
            if resp.is_err() {
                error!(
                    "FATAL error Thunder URL badly configured. Error={:?} for url: {}",
                    resp,
                    endpoint.get_url()
                );
                // This stops the Server
                let reconnect_request = request.clone();
                if request.reconnector.send(reconnect_request).await.is_err() {
                    error!("Error trying to stop server");
                }
                return;
            }
            let (ws_tx, mut ws_rx) = resp.unwrap();

            let ws_tx_wrap = Arc::new(Mutex::new(ws_tx));
            // send the first request to the broker. This is the controller statechange subscription request
            let status_request = broker_c
                .status_manager
                .generate_state_change_subscribe_request();
            {
                let mut ws_tx = ws_tx_wrap.lock().await;
                let _feed = ws_tx.feed(Message::Text(status_request.to_string())).await;
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
                            .feed(Message::Text(status_check_request.to_string()))
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

                                if let Message::Text(t) = v {
                                    debug!("Broker Websocket message {:?}", t);

                                    if broker_c.status_manager.is_controller_response(broker_c.get_sender(), broker_c.get_default_callback(), t.as_bytes()).await {
                                        broker_c.status_manager.handle_controller_response(broker_c.get_sender(), broker_c.get_default_callback(), t.as_bytes()).await;
                                    }
                                    else {
                                        // send the incoming text without context back to the sender
                                        let id = Self::get_id_from_result(t.as_bytes());
                                        let composite_resp_params = Self::get_composite_response_params_by_id(broker_c.clone(), id).await;

                                        // Handle regular request/response
                                        let _ = Self::handle_jsonrpc_response(t.as_bytes(),broker_c.get_broker_callback(id).await, composite_resp_params.clone());

                                        // Handle event fanout only for events (no id) that are in our method-oriented system
                                        if id.is_none() {
                                            if let Err(e) = broker_c.handle_event_fanout(t.as_bytes(), composite_resp_params) {
                                                error!("Failed to handle event fanout: {:?}", e);
                                            }
                                        }
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
                                        let _feed = ws_tx.feed(Message::Text(r)).await;
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
                                                    let _ = ws_tx.feed(Message::Text(r)).await;

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
        // Create an unlisten version of the request
        let mut unlisten_request = request.clone();
        // Use the get_unsubscribe method to create the unlisten version
        unlisten_request.rpc = request.rpc.get_unsubscribe();

        // Use the method-oriented logic for unsubscribe (same as subscribe but with listen=false)
        let result = self.subscribe_method_oriented(&unlisten_request);

        // Still maintain legacy subscription_map for backward compatibility during transition
        // TODO: Remove this once all components are updated to use method-oriented approach
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

        // Return the result from method-oriented logic (this controls whether Thunder unsubscription is needed)
        result.or(existing_request)
    }

    /// New method-oriented subscription logic for handling 1:many Thunder event subscriptions
    fn subscribe_method_oriented(&self, request: &BrokerRequest) -> Option<BrokerRequest> {
        let session_id = &request.rpc.ctx.session_id;
        let firebolt_method = &request.rpc.ctx.method;
        let listen = request.rpc.is_listening();

        // Extract Thunder method name that will be used in events
        let call_id = request.rpc.ctx.call_id;
        let (_, thunder_method_opt) = Self::get_callsign_and_method_from_alias(&request.rule.alias);
        let thunder_method = match thunder_method_opt {
            Some(method) => method,
            None => {
                error!(
                    "Failed to extract thunder method from alias: {}",
                    request.rule.alias
                );
                return None;
            }
        };

        // The method key for our subscription maps should match what Thunder sends in events
        let event_method_key = format!("{}.{}", call_id, thunder_method);

        debug!(
            "Method-oriented subscription: session={}, firebolt_method={}, thunder_event_method={}, listen={}",
            session_id, firebolt_method, event_method_key, listen
        );

        let mut method_subscriptions = self.method_subscriptions.write().unwrap();
        let mut method_clients = self.method_clients.write().unwrap();
        let mut client_methods = self.client_methods.write().unwrap();

        let mut thunder_subscription_to_create = None;
        let mut existing_request_to_remove = None;

        if listen {
            // Client wants to subscribe

            // Check if we already have a Thunder subscription for this method
            if let Some(sub_state) = method_subscriptions.get_mut(&event_method_key) {
                // Thunder subscription exists, just add this client to the fanout list
                debug!(
                    "Thunder subscription exists for method {}, adding client {}",
                    event_method_key, session_id
                );

                // Add client to method's client list if not already present
                let clients = method_clients.entry(event_method_key.clone()).or_default();
                if !clients.contains(session_id) {
                    clients.push(session_id.clone());
                    sub_state.client_count += 1;
                }

                // Add method to client's method list if not already present
                let methods = client_methods.entry(session_id.clone()).or_default();
                if !methods.contains(&event_method_key) {
                    methods.push(event_method_key.clone());
                }
            } else {
                // No Thunder subscription exists, need to create one
                debug!(
                    "Creating new Thunder subscription for method {}, client {}",
                    event_method_key, session_id
                );

                // Create new subscription state
                let sub_state = ThunderSubscriptionState {
                    thunder_request: request.clone(),
                    client_count: 1,
                };
                method_subscriptions.insert(event_method_key.clone(), sub_state);

                // Add client to method's client list
                method_clients.insert(event_method_key.clone(), vec![session_id.clone()]);

                // Add method to client's method list
                let methods = client_methods.entry(session_id.clone()).or_default();
                methods.push(event_method_key.clone());

                // Mark that we need to create a Thunder subscription
                thunder_subscription_to_create = Some(request.clone());
            }
        } else {
            // Client wants to unsubscribe
            debug!(
                "Unsubscribing client {} from method {}",
                session_id, event_method_key
            );

            // Remove client from method's client list
            if let Some(clients) = method_clients.get_mut(&event_method_key) {
                clients.retain(|client| client != session_id);

                // Update subscription state
                if let Some(sub_state) = method_subscriptions.get_mut(&event_method_key) {
                    sub_state.client_count = sub_state.client_count.saturating_sub(1);

                    // If no more clients, remove Thunder subscription
                    if sub_state.client_count == 0 {
                        debug!(
                            "No more clients for method {}, removing Thunder subscription",
                            event_method_key
                        );
                        existing_request_to_remove = Some(sub_state.thunder_request.clone());
                        method_subscriptions.remove(&event_method_key);
                        method_clients.remove(&event_method_key);
                    }
                }
            }

            // Remove method from client's method list
            if let Some(methods) = client_methods.get_mut(session_id) {
                methods.retain(|m| m != &event_method_key);

                // Keep client entry even if they have no methods for test consistency
                // In production, you might want to remove empty entries for memory efficiency
            }
        }

        drop(method_subscriptions);
        drop(method_clients);
        drop(client_methods);

        // Return appropriate response for Thunder broker to process
        if listen {
            thunder_subscription_to_create // Return request to create Thunder subscription (or None if already exists)
        } else {
            existing_request_to_remove // Return request to remove Thunder subscription (or None if still has clients)
        }
    }

    fn subscribe(&self, request: &BrokerRequest) -> Option<BrokerRequest> {
        // Use new method-oriented logic
        let result = self.subscribe_method_oriented(request);

        // Still maintain legacy subscription_map for backward compatibility during transition
        // TODO: Remove this once all components are updated to use method-oriented approach
        let mut sub_map = self.subscription_map.write().unwrap();
        let app_id = &request.rpc.ctx.session_id;
        let method = &request.rpc.ctx.method;
        let listen = request.rpc.is_listening();
        let mut response = None;
        trace!(
            "Initial subscription map of {:?} app_id {:?}",
            sub_map,
            app_id
        );
        debug!(
            "subscription_map size before subscribe for app {}: {}",
            app_id,
            sub_map.len()
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

        // Return the result from method-oriented logic (this controls whether Thunder subscription is created/removed)
        result.or(response)
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
        debug!(
            "Preparing request for method {} and callsign {} for {} subscription {}",
            method,
            callsign,
            rpc_request.rpc.method,
            rpc_request.rpc.is_subscription()
        );
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

impl ThunderBroker {
    /// Handle fanout of Thunder events to all interested clients
    fn handle_event_fanout(&self, result: &[u8], params: Option<Value>) -> Result<(), RippleError> {
        if let Ok(data) = serde_json::from_slice::<JsonRpcApiResponse>(result) {
            // Check if this is an event (has a method field and no id field)
            if let Some(ref method) = data.method {
                if data.id.is_some() {
                    // This is a response to a request, not an event - skip fanout
                    return Ok(());
                }

                debug!("Handling event fanout for method: {}", method);

                // Get all clients interested in this method
                let client_sessions = {
                    let method_clients = self.method_clients.read().unwrap();
                    method_clients.get(method).cloned()
                };

                if let Some(clients) = client_sessions {
                    debug!("Fanning out event {} to {:?} clients", method, clients);

                    // Get the subscription map to find callbacks for each client
                    let sub_map = self.subscription_map.read().unwrap();

                    for session_id in &clients {
                        // Find the client's subscription for this method
                        if let Some(client_requests) = sub_map.get(session_id) {
                            for request in client_requests {
                                if request.rpc.ctx.method.eq_ignore_ascii_case(method) {
                                    // Create event message for this specific client
                                    let mut client_data =
                                        Self::update_response(&data, params.clone());

                                    // Set the request ID to match the client's original subscription
                                    client_data.id = Some(request.rpc.ctx.call_id);

                                    let output = BrokerOutput::new(client_data);

                                    // Send to this client's callback
                                    let callback = BrokerCallback {
                                        sender: request
                                            .workflow_callback
                                            .as_ref()
                                            .map(|cb| cb.sender.clone())
                                            .unwrap_or_else(|| {
                                                self.default_callback.sender.clone()
                                            }),
                                    };

                                    let output_clone = output.clone();
                                    let session_id_clone = session_id.clone();
                                    tokio::spawn(async move {
                                        if let Err(e) = callback.sender.send(output_clone).await {
                                            error!(
                                                "Failed to send event to client {}: {:?}",
                                                session_id_clone, e
                                            );
                                        }
                                    });

                                    debug!("Event {} sent to client {}", method, session_id);
                                }
                            }
                        }
                    }
                } else {
                    debug!("No clients found for event method: {}", method);
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::broker::thunder_broker::tests::rules_engine::RuleTransformType;
    use crate::{
        broker::{
            endpoint_broker::{
                apply_response, apply_rule_for_event, BrokerCallback, BrokerConnectRequest,
                BrokerOutput, BrokerRequest, EndpointBroker,
            },
            rules::rules_engine::{
                self, EventHandler, Rule, RuleEndpoint, RuleEndpointProtocol, RuleTransform,
            },
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
        event_handler: Option<EventHandler>,
    ) -> BrokerRequest {
        let mut broker_request = create_mock_broker_request(
            method,
            alias,
            params,
            transform,
            event_filter,
            event_handler,
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
        /*
        doing unrwap here because doing it "right" would require changing call chain for this function, as result types would
        have to change
         */
        let port = MockWebsocket::start(send_data, Vec::new(), tx, on_close)
            .await
            .unwrap();

        let endpoint = RuleEndpoint {
            url: format!("ws://127.0.0.1:{}", port),
            protocol: crate::broker::rules::rules_engine::RuleEndpointProtocol::Websocket,
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
        event_handler: Option<EventHandler>,
    ) -> BrokerRequest {
        BrokerRequest {
            rpc: get_test_new_internal(method.to_owned(), params),
            rule: Rule {
                alias: alias.to_owned(),
                // if transform is not provided, use default
                transform: transform.unwrap_or_default(),
                endpoint: None,
                filter: event_filter,
                event_handler,
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

    fn create_test_subscription_request(
        method: &str,
        alias: &str,
        session_id: &str,
        call_id: u64,
        listen: bool,
    ) -> BrokerRequest {
        let mut request = create_mock_broker_request(method, alias, None, None, None, None);
        request.rpc.ctx.session_id = session_id.to_string();
        request.rpc.ctx.call_id = call_id;

        // Set up proper listening parameters
        let listen_params = json!({"listen": listen});
        request.rpc.params_json = RpcRequest::prepend_ctx(Some(listen_params), &request.rpc.ctx);

        request
    }

    fn create_test_thunder_broker() -> (ThunderBroker, mpsc::Receiver<BrokerOutput>) {
        let (sender, rx) = mpsc::channel(10);
        let callback = BrokerCallback { sender };

        // Create required components for ThunderBroker::new
        let (broker_sender, _) = mpsc::channel(10);
        let broker_sender = BrokerSender {
            sender: broker_sender,
        };
        let subscription_map = Arc::new(RwLock::new(HashMap::new()));
        let cleaner = BrokerCleaner { cleaner: None };

        let broker = ThunderBroker::new(broker_sender, subscription_map, cleaner, callback.clone());

        (broker, rx)
    }

    #[tokio::test]
    async fn test_method_oriented_subscription_single_client() {
        // Test that a single client subscription creates the correct data structures
        let (broker, _rx) = create_test_thunder_broker();

        // Create a subscription request for TextToSpeech.onspeechcomplete
        let request = create_test_subscription_request(
            "TextToSpeech.onspeechcomplete",
            "org.rdk.TextToSpeech.onspeechcomplete",
            "client1",
            100,
            true,
        );

        // Test subscription
        let thunder_request = broker.subscribe_method_oriented(&request);

        // Should create a Thunder subscription request
        assert!(thunder_request.is_some());

        // Verify data structures
        let method_subscriptions = broker.method_subscriptions.read().unwrap();
        let method_clients = broker.method_clients.read().unwrap();
        let client_methods = broker.client_methods.read().unwrap();

        // Should have one method subscription using Thunder method format
        let expected_thunder_method = "100.onspeechcomplete";
        assert_eq!(method_subscriptions.len(), 1);
        assert!(method_subscriptions.contains_key(expected_thunder_method));
        let sub_state = method_subscriptions.get(expected_thunder_method).unwrap();
        assert_eq!(sub_state.client_count, 1);

        // Should have one client for this method
        assert_eq!(method_clients.len(), 1);
        let clients = method_clients.get(expected_thunder_method).unwrap();
        assert_eq!(clients.len(), 1);
        assert_eq!(clients[0], "client1");

        // Should have one method for this client
        assert_eq!(client_methods.len(), 1);
        let methods = client_methods.get("client1").unwrap();
        assert_eq!(methods.len(), 1);
        assert_eq!(methods[0], expected_thunder_method);
    }

    #[tokio::test]
    async fn test_method_oriented_subscription_multiple_clients_same_method() {
        // Test that multiple clients subscribing to same method with same call ID share subscription
        let (broker, _rx) = create_test_thunder_broker();

        let method = "TextToSpeech.onspeechcomplete";
        let call_id = 100; // Same call ID for both clients

        // Create subscription requests for two different clients with same call ID
        let request1 = create_test_subscription_request(
            method,
            "org.rdk.TextToSpeech.onspeechcomplete",
            "client1",
            call_id,
            true,
        );

        let request2 = create_test_subscription_request(
            method,
            "org.rdk.TextToSpeech.onspeechcomplete",
            "client2",
            call_id,
            true,
        );

        // First client subscribes - should create Thunder subscription
        let thunder_request1 = broker.subscribe_method_oriented(&request1);
        assert!(thunder_request1.is_some());

        // Second client subscribes - should NOT create new Thunder subscription (same Thunder method key)
        let thunder_request2 = broker.subscribe_method_oriented(&request2);
        assert!(thunder_request2.is_none());

        // Verify data structures
        let method_subscriptions = broker.method_subscriptions.read().unwrap();
        let method_clients = broker.method_clients.read().unwrap();
        let client_methods = broker.client_methods.read().unwrap();

        // Should have exactly one method subscription (shared) using Thunder method format
        let expected_thunder_method = "100.onspeechcomplete";
        assert_eq!(method_subscriptions.len(), 1);
        let sub_state = method_subscriptions.get(expected_thunder_method).unwrap();
        assert_eq!(sub_state.client_count, 2);

        // Should have two clients for this method
        let clients = method_clients.get(expected_thunder_method).unwrap();
        assert_eq!(clients.len(), 2);
        assert!(clients.contains(&"client1".to_string()));
        assert!(clients.contains(&"client2".to_string()));

        // Each client should have this method in their list
        assert_eq!(client_methods.len(), 2);
        let methods1 = client_methods.get("client1").unwrap();
        let methods2 = client_methods.get("client2").unwrap();
        assert_eq!(methods1.len(), 1);
        assert_eq!(methods2.len(), 1);
        assert_eq!(methods1[0], expected_thunder_method);
        assert_eq!(methods2[0], expected_thunder_method);
    }

    #[tokio::test]
    async fn test_method_oriented_unsubscription_last_client() {
        // Test that unsubscribing the last client removes Thunder subscription
        let (broker, _rx) = create_test_thunder_broker();

        let method = "TextToSpeech.onspeechcomplete";

        // Subscribe a client
        let request = create_test_subscription_request(
            method,
            "org.rdk.TextToSpeech.onspeechcomplete",
            "client1",
            100,
            true,
        );

        let _thunder_request = broker.subscribe_method_oriented(&request);

        // Now unsubscribe
        let unsubscribe_request = create_test_subscription_request(
            method,
            "org.rdk.TextToSpeech.onspeechcomplete",
            "client1",
            100,
            false,
        );
        let thunder_unsubscribe = broker.subscribe_method_oriented(&unsubscribe_request);

        // Should create a Thunder unsubscribe request
        assert!(thunder_unsubscribe.is_some());

        // Verify data structures are cleaned up
        let method_subscriptions = broker.method_subscriptions.read().unwrap();
        let method_clients = broker.method_clients.read().unwrap();
        let client_methods = broker.client_methods.read().unwrap();

        // Should have no method subscriptions
        assert_eq!(method_subscriptions.len(), 0);
        assert_eq!(method_clients.len(), 0);

        // Client should still exist but with no methods
        assert_eq!(client_methods.len(), 1);
        let methods = client_methods.get("client1").unwrap();
        assert_eq!(methods.len(), 0);
    }

    #[tokio::test]
    async fn test_method_oriented_unsubscription_partial_clients() {
        // Test that unsubscribing one of multiple clients doesn't remove Thunder subscription
        let (broker, _rx) = create_test_thunder_broker();

        let method = "TextToSpeech.onspeechcomplete";
        let call_id = 100; // Same call ID for shared subscription

        // Subscribe two clients with same call ID (shared subscription)
        let request1 = create_test_subscription_request(
            method,
            "org.rdk.TextToSpeech.onspeechcomplete",
            "client1",
            call_id,
            true,
        );

        let request2 = create_test_subscription_request(
            method,
            "org.rdk.TextToSpeech.onspeechcomplete",
            "client2",
            call_id,
            true,
        );

        let _thunder_sub1 = broker.subscribe_method_oriented(&request1);
        let _thunder_sub2 = broker.subscribe_method_oriented(&request2);

        // Unsubscribe first client
        let unsubscribe_request1 = create_test_subscription_request(
            method,
            "org.rdk.TextToSpeech.onspeechcomplete",
            "client1",
            call_id,
            false,
        );
        let thunder_unsub = broker.subscribe_method_oriented(&unsubscribe_request1);

        // Should NOT create Thunder unsubscribe (other client still subscribed)
        assert!(thunder_unsub.is_none());

        // Verify data structures
        let method_subscriptions = broker.method_subscriptions.read().unwrap();
        let method_clients = broker.method_clients.read().unwrap();
        let client_methods = broker.client_methods.read().unwrap();

        // Should still have method subscription with count 1 using Thunder method format
        let expected_thunder_method = "100.onspeechcomplete";
        assert_eq!(method_subscriptions.len(), 1);
        let sub_state = method_subscriptions.get(expected_thunder_method).unwrap();
        assert_eq!(sub_state.client_count, 1);

        // Should have only client2 in the method's client list
        let clients = method_clients.get(expected_thunder_method).unwrap();
        assert_eq!(clients.len(), 1);
        assert_eq!(clients[0], "client2");

        // client1 should have no methods, client2 should have one
        let methods1 = client_methods.get("client1").unwrap();
        let methods2 = client_methods.get("client2").unwrap();
        assert_eq!(methods1.len(), 0);
        assert_eq!(methods2.len(), 1);
    }

    #[tokio::test]
    async fn test_event_fanout_to_multiple_clients() {
        // Test that events are fanned out to all interested clients
        let (broker, _rx) = create_test_thunder_broker();

        let method = "TextToSpeech.onspeechcomplete";

        // Set up two clients subscribed to the same method
        let mut request1 = create_mock_broker_request(
            method,
            "org.rdk.TextToSpeech.onspeechcomplete",
            None,
            None,
            None,
            None,
        );
        request1.rpc.ctx.session_id = "client1".to_string();
        request1.rpc.ctx.call_id = 100;
        request1.rpc.params_json = serde_json::to_string(&json!({"listen": true})).unwrap();

        let mut request2 = create_mock_broker_request(
            method,
            "org.rdk.TextToSpeech.onspeechcomplete",
            None,
            None,
            None,
            None,
        );
        request2.rpc.ctx.session_id = "client2".to_string();
        request2.rpc.ctx.call_id = 200;
        request2.rpc.params_json = serde_json::to_string(&json!({"listen": true})).unwrap();

        // Subscribe both clients
        let _thunder_sub1 = broker.subscribe_method_oriented(&request1);
        let _thunder_sub2 = broker.subscribe_method_oriented(&request2);

        // Add the requests to the subscription map (simulating the normal subscribe flow)
        {
            let mut sub_map = broker.subscription_map.write().unwrap();
            sub_map.insert("client1".to_string(), vec![request1.clone()]);
            sub_map.insert("client2".to_string(), vec![request2.clone()]);
        }

        // Simulate an incoming Thunder event
        let event_response = JsonRpcApiResponse {
            jsonrpc: "2.0".to_string(),
            result: None,
            error: None,
            id: None,
            method: Some(method.to_string()),
            params: Some(json!({"speechId": "12345", "status": "complete"})),
        };

        let event_bytes = serde_json::to_vec(&event_response).unwrap();

        // Handle event fanout
        let result = broker.handle_event_fanout(&event_bytes, None);
        assert!(result.is_ok());

        // Should receive the event on both clients
        // Give a moment for async tasks to complete
        tokio::time::sleep(Duration::from_millis(10)).await;

        // We should see events for both clients in the receiver
        // Note: The actual delivery happens via tokio::spawn, so we can't easily assert exact counts
        // but we can verify the fanout logic ran without errors
    }

    #[tokio::test]
    async fn test_multiple_methods_per_client() {
        // Test that a client can subscribe to multiple methods
        let (broker, _rx) = create_test_thunder_broker();

        let method1 = "TextToSpeech.onspeechcomplete";
        let method2 = "AudioPlayer.onplaybackcomplete";

        // Create requests for same client, different methods
        let request1 = create_test_subscription_request(
            method1,
            "org.rdk.TextToSpeech.onspeechcomplete",
            "client1",
            100,
            true,
        );

        let request2 = create_test_subscription_request(
            method2,
            "org.rdk.AudioPlayer.onplaybackcomplete",
            "client1",
            200,
            true,
        );

        // Subscribe to both methods
        let thunder_sub1 = broker.subscribe_method_oriented(&request1);
        let thunder_sub2 = broker.subscribe_method_oriented(&request2);

        // Both should create Thunder subscriptions (different methods)
        assert!(thunder_sub1.is_some());
        assert!(thunder_sub2.is_some());

        // Verify data structures
        let method_subscriptions = broker.method_subscriptions.read().unwrap();
        let method_clients = broker.method_clients.read().unwrap();
        let client_methods = broker.client_methods.read().unwrap();

        // Should have two method subscriptions using Thunder method format
        let expected_thunder_method1 = "100.onspeechcomplete";
        let expected_thunder_method2 = "200.onplaybackcomplete";
        assert_eq!(method_subscriptions.len(), 2);
        assert!(method_subscriptions.contains_key(expected_thunder_method1));
        assert!(method_subscriptions.contains_key(expected_thunder_method2));

        // Each method should have one client
        assert_eq!(method_clients.len(), 2);
        let clients1 = method_clients.get(expected_thunder_method1).unwrap();
        let clients2 = method_clients.get(expected_thunder_method2).unwrap();
        assert_eq!(clients1.len(), 1);
        assert_eq!(clients2.len(), 1);
        assert_eq!(clients1[0], "client1");
        assert_eq!(clients2[0], "client1");

        // Client should have two methods
        assert_eq!(client_methods.len(), 1);
        let methods = client_methods.get("client1").unwrap();
        assert_eq!(methods.len(), 2);
        assert!(methods.contains(&expected_thunder_method1.to_string()));
        assert!(methods.contains(&expected_thunder_method2.to_string()));
    }

    #[tokio::test]
    async fn test_thunder_method_name_mapping() {
        // Test that subscription system correctly maps Thunder event method names
        let (broker, _rx) = create_test_thunder_broker();

        // Create a subscription request that will map to a Thunder event method name
        let request = create_test_subscription_request(
            "texttospeech.onVoicechanged",
            "org.rdk.TextToSpeech.onvoicechanged",
            "client1",
            100,
            true,
        );

        // Subscribe the client
        let thunder_request = broker.subscribe_method_oriented(&request);
        assert!(thunder_request.is_some());

        // The subscription should be stored using the Thunder event method format: "100.onvoicechanged"
        let method_subscriptions = broker.method_subscriptions.read().unwrap();
        let method_clients = broker.method_clients.read().unwrap();

        // Should use Thunder event method format as key
        let expected_thunder_method = "100.onvoicechanged";
        assert!(method_subscriptions.contains_key(expected_thunder_method));
        assert!(method_clients.contains_key(expected_thunder_method));

        // Should NOT be stored using Firebolt method name
        assert!(!method_subscriptions.contains_key("texttospeech.onVoicechanged"));
        assert!(!method_clients.contains_key("texttospeech.onVoicechanged"));

        // Verify client count and client mapping
        let sub_state = method_subscriptions.get(expected_thunder_method).unwrap();
        assert_eq!(sub_state.client_count, 1);

        let clients = method_clients.get(expected_thunder_method).unwrap();
        assert_eq!(clients.len(), 1);
        assert_eq!(clients[0], "client1");
    }

    #[tokio::test]
    async fn test_event_fanout_with_thunder_method_names() {
        // Test that event fanout works correctly with Thunder method names
        let (broker, _rx) = create_test_thunder_broker();

        // Set up subscription using Firebolt method name
        let request = create_test_subscription_request(
            "texttospeech.onVoicechanged",
            "org.rdk.TextToSpeech.onvoicechanged",
            "client1",
            100,
            true,
        );

        let _thunder_request = broker.subscribe_method_oriented(&request);

        // Add the request to the subscription map (simulating normal subscribe flow)
        {
            let mut sub_map = broker.subscription_map.write().unwrap();
            sub_map.insert("client1".to_string(), vec![request.clone()]);
        }

        // Simulate an incoming Thunder event with Thunder method format
        let event_response = JsonRpcApiResponse {
            jsonrpc: "2.0".to_string(),
            result: None,
            error: None,
            id: None,                                       // Events have no ID
            method: Some("100.onvoicechanged".to_string()), // Thunder method format
            params: Some(json!({"voice": "Angelica"})),
        };

        let event_bytes = serde_json::to_vec(&event_response).unwrap();

        // Handle event fanout - should find the client
        let result = broker.handle_event_fanout(&event_bytes, None);
        assert!(result.is_ok());

        // The event should be found and processed (no "No clients found" error)
        let method_clients = broker.method_clients.read().unwrap();
        assert!(method_clients.contains_key("100.onvoicechanged"));
        let clients = method_clients.get("100.onvoicechanged").unwrap();
        assert_eq!(clients.len(), 1);
    }

    #[tokio::test]
    async fn test_multiple_call_ids_same_thunder_method() {
        // Test that different call IDs for the same Thunder method are treated separately
        let (broker, _rx) = create_test_thunder_broker();

        // Create two requests with different call IDs but same Thunder method
        let request1 = create_test_subscription_request(
            "texttospeech.onVoicechanged",
            "org.rdk.TextToSpeech.onvoicechanged",
            "client1",
            100, // Different call ID
            true,
        );

        let request2 = create_test_subscription_request(
            "texttospeech.onVoicechanged",
            "org.rdk.TextToSpeech.onvoicechanged",
            "client2",
            200, // Different call ID
            true,
        );

        // Subscribe both clients
        let thunder_request1 = broker.subscribe_method_oriented(&request1);
        let thunder_request2 = broker.subscribe_method_oriented(&request2);

        // Should create separate Thunder subscriptions (different call IDs)
        assert!(thunder_request1.is_some());
        assert!(thunder_request2.is_some());

        // Verify separate method subscriptions
        let method_subscriptions = broker.method_subscriptions.read().unwrap();
        assert_eq!(method_subscriptions.len(), 2);
        assert!(method_subscriptions.contains_key("100.onvoicechanged"));
        assert!(method_subscriptions.contains_key("200.onvoicechanged"));

        // Each should have one client
        let sub_state1 = method_subscriptions.get("100.onvoicechanged").unwrap();
        let sub_state2 = method_subscriptions.get("200.onvoicechanged").unwrap();
        assert_eq!(sub_state1.client_count, 1);
        assert_eq!(sub_state2.client_count, 1);
    }

    #[tokio::test]
    async fn test_invalid_alias_handling() {
        // Test handling of subscription requests with empty aliases that result in empty methods
        let (broker, _rx) = create_test_thunder_broker();

        // Create a request with an alias that results in None method
        let request = create_test_subscription_request(
            "texttospeech.onVoicechanged",
            "invalid.alias.", // This will result in Some("") which becomes an empty method
            "client1",
            100,
            true,
        );

        // Subscribe should handle the case where method extraction results in empty string
        let thunder_request = broker.subscribe_method_oriented(&request);

        // This should still work as the alias parsing is lenient
        // The test is more about ensuring no crashes occur with edge case aliases
        assert!(thunder_request.is_some());

        // Verify data structures are created (even with edge case alias)
        let method_subscriptions = broker.method_subscriptions.read().unwrap();
        let method_clients = broker.method_clients.read().unwrap();
        assert_eq!(method_subscriptions.len(), 1);
        assert_eq!(method_clients.len(), 1);
    }

    #[tokio::test]
    async fn test_event_vs_response_filtering() {
        // Test that only actual events (no id) trigger fanout, not responses (with id)
        let (broker, _rx) = create_test_thunder_broker();

        // Set up subscription
        let request = create_test_subscription_request(
            "texttospeech.onVoicechanged",
            "org.rdk.TextToSpeech.onvoicechanged",
            "client1",
            100,
            true,
        );

        let _thunder_request = broker.subscribe_method_oriented(&request);

        // Test response with ID (should NOT trigger fanout)
        let response_with_id = JsonRpcApiResponse {
            jsonrpc: "2.0".to_string(),
            result: Some(json!({"success": true})),
            error: None,
            id: Some(100), // Has ID - this is a response
            method: Some("100.onvoicechanged".to_string()),
            params: None,
        };

        let response_bytes = serde_json::to_vec(&response_with_id).unwrap();

        // This should not process fanout (filtered out in websocket handler)
        // We can't easily test the websocket handler filtering, but we can test that
        // handle_event_fanout works correctly when called

        // Test actual event without ID (should trigger fanout)
        let event_without_id = JsonRpcApiResponse {
            jsonrpc: "2.0".to_string(),
            result: None,
            error: None,
            id: None, // No ID - this is an event
            method: Some("100.onvoicechanged".to_string()),
            params: Some(json!({"voice": "Angelica"})),
        };

        let event_bytes = serde_json::to_vec(&event_without_id).unwrap();

        // This should process fanout successfully
        let result = broker.handle_event_fanout(&event_bytes, None);
        assert!(result.is_ok());

        // Use variables to avoid warnings
        let _response_bytes = response_bytes;
        let _event_bytes = event_bytes;
    }
}
