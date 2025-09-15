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

use serde_json::json;
use std::collections::HashMap;

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
use jsonrpsee::core::{server::rpc_module::Methods, RpcResult};
use log::{debug, error, info, trace, warn};
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::sync::{Arc, RwLock};
use tokio::sync::{mpsc, oneshot};
use tokio::sync::{mpsc::Sender as MSender, oneshot::Sender as OSender};
use tokio_tungstenite::tungstenite::Message;
use uuid::Uuid;

use super::service_message::{
    JsonRpcNotification, JsonRpcSuccess, ServiceMessage, ServiceRequestType,
};
#[derive(Debug, Clone, Default)]
pub struct ServiceClient {
    pub service_sender: Option<MSender<ServiceMessage>>,
    pub service_router: Arc<RwLock<RouterState>>,
    response_processors: Arc<RwLock<HashMap<String, OSender<ServiceMessage>>>>,
    event_processors: Arc<RwLock<HashMap<String, MSender<ServiceMessage>>>>,
    pub extn_client: Option<ExtnClient>,
    // TBD: Remove this field after implementing service.register API call.
    pub service_id: Option<ExtnId>,
    pub outbound_extn_rx: Arc<RwLock<Option<mpsc::Receiver<ApiMessage>>>>,
    pub outbound_service_rx: Arc<RwLock<Option<mpsc::Receiver<ServiceMessage>>>>,
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

