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

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use crate::api::gateway::rpc_gateway_api::CallContext;
use crate::api::{
    gateway::rpc_gateway_api::{ApiMessage, ApiProtocol},
    manifest::extn_manifest::ExtnSymbol,
};
use crate::extn::extn_id::ExtnId;
use crate::extn::{client::extn_client::ExtnClient, extn_client_message::ExtnMessage};
use crate::processor::rpc_router::RouterState;
use crate::service::service_message::{Id, JsonRpcMessage};
use crate::service::service_rpc_router::route_service_message;
use crate::utils::extn_utils::ExtnStackSize;
#[cfg(any(test, feature = "mock"))]
use crate::utils::mock_utils::get_next_mock_service_response;
use crate::utils::{error::RippleError, ws_utils::WebSocketUtils};
use futures_util::{SinkExt, StreamExt};
use jsonrpsee::core::server::rpc_module::Methods;
use log::{debug, error, info, trace, warn};
use serde_json::Value;
use tokio::sync::{mpsc, oneshot};
use tokio::sync::{mpsc::Sender as MSender, oneshot::Sender as OSender};
use tokio_tungstenite::tungstenite::Message;
use uuid::Uuid;

use super::service_message::ServiceMessage;
#[derive(Debug, Clone, Default)]
pub struct ServiceClient {
    pub service_sender: Option<MSender<ServiceMessage>>,
    pub service_router: Arc<RwLock<RouterState>>,
    response_processors: Arc<RwLock<HashMap<String, OSender<ServiceMessage>>>>,
    pub extn_client: Option<ExtnClient>,
    // TBD: Remove this field after implementing service.register API call.
    pub service_id: Option<ExtnId>,
}

pub struct ServiceClientBuilder {
    extn_symbol: Option<ExtnSymbol>,
}

