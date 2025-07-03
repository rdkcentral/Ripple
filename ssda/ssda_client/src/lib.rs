use jsonrpsee::RpcModule;
use ssda_types::{service::ServiceRegistration, APIGatewayClient, WebsocketAPIGatewayClient};
pub enum APIGatewayClientTransport {
    WebSocket,
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

    pub fn build<T>(&self, rpc_server: RpcModule<T>) -> Box<dyn APIGatewayClient> {
        let methods = rpc_server.clone();
        match self.transport {
            APIGatewayClientTransport::WebSocket => Box::new(WebsocketAPIGatewayClient::new(
                methods,
                self.service_registration.service_id.clone(),
            )),
        }
    }
}
