/*
This is the API gateway, and it meant to be hosted in the main ripple process
*/

use std::{collections::HashMap, f32::consts::E, sync::Arc};

use futures_util::stream::SplitSink;
use futures_util::stream::StreamExt;
use futures_util::SinkExt;
use http::Uri;
use log::{error, info};

use ripple_sdk::api::rules_engine::{Rule, RuleEngineProvider, RuleTransform};
use tokio::sync::{mpsc, RwLock};
pub struct ServiceMap {
    service_map: HashMap<ServiceId, Vec<FireboltMethodHandlerAPIRegistration>>,
}
impl ServiceMap {
    fn new() -> Self {
        Self {
            service_map: HashMap::new(),
        }
    }
    fn add_service(
        &mut self,
        service_id: ServiceId,
        registrations: Vec<FireboltMethodHandlerAPIRegistration>,
    ) {
        self.service_map.insert(service_id, registrations);
    }
    fn remove_service(&mut self, service_id: &ServiceId) {
        self.service_map.remove(service_id);
    }
    fn get_registrations(
        &self,
        service_id: &ServiceId,
    ) -> Vec<FireboltMethodHandlerAPIRegistration> {
        self.service_map.get(service_id).unwrap_or(&vec![]).clone()
    }
    fn get_service_for_method(
        &self,
        method: &str,
    ) -> Option<(ServiceId, FireboltMethodHandlerAPIRegistration)> {
        for (service_id, registrations) in self.service_map.iter() {
            for registration in registrations.iter() {
                if registration.firebolt_method == method {
                    return Some((service_id.clone(), registration.clone()));
                }
            }
        }
        None
    }
}
pub struct WebSocketChannels {
    pub tx: mpsc::Sender<WebsocketServiceRequest>,
    pub rx: mpsc::Receiver<WebsocketServiceRequest>,
}
pub struct ApiGateway {
    pub service_endpoints: Arc<tokio::sync::RwLock<HashMap<String, mpsc::Sender<Message>>>>,
    pub rules_engine: Arc<tokio::sync::RwLock<Box<dyn RuleEngineProvider + Send + Sync>>>,
    pub methods_2_services: Arc<tokio::sync::RwLock<ServiceMap>>,
    pub broker_sender: mpsc::Sender<ServiceRoutingRequest>,
    pub services_2_rxs:
        Arc<tokio::sync::RwLock<HashMap<ServiceId, mpsc::Sender<WebsocketServiceRequest>>>>,
    pub reply_to_tx: mpsc::Sender<ServiceRoutingResponse>,
}
pub enum APIGatewayClientState {
    Failed(String),
    Connecting,
    Registering(APIGatewayServiceRegistrationRequest),
    Connected,
    Closed,
    Message(ServiceRequestId, Value),
    ServiceCallFailed(ServiceRequestId, String),
}
type RequestIds2SendersType = Arc<
    RwLock<
        HashMap<ServiceRequestId, Arc<Mutex<Option<oneshot::Sender<WebsocketServiceResponse>>>>>,
    >,
>;

