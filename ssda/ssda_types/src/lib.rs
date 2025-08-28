use futures_util::{SinkExt, StreamExt};
use gateway::ServiceRoutingRequest;
use jsonrpsee::Methods;
use ripple_sdk::api::rules_engine::{Rule, RuleTransform};
use ripple_sdk::log::{debug, error, info};
use serde::{Deserialize, Serialize};
use std::time::Duration;

use service::{APIClientMessages, APIGatewayServiceRegistrationRequest};

/*
Who what why
# API Gateway
The API gateway is a standalone binary component
that listens for Firebolt requests on a websocket. Upon receiving a firebolt
request, the gateway will look up a handler service (based on the method name) in it's
runtime (dynamically created) registry, and (assumign a service is registered to for the method),
will wrap the request with metadata, and dispatch the request to the handler service. Upon receiving
a response from the service, the API Gateway will translate the service response (using the rules engine)
into a Firebolt compatible (success or failure) result.

# Servicesprintln
ServiceRequestHandler. This design is motivated by the need to free the Service from as much connection oriented
detail and let the developer focus on business logic. The API Gateway client is instanced as a crate that can be
consumed by a service at the highest possible level of abstraction (and ease of use) - it should only requuire a bit of
bootstrapping, and a ServiceRequestHandler instance/implementation, and then it should manage all the details of the
interactions between the Service and the API gateway with the Service being as blissfully ignorant of the details
as possible.
*/
/*

register: Service -> API Gateway Client -> API Gateway

*/

pub mod api_gateway_client;
pub mod api_gateway_server;
pub mod service_api;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]

pub struct JqRule {
    pub alias: String,
    pub rule: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct StaticRule {
    pub alias: String,
    pub rule: String,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]

pub struct HandlerId {
    pub handler_id: String,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]

pub struct ServiceHandler {
    pub handler_id: HandlerId,
    pub handler_type: Handler,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub enum Handler {
    #[default]
    None,
    JqRule(JqRule),
    StaticRule(StaticRule),
}
impl std::fmt::Display for Handler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Handler::None => write!(f, "none"),
            Handler::JqRule(jq_rule) => write!(f, "{}", jq_rule.alias),
            Handler::StaticRule(static_rule) => write!(f, "{}", static_rule.alias),
        }
    }
}

impl From<Handler> for ripple_sdk::api::rules_engine::Rule {
    fn from(handler: Handler) -> Self {
        match handler {
            Handler::None => Rule {
                alias: "none".to_string(),
                filter: None,
                transform: RuleTransform::default(),
                event_handler: None,
                endpoint: None,
                sources: None,
            },
            Handler::JqRule(jq_rule) => Rule {
                alias: jq_rule.alias,
                filter: Some(jq_rule.rule),
                ..Default::default()
            },
            Handler::StaticRule(static_rule) => Rule {
                alias: static_rule.alias,
                filter: Some(static_rule.rule),
                ..Default::default()
            },
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ServiceId {
    pub service_id: String,
}
impl ServiceId {
    pub fn new(service_id: String) -> Self {
        ServiceId { service_id }
    }
}

impl PartialEq for ServiceId {
    fn eq(&self, other: &Self) -> bool {
        self.service_id == other.service_id
    }
}

impl Eq for ServiceId {}

impl std::hash::Hash for ServiceId {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.service_id.hash(state);
    }
}
impl std::fmt::Display for ServiceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.service_id)
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ServiceRequestId {
    pub request_id: u64,
}
impl PartialEq for ServiceRequestId {
    fn eq(&self, other: &Self) -> bool {
        self.request_id == other.request_id
    }
}

impl Eq for ServiceRequestId {}

impl std::hash::Hash for ServiceRequestId {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.request_id.hash(state);
    }
}
impl ServiceRequestId {
    pub fn new(request_id: u64) -> Self {
        ServiceRequestId { request_id }
    }
}

/*
gateway messages: from endpoint broker to api gateway and back.
*/
pub mod gateway {
    use crate::ServiceId;
    use http::Uri;
    use ripple_sdk::api::gateway::rpc_gateway_api::{
        JsonRpcApiError, JsonRpcApiResponse, RpcRequest,
    };
    use serde::{Deserialize, Serialize};
    use serde_json::Value;
    use tokio::sync::oneshot::Sender;

