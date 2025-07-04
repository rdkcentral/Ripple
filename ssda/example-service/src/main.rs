use std::fs::File;
use std::{sync::Arc, vec};

use async_trait::async_trait;
use jsonrpsee::core::RpcResult;
use jsonrpsee::proc_macros::rpc;
use log::info;
use serde_json::json;
use ssda_client::APIGatewayClientBuilder;
use ssda_types::client::ServiceRequestHandlerImpl;
use ssda_types::service::{
    FireboltMethodHandlerAPIRegistration, FireboltMethodHandlerRegistration, ServiceCall,
    ServiceCallErrorResponse, ServiceCallSuccessResponse, ServiceRegistration,
};
use ssda_types::{JqRule, ServiceId};

#[rpc(server)]
pub trait ExampleServiceRpc {
    #[method(name = "device.audio")]
    async fn audio(&self) -> RpcResult<String>;
    #[method(name = "device.make")]
    async fn make(&self) -> RpcResult<String>;
}

#[derive(Debug, Clone)]
pub struct ExampleService {}

impl ExampleService {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl ExampleServiceRpcServer for ExampleService {
    async fn audio(&self) -> RpcResult<String> {
        Ok("Audio response".to_string())
    }
    async fn make(&self) -> RpcResult<String> {
        Ok("Make response".to_string())
    }
}

#[async_trait::async_trait]
impl ServiceRequestHandlerImpl for ExampleService {
    fn register(&self) -> Vec<FireboltMethodHandlerRegistration> {
        vec![]
    }
    fn handle_request(
        &self,
        request: ServiceCall,
    ) -> Result<ServiceCallSuccessResponse, ServiceCallErrorResponse> {
        // Handle the request and return a response
        info!("handle_request: handling request {:?}", request);
        match request.method.as_str() {
            "device.audio" => Ok(ServiceCallSuccessResponse {
                request_id: request.request_id,
                response: serde_json::json!({ "status": "success" }),
            }),
            bad_method => {
                return Err(ServiceCallErrorResponse {
                    request_id: request.request_id,
                    error: format!("Unknown method: {}", bad_method),
                });
            }
        }
    }
    fn on_connected(&self) {
        println!("example connected")
    }
    fn on_disconnected(&self) {
        println!("disconnected")
    }
    fn healthy(&self) -> bool {
        todo!()
    }
}
#[tokio::main]
async fn main() {
    let module = ExampleService::new().into_rpc();
    // let request = json!({
    //     "jsonrpc": "2.0",
    //     "id": 1,
    //     "method": "device.aasdf",
    //     "params": []
    // });
    // let request= serde_json::to_string(&request).unwrap();
    // let f = module.raw_json_request(&request, 1).await.unwrap();
    // println!("Response: {:?}", f);

    let my_handler = ExampleService::new();
    let mut firebolt_handlers = Vec::new();

    firebolt_handlers.push(FireboltMethodHandlerAPIRegistration {
        firebolt_method: "device.audio".to_string(),
        jq_rule: Some(JqRule {
            alias: "device.audio".to_string(),
            rule: "jq_type".to_string(),
        }),
    });
    firebolt_handlers.push(FireboltMethodHandlerAPIRegistration {
        firebolt_method: "device.make".to_string(),
        jq_rule: Some(JqRule {
            alias: "device.make".to_string(),
            rule: "jq_type".to_string(),
        }),
    });

    let registration = ServiceRegistration {
        service_id: ServiceId {
            service_id: "example".to_string(),
        },
        firebolt_handlers: firebolt_handlers,
    };

    let _ = APIGatewayClientBuilder::<ExampleService>::new(registration)
        .websocket()
        .build(module)
        .start()
        .await;
}