impl ApiGateway {
    pub fn new(
        rules_engine: Arc<tokio::sync::RwLock<Box<dyn RuleEngineProvider + Send + Sync>>>,
    ) -> Self {
        let (tx, rx) = mpsc::channel::<ServiceRoutingRequest>(32);
        let (reply_to_tx, _) = mpsc::channel::<ServiceRoutingResponse>(32);
        let services_2_rxs = Arc::new(tokio::sync::RwLock::new(HashMap::new()));

        let me = Self {
            service_endpoints: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
            rules_engine,
            methods_2_services: Arc::new(tokio::sync::RwLock::new(ServiceMap::new())),
            broker_sender: tx.clone(),
            services_2_rxs: services_2_rxs.clone(),
            reply_to_tx,
        };

        me.start(rx, Arc::clone(&services_2_rxs));

        me
    }
    /*
    start the task that handles the service routing requests, and routes them to the appropriate service
    */
    pub fn start(
        &self,
        mut rx: mpsc::Receiver<ServiceRoutingRequest>,
        services_2_rxes: Arc<
            tokio::sync::RwLock<HashMap<ServiceId, mpsc::Sender<WebsocketServiceRequest>>>,
        >,
    ) {
        let methods_2_services = self.methods_2_services.clone();
        let services_2_rxs = Arc::clone(&services_2_rxes);
        tokio::spawn(async move {
            while let Some(request) = rx.recv().await {
                let methods_2_services = methods_2_services.read().await;
                let method = request.payload.method.clone();
                match methods_2_services.get_service_for_method(method.as_str()) {
                    Some((service_id, _registration)) => {
                        /*
                        send the request to the websocket handler for the service
                        */
                        if let Some(service) = services_2_rxs.read().await.get(&service_id) {
                            let (send, receive) = oneshot::channel::<WebsocketServiceResponse>();
                            let payload = request.payload.clone();
                            let websocket_request = WebsocketServiceRequest {
                                method: payload.method.clone(),
                                request_id: request.request_id.clone(),
                                payload: serde_json::to_value(payload).unwrap(),
                                respond_to: send,
                            };
                            let _ = service.send(websocket_request).await;
                            match receive.await {
                                Ok(response) => {
                                    info!("start Received response from service: {:?}", response);
                                    match response {
                                        WebsocketServiceResponse::Success(request_id, result) => {
                                            info!("Received success response: {:?}", result);
                                            // Send the response back to the broker
                                            let _ = request.respond_to.send(
                                                ServiceRoutingResponse::Success(
                                                    ServiceRoutingSuccessResponse {
                                                        request_id,
                                                        response: result,
                                                    },
                                                ),
                                            );
                                        }
                                        WebsocketServiceResponse::Error(request_id, error) => {
                                            info!("Received error response: {:?}", error);
                                            // Send the error response back to the broker
                                            let _ = request.respond_to.send(
                                                ServiceRoutingResponse::Error(
                                                    ServiceRoutingErrorResponse {
                                                        request_id,
                                                        error,
                                                    },
                                                ),
                                            );
                                        }
                                    }
                                }
                                Err(e) => {
                                    error!("Failed to receive response from service: {:?}", e);
                                }
                            }
                        } else {
                            error!(
                                "else No service sender found for method: {:?} in {:?} ",
                                method,
                                services_2_rxs.read().await
                            );
                        }
                    }

                    None => {
                        error!("None No service found for method: {:?}", method);
                    }
                }
            }
        });
    }
    /// This function is used to check if the given URI is a service URL
    pub fn is_apigateway_connection(
        uri: &http::Uri,
    ) -> Result<APIGatewayServiceConnectionDisposition, APIGatewayServiceConnectionError> {
        // Check if the URI is a service URL
        // This is a placeholder implementation
        if !uri.path().starts_with("/apigateway") {
            return Err(APIGatewayServiceConnectionError::NotAService);
        }

        if let Some(query) = uri.query() {
            let query_pairs = form_urlencoded::parse(query.as_bytes());
            for (key, value) in query_pairs {
                if key == "serviceId" {
                    return Ok(APIGatewayServiceConnectionDisposition::Accept(
                        ServiceId::new(value.to_string()),
                    ));
                }
            }
        }
        Err(APIGatewayServiceConnectionError::NotAService)
    }
    pub fn classify_message(message: &Message) -> APIGatewayClientState {
        // Classify the message based on its content
        // This is a placeholder implementation
        if let Message::Text(text) = message {
            let parsed: Result<APIClientMessages, _> = serde_json::from_str(text);
            info!("Parsed message: {:?}", parsed);
            if let Ok(msg) = parsed {
                match msg {
                    APIClientMessages::Register(registration) => {
                        return APIGatewayClientState::Registering(registration);
                    }

                    APIClientMessages::ServiceCallSuccessResponse(succcess) => {
                        return APIGatewayClientState::Message(
                            succcess.request_id,
                            succcess.response,
                        )
                    }
                    APIClientMessages::ServiceCallErrorResponse(error) => {
                        return APIGatewayClientState::ServiceCallFailed(
                            error.request_id,
                            error.error,
                        );
                    }
                    e => {
                        info!("Classified message: {:?}", e);
                        return APIGatewayClientState::Failed(format!(
                            "Unknown message type: {:?}",
                            e
                        ));
                    }
                }
            }
        } else if let Message::Close(_) = message {
            return APIGatewayClientState::Closed;
        }

        APIGatewayClientState::Failed(format!("Failed to parse message: {}", message))
    }
    fn jq_rule_to_string(jq_rule: Option<JqRule>) -> Option<String> {
        if let Some(rule) = jq_rule {
            return Some(rule.rule);
        }
        None
    }
    async fn handle_registration(
        service_id: ServiceId,
        rule_engine: Arc<tokio::sync::RwLock<Box<dyn RuleEngineProvider + Send + Sync>>>,
        registration: &APIGatewayServiceRegistrationRequest,
        methods_2_services: Arc<tokio::sync::RwLock<ServiceMap>>,
    ) {
        let rules = &registration.firebolt_handlers;

        let mut rule_engine = rule_engine.write().await;

        for handle_rule in rules {
            let rule = Rule {
                alias: handle_rule.firebolt_method.clone(),
                transform: RuleTransform::default(),
                filter: Self::jq_rule_to_string(handle_rule.jq_rule.clone()),
                event_handler: None,
                endpoint: Some("service".to_string()),
                sources: None,
            };
            /*
            todo, save off existing rule for rollback (if it exists)
            */
            rule_engine.add_rule(rule);
        }
        methods_2_services
            .write()
            .await
            .add_service(service_id.clone(), rules.clone());
    }
    async fn handle_unregister(
        service_id: &ServiceId,
        methods_2_services: Arc<tokio::sync::RwLock<ServiceMap>>,
        rule_engine: Arc<tokio::sync::RwLock<Box<dyn RuleEngineProvider + Send + Sync>>>,
    ) {
        let mut rule_engine = rule_engine.write().await;
        let aliases = methods_2_services
            .read()
            .await
            .get_registrations(service_id);
        for handle_rule in aliases {
            rule_engine.remove_rule(&handle_rule.firebolt_method);
        }
        methods_2_services.write().await.remove_service(service_id);
    }