    use crate::ServiceRequestId;

    /*
    ServiceRequest is the request that is sent from the API Gateway to the service
    */
    #[derive(Debug)]
    pub struct ServiceRoutingRequest {
        pub request_id: ServiceRequestId,
        pub payload: RpcRequest,
        pub respond_to: Sender<ServiceRoutingResponse>,
    }
    #[derive(Debug, Default, Clone, Serialize, Deserialize)]
    pub struct ServiceRoutingSuccessResponse {
        pub request_id: ServiceRequestId,
        pub response: Value,
    }
    impl From<ServiceRoutingResponse> for JsonRpcApiResponse {
        fn from(response: ServiceRoutingResponse) -> Self {
            match response {
                ServiceRoutingResponse::Error(_error) => JsonRpcApiError::default().into(),
                ServiceRoutingResponse::Success(success) => JsonRpcApiResponse {
                    id: Some(success.request_id.request_id),
                    jsonrpc: "2.0".to_string(),
                    result: Some(success.response),
                    error: None,
                    method: None,
                    params: None,
                },
            }
        }
    }
    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct ServiceRoutingErrorResponse {
        pub request_id: ServiceRequestId,
        pub error: String,
    }
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub enum ServiceRoutingResponse {
        Success(ServiceRoutingSuccessResponse),
        Error(ServiceRoutingErrorResponse),
    }
    #[derive(Debug)]
    pub enum APIGatewayServiceConnectionDisposition {
        Accept(ServiceId),
        Connected(ServiceId),
    }
    #[derive(Debug)]
    pub enum APIGatewayServiceConnectionError {
        ConnectionError,
        NotAService,
    }

    /*
    This is the API gateway, and it meant to be hosted in the main ripple process
    */

    #[async_trait::async_trait]
    pub trait ApiGatewayServer: Send + Sync {
        async fn is_service_connect(
            &self,
            uri: Uri,
        ) -> Result<APIGatewayServiceConnectionDisposition, APIGatewayServiceConnectionError>;
        async fn service_connect(
            &mut self,
            service_id: ServiceId,
            ws_stream: tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>,
        ) -> Result<APIGatewayServiceConnectionDisposition, APIGatewayServiceConnectionError>;
        fn get_sender(&self) -> tokio::sync::mpsc::Sender<ServiceRoutingRequest>;
    }
}
/*
service message: from api gateawy to services and back (over websockets)
*/
pub mod service {

    use ripple_sdk::api::gateway::rpc_gateway_api::JsonRpcApiRequest;
    use serde::{Deserialize, Serialize};
    use serde_json::Value;

