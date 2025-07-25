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
use ripple_sdk::api::context::{ActivationStatus, FeatureUpdate, RippleContext};
use ripple_sdk::api::context::{RippleContextUpdateRequest, RippleContextUpdateType};
use ripple_sdk::api::device::device_request::{
    AccountToken, InternetConnectionStatus, PowerState, TimeZone,
};
use ripple_sdk::service::service_message::JsonRpcNotification;
use std::{collections::HashMap, net::SocketAddr, sync::Arc};

use futures::{stream::SplitStream, SinkExt, StreamExt};
use ripple_sdk::api::gateway::rpc_gateway_api::{CallContext, JsonRpcApiResponse};
use ripple_sdk::{
    api::{gateway::rpc_gateway_api::ApiMessage, manifest::extn_manifest::ExtnSymbol},
    extn::{
        extn_client_message::{ExtnMessage, ExtnPayload, ExtnResponse},
        extn_id::ExtnId,
    },
    framework::ripple_contract::RippleContract,
    log::{debug, error, info, trace},
    service::service_message::{Id, JsonRpcMessage, JsonRpcSuccess, ServiceMessage},
    tokio::{
        self,
        net::TcpStream,
        sync::{mpsc, Mutex},
    },
    tokio_tungstenite::{tungstenite::Message, WebSocketStream},
    utils::error::RippleError,
    uuid::Uuid,
};

use crate::service::ripple_service::service_controller_state;
use crate::{
    broker::endpoint_broker::{BrokerCallback, BrokerOutput},
    firebolt::{firebolt_gateway::FireboltGatewayCommand, firebolt_ws::ClientIdentity},
    service::extn::ripple_client::RippleClient,
    state::{platform_state::PlatformState, session_state::Session},
};

use super::service_registry::ServiceRegistry;
use serde_json::Value;
const ALLOWED_SERVICES_LIST: [&str; 2] = [
    "ripple:channel:gateway:badger",
    "ripple:channel:distributor:eos",
];

#[derive(Debug, Clone)]
pub struct ServiceInfo {
    pub connection_id: String,
    pub tx: mpsc::Sender<Message>,
    pub is_sevice_registered: bool,
    callback_list: Arc<Mutex<HashMap<u64, BrokerCallback>>>,
}

#[derive(Debug, Clone, Default)]
pub struct ServiceControllerState {
    pub service_info: Arc<Mutex<ServiceRegistry>>,
}