impl Default for ServiceClientBuilder {
    fn default() -> Self {
        Self::new()
    }
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
                    response_processors: Arc::new(RwLock::new(HashMap::new())),
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
                    response_processors: Arc::new(RwLock::new(HashMap::new())),
                },
                None,
                Some(service_tr),
            )
        }
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
                        match sm.message {
                            JsonRpcMessage::Request(ref _json_rpc_request) => {
                                if let Some(sender) = &self.service_sender {
                                    route_service_message(
                                        sender,
                                        &self.service_router.read().unwrap(),
                                        sm.clone(),
                                    )
                                    .unwrap_or_else(|e| {
                                        error!("Error handling service message: {:?}", e);
                                    })
                                } else {
                                    error!("Service sender is not available");
                                }
                            }
                            JsonRpcMessage::Notification(_json_rpc_notification) => todo!(),
                            JsonRpcMessage::Success(ref json_rpc_success) => {
                                debug!(
                                    "Received Service Success: {:?} context {:?}",
                                    json_rpc_success,
                                    sm.context.clone().unwrap()
                                );
                                self.send_service_response(sm.clone());
                            }
                            JsonRpcMessage::Error(ref json_rpc_error) => {
                                error!("Received Service Error: {:?}", json_rpc_error);
                                let mut service_message = sm.clone();
                                service_message.message =
                                    JsonRpcMessage::Error(json_rpc_error.clone());
                                self.send_service_response(service_message.clone());
                            }
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

    fn send_service_response(&self, sm: ServiceMessage) {
        if let Some(context) = &sm.context {
            if let Some(Value::String(id)) = context
                .get("context")
                .and_then(|c| c.as_array())
                .and_then(|a| a.first())
            {
                if let Some(processor) = self.response_processors.write().unwrap().remove(id) {
                    if let Err(e) = processor.send(sm) {
                        error!("Failed to send service response: {:?}", e);
                    }
                } else {
                    warn!("No processor found for id: {}", id);
                }
            } else {
                warn!("Context does not contain a valid sender_id");
            }
        } else {
            warn!("Service message context is None");
        }
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

    #[allow(unused_variables)]
    pub async fn request_with_timeout_main(
        &mut self,
        method: String,
        params: Option<Value>,
        ctx: &CallContext,
        timeout_in_msecs: u64,
        service_id: String,
    ) -> Result<ServiceMessage, RippleError> {
        #[cfg(all(not(feature = "mock"), not(test)))]
        {
            let resp = tokio::time::timeout(
                std::time::Duration::from_millis(timeout_in_msecs),
                self.send_rpc_main(method, params, ctx, service_id),
            )
            .await;

            match resp {
                Ok(Ok(message)) => Ok(message),
                Ok(Err(e)) => Err(e),
                Err(_) => Err(RippleError::TimeoutError),
            }
        }
        // if mock is enabled for testing
        #[cfg(any(test, feature = "mock"))]
        {
            // Get the mock response using the ctx_id
            let ctx_id = ctx.get_id();
            if let Some(response) = get_next_mock_service_response(ctx_id) {
                return response;
            }

            // If no mock response found or no test_context provided
            Err(RippleError::TimeoutError)
        }
    }

    pub async fn send_rpc_main(
        &mut self,
        method: String,
        params: Option<Value>,
        ctx: &CallContext,
        service_id: String,
    ) -> Result<ServiceMessage, RippleError> {
        let id = uuid::Uuid::new_v4().to_string();
        let (tx, rx) = oneshot::channel();
        add_single_processor(id.clone(), Some(tx), self.response_processors.clone());

        let mut service_request =
            ServiceMessage::new_request(method.to_owned(), params, Id::String(id.clone()));

        let mut context = ctx.clone();
        context.protocol = ApiProtocol::Service;
        let vec = vec![id, service_id];
        context.context = vec;

        let service_message_context = serde_json::to_value(context).unwrap();
        service_request.set_context(Some(service_message_context));

        if let Some(sender) = &self.service_sender {
            match sender.try_send(service_request) {
                Ok(_) => {
                    if let Ok(r) = rx.await {
                        return Ok(r);
                    }
                    Err(RippleError::ExtnError)
                }
                Err(e) => {
                    error!("Error sending service request: {:?}", e);
                    Err(RippleError::ServiceError)
                }
            }
        } else {
            error!("Service sender is not available");
            Err(RippleError::ServiceError)
        }
    }

    fn get_default_service_call_context(method: String) -> CallContext {
        CallContext::new(
            Uuid::new_v4().to_string(),
            Uuid::new_v4().to_string(),
            "internal".into(),
            1,
            crate::api::gateway::rpc_gateway_api::ApiProtocol::Service,
            method.clone(),
            None,
            false,
        )
    }
    pub fn request_transient(
        &self,
        method: String,
        params: Option<Value>,
        ctx: Option<&CallContext>,
        service_id: String,
    ) -> Result<String, RippleError> {
        // if ctx is None, create a default CallContext using get_default_service_call_context
        let default_ctx;
        let ctx = match ctx {
            Some(c) => c,
            None => {
                default_ctx = Self::get_default_service_call_context(method.clone());
                &default_ctx
            }
        };

        let id = Uuid::new_v4().to_string();
        let mut service_request =
            ServiceMessage::new_request(method.to_owned(), params, Id::String(id.clone()));
        let mut context = ctx.clone();
        context.protocol = ApiProtocol::Service;
        let vec = vec![id.clone(), service_id];
        context.context = vec;
        let service_message_context = serde_json::to_value(context).unwrap();
        service_request.set_context(Some(service_message_context));
        if let Some(sender) = &self.service_sender {
            match sender.try_send(service_request) {
                Ok(_) => Ok(id),
                Err(e) => {
                    error!("Error sending service request: {:?}", e);
                    Err(RippleError::ServiceError)
                }
            }
        } else {
            error!("Service sender is not available");
            Err(RippleError::ServiceError)
        }
    }
}

fn add_single_processor<P>(id: String, processor: Option<P>, map: Arc<RwLock<HashMap<String, P>>>) {
    if let Some(processor) = processor {
        let mut processor_state = map.write().unwrap();
        processor_state.insert(id, processor);
    }
}

#[cfg(test)]
pub mod tests {
    use crate::api::gateway::rpc_gateway_api::ApiProtocol;
    #[cfg(test)]
    use crate::{
        extn::extn_id::ExtnId, service::service_client::CallContext,
        service::service_message::ServiceMessage, utils::error::RippleError,
        utils::mock_utils::queue_mock_service_response, uuid::Uuid,
    };
    use serde_json::json;
    use tokio::sync::mpsc::Sender;

    use super::*;
    #[cfg(test)]
    pub trait Mockable {
        fn mock() -> ServiceClient
        where
            Self: Sized;
        fn mock_with_params(
            service_sender: Option<Sender<ServiceMessage>>,
            service_rpc_router: Arc<RwLock<RouterState>>,
            response_processors: Arc<RwLock<HashMap<String, oneshot::Sender<ServiceMessage>>>>,
            extn_client: Option<ExtnClient>,
        ) -> ServiceClient
        where
            Self: Sized;
    }

    #[cfg(test)]
    impl Mockable for ServiceClient {
        fn mock() -> ServiceClient {
            let service_router = Arc::new(RwLock::new(RouterState::new()));
            let (service_sender, _service_tr) = mpsc::channel::<ServiceMessage>(32);
            ServiceClient {
                service_sender: Some(service_sender),
                service_router,
                extn_client: None,
                service_id: Some(
                    ExtnId::try_from("ripple:channel:gateway:service1".to_string()).unwrap(),
                ),
                response_processors: Arc::new(RwLock::new(HashMap::new())),
            }
        }

        fn mock_with_params(
            _service_sender: Option<Sender<ServiceMessage>>,
            _service_rpc_router: Arc<RwLock<RouterState>>,
            _response_processors: Arc<RwLock<HashMap<String, oneshot::Sender<ServiceMessage>>>>,
            _extn_client: Option<ExtnClient>,
        ) -> ServiceClient
        where
            Self: Sized,
        {
            todo!()
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_request_with_timeout() {
        let mut client = ServiceClient::mock();
        let id = Uuid::new_v4().to_string();
        queue_mock_service_response(
            &id,
            Ok(ServiceMessage::new_success(
                json!({"result": "success"}),
                Id::Null,
            )),
        );

        let context = CallContext::new(
            id.to_string(),
            "test_method".to_string(),
            "app1".to_string(),
            123122_u64,
            ApiProtocol::Service,
            "method.1".to_string(),
            None,
            false,
        );
        let result: Result<ServiceMessage, RippleError> = client
            .request_with_timeout_main("method.1".to_string(), None, &context, 5000, id)
            .await;
        println!("result: {:?}", result);
        assert!(result.is_ok());
    }
}