    use crate::{Handler, JqRule, ServiceId, ServiceRequestId};

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct FireboltMethodHandlerRegistration {
        pub firebolt_method: Handler,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
    pub struct FireboltMethodHandlerAPIRegistration {
        pub firebolt_method: String,
        pub jq_rule: Option<JqRule>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, Default)]
    pub struct APIGatewayServiceRegistrationRequest {
        pub firebolt_handlers: Vec<FireboltMethodHandlerAPIRegistration>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, Default)]
    pub struct APIGatewayServiceRegistrationResponse {
        pub firebolt_handlers: Vec<FireboltMethodHandlerAPIRegistration>,
    }
    impl From<APIGatewayServiceRegistrationRequest> for APIGatewayServiceRegistrationResponse {
        fn from(
            request: APIGatewayServiceRegistrationRequest,
        ) -> APIGatewayServiceRegistrationResponse {
            APIGatewayServiceRegistrationResponse {
                firebolt_handlers: request.firebolt_handlers,
            }
        }
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct ServiceRequest {
        pub service_id: ServiceId,
        pub firebolt_method: Handler,
        pub payload: serde_json::Value,
    }
    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct ServiceErrorResponse {
        pub service_id: ServiceId,
        pub firebolt_method: Handler,
        pub error: String,
    }
    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct ServiceSuccessResponse {
        pub service_id: ServiceId,
        pub firebolt_method: Handler,
        pub payload: serde_json::Value,
    }

    /*
    send by api client to the api gateway over websocket

    */
    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct APIClientRegistration {
        pub firebolt_handlers: Vec<FireboltMethodHandlerRegistration>,
    }
    /*
    Sent during callback registration in service
    */
    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct ServiceRegistration {
        pub service_id: ServiceId,
        pub firebolt_handlers: Vec<FireboltMethodHandlerAPIRegistration>,
    }

    impl ServiceRegistration {
        pub fn new(
            service_id: ServiceId,
            firebolt_handlers: Vec<FireboltMethodHandlerAPIRegistration>,
        ) -> Self {
            ServiceRegistration {
                service_id,
                firebolt_handlers,
            }
        }
        pub fn get_rule_registrations(&self) -> Vec<FireboltMethodHandlerAPIRegistration> {
            self.firebolt_handlers.clone()
        }
    }

    pub struct ServiceRegistrationBuilder {
        service_id: ServiceId,
        firebolt_handlers: Vec<FireboltMethodHandlerAPIRegistration>,
    }
    impl ServiceRegistrationBuilder {
        pub fn new(service_id: ServiceId) -> Self {
            ServiceRegistrationBuilder {
                service_id,
                firebolt_handlers: Vec::new(),
            }
        }
        pub fn add_handler(&mut self, firebolt_method: Handler) -> &mut Self {
            self.firebolt_handlers.push(match firebolt_method {
                Handler::None => todo!(),
                Handler::JqRule(jq_rule) => FireboltMethodHandlerAPIRegistration {
                    firebolt_method: jq_rule.alias.clone(),
                    jq_rule: Some(jq_rule),
                },
                Handler::StaticRule(static_rule) => FireboltMethodHandlerAPIRegistration {
                    firebolt_method: static_rule.alias.clone(),
                    jq_rule: Some(JqRule {
                        alias: static_rule.alias,
                        rule: static_rule.rule,
                    }),
                },
            });
            self
        }
        pub fn build(&self) -> ServiceRegistration {
            ServiceRegistration {
                service_id: self.service_id.clone(),
                firebolt_handlers: self.firebolt_handlers.clone(),
            }
        }
    }
    /*
    ServiceCall is the request that is presented to a callback handler.
    */
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct ServiceCall {
        pub request_id: ServiceRequestId,
        pub method: String,
        pub payload: serde_json::Value,
    }
    impl From<ServiceCall> for JsonRpcApiRequest {
        fn from(call: ServiceCall) -> Self {
            JsonRpcApiRequest {
                id: Some(call.request_id.request_id),
                jsonrpc: "2.0".to_string(),
                method: call.method,
                params: Some(call.payload),
            }
        }
    }
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct ServiceCallSuccessResponse {
        pub request_id: ServiceRequestId,
        pub response: serde_json::Value,
    }
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct ServiceCallErrorResponse {
        pub request_id: ServiceRequestId,
        pub error: String,
    }
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub enum ServiceCallResponse {
        Success(ServiceCallSuccessResponse),
        Error(ServiceCallErrorResponse),
    }
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub enum APIClientMessages {
        Register(APIGatewayServiceRegistrationRequest),
        Registered(APIGatewayServiceRegistrationResponse),
        Error(String),
        Unregister(ServiceId),
        ServiceCall(ServiceCall),
        ServiceCallSuccessResponse(ServiceCallSuccessResponse),
        ServiceCallErrorResponse(ServiceCallErrorResponse),
    }
    impl Default for APIClientMessages {
        fn default() -> Self {
            APIClientMessages::Register(APIGatewayServiceRegistrationRequest::default())
        }
    }

    pub struct ServiceRegistrationResponse {
        pub service_id: ServiceId,
    }
    pub struct ServiceRegistrationFailure {
        pub service_id: ServiceId,
        pub error: String,
    }
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub enum WebsocketServiceResponse {
        Success(ServiceRequestId, serde_json::Value),
        Error(ServiceRequestId, String),
    }
    impl WebsocketServiceResponse {
        pub fn get_id(&self) -> ServiceRequestId {
            match self {
                WebsocketServiceResponse::Success(id, _) => id.clone(),
                WebsocketServiceResponse::Error(id, _) => id.clone(),
            }
        }
    }
    #[derive(Debug)]
    pub struct WebsocketServiceRequest {
        pub request_id: ServiceRequestId,
        pub method: String,
        pub payload: Value,
        pub respond_to: tokio::sync::oneshot::Sender<WebsocketServiceResponse>,
    }

    /*
    individiual services implement this trait to handle requests from the API gateway
    This trait should faciliate fun testing of actual implementations
    */

    pub trait ServiceRequestHandler: Send + Sync {
        /*
          called by the client to allow the ServiceRequestHander return a Vec<FireboltMethodHandler>
          to be used by the client to route requests. This is a blocking call, and will be called
          after the client `on_connect`s
        **/
        fn register(&self) -> Vec<FireboltMethodHandlerRegistration>;
        fn handle_request(
            &self,
            request: ServiceCall,
        ) -> Result<ServiceCallSuccessResponse, ServiceCallErrorResponse>;
        fn on_connected(&self);
        fn on_disconnected(&self);
        fn healthy(&self) -> bool;
    }
}
pub mod client {
    use mockall::automock;