    async fn handle_message(
        message: Result<Message, Error>,
        tx: &mut SplitSink<WebSocketStream<TcpStream>, Message>,
        service_id: &ServiceId,
        rule_engine: Arc<tokio::sync::RwLock<Box<dyn RuleEngineProvider + Send + Sync>>>,
        methods_2_services: Arc<tokio::sync::RwLock<ServiceMap>>,
        services_2_rxes: Arc<
            tokio::sync::RwLock<HashMap<ServiceId, mpsc::Sender<WebsocketServiceRequest>>>,
        >,
        bridge_tx: mpsc::Sender<WebsocketServiceResponse>,
    ) {
        match message {
            Ok(msg) => {
                info!("gateway server received websocket message: {:?}", msg);
                match Self::classify_message(&msg) {
                    APIGatewayClientState::Registering(registration) => {
                        info!("Registering service: {:?}", registration);
                        Self::handle_unregister(
                            service_id,
                            methods_2_services.clone(),
                            rule_engine.clone(),
                        )
                        .await;

                        Self::handle_registration(
                            service_id.clone(),
                            rule_engine.clone(),
                            &registration,
                            methods_2_services.clone(),
                        )
                        .await;

                        let response: APIGatewayServiceRegistrationResponse = registration.into();
                        let response = APIClientMessages::Registered(response);
                        let _ = tx
                            .send(Message::Text(serde_json::to_string(&response).unwrap()))
                            .await;
                        info!("Sent registration response: {:?}", response)
                    }
                    APIGatewayClientState::Failed(e) => {
                        error!("Failed to classify message {} err {},{}", msg, E, e);

                        let _ = tx.send(Message::Close(None)).await;
                        tx.close().await.unwrap();
                    }
                    APIGatewayClientState::Closed => {
                        info!("Client closed connection");
                        Self::handle_unregister(service_id, methods_2_services, rule_engine).await;
                        tx.close().await.unwrap();
                    }
                    APIGatewayClientState::Message(request_id, msg) => {
                        info!(
                            "got msg from websocket:  {} for request {:?}",
                            msg, request_id
                        );
                        bridge_tx
                            .send(WebsocketServiceResponse::Success(request_id, msg))
                            .await
                            .unwrap();
                    }
                    APIGatewayClientState::ServiceCallFailed(id, error) => {
                        info!("Service call failed: {:?}", error);
                        let fail = WebsocketServiceResponse::Error(id, error);
                        let _ = bridge_tx.send(fail).await;
                    }

                    _ => {
                        info!("handle_message: Unknown message type: {:?}", msg);
                        let _ = tx.send(Message::Close(None)).await;
                        tx.close().await.unwrap();
                    }
                }
            }
            Err(e) => {
                /*
                for now, just treat all errors as fatal and cleanup, drop tx  and let client reconnect
                */
                info!("Client closed connection. Error: {:?}", e);
                info!("Unregistering service: {:?}", service_id);

                Self::handle_unregister(service_id, methods_2_services, rule_engine.clone()).await;
                services_2_rxes.write().await.remove(service_id).unwrap();

                tx.close().await.unwrap();
            }
        }
    }
    /*
    Spawn a task to handle exactly one service connection over websocket (for now, maybe dbus later)
    */

