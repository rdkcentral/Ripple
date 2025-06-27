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

use std::sync::{Arc, RwLock};

use futures_util::{SinkExt, StreamExt};
use jsonrpsee::core::server::rpc_module::Methods;
use log::{debug, error, info, trace, warn};
use tokio::sync::mpsc;
use tokio::sync::mpsc::Sender as MSender;
use tokio_tungstenite::tungstenite::Message;

use crate::api::{gateway::rpc_gateway_api::ApiMessage, manifest::extn_manifest::ExtnSymbol};
use crate::extn::extn_id::ExtnId;
use crate::extn::{client::extn_client::ExtnClient, extn_client_message::ExtnMessage};
use crate::processor::rpc_router::RouterState;
use crate::service::service_rpc_router::route_service_message;
use crate::utils::extn_utils::ExtnStackSize;
use crate::utils::{error::RippleError, ws_utils::WebSocketUtils};

use super::service_message::ServiceMessage;
#[derive(Debug, Clone)]
pub struct ServiceClient {
    pub service_sender: Option<MSender<ServiceMessage>>,
    pub service_router: Arc<RwLock<RouterState>>,
    pub extn_client: Option<ExtnClient>,
    // TBD: Remove this field after implementing service.register API call.
    pub service_id: Option<ExtnId>,
}

pub struct ServiceClientBuilder {
    extn_symbol: Option<ExtnSymbol>,
}

impl ServiceClientBuilder {
    pub fn new() -> Self {
        Self { extn_symbol: None }
    }

    pub fn with_extension(mut self, symbol: ExtnSymbol) -> Self {
        self.extn_symbol = Some(symbol);
        self
    }

    pub fn build(
        self,
    ) -> (
        ServiceClient,
        Option<mpsc::Receiver<ApiMessage>>,
        Option<mpsc::Receiver<ServiceMessage>>,
    ) {
        let service_router = Arc::new(RwLock::new(RouterState::new()));
        let (service_sender, service_tr) = mpsc::channel::<ServiceMessage>(32);

        if let Some(symbol) = self.extn_symbol {
            let (extn_client, ext_tr) = ExtnClient::new_extn(symbol.clone());
            (
                ServiceClient {
                    service_sender: Some(service_sender),
                    service_router,
                    extn_client: Some(extn_client),
                    service_id: Some(ExtnId::try_from(symbol.id.clone()).unwrap()),
                },
                Some(ext_tr),
                Some(service_tr),
            )
        } else {
            (
                ServiceClient {
                    service_sender: Some(service_sender),
                    service_router,
                    extn_client: None,
                    service_id: None,
                },
                None,
                Some(service_tr),
            )
        }
    }
}

impl Default for ServiceClientBuilder {
    fn default() -> Self {
        Self::new()
    }
}
impl ServiceClient {
    pub fn builder() -> ServiceClientBuilder {
        ServiceClientBuilder::new()
    }

    pub fn set_service_rpc_route(&mut self, methods: Methods) -> Result<(), RippleError> {
        let service_routes = self.service_router.write().unwrap();
        service_routes.update_methods(methods.clone());
        Ok(())
    }

    pub fn get_service_router_state(&self) -> RouterState {
        self.service_router.read().unwrap().clone()
    }

    /// Initializes the service client, handling both extension and service messages.
    pub async fn initialize(
        &self,
        mut outbound_extn_rx: Option<mpsc::Receiver<ApiMessage>>,
        outbound_service_rx: Option<mpsc::Receiver<ServiceMessage>>,
    ) {
        debug!("Starting Service Client initialize");
        let service_id = self.service_id.clone().unwrap();
        let base_path = std::env::var("RIPPLE_SERVICE_HANDSHAKE_PATH")
            .unwrap_or_else(|_| "127.0.0.1:3474".to_string());
        let path = tokio_tungstenite::tungstenite::http::Uri::builder()
            .scheme("ws")
            .authority(base_path.as_str())
            .path_and_query(format!("/?service_handshake={}", service_id))
            .build()
            .unwrap()
            .to_string();

        let mut outbound_service_rx = match outbound_service_rx {
            Some(rx) => rx,
            None => {
                error!("No service receiver provided to ServiceClient::initialize");
                return;
            }
        };

        if let Ok((mut ws_tx, mut ws_rx)) = WebSocketUtils::get_ws_stream(&path, None).await {
            let handle_ws_message = |msg: Message| {
                if let Message::Text(message) = msg.clone() {
                    // Service message
                    if let Ok(sm) = serde_json::from_str::<ServiceMessage>(&message) {
                        debug!("Received Service Message: {:#?}", sm);
                        if let Some(sender) = &self.service_sender {
                            route_service_message(
                                sender,
                                &self.service_router.read().unwrap(),
                                sm.clone(),
                            )
                            .unwrap_or_else(|e| {
                                error!("Error handling service message: {:?}", e);
                            })
                        }
                    // Extension message
                    } else if let Ok(extn_message) = ExtnMessage::try_from(message) {
                        if let Some(extn_client) = &self.extn_client {
                            extn_client.handle_message(extn_message);
                        } else {
                            warn!("Received extension message but no extn_client present");
                        }
                    };
                } else if let Message::Close(_) = msg {
                    info!("Received Close message, exiting initialize");
                    return false;
                } else {
                    warn!("Received unexpected message: {:?}", msg);
                }
                true
            };
            tokio::pin! {
                let read_pin = ws_rx.next();
            }

            loop {
                tokio::select! {
                    Some(value) = &mut read_pin => {
                        match value {
                            Ok(msg) => {
                                if !handle_ws_message(msg) {
                                    break;
                                }
                            }
                            Err(e) => {
                                error!("Service Websocket error on read {:?}", e);
                                break;
                            }
                        }
                    },
                    Some(request) = async {
                        match outbound_extn_rx.as_mut() {
                            Some(rx) => rx.recv().await,
                            None => None,
                        }
                    }, if outbound_extn_rx.is_some() => {
                        trace!("IEC send: {:?}", request.jsonrpc_msg);
                        let _feed = ws_tx.feed(Message::Text(request.jsonrpc_msg)).await;
                        let _flush = ws_tx.flush().await;
                    }
                    Some(request) = outbound_service_rx.recv() => {
                        trace!("Service Message send: {:?}", request);
                        let _feed = ws_tx.feed(Message::Text(request.into())).await;
                        let _flush = ws_tx.flush().await;
                    }
                }
            }
        }
        debug!("Initialize Ended Abruptly");
    }
    pub fn get_extn_client(&self) -> Option<ExtnClient> {
        self.extn_client.clone()
    }
    pub fn get_service_sender(&self) -> Option<MSender<ServiceMessage>> {
        self.service_sender.clone()
    }
    pub fn get_service_router(&self) -> Arc<RwLock<RouterState>> {
        self.service_router.clone()
    }
    pub fn get_stack_size(&self) -> Option<ExtnStackSize> {
        self.extn_client.as_ref().and_then(|ec| ec.get_stack_size())
    }
}