    use crate::{HandlerId, ServiceId};

    use crate::service::{
        FireboltMethodHandlerRegistration, ServiceCall, ServiceCallErrorResponse,
        ServiceCallSuccessResponse, ServiceErrorResponse, ServiceRegistration,
        ServiceRegistrationFailure, ServiceRegistrationResponse, ServiceRequest,
        ServiceRequestHandler, ServiceSuccessResponse,
    };

    #[async_trait::async_trait]
    /*
    this is a trait that is implemented by a service client.
    There will probably only be 2 of these for the foreseeable future:
    1) a real, websocket client
    2) a mock, for unit/integration testing
    the point of this trait , and it's implementers, is to abstract the details related to auth, connection, etc. from the service (business logic)
    that needs it.

    the methods in the actual interface are concerned with what to do during lifecycle transition events. the graph/steps/etc. of the lifecyle and
    when to call these methods (and any state needed to call them) is the responsiblity of the concrete implementation.
    */
    #[automock]
    pub trait ServiceClientTrait: Send + Sync {
        // this is a request to the service to register itself with the API gateway
        fn register(
            &self,
            registraton: Box<ServiceRegistration>,
        ) -> Result<ServiceRegistrationResponse, ServiceRegistrationFailure>;
        fn set_handler(&mut self, handler: Box<dyn ServiceRequestHandler>);
        fn unregister_service(&mut self, service_id: ServiceId) -> Result<(), String>;
        fn register_handler(
            &mut self,
            handler: FireboltMethodHandlerRegistration,
        ) -> Result<(), String>;
        fn unregister_handler(&mut self, handler_id: HandlerId) -> Result<(), String>;
        async fn invoke_handler(
            &mut self,
            request: ServiceRequest,
        ) -> Result<ServiceSuccessResponse, ServiceErrorResponse>;
    }
    impl<T> ServiceRequestHandler for T
    where
        T: Send + Sync + Clone + 'static + ServiceRequestHandlerImpl,
    {
        fn register(&self) -> Vec<FireboltMethodHandlerRegistration> {
            self.register()
        }

        fn handle_request(
            &self,
            request: ServiceCall,
        ) -> Result<ServiceCallSuccessResponse, ServiceCallErrorResponse> {
            self.handle_request(request)
        }

        fn on_connected(&self) {
            self.on_connected()
        }

        fn on_disconnected(&self) {
            self.on_disconnected()
        }

        fn healthy(&self) -> bool {
            self.healthy()
        }
    }

    pub trait ServiceRequestHandlerImpl: Send + Sync + Clone {
        fn register(&self) -> Vec<FireboltMethodHandlerRegistration>;
        fn handle_request(
            &self,
            request: ServiceCall,
        ) -> Result<ServiceCallSuccessResponse, ServiceCallErrorResponse>;
        fn on_connected(&self);
        fn on_disconnected(&self);
        fn healthy(&self) -> bool;
    }
}
/*
This is the alternative API surface that services can use to communicate with the API gateway.
Firebolt calls are bidirectional, so the API gateway can send messages to the service, and the service can send messages to the API gateway.
*/
mod api_surface {
    use serde::{Deserialize, Serialize};
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct FireboltRequest {
        pub request_id: String,
        pub payload: String,
    }
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct FireboltResponse {
        pub request_id: String,
        pub payload: String,
    }
}