    async fn handle_service_connection(
        service_id: ServiceId,
        ws_stream: tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>,
        rule_engine: Arc<tokio::sync::RwLock<Box<dyn RuleEngineProvider + Send + Sync>>>,
        methods_2_services: Arc<tokio::sync::RwLock<ServiceMap>>,
        mut websocket_service_request_rx: mpsc::Receiver<WebsocketServiceRequest>,
        service_2_rxes: Arc<
            tokio::sync::RwLock<HashMap<ServiceId, mpsc::Sender<WebsocketServiceRequest>>>,
        >,
    ) -> Result<(), APIGatewayServiceConnectionError> {
        info!("Handling service connection: {:?}", service_id);

        let (mut websocket_tx, mut websocket_rx) = ws_stream.split();

        let (bridge_tx, mut bridge_rx) = mpsc::channel::<WebsocketServiceResponse>(32);

        let requests_2_requestors: RequestIds2SendersType = Arc::new(RwLock::new(HashMap::new()));

        loop {
            tokio::select! {
                // Handle incoming messages from the WebSocket stream
                Some(message) = websocket_rx.next() => {
                    info!("gateway Received websocket message: {:?}", message);
                    Self::handle_message(
                        message,
                        &mut websocket_tx,
                        &service_id,
                        rule_engine.clone(),
                        methods_2_services.clone(),
                        service_2_rxes.clone(),
                        bridge_tx.clone(),
                    ).await
                }
                // this is a request from the main api gateway thread that needs to be sent to the service
                //via the websocket
                Some(request) = websocket_service_request_rx.recv() => {
                    let request_id = request.request_id.clone();
                    let sender_entry = Arc::new(Mutex::new(Some(request.respond_to)));
                    requests_2_requestors.write().await.insert(request_id.clone(), sender_entry.clone());

                    let service_call =  APIClientMessages::ServiceCall(
                        ServiceCall {
                            method: request.method.clone(),
                            request_id,
                            payload: request.payload.clone(),
                        }
                     );
                     let service_call = serde_json::to_string(&service_call).unwrap();
                     websocket_tx.send(Message::Text(service_call)).await.unwrap();

                }
                Some(bridge_message) = bridge_rx.recv() => {
                    info!("Received message from bridge: {:?}", bridge_message);

                    let id = bridge_message.get_id();

                    if let Some(requestor) = requests_2_requestors.write().await.remove(&id) {
                        let mut requestor_lock = requestor.lock().await;
                        if let Some(sender) = requestor_lock.take() {
                            let _ = sender.send(bridge_message.clone());
                        }
                    }
                }
               else => {
                    info!("Service connection closed: {:?}", service_id);
                    // Handle service disconnection
                    // Clean up resources, etc.
                    Self::handle_unregister(
                        &service_id,
                        methods_2_services.clone(),
                        rule_engine.clone(),
                    ).await;
                    service_2_rxes
                        .write()
                        .await
                        .remove(&service_id)
                        .unwrap();
                    break Ok(());
                }
            }
        }
    }
}
use serde_json::Value;
use ssda_types::gateway::{
    APIGatewayServiceConnectionDisposition, APIGatewayServiceConnectionError, ApiGatewayServer,
    ServiceRoutingErrorResponse, ServiceRoutingRequest, ServiceRoutingResponse,
    ServiceRoutingSuccessResponse,
};
use ssda_types::service::{
    APIClientMessages, APIGatewayServiceRegistrationRequest, APIGatewayServiceRegistrationResponse,
    FireboltMethodHandlerAPIRegistration, ServiceCall, WebsocketServiceRequest,
    WebsocketServiceResponse,
};
use ssda_types::{JqRule, ServiceId, ServiceRequestId};
use tokio::net::TcpStream;
use tokio::sync::{oneshot, Mutex};
use tokio_tungstenite::tungstenite::{Error, Message};
use tokio_tungstenite::WebSocketStream;
use url::form_urlencoded;

