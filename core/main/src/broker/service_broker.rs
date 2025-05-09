use std::sync::Arc;

use futures_channel::oneshot;
use ripple_sdk::{
    api::rules_engine::RuleEngineProvider,
    log::{error, info},
    tokio::{
        self,
        sync::mpsc::{self, Sender},
    },
    utils::rpc_utils,
};
use ssda_types::gateway::{ServiceRequest, ServiceResponse};
use ssda_types::ServiceRequestId;
use tokio_tungstenite::tungstenite::http::request;

use crate::state::platform_state::{self, PlatformState};

use super::endpoint_broker::{
    BrokerCallback, BrokerCleaner, BrokerConnectRequest, BrokerRequest, BrokerSender,
    EndpointBroker, EndpointBrokerState, BROKER_CHANNEL_BUFFER_SIZE,
};

pub struct ServiceBroker {
    platform_state: Option<PlatformState>,
    connect_request: BrokerConnectRequest,
    broker_callback: BrokerCallback,
    endpoint_broker_state: EndpointBrokerState,
    broker_sender: BrokerSender,
    cleaner: BrokerCleaner,
}

impl ServiceBroker {}
impl EndpointBroker for ServiceBroker {
    fn get_broker(
        ps: Option<crate::state::platform_state::PlatformState>,
        connect_request: super::endpoint_broker::BrokerConnectRequest,
        broker_callback: super::endpoint_broker::BrokerCallback,
        endpoint_broker: &mut super::endpoint_broker::EndpointBrokerState,
    ) -> Self {
        //todo!();
        // let endpoint = request.endpoint.clone();
        let (tx, mut tr) = mpsc::channel(BROKER_CHANNEL_BUFFER_SIZE);
        let broker_sender = BrokerSender { sender: tx };

        if let Some(platform_state) = ps.clone() {
            tokio::spawn(async move {
                while let Some(request) = tr.recv().await {
                    let services_tx = platform_state
                        .services_gateway_api
                        .lock()
                        .await
                        .get_sender();
                    use tokio::sync::oneshot;
                    let (oneshot_tx, mut oneshot_rx) = oneshot::channel::<ServiceResponse>();
                    let service_request = ServiceRequest {
                        request_id: ServiceRequestId {
                            request_id: request.get_id(),
                        },
                        payload: request.rpc.clone(),
                        respond_to: oneshot_tx,
                    };

                    services_tx.try_send(service_request).unwrap();

                    match oneshot_rx.await {
                        Ok(response) => {
                            info!("ServiceBroker received response: {:?}", response);
                            //let _ = request.respond(response);
                        }
                        Err(e) => {
                            error!("ServiceBroker failed to receive response {}", e);
                            // let _ = request.respond(ripple_sdk::api::rpc_utils::ErrorResponse {
                            //     error: ripple_sdk::api::rpc_utils::Error {
                            //         code: -32603,
                            //         message: "Failed to receive response".to_string(),
                            //         data: None,
                            //     },
                            // });
                        }
                    }
                    info!("ServiceBroker received request: {:?}", request);
                }
            });
        } else {
            panic!("Platform state is required");
        };

        Self {
            platform_state: ps.clone(),
            connect_request: connect_request,
            broker_callback: broker_callback,
            endpoint_broker_state: endpoint_broker.clone(),
            broker_sender: broker_sender,
            cleaner: BrokerCleaner { cleaner: None },
        }
    }

    fn get_sender(&self) -> super::endpoint_broker::BrokerSender {
        self.broker_sender.clone()
    }

    fn get_cleaner(&self) -> super::endpoint_broker::BrokerCleaner {
        BrokerCleaner::default()
    }
}