impl ServiceInfo {
    pub fn new(
        connection_id: String,
        tx: mpsc::Sender<Message>,
        is_sevice_registered: bool,
    ) -> Self {
        ServiceInfo {
            connection_id,
            tx,
            is_sevice_registered,
            callback_list: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn add_callback(&mut self, request_id: u64, callback: BrokerCallback) {
        let mut callback_list = self.callback_list.lock().await;
        callback_list.insert(request_id, callback);
    }

    // add function to get and remove callbacks for a given request_id
    pub async fn get_and_remove_callback(&mut self, request_id: u64) -> Option<BrokerCallback> {
        let mut callback_list = self.callback_list.lock().await;
        callback_list.remove(&request_id)
    }
    pub async fn remove_callback(&mut self, request_id: u64) {
        let mut callback_list = self.callback_list.lock().await;
        callback_list.remove(&request_id);
    }
    pub async fn has_callback(&self, request_id: u64) -> bool {
        let callback_list = self.callback_list.lock().await;
        callback_list.contains_key(&request_id)
    }
    pub fn is_registered(&self) -> bool {
        self.is_sevice_registered
    }
    pub fn set_registered(&mut self, registered: bool) {
        self.is_sevice_registered = registered;
    }
    pub fn get_connection_id(&self) -> &str {
        &self.connection_id
    }
    pub fn get_sender(&self) -> &mpsc::Sender<Message> {
        &self.tx
    }
}

impl ServiceControllerState {
    pub fn new() -> Self {
        ServiceControllerState {
            service_info: Arc::new(Mutex::new(ServiceRegistry::default())),
        }
    }
    // Ripple Main processing the inbound ServiceMessage received from a service.
    // This is not the brokerage path.
    async fn process_inbound_service_message(
        state: &PlatformState,
        connection_id: &str,
        sm: &ServiceMessage,
        app_id: String,
        _session_id: String,
    ) {
        match &sm.message {
            JsonRpcMessage::Request(json_rpc_request) => {
                // In Ripple Service Architecture Ripple Main will not honor any request originated from any connected service that is not included in `ALLOWED_SERVICES_LIST`
                // other than service registration and unregistration request
                // (TBD) Handling register/unregister

                if let Some(context) = sm.context.clone() {
                    if !(Self::validate_sender(context).await) {
                        let message = ServiceMessage::new_error(
                            -32600,
                            "Ripple Main does not support this request from Service".to_string(),
                            None,
                            Id::Number(sm.get_request_id() as i64),
                        );
                        error!(
                            "Error handling inbound service message: {} {} ",
                            json_rpc_request, message
                        );

                        //send the error message back to the service through the service connection
                        if let Some(sender) = state
                            .service_controller_state
                            .get_sender(&connection_id.to_string())
                            .await
                        {
                            if let Err(err) = sender
                                .send(Message::Text(serde_json::to_string(&message).unwrap()))
                                .await
                            {
                                error!("Failed to send error message back to service: {}", err);
                            }
                        } else {
                            error!(
                                "No sender found for service connection_id: {}",
                                connection_id
                            );
                        }
                        return;
                    }
                }
                let msg = FireboltGatewayCommand::HandleRpcForService { msg: sm.clone() };
                if let Err(e) = state.get_client().send_gateway_command(msg) {
                    error!("failed to send request {:?}", e);
                };
            }
            JsonRpcMessage::Notification(json_rpc_notification) => {
                info!("Received service notification: {:#?}", sm);
                if let Some((context_update, update_type)) =
                    json_rpc_notification.method.split_once(".")
                {
                    debug!("Received service event notification request event type {:?} update type {:?}", context_update, update_type);
                    match context_update {
                        "rippleContextEvent" => {
                            state
                                .service_event_state
                                .process_event_notification(update_type, sm.clone());
                        }
                        "rippleContextUpdateRequest" => {
                            Self::process_event_update(state, update_type, sm.clone());
                        }
                        _ => {
                            error!("Invalid context update request: {}", context_update);
                            return;
                        }
                    }
                    debug!(
                        " service event subscribers: {:?}",
                        state.service_event_state.event_subscribers
                    );
                } else {
                    error!("Invalid service event request format");
                }
                return;
            }
            JsonRpcMessage::Success(_) | JsonRpcMessage::Error(_) => {
                // Handling response message
                let request_id = sm.get_request_id();
                let callback = state
                    .service_controller_state
                    .extract_broker_callback(&app_id, request_id)
                    .await
                    .unwrap_or(None);

                if let Some(callback) = callback {
                    // Handle the message using the callback
                    if let Err(err) = Self::handle_service_response(sm, callback) {
                        error!("Error handling service message: {}", err);
                    }
                } else {
                    error!("No broker callback found for app_id: {}", app_id);
                }
            }
        }
    }

    fn is_contract_used_for_routing(symbol: &ExtnSymbol) -> bool {
        !symbol.uses.is_empty() || !symbol.fulfills.is_empty()
    }

    async fn validate_sender(context: Value) -> bool {
        let ctx = serde_json::from_value::<serde_json::Map<String, Value>>(context);
        match ctx {
            Ok(c) => {
                let context = c.get("context");
                if let Some(context) = context {
                    if let Some(context_array) = context.as_array() {
                        if context_array.len() < 2 {
                            error!("Context does not contain a valid service id");
                        }
                        let id = context[1].as_str();
                        if let Some(service_id) = id {
                            if !ALLOWED_SERVICES_LIST.contains(&service_id) {
                                error!(
                                    "Service id {:?} is not allowed to send messages to Ripple Main",
                                    service_id
                                );
                            } else {
                                return true;
                            }
                        } else {
                            error!("Context does not contain a valid service id");
                        }
                    }
                } else {
                    error!("Context does not contain a valid service id");
                }
            }
            Err(e) => {
                error!("Failed to parse context: {}", e);
            }
        }
        false
    }

    pub async fn handle_service_connection(
        _client_addr: SocketAddr,
        ws_stream: WebSocketStream<TcpStream>,
        state: PlatformState,
        identity: ClientIdentity,
        connection_id: String,
        symbol: ExtnSymbol,
    ) {
        let app_id = identity.app_id.clone();
        let session_id = identity.session_id.clone();
        let client = state.get_client();

        info!(
            "Creating new service connection_id={} app_id={} session_id={}, gateway_secure={}, port={}",
            connection_id,
            app_id,
            session_id,
            identity.rpc_v2,
            _client_addr.port()
        );

        // Create communication channels
        let (message_tx, mut message_rx) = mpsc::channel::<Message>(32);
        let (api_message_tx, mut api_message_rx) = mpsc::channel::<ApiMessage>(32);

        let _ = Self::register_service_channel(
            &state,
            app_id.clone(),
            connection_id.clone(),
            message_tx.clone(),
        )
        .await;

        let is_using_extn_contracts = Self::is_contract_used_for_routing(&symbol);

        if is_using_extn_contracts {
            // Register the session for extensions
            Self::register_extn_contract_session(
                &state,
                &client,
                &identity,
                &app_id,
                &session_id,
                &symbol,
                api_message_tx.clone(),
            );
        }

        let (sender, mut receiver) = ws_stream.split();
        let sender_wrap = Arc::new(Mutex::new(sender));

        // Spawn a task to handle outgoing `Message`
        let sender_clone = Arc::clone(&sender_wrap);
        tokio::spawn(async move {
            while let Some(msg) = message_rx.recv().await {
                let mut sender = sender_clone.lock().await;
                if let Err(err) = sender.send(msg.clone()).await {
                    error!("Failed to send service message: {:?}", err);
                } else {
                    trace!("Sent service message {:#?}", msg);
                }
            }
        });

        // Spawn a task to handle outgoing `ApiMessage`
        if is_using_extn_contracts {
            let sender_clone = Arc::clone(&sender_wrap);
            tokio::spawn(async move {
                while let Some(api_message) = api_message_rx.recv().await {
                    let mut sender = sender_clone.lock().await;
                    let send_result = sender
                        .send(Message::Text(api_message.jsonrpc_msg.clone()))
                        .await;
                    match send_result {
                        Ok(_) => {
                            trace!("Sent service ApiMessage {}", api_message.jsonrpc_msg);
                        }
                        Err(err) => {
                            error!("Failed to send service ApiMessage: {:?}", err);
                        }
                    }
                }
            });
        }

        // Handle incoming messages for extensions/service (blocking)
        Self::handle_incoming_service_messages(
            &mut receiver,
            &state,
            &connection_id,
            &identity,
            &client,
        )
        .await;

        // Cleanup service connection session
        Self::cleanup_service_connection(
            &connection_id,
            &app_id,
            &session_id,
            is_using_extn_contracts,
            &client,
            symbol,
            &state,
        )
        .await;
    }

    async fn register_service_channel(
        state: &PlatformState,
        app_id: String,
        connection_id: String,
        message_tx: mpsc::Sender<Message>,
    ) -> Result<(), RippleError> {
        // Add the Message channel to the service registry
        let service_info = ServiceInfo::new(
            connection_id.clone(),
            message_tx.clone(),
            false, // Initially not registered
        );

        state
            .service_controller_state
            .add_service_info(app_id, service_info)
            .await
    }

    fn register_extn_contract_session(
        state: &PlatformState,
        client: &RippleClient,
        identity: &ClientIdentity,
        app_id: &str,
        session_id: &str,
        symbol: &ExtnSymbol,
        api_message_tx: mpsc::Sender<ApiMessage>,
    ) {
        // Add the ApiMessage sender to RippleClient to support sending ApiMessages
        let session = Session::new(identity.app_id.clone(), Some(api_message_tx.clone()));

        if let Some(sender) = session.get_sender() {
            // Gateway will probably not necessarily be ready when extensions start
            state
                .session_state
                .add_session(session_id.to_string(), session.clone());
            client
                .get_extn_client()
                .add_sender(app_id.to_string(), symbol.clone(), sender);
        }
    }

    async fn handle_incoming_service_messages(
        receiver: &mut SplitStream<WebSocketStream<TcpStream>>,
        state: &PlatformState,
        connection_id: &str,
        identity: &ClientIdentity,
        client: &RippleClient,
    ) {
        while let Some(msg) = receiver.next().await {
            match msg {
                Ok(msg) if msg.is_text() && !msg.is_empty() => {
                    let req_text = msg.to_text().unwrap().to_string();

                    if let Ok(sm) = serde_json::from_str::<ServiceMessage>(&req_text) {
                        Self::process_inbound_service_message(
                            state,
                            connection_id,
                            &sm,
                            identity.app_id.clone(),
                            identity.session_id.clone(),
                        )
                        .await;
                    } else if let Ok(extn_msg) = ExtnMessage::try_from(req_text.clone()) {
                        client.get_extn_client().handle_message(extn_msg);
                    } else {
                        error!("Failed to parse incoming message: {}", req_text);
                        return_invalid_service_error_message(
                            state,
                            connection_id,
                            RippleError::ParseError,
                        )
                        .await;
                    }
                }
                Ok(_) => {}
                Err(e) => {
                    error!(
                        "WebSocket error for service connection_id={}: {:?}",
                        connection_id, e
                    );
                }
            }
        }
    }

    async fn cleanup_service_connection(
        connection_id: &str,
        app_id: &str,
        session_id: &str,
        is_using_extn_contracts: bool,
        client: &RippleClient,
        symbol: ExtnSymbol,
        state: &PlatformState,
    ) {
        info!(
            "Unregistering service connection_id={} app_id={} session_id={}",
            connection_id, app_id, session_id
        );

        if is_using_extn_contracts {
            client
                .get_extn_client()
                .remove_sender(app_id.to_string(), symbol);
        }

        let _ = state
            .service_controller_state
            .remove_service_info(&connection_id.to_string())
            .await;
    }

    fn handle_service_response(
        sm: &ServiceMessage,
        callback: BrokerCallback,
    ) -> Result<BrokerOutput, RippleError> {
        let data = sm.message.clone();
        // get JsonRpcApiResponse from JsonRpcApiResponse
        let response = serde_json::to_string(&data).map_err(|_| RippleError::ParseError)?;
        let data = match serde_json::from_str::<JsonRpcApiResponse>(&response) {
            Ok(data) => data,
            Err(_) => {
                error!("Failed to parse JsonRpcApiResponse from service message");
                return Err(RippleError::ParseError);
            }
        };
        let final_result = Ok(BrokerOutput::new(data));

        if let Ok(output) = final_result.clone() {
            tokio::spawn(async move { callback.sender.try_send(output) });
        } else {
            error!("Bad broker response {:?}", sm.message);
        }
        final_result
    }
    pub async fn add_service_info(
        &self,
        service_id: String,
        info: ServiceInfo,
    ) -> Result<(), RippleError> {
        self.service_info
            .lock()
            .await
            .add_service_info(service_id, info)
            .await
    }

    pub async fn remove_service_info(&self, service_id: &String) -> Result<(), RippleError> {
        self.service_info
            .lock()
            .await
            .remove_service_info(service_id)
            .await
    }
    pub async fn set_broker_callback(
        &self,
        service_id: &String,
        request_id: u64,
        callback: BrokerCallback,
    ) -> Result<(), RippleError> {
        self.service_info
            .lock()
            .await
            .set_broker_callback(service_id, request_id, callback)
            .await
    }
    pub async fn extract_broker_callback(
        &self,
        service_id: &String,
        request_id: u64,
    ) -> Result<Option<BrokerCallback>, RippleError> {
        self.service_info
            .lock()
            .await
            .extract_broker_callback(service_id, request_id)
            .await
    }
    pub async fn get_sender(&self, service_id: &String) -> Option<mpsc::Sender<Message>> {
        self.service_info.lock().await.get_sender(service_id).await
    }

    pub fn process_event_update(
        platform_state: &PlatformState,
        update_type: &str,
        sm: ServiceMessage,
    ) {
        let message = sm.message.clone();
        match message {
            JsonRpcMessage::Notification(notification) => {
                info!("Received event notification: {:?}", notification);
                let params = notification.params;
                if let Some(params) = params {
                    match update_type {
                        "Token" => {
                            if let Ok(token) =
                                serde_json::from_value::<AccountToken>(params.clone())
                            {
                                let request = RippleContextUpdateRequest::Token(token);
                                Self::context_update(update_type, request, platform_state, sm);
                            } else {
                                error!("Failed to parse token parameters: {:?}", params);
                            }
                        }
                        "Activation" => {
                            todo!()
                            // if let Ok(status) = serde_json::from_value::<ActivationStatus>(params.clone()) {
                            //     request = RippleContextUpdateRequest::Activation(status);
                            //     Self::context_update(update_type, request, platform_state, sm);
                            // } else {
                            //     error!("Failed to parse token parameters: {:?}", params);
                            // }
                        }
                        "InternetStatus" => {
                            todo!()
                            // if let Ok(status) = serde_json::from_value::<InternetConnectionStatus>(params.clone()) {
                            //     request = RippleContextUpdateRequest::InternetStatus(status);
                            //     Self::context_update(update_type, request, platform_state, sm);
                            // } else {
                            //     error!("Failed to parse token parameters: {:?}", params);
                            // }
                        }
                        "PowerState" => {
                            todo!()
                            // if let Ok(state) = serde_json::from_value::<PowerState>(params.clone()) {
                            //         request = RippleContextUpdateRequest::PowerState(state);
                            // } else {
                            //     error!("Failed to parse token parameters: {:?}", params);
                            // }
                        }
                        "TimeZone" => {
                            todo!()
                            // if let Ok(tz) = serde_json::from_value::<TimeZone>(params.clone()) {
                            //         request = RippleContextUpdateRequest::TimeZone(tz);
                            // } else {
                            //     error!("Failed to parse token parameters: {:?}", params);
                            // }
                        }
                        "UpdateFeatures" => {
                            todo!()
                            // if let Ok(features) = serde_json::from_value::<FeatureUpdate>(params.clone()) {
                            //         request = RippleContextUpdateRequest::UpdateFeatures(features);
                            // } else {
                            //     error!("Failed to parse token parameters: {:?}", params);
                            // }
                        }
                        &_ => todo!(),
                    }
                    //Self::context_update(update_type, request, platform_state, sm);
                } else {
                    error!("parameters are missing in the message");
                }
            }
            _ => {
                error!("Invalid message type for event update");
            }
        }
    }

    pub fn context_update(
        update_type: &str,
        request: RippleContextUpdateRequest,
        platform_state: &PlatformState,
        sm: ServiceMessage,
    ) {
        let propagate = {
            let mut ripple_context = platform_state
                .service_event_state
                .ripple_context
                .write()
                .unwrap();
            debug!(
                "Received context update request: {:?} current ripple_context: {:?}",
                request, ripple_context
            );
            ripple_context.update(request.clone())
        };
        let new_ripple_context = {
            platform_state
                .service_event_state
                .ripple_context
                .read()
                .unwrap()
                .clone()
        };

        if propagate {
            let update_type = format!("\"{}Changed\"", update_type);
            let update_type = serde_json::from_str::<RippleContextUpdateType>(&update_type);
            match update_type {
                Ok(update_type) => {
                    let processors = platform_state
                        .service_event_state
                        .get_event_processors(Some(update_type.clone()));
                    for processor in processors {
                        debug!(
                            "Subscriber found for update type: {:?} subscriber: {:?}",
                            update_type, processor
                        );

                        let processor = processor.clone();
                        let collect = processor.split('&').collect::<Vec<&str>>();
                        let processor_arr = collect
                            .iter()
                            .map(|s| s.to_string())
                            .collect::<Vec<String>>();
                        let sender_id = processor_arr.get(0).unwrap().to_string();
                        let service_id = processor_arr.get(1).unwrap().to_string();
                        let request_type = processor_arr.get(2).unwrap().to_string();

                        let service_controller_state =
                            platform_state.service_controller_state.clone();
                        let context = sm.context.clone();

                        let new_ripple_context =
                            serde_json::to_string(&new_ripple_context).unwrap();

                        tokio::spawn(async move {
                            if let Some(sender) =
                                service_controller_state.get_sender(&service_id).await
                            {
                                let context = vec![sender_id, service_id, request_type];
                                let service_message = ServiceMessage {
                                    message: JsonRpcMessage::Notification(JsonRpcNotification {
                                        jsonrpc: "2.0".to_string(),
                                        method: "service.eventNotification".to_string(),
                                        params: Some(new_ripple_context.into()),
                                    }),
                                    context: Some(context.into()),
                                };
                                let msg_str = serde_json::to_string(&service_message).unwrap();
                                let mes = Message::Text(msg_str.clone());
                                let send_res = sender.try_send(mes);
                                trace!("Send to processor result: {:?}", send_res);
                            }
                        });
                    }
                }
                Err(e) => {
                    error!("Failed to parse update type: {}", e);
                }
            }
        } else {
            trace!("Context information is already updated. Hence not propagating");
        }
    }
}

async fn return_invalid_service_error_message(
    state: &PlatformState,
    connection_id: &str,
    e: RippleError,
) {
    if let Some(session) = state
        .session_state
        .get_session_for_connection_id(connection_id)
    {
        let id = if let RippleError::BrokerError(id) = e.clone() {
            id
        } else {
            Uuid::new_v4().to_string()
        };
        let msg = ExtnMessage {
            id: id.clone(),
            payload: ExtnPayload::Response(ExtnResponse::Error(e)),
            requestor: ExtnId::try_from(session.get_app_id()).unwrap(),
            target: RippleContract::Internal,
            target_id: None,
            ts: None,
        };
        let _ = session.send_json_rpc(msg.into()).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_validate_sender() {
        let context = serde_json::json!({
            "context": ["some_value", "ripple:channel:gateway:badger"]
        });
        let result = ServiceControllerState::validate_sender(context).await;
        assert!(result, "{}", true);

        let context = serde_json::json!({
            "context": ["some_value", "invalid_service_id"]
        });
        let result = ServiceControllerState::validate_sender(context).await;
        assert!(!result, "{}", false);
    }
}
