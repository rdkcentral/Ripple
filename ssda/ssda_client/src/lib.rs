use std::sync::Arc;

use jsonrpsee::RpcModule;
use ssda_types::{
    service::{ServiceRegistration, ServiceRequestHandler},
    APIGatewayClient, WebsocketAPIGatewayClient,
};
pub enum APIGatewayClientTransport {
    WebSocket,
}
pub struct APIGatewayClientBuilder<T> {
    transport: APIGatewayClientTransport,
    service_registration: ServiceRegistration,
    rpc_server: Option<RpcModule<T>>,
}

impl<T> APIGatewayClientBuilder<T> {
    pub fn new(service_registration: ServiceRegistration) -> Self {
        APIGatewayClientBuilder {
            transport: APIGatewayClientTransport::WebSocket,
            service_registration,
            rpc_server: None,
        }
    }
    pub fn with_service_registration(
        &mut self,
        service_registration: ServiceRegistration,
    ) -> &mut Self {
        self.service_registration = service_registration;
        self
    }

    pub fn websocket(&mut self) -> &mut Self {
        // Initialize WebSocket client
        self.transport = APIGatewayClientTransport::WebSocket;
        self
    }

    pub fn build(&self, rpc_server: RpcModule<T>) -> Box<dyn APIGatewayClient> {
        let methods = rpc_server.clone();
        match self.transport {
            APIGatewayClientTransport::WebSocket => Box::new(WebsocketAPIGatewayClient::new(
                methods,
                self.service_registration.service_id.clone(),
            )),
        }
    }
}
