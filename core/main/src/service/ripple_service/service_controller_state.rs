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
use std::{collections::HashMap, net::SocketAddr, sync::Arc};

use futures::{SinkExt, StreamExt};
use ripple_sdk::{
    api::{
        gateway::rpc_gateway_api::{ApiMessage, JsonRpcApiResponse},
        manifest::extn_manifest::ExtnSymbol,
    },
    extn::{
        extn_client_message::{ExtnMessage, ExtnPayload, ExtnResponse},
        extn_id::ExtnId,
    },
    framework::ripple_contract::RippleContract,
    log::{error, info, trace},
    service::service_message::{Id, JsonRpcMessage, ServiceMessage},
    tokio::{
        self,
        net::TcpStream,
        sync::{mpsc, Mutex},
    },
    tokio_tungstenite::{tungstenite::Message, WebSocketStream},
    utils::error::RippleError,
    uuid::Uuid,
};

use crate::{
    broker::endpoint_broker::{BrokerCallback, BrokerOutput},
    firebolt::firebolt_ws::ClientIdentity,
    state::{platform_state::PlatformState, session_state::Session},
};

use super::service_registry::ServiceRegistry;

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
                // In Ripple Service Architecture Ripple Main will not honor any request originated from any connected service other than
                // service registration and unregistration request
                // (TBD) Handling register/unregister

                // send a rejection for all other responses
                //let request_id = sm.get_request_id();
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
                // send the error message back to the service through the service connection
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
            }
            JsonRpcMessage::Notification(_) => {
                // TBD: Handle notifications.
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

        // Add the Message channel to the service registry
        let service_info = ServiceInfo::new(
            connection_id.clone(),
            message_tx.clone(),
            false, // Initially not registered
        );

        let _ = state
            .service_controller_state
            .add_service_info(app_id.clone(), service_info.clone())
            .await;

        let is_using_extn_contracts = Self::is_contract_used_for_routing(&symbol);

        if is_using_extn_contracts {
            // Add the ApiMessage sender to RippleClient to support sending ApiMessages
            let session = Session::new(identity.app_id.clone(), Some(api_message_tx.clone()));

            if let Some(sender) = session.get_sender() {
                // Gateway will probably not necessarily be ready when extensions start
                state
                    .session_state
                    .add_session(session_id.clone(), session.clone());
                client
                    .get_extn_client()
                    .add_sender(app_id.clone(), symbol.clone(), sender);
            }
        }

        let (sender, mut receiver) = ws_stream.split();

        // Wrap `sender` in an `Arc<Mutex<_>>` to allow shared ownership
        let sender_wrap = Arc::new(Mutex::new(sender));

        // Spawn a task to handle outgoing `Message`
        let sender_clone = Arc::clone(&sender_wrap);
        tokio::spawn(async move {
            while let Some(msg) = message_rx.recv().await {
                let mut sender = sender_clone.lock().await;
                if let Err(err) = sender.send(msg).await {
                    error!("Failed to send service message: {:?}", err);
                } else {
                    trace!("Sent service message");
                }
            }
        });

        if is_using_extn_contracts {
            // Spawn a task to handle outgoing `ApiMessage`
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

        // Handle incoming messages for extensions/service
        while let Some(msg) = receiver.next().await {
            match msg {
                Ok(msg) => {
                    if msg.is_text() && !msg.is_empty() {
                        let req_text = String::from(msg.to_text().unwrap());

                        if let Ok(sm) = serde_json::from_str::<ServiceMessage>(&req_text) {
                            Self::process_inbound_service_message(
                                &state,
                                &connection_id,
                                &sm,
                                identity.app_id.clone(),
                                identity.session_id.clone(),
                            )
                            .await;
                        } else {
                            match ExtnMessage::try_from(req_text.clone()) {
                                Ok(extn_msg) => {
                                    // If the message is an ExtnMessage, handle it
                                    client.get_extn_client().handle_message(extn_msg);
                                }
                                Err(e) => {
                                    // If the message is not a ServiceMessage or ExtnMessage, log an error
                                    error!("Failed to parse service message: {}", req_text);
                                    return_invalid_service_error_message(&state, &connection_id, e)
                                        .await;
                                }
                            }
                            // If the message is not a ServiceMessage, try to parse it as an ExtnMessage
                            if let Ok(extn_msg) = ExtnMessage::try_from(req_text.clone()) {
                                client.get_extn_client().handle_message(extn_msg);
                            } else {
                                error!("Failed to parse service message: {}", req_text);
                                return_invalid_service_error_message(
                                    &state,
                                    &connection_id,
                                    RippleError::ParseError,
                                )
                                .await;
                            }
                        }
                    }
                }
                Err(e) => {
                    error!(
                        "WebSocket error for service connection_id={}: {:?}",
                        connection_id, e
                    );
                }
            }
        }
        // Cleanup service connection session
        info!(
            "Unregistering service connection_id={} app_id={} session_id={}",
            connection_id, app_id, session_id
        );

        if is_using_extn_contracts {
            client.get_extn_client().remove_sender(app_id, symbol);
        }

        let _ = state
            .service_controller_state
            .remove_service_info(&connection_id)
            .await;
    }

    fn handle_service_response(
        sm: &ServiceMessage,
        callback: BrokerCallback,
    ) -> Result<BrokerOutput, RippleError> {
        let data = sm.message.clone();
        // get JsonRpcApiResponse from JsonRpcApiResponse
        let resposne = serde_json::to_string(&data).map_err(|_| RippleError::ParseError)?;
        let data = match serde_json::from_str::<JsonRpcApiResponse>(&resposne) {
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