/*
This trait represents the layer that will actually "talk" to the API gateway. it handles transports, etc.
This is a trait to :
1) Enable testing
2) Abstract the details of the transport from the service
3) Allow for different transports (websocket, http, etc.)z
*/
#[async_trait::async_trait]
pub trait APIGatewayClient {
    //fn connect(&self);
    //fn disconnect(&self);
    fn dispatch(&self, request: gateway::ServiceRoutingRequest);
    async fn start(&self);
    //  async fn stop(&self);
}
pub struct WebsocketAPIGatewayClient {
    rpc_server: Methods,
    service_id: ServiceId,
}

use tokio_tungstenite::{connect_async, tungstenite::Message};
use url::Url;

use crate::service::{
    FireboltMethodHandlerAPIRegistration, ServiceCallErrorResponse, ServiceCallSuccessResponse,
};

impl WebsocketAPIGatewayClient {
    pub fn new(rpc_server: Methods, service_id: ServiceId) -> WebsocketAPIGatewayClient {
        WebsocketAPIGatewayClient {
            rpc_server,
            service_id,
        }
    }
    pub async fn handle_messages(
        stream: tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
        rpc_server: Methods,
        _api_gateway_clientfirebolt_handlers: Vec<FireboltMethodHandlerAPIRegistration>,
        // handler: Arc<dyn ServiceRequestHandler>,
        //registration: ServiceRegistration,
    ) {
        let (mut tx, mut rx) = stream.split();
        let firebolt_handlers: Vec<FireboltMethodHandlerAPIRegistration> = rpc_server
            .method_names()
            .map(|method| {
                info!("Registered method: {}", method);
                FireboltMethodHandlerAPIRegistration {
                    firebolt_method: method.to_string(),
                    jq_rule: None, // Assuming no jq rule for now, can be modified as needed
                }
            })
            .collect();

        let registration_request = APIGatewayServiceRegistrationRequest {
            firebolt_handlers: firebolt_handlers.clone(),
        };
        let msg = serde_json::to_string(&APIClientMessages::Register(registration_request.clone()));
        if let Ok(msg) = msg {
            let _ = tx.send(Message::Text(msg)).await;
        } else {
            error!("websocket: Failed to serialize registration request");
        }

        while let Some(message) = rx.next().await {
            match message {
                Ok(msg) => {
                    /*
                    attempt to marshal the message into a ServiceRequest
                    if it fails, log the error and continue
                    */
                    info!("websocket: Received message via websocket: {:?}", msg);
                    let msg = msg.into_text().unwrap_or_default();
                    if let Ok(request) = serde_json::from_str::<APIClientMessages>(&msg) {
                        match request {
                            APIClientMessages::Register(registration_request) => {
                                info!("Received registration request: {:?}", registration_request);
                            }
                            APIClientMessages::Registered(registration_request) => {
                                info!("Received registered response: {:?}", registration_request);
                                //handler.on_connected();
                            }
                            APIClientMessages::Error(error) => {
                                info!("Received error: {:?}", error);
                            }
                            APIClientMessages::Unregister(service_id) => {
                                info!("Received unregister request: {:?}", service_id);
                            }
                            APIClientMessages::ServiceCall(service_call) => {
                                debug!(
                                    "Received service call: {:?}",
                                    service_call.payload.to_string()
                                );
                                let request_id = service_call.request_id.clone();
                                let json_rpc_request = service_call.payload.clone();
                                debug!("Raw JSON-RPC request: {:?}", json_rpc_request);
                                let json_rpc_request =
                                    serde_json::to_value(json_rpc_request).unwrap().to_string();
                                debug!("sending request to RPC server: {:?}", json_rpc_request);

                                match rpc_server.raw_json_request(&json_rpc_request, 1).await {
                                    Ok(okie) => {
                                        debug!("Received response from RPC server: {:?}", okie);
                                        let raw_response = okie.0.clone();
                                        debug!("Raw response: {:?}", raw_response);
                                        let response = serde_json::from_str::<serde_json::Value>(
                                            &raw_response,
                                        );
                                        if let Err(e) = response {
                                            debug!("Failed to parse response: {:?}", e);
                                            let error_response =
                                                APIClientMessages::ServiceCallErrorResponse(
                                                    ServiceCallErrorResponse {
                                                        request_id,
                                                        error: e.to_string(),
                                                    },
                                                );
                                            let error_response =
                                                serde_json::to_string(&error_response);
                                            if let Ok(error_response) = error_response {
                                                let _ = tx
                                                    .send(Message::Text(error_response.clone()))
                                                    .await;
                                                debug!(
                                                    "Sending error response: {:?}",
                                                    error_response
                                                );
                                            } else {
                                                debug!("Failed to serialize error response");
                                            }
                                            return;
                                        } else {
                                            let response = response.unwrap();
                                            info!(
                                                "Parsed response from called service: {:?}",
                                                response
                                            );
                                            let success_response = ServiceCallSuccessResponse {
                                                request_id,
                                                response,
                                            };
                                            let success_response =
                                                APIClientMessages::ServiceCallSuccessResponse(
                                                    success_response,
                                                );
                                            let success_response =
                                                serde_json::to_string(&success_response);
                                            if let Ok(success_response) = success_response {
                                                let _ = tx
                                                    .send(Message::Text(success_response.clone()))
                                                    .await;
                                                debug!(
                                                    "Sending success response: {:?}",
                                                    success_response
                                                );
                                            } else {
                                                error!("Failed to serialize success response");
                                            }
                                        }
                                    }
                                    Err(nope) => {
                                        error!("nope! {:?}", nope);
                                        let error_response =
                                            APIClientMessages::ServiceCallErrorResponse(
                                                ServiceCallErrorResponse {
                                                    request_id,
                                                    error: nope.to_string(),
                                                },
                                            );
                                        let error_response = serde_json::to_string(&error_response);
                                        if let Ok(error_response) = error_response {
                                            let _ = tx
                                                .send(Message::Text(error_response.clone()))
                                                .await;
                                            debug!("Sending error response: {:?}", error_response);
                                        } else {
                                            error!("Failed to serialize error response");
                                        }
                                    }
                                }
                            }
                            _ => {}
                        }
                    } else {
                        error!(" I don't understand this message: {:?}", msg);
                    }
                }
                Err(err) => {
                    error!("Error receiving message: {:?}", err);
                    // Handle the error (e.g., log it, retry, etc.)
                    // You might want to break the loop or handle reconnection logic here
                    // break;
                }
            }
        }
    }
    pub async fn connect(
        &self,
        endpoint_url: Option<String>,
        service_id: ServiceId,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let url = Url::parse(&endpoint_url.unwrap_or_else(|| {
            format!(
                "ws://localhost:3474/apigateway?serviceId={}",
                &service_id.service_id
            )
        }))?;
        let mut backoff = Duration::from_secs(1);
        println!("Connecting to WebSocket at {}", url);

        loop {
            match connect_async(url.clone()).await {
                Ok((ws_stream, _)) => {
                    let rpc_server = self.rpc_server.clone();
                    println!("✅ Connected to WebSocket");

                    // Reset backoff after successful connection
                    backoff = Duration::from_secs(1);

                    // This handles the message loop and returns on disconnect
                    Self::handle_messages(ws_stream, rpc_server, vec![]).await;
                    info!("🔌 Disconnected, retrying...");
                }
                Err(err) => {
                    error!("Error connecting to WebSocket: {:?}", err);

                    // Exponential backoff (up to 1 minute)
                    tokio::time::sleep(backoff).await;
                    if backoff >= Duration::from_secs(60) {
                        backoff = Duration::from_secs(1);
                    }

                    backoff = std::cmp::min(backoff * 2, Duration::from_secs(60));
                }
            }
        }
    }
}

