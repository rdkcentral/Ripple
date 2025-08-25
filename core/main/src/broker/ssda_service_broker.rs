use ripple_sdk::{
    api::gateway::rpc_gateway_api::{JsonRpcApiError, JsonRpcApiResponse},
    log::{error, info},
    tokio::sync::mpsc,
};
use ssda_types::gateway::{ServiceRoutingRequest, ServiceRoutingResponse};
use ssda_types::ServiceRequestId;

use super::endpoint_broker::{
    BrokerCleaner, BrokerOutputForwarder, BrokerSender, EndpointBroker, BROKER_CHANNEL_BUFFER_SIZE,
};
use ripple_sdk::tokio;

pub struct SsdaServiceBroker {
    broker_sender: BrokerSender,
}

impl SsdaServiceBroker {}
impl EndpointBroker for SsdaServiceBroker {
    fn get_broker(
        ps: Option<crate::state::platform_state::PlatformState>,
        _connect_request: super::endpoint_broker::BrokerConnectRequest,
        broker_callback: super::endpoint_broker::BrokerCallback,
        _endpoint_broker: &mut super::endpoint_broker::EndpointBrokerState,
    ) -> Self {
        //todo!();
        // let endpoint = request.endpoint.clone();
        let (tx, mut tr) = mpsc::channel(BROKER_CHANNEL_BUFFER_SIZE);
        let broker_sender = BrokerSender { sender: tx };
        let callback = broker_callback.clone();
        if let Some(platform_state) = ps.clone() {
            tokio::spawn(async move {
                while let Some(request) = tr.recv().await {
                    info!("ServiceBroker received request: {:?}", request);
                    let services_tx = platform_state
                        .services_gateway_api
                        .lock()
                        .await
                        .get_sender();
                    use tokio::sync::oneshot;

                    let (oneshot_tx, oneshot_rx) = oneshot::channel::<ServiceRoutingResponse>();

                    let service_request = ServiceRoutingRequest {
                        request_id: ServiceRequestId {
                            request_id: request.rpc.ctx.call_id,
                        },
                        payload: request.rpc.clone(),
                        respond_to: oneshot_tx,
                    };
                    info!(
                        "ServiceBroker sending service request: {:?}",
                        service_request
                    );

                    services_tx.try_send(service_request).unwrap();

                    match oneshot_rx.await {
                        Ok(response) => {
                            info!("ServiceBroker received response: {:?}", response);
                            match response {
                                ServiceRoutingResponse::Error(e) => {
                                    error!("ServiceBroker received error response: {:?}", e);
                                    let err = JsonRpcApiError::default()
                                        .with_id(e.request_id.request_id)
                                        .with_message(e.error)
                                        .to_response();
                                    BrokerOutputForwarder::send_json_rpc_response_to_broker(
                                        err,
                                        callback.clone(),
                                    );
                                    // send_broker_response(&callback, &request, &err.as_bytes())
                                    //     .await;
                                }
                                ServiceRoutingResponse::Success(response) => {
                                    info!(
                                        "ServiceBroker received success response: {:?}",
                                        response.response
                                    );
                                    let json_rpc_response =
                                        JsonRpcApiResponse::from_value(response.response).unwrap();
                                    info!(
                                        "ServiceBroker creating JSON-RPC response: {:?}",
                                        json_rpc_response
                                    );
                                    // Convert the response to
                                    let win: Vec<u8> = json_rpc_response.to_string().into_bytes();
                                    info!("ServiceBroker sending response: {:?}", win);
                                    BrokerOutputForwarder::send_json_rpc_response_to_broker(
                                        json_rpc_response,
                                        callback.clone(),
                                    );
                                }
                            }
                        }
                        Err(e) => {
                            error!("ServiceBroker failed to receive response {}", e);
                        }
                    }
                }
            });
        } else {
            panic!("Platform state is required");
        };

        Self {
            // platform_state: ps.clone(),
            // connect_request,
            // broker_callback,
            // endpoint_broker_state: endpoint_broker.clone(),
            broker_sender,
            //cleaner: BrokerCleaner { cleaner: None },
        }
    }

    fn get_sender(&self) -> super::endpoint_broker::BrokerSender {
        self.broker_sender.clone()
    }

    fn get_cleaner(&self) -> super::endpoint_broker::BrokerCleaner {
        BrokerCleaner::default()
    }
}
