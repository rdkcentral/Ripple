use std::sync::Arc;

use ssda_types::{
    service::{ServiceRegistration, ServiceRequestHandler},
    APIGatewayClient, DBUSAPIGatewayClient, WebsocketAPIGatewayClient,
};
pub enum APIGatewayClientTransport {
    WebSocket,
    Dbus,
}
pub struct APIGatewayClientBuilder {
    transport: APIGatewayClientTransport,
    service_registration: ServiceRegistration,
}

impl APIGatewayClientBuilder {
    pub fn new(service_registration: ServiceRegistration) -> Self {
        APIGatewayClientBuilder {
            transport: APIGatewayClientTransport::WebSocket,
            service_registration,
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
    pub fn dbus(&mut self) -> &mut Self {
        // Initialize DBus client
        self.transport = APIGatewayClientTransport::Dbus;
        self
    }
    pub fn build(&self, handler: Arc<dyn ServiceRequestHandler>) -> Box<dyn APIGatewayClient> {
        match self.transport {
            APIGatewayClientTransport::WebSocket => Box::new(WebsocketAPIGatewayClient::new(
                handler,
                self.service_registration.clone(),
            )),
            APIGatewayClientTransport::Dbus => Box::new(DBUSAPIGatewayClient {}),
        }
    }
}