    pub fn build(self) -> ServiceClient {
        let service_router = Arc::new(RwLock::new(RouterState::new()));
        let (service_sender, service_tr) = mpsc::channel::<ServiceMessage>(32);

        if let Some(symbol) = self.extn_symbol {
            let (extn_client, ext_tr) = ExtnClient::new_extn(symbol.clone());
            ServiceClient {
                service_sender: Some(service_sender),
                service_router,
                extn_client: Some(extn_client),
                service_id: Some(ExtnId::try_from(symbol.id.clone()).unwrap()),
                response_processors: Arc::new(RwLock::new(HashMap::new())),
                event_processors: Arc::new(RwLock::new(HashMap::new())),
                outbound_extn_rx: Arc::new(RwLock::new(Some(ext_tr))),
                outbound_service_rx: Arc::new(RwLock::new(Some(service_tr))),
            }
        } else {
            ServiceClient {
                service_sender: Some(service_sender),
                service_router,
                extn_client: None,
                service_id: None,
                response_processors: Arc::new(RwLock::new(HashMap::new())),
                event_processors: Arc::new(RwLock::new(HashMap::new())),
                outbound_extn_rx: Arc::new(RwLock::new(None)),
                outbound_service_rx: Arc::new(RwLock::new(None)),
            }
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

    fn get_outbound_extn_rx(&self) -> Result<mpsc::Receiver<ApiMessage>, RippleError> {
        let mut outbound_extn_rx = self.outbound_extn_rx.write().unwrap();
        if let Some(t) = outbound_extn_rx.take() {
            Ok(t)
        } else {
            Err(RippleError::ClientMissing)
        }
    }

    fn get_outbound_service_rx(&self) -> Result<mpsc::Receiver<ServiceMessage>, RippleError> {
        let mut outbound_service_rx = self.outbound_service_rx.write().unwrap();
        if let Some(t) = outbound_service_rx.take() {
            Ok(t)
        } else {
            Err(RippleError::ClientMissing)
        }
    }

    pub fn get_service_router_state(&self) -> RouterState {
        self.service_router.read().unwrap().clone()
    }

    /// Initializes the service client, handling both extension and service messages.
    pub async fn initialize(&self) {
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

        let outbound_service_rx = self.get_outbound_service_rx();
        let mut outbound_service_rx = match outbound_service_rx {
            Ok(rx) => rx,
            Err(e) => {
                error!("Failed to get outbound service receiver: {:?}", e);
                return;
            }
        };
        let outbound_extn_rx = self.get_outbound_extn_rx();
        let mut outbound_extn_rx = match outbound_extn_rx {
            Ok(rx) => Some(rx),
            Err(e) => {
                error!("Failed to get outbound extension receiver: {:?}", e);
                return;
            }
        };
        loop {
            debug!("Connecting to WebSocket at {}", path);
            Self::connect_websocket(self, &path, &mut outbound_service_rx, &mut outbound_extn_rx)
                .await;
            debug!("Initialize Ended Abruptly");
        }
    }

    async fn connect_websocket(
        &self,
        path: &str,
        outbound_service_rx: &mut mpsc::Receiver<ServiceMessage>,
        outbound_extn_rx: &mut Option<mpsc::Receiver<ApiMessage>>,
    ) {
        if let Ok((mut ws_tx, mut ws_rx)) = WebSocketUtils::get_ws_stream(path, None).await {
            let handle_ws_message = |msg: Message| {
                if let Message::Text(message) = msg.clone() {
                    // Service message
                    debug!("Received Service Message: {:#?}", message);
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
                            JsonRpcMessage::Notification(ref json_rpc_notification) => {
                                debug!(
                                    "Received Service Notification: {:?} context {:?}",
                                    json_rpc_notification,
                                    sm.context.clone().unwrap()
                                );

                                let new_context = json!({"context": sm.context.clone()});

                                let new_sm = ServiceMessage {
                                    message: JsonRpcMessage::Notification(JsonRpcNotification {
                                        jsonrpc: "2.0".to_string(),
                                        method: json_rpc_notification.method.clone(),
                                        params: json_rpc_notification.params.clone(),
                                    }),
                                    context: Some(new_context.clone()),
                                };
                                self.send_service_response(new_sm.clone());
                            }
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
                                     error!("handle_ws_message failed");
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
    }

    fn send_service_response(&self, sm: ServiceMessage) {
        if let Some(context) = &sm.context {
            if let Some(request_type) = sm.get_request_type() {
                debug!("Service response for request_type: {}", request_type,);
                // extract service id from context
                if let Some(Value::String(id)) = context
                    .get("context")
                    .and_then(|c| c.as_array())
                    .and_then(|a| a.first())
                {
                    match request_type {
                        ServiceRequestType::Request => {
                            if let Some(response_processor) =
                                self.response_processors.write().unwrap().remove(id)
                            {
                                if let Err(e) = response_processor.send(sm) {
                                    error!("Failed to send service response: {:?}", e);
                                }
                            } else {
                                warn!("No response processor found for id: {}", id);
                            }
                        }
                        ServiceRequestType::Event => {
                            if let Some(event_processor) =
                                self.event_processors.write().unwrap().get(id).cloned()
                            {
                                debug!(
                                    "Sending service event for id: {} event_processors {:?}",
                                    id, self.event_processors
                                );
                                tokio::spawn(async move {
                                    if let Err(e) = event_processor.send(sm).await {
                                        error!("Failed to send service event: {:?}", e);
                                    }
                                });
                            } else {
                                warn!("No event processor found for id: {}", id);
                            }
                        }
                        ServiceRequestType::Transient => debug!("Transient request type"),
                    }
                } else if request_type != ServiceRequestType::Transient {
                    warn!(
                        "Service ID not found in context for service message: {:?}",
                        sm
                    );
                }
            } else {
                warn!(
                    "Service message RequestType is None, cannot send response: {:?}",
                    sm
                );
            }
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

    pub async fn call_and_parse_ripple_main_rpc<T: DeserializeOwned>(
        &mut self,
        method: &str,
        params: Option<serde_json::Value>,
        ctx: Option<&CallContext>,
        timeout: u64,
        service_id: &str,
        error_msg: &str,
    ) -> RpcResult<T> {
        let res = self
            .request_with_timeout_main(
                method.to_string(),
                params,
                ctx,
                timeout,
                service_id.to_string(),
                None,
                ServiceRequestType::Request,
            )
            .await
            .map_err(|_| jsonrpsee::core::Error::Custom(error_msg.to_string()))?;

        match res.message {
            JsonRpcMessage::Success(v) => serde_json::from_value::<T>(v.result).map_err(|_| {
                jsonrpsee::core::Error::Custom(format!("Failed to parse response for {}", method))
            }),
            _ => Err(jsonrpsee::core::Error::Custom(format!(
                "Failed to get Success response for {}",
                method
            ))),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn call_and_parse_ripple_event_rpc(
        &mut self,
        method: &str,
        params: Option<serde_json::Value>,
        ctx: Option<&CallContext>,
        timeout: u64,
        service_id: &str,
        _error_msg: &str,
        event_sender: MSender<ServiceMessage>,
    ) -> RpcResult<bool> {
        let res = self
            .request_with_timeout_main(
                method.to_string(),
                params,
                ctx,
                timeout,
                service_id.to_string(),
                Some(event_sender),
                ServiceRequestType::Event,
            )
            .await;

        if let Ok(r) = res {
            match r.message {
                JsonRpcMessage::Success(_) => Ok(true),
                _ => Ok(false),
            }
        } else {
            Ok(false)
        }
    }

    #[allow(unused_variables)]
    #[allow(clippy::too_many_arguments)]
    pub async fn request_with_timeout_main(
        &mut self,
        method: String,
        params: Option<Value>,
        ctx: Option<&CallContext>,
        timeout_in_msecs: u64,
        service_id: String,
        event_sender: Option<MSender<ServiceMessage>>,
        request_type: ServiceRequestType,
    ) -> Result<ServiceMessage, RippleError> {
        let default_ctx;
        let ctx = match ctx {
            Some(c) => c,
            None => {
                default_ctx = self.get_default_service_call_context(method.clone());
                &default_ctx
            }
        };

        #[cfg(all(not(feature = "mock"), not(test)))]
        {
            let resp = tokio::time::timeout(
                std::time::Duration::from_millis(timeout_in_msecs),
                self.send_rpc_main(method, params, ctx, service_id, event_sender, request_type),
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
        event_sender: Option<MSender<ServiceMessage>>,
        request_type: ServiceRequestType,
    ) -> Result<ServiceMessage, RippleError> {
        let id = uuid::Uuid::new_v4().to_string();
        let mut service_req: ServiceMessage = {
            match request_type {
                ServiceRequestType::Request => {
                    ServiceMessage::new_request(method.to_owned(), params, Id::String(id.clone()))
                }
                ServiceRequestType::Event | ServiceRequestType::Transient => {
                    ServiceMessage::new_notification(method.to_owned(), params)
                }
            }
        };
        let mut context = ctx.clone();
        context.protocol = ApiProtocol::Service;

        let vec = vec![id.clone(), service_id, request_type.to_string()];
        context.context = vec;

        let service_message_context = serde_json::to_value(context).unwrap();
        service_req.set_context(Some(service_message_context));
        if event_sender.is_none() {
            let (tx, rx) = oneshot::channel();
            add_response_processor(id, Some(tx), self.response_processors.clone());
            if let Some(sender) = &self.service_sender {
                match sender.try_send(service_req) {
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
                error!("request sender is not available");
                Err(RippleError::ServiceError)
            }
        } else {
            add_response_processor(id.clone(), event_sender, self.event_processors.clone());
            if let Some(sender) = &self.service_sender {
                match sender.try_send(service_req) {
                    Ok(_) => {
                        let service_message = ServiceMessage {
                            message: JsonRpcMessage::Success(JsonRpcSuccess {
                                result: Value::Bool(true),
                                jsonrpc: "2.0".to_string(),
                                id: Id::Null,
                            }),
                            context: None,
                        };
                        Ok(service_message)
                    }
                    Err(e) => {
                        error!("Error sending service request: {:?}", e);
                        Err(RippleError::ServiceError)
                    }
                }
            } else {
                error!("request sender is not available");
                Err(RippleError::ServiceError)
            }
        }
    }

    pub fn get_default_service_call_context(&self, method: String) -> CallContext {
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
                default_ctx = self.get_default_service_call_context(method.clone());
                &default_ctx
            }
        };

        let id = Uuid::new_v4().to_string();
        let mut service_message = ServiceMessage::new_notification(method.to_owned(), params);
        let mut context = ctx.clone();
        context.protocol = ApiProtocol::Service;
        let vec = vec![id.clone(), service_id];
        context.context = vec;
        let service_message_context = serde_json::to_value(context).unwrap();
        service_message.set_context(Some(service_message_context));
        if let Some(sender) = &self.service_sender {
            match sender.try_send(service_message) {
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

fn add_response_processor<P>(
    id: String,
    processor: Option<P>,
    map: Arc<RwLock<HashMap<String, P>>>,
) {
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
            let (service_sender, service_tr) = mpsc::channel::<ServiceMessage>(32);
            let (_extn_sender, extn_tr) = mpsc::channel::<ApiMessage>(32);
            ServiceClient {
                service_sender: Some(service_sender),
                service_router,
                extn_client: None,
                service_id: Some(
                    ExtnId::try_from("ripple:channel:gateway:service1".to_string()).unwrap(),
                ),
                response_processors: Arc::new(RwLock::new(HashMap::new())),
                outbound_extn_rx: Arc::new(RwLock::new(Some(extn_tr))),
                outbound_service_rx: Arc::new(RwLock::new(Some(service_tr))),
                event_processors: Arc::new(RwLock::new(HashMap::new())),
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
            .request_with_timeout_main(
                "method.1".to_string(),
                None,
                Some(&context),
                5000,
                id,
                None,
                ServiceRequestType::Request,
            )
            .await;
        println!("result: {:?}", result);
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_get_outbound_service_rx() {
        let client = ServiceClient::mock();
        let rx = client.get_outbound_service_rx();
        assert!(rx.is_ok());
    }

    #[tokio::test]
    async fn test_get_outbound_extn_rx() {
        let client = ServiceClient::mock();
        let rx = client.get_outbound_extn_rx();
        assert!(rx.is_ok());
    }
}