#[async_trait::async_trait]
impl APIGatewayClient for WebsocketAPIGatewayClient {
    // fn connect(&self) {
    //     // connect to the API gateway
    // }
    // fn disconnect(&self) {
    //     // disconnect from the API gateway
    // }
    fn dispatch(&self, _request: ServiceRoutingRequest) {
        // send a request to the API gateway
    }

    async fn start(&self) {
        env_logger::init();
        info!("starting websocket client");
        self.connect(None, self.service_id.clone()).await.unwrap();
        // self.handler.on_connected();
    }

    // async fn stop(&self) {
    //     todo!()
    // }
}
#[cfg(test)]
pub mod tests {
    use std::collections::HashSet;

    use ripple_sdk::api::rules_engine::Rule;
    use serde_json::json;

    use crate::{
        service, Handler, HandlerId, JqRule, ServiceHandler, ServiceId, ServiceRequestId,
        StaticRule,
    };

    #[test]
    fn test_service_id_equality() {
        let id1 = ServiceId::new("service1".to_string());
        let id2 = ServiceId::new("service1".to_string());
        let id3 = ServiceId::new("service2".to_string());
        assert_eq!(id1, id2);
        assert_ne!(id1, id3);
    }

    #[test]
    fn test_service_id_hash() {
        let id1 = ServiceId::new("service1".to_string());
        let id2 = ServiceId::new("service1".to_string());
        let mut set = HashSet::new();
        set.insert(id1);
        assert!(set.contains(&id2));
    }