pub struct WebsocketHandler {
    pub service_id: ServiceId,
}
#[async_trait::async_trait]
impl ApiGatewayServer for ApiGateway {
    async fn is_service_connect(
        &self,
        uri: Uri,
    ) -> Result<APIGatewayServiceConnectionDisposition, APIGatewayServiceConnectionError> {
        Self::is_apigateway_connection(&uri)
    }
    async fn service_connect(
        &mut self,
        service_id: ServiceId,
        ws_stream: tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>,
    ) -> Result<APIGatewayServiceConnectionDisposition, APIGatewayServiceConnectionError> {
        info!("new Service connected: {:?}", service_id);

        let spawn_service_id = service_id.clone();
        let rule_engine_for_service = self.rules_engine.clone();
        /*
        The broker_rx is a channel that receives service routing requests from the broker. It is the responsibility
        of this (APIGateway) to handle the service routing requests and send them to the appropriate service.
        Each service connection will have its own broker_rx channel, and the APIGateway will handle the routing
        */
        let (websocket_handler_sender, websocket_handler_receiver) =
            mpsc::channel::<WebsocketServiceRequest>(32);

        /*
        map sender for the websocket handler to the service id for later use
        */
        {
            let mut updater = self.services_2_rxs.write().await;
            updater.insert(service_id.clone(), websocket_handler_sender);
        }
        let methods_2_services = self.methods_2_services.clone();
        let services_2_rxs = Arc::clone(&self.services_2_rxs);
        tokio::spawn(async move {
            let _ = Self::handle_service_connection(
                spawn_service_id,
                ws_stream,
                rule_engine_for_service,
                methods_2_services.clone(),
                websocket_handler_receiver,
                services_2_rxs.clone(),
            )
            .await;
        });

        Ok(APIGatewayServiceConnectionDisposition::Accept(service_id))
    }
    fn get_sender(&self) -> tokio::sync::mpsc::Sender<ServiceRoutingRequest> {
        self.broker_sender.clone()
    }
}
