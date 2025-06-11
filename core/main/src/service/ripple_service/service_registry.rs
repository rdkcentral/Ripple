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

use futures::SinkExt;
use futures::StreamExt;
use ripple_sdk::{
    api::gateway::rpc_gateway_api::ApiMessage,
    log::{error, info, trace},
    tokio::{
        net::TcpStream,
        sync::{mpsc, Mutex},
    },
};
use ripple_sdk::{
    api::manifest::extn_manifest::ExtnSymbol,
    extn::extn_client_message::ExtnMessage,
    tokio,
    tokio_tungstenite::{tungstenite::Message, WebSocketStream},
    utils::error::RippleError,
};

use crate::{
    firebolt::firebolt_ws::ClientIdentity,
    state::{platform_state::PlatformState, session_state::Session},
};

#[derive(Debug, Clone)]
pub struct ServiceInfo {
    pub connection_id: String,
    pub tx: mpsc::Sender<Message>,
    pub is_sevice_registered: bool,
    pub service_id: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct ServiceRegistry {
    service_registry: Arc<Mutex<HashMap<String, ServiceInfo>>>,
}

impl ServiceRegistry {
    pub async fn add_service_info(
        &self,
        service_id: String,
        info: ServiceInfo,
    ) -> Result<(), RippleError> {
        self.service_registry.lock().await.insert(service_id, info);
        Ok(())
    }

    pub async fn remove_service_info(&self, service_id: &String) -> Result<(), RippleError> {
        let mut registry = self.service_registry.lock().await;
        if registry.remove(service_id).is_some() {
            Ok(())
        } else {
            Err(RippleError::InvalidInput)
        }
    }

    // get sender for a given service_id
    pub async fn get_sender(&self, service_id: &String) -> Option<mpsc::Sender<Message>> {
        let registry = self.service_registry.lock().await;
        registry.get(service_id).map(|info| info.tx.clone())
    }
    async fn process_service_message(
        _state: &PlatformState,
        _connection_id: &str,
        _msg: serde_json::Value,
        _app_id: String,
        _session_id: String,
    ) {
        // TBD: Implement the logic to process the service message
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
        let _ = state
            .service_registry
            .add_service_info(
                connection_id.clone(),
                ServiceInfo {
                    connection_id: connection_id.clone(),
                    tx: message_tx.clone(),
                    is_sevice_registered: false,
                    service_id: Some(symbol.id.clone()),
                },
            )
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

        // Handle incoming messages
        while let Some(msg) = receiver.next().await {
            match msg {
                Ok(msg) => {
                    if msg.is_text() && !msg.is_empty() {
                        let req_text = String::from(msg.to_text().unwrap());
                        match ExtnMessage::try_from(req_text.clone()) {
                            Ok(extn_msg) => {
                                client.get_extn_client().handle_message(extn_msg);
                            }
                            Err(_) => {
                                // Handle the JSON-RPC message
                                if let Ok(v) = serde_json::from_str::<serde_json::Value>(
                                    msg.to_text().unwrap(),
                                ) {
                                    Self::process_service_message(
                                        &state,
                                        &connection_id,
                                        v,
                                        identity.app_id.clone(),
                                        identity.session_id.clone(),
                                    )
                                    .await;
                                }
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
            .service_registry
            .remove_service_info(&connection_id)
            .await;
    }
}