    #[test]
    fn test_service_id_display() {
        let id = ServiceId::new("abc".to_string());
        assert_eq!(format!("{}", id), "abc");
    }

    #[test]
    fn test_service_request_id_equality() {
        let id1 = ServiceRequestId::new(1);
        let id2 = ServiceRequestId::new(1);
        let id3 = ServiceRequestId::new(2);
        assert_eq!(id1, id2);
        assert_ne!(id1, id3);
    }

    #[test]
    fn test_service_request_id_hash() {
        let id1 = ServiceRequestId::new(1);
        let id2 = ServiceRequestId::new(1);
        let mut set = HashSet::new();
        set.insert(id1);
        assert!(set.contains(&id2));
    }

    #[test]
    fn test_handler_display() {
        let jq = Handler::JqRule(JqRule {
            alias: "jq".to_string(),
            rule: ".foo".to_string(),
        });
        let static_rule = Handler::StaticRule(StaticRule {
            alias: "static".to_string(),
            rule: "bar".to_string(),
        });
        let none = Handler::None;
        assert_eq!(format!("{}", jq), "jq");
        assert_eq!(format!("{}", static_rule), "static");
        assert_eq!(format!("{}", none), "none");
    }

    #[test]
    fn test_handler_into_rule() {
        let jq = Handler::JqRule(JqRule {
            alias: "jq".to_string(),
            rule: ".foo".to_string(),
        });
        let rule: Rule = jq.clone().into();
        assert_eq!(rule.alias, "jq");
        assert_eq!(rule.filter, Some(".foo".to_string()));

        let static_rule = Handler::StaticRule(StaticRule {
            alias: "static".to_string(),
            rule: "bar".to_string(),
        });
        let rule: Rule = static_rule.clone().into();
        assert_eq!(rule.alias, "static");
        assert_eq!(rule.filter, Some("bar".to_string()));

        let none = Handler::None;
        let rule: Rule = none.into();
        assert_eq!(rule.alias, "none");
        assert!(rule.filter.is_none());
    }

    #[test]
    fn test_service_registration_builder() {
        let service_id = ServiceId::new("svc".to_string());
        let mut builder = service::ServiceRegistrationBuilder::new(service_id.clone());
        builder.add_handler(Handler::JqRule(JqRule {
            alias: "jq".to_string(),
            rule: ".foo".to_string(),
        }));
        let reg = builder.build();
        assert_eq!(reg.service_id, service_id);
        assert_eq!(reg.firebolt_handlers.len(), 1);
        assert_eq!(reg.firebolt_handlers[0].firebolt_method, "jq");
    }

    #[test]
    fn test_service_registration_get_rule_registrations() {
        let reg = service::ServiceRegistration {
            service_id: ServiceId::new("svc".to_string()),
            firebolt_handlers: vec![service::FireboltMethodHandlerAPIRegistration {
                firebolt_method: "foo".to_string(),
                jq_rule: None,
            }],
        };
        let rules = reg.get_rule_registrations();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].firebolt_method, "foo");
    }

    #[test]
    fn test_service_call_into_jsonrpc_api_request() {
        let call = service::ServiceCall {
            request_id: ServiceRequestId::new(42),
            method: "test".to_string(),
            payload: json!({"foo": "bar"}),
        };
        let req: ripple_sdk::api::gateway::rpc_gateway_api::JsonRpcApiRequest = call.into();
        assert_eq!(req.id, Some(42));
        assert_eq!(req.method, "test");
        assert_eq!(req.params, Some(json!({"foo": "bar"})));
    }

    #[test]
    fn test_apigatewayserviceregistrationrequest_into_response() {
        let req = service::APIGatewayServiceRegistrationRequest {
            firebolt_handlers: vec![service::FireboltMethodHandlerAPIRegistration {
                firebolt_method: "foo".to_string(),
                jq_rule: None,
            }],
        };
        let resp: service::APIGatewayServiceRegistrationResponse = req.clone().into();
        assert_eq!(resp.firebolt_handlers, req.firebolt_handlers);
    }

    #[test]
    fn test_websocketserviceresponse_get_id() {
        let id = ServiceRequestId::new(7);
        let resp = service::WebsocketServiceResponse::Success(id.clone(), json!({}));
        assert_eq!(resp.get_id(), id);
        let resp = service::WebsocketServiceResponse::Error(id.clone(), "err".to_string());
        assert_eq!(resp.get_id(), id);
    }

    #[test]
    fn test_apiclientmessages_default() {
        let msg = service::APIClientMessages::default();
        match msg {
            service::APIClientMessages::Register(_) => {}
            _ => panic!("Default should be Register"),
        }
    }

    #[test]
    fn test_servicecallresponse_serialization() {
        let resp = service::ServiceCallResponse::Success(service::ServiceCallSuccessResponse {
            request_id: ServiceRequestId::new(1),
            response: json!({"foo": "bar"}),
        });
        let s = serde_json::to_string(&resp).unwrap();
        let de: service::ServiceCallResponse = serde_json::from_str(&s).unwrap();
        match de {
            service::ServiceCallResponse::Success(success) => {
                assert_eq!(success.request_id, ServiceRequestId::new(1));
                assert_eq!(success.response, json!({"foo": "bar"}));
            }
            _ => panic!("Expected Success"),
        }
    }

    #[test]
    fn test_servicecallerrorresponse_serialization() {
        let resp = service::ServiceCallResponse::Error(service::ServiceCallErrorResponse {
            request_id: ServiceRequestId::new(2),
            error: "fail".to_string(),
        });
        let s = serde_json::to_string(&resp).unwrap();
        let de: service::ServiceCallResponse = serde_json::from_str(&s).unwrap();
        match de {
            service::ServiceCallResponse::Error(err) => {
                assert_eq!(err.request_id, ServiceRequestId::new(2));
                assert_eq!(err.error, "fail");
            }
            _ => panic!("Expected Error"),
        }
    }

    #[test]
    fn test_handlerid_serialize_deserialize() {
        let handler_id = HandlerId {
            handler_id: "abc".to_string(),
        };
        let s = serde_json::to_string(&handler_id).unwrap();
        let de: HandlerId = serde_json::from_str(&s).unwrap();
        assert_eq!(handler_id, de);
    }

    #[test]
    fn test_jqrule_serialize_deserialize() {
        let jq = JqRule {
            alias: "a".to_string(),
            rule: "b".to_string(),
        };
        let s = serde_json::to_string(&jq).unwrap();
        let de: JqRule = serde_json::from_str(&s).unwrap();
        assert_eq!(jq, de);
    }

    #[test]
    fn test_statirule_serialize_deserialize() {
        let sr = StaticRule {
            alias: "a".to_string(),
            rule: "b".to_string(),
        };
        let s = serde_json::to_string(&sr).unwrap();
        let de: StaticRule = serde_json::from_str(&s).unwrap();
        assert_eq!(sr, de);
    }

    #[test]
    fn test_servicehandler_serialize_deserialize() {
        let handler = ServiceHandler {
            handler_id: HandlerId {
                handler_id: "id".to_string(),
            },
            handler_type: Handler::None,
        };
        let s = serde_json::to_string(&handler).unwrap();
        let de: ServiceHandler = serde_json::from_str(&s).unwrap();
        assert_eq!(handler.handler_id, de.handler_id);
        match de.handler_type {
            Handler::None => {}
            _ => panic!("Expected None"),
        }
    }
}
