use super::endpoint_broker::{
    BrokerCallback, BrokerCleaner, BrokerConnectRequest, BrokerRequest, BrokerSender,
    EndpointBroker,
};
use super::rules_engine::JsonDataSource;
use crate::broker::endpoint_broker::{BrokerOutput, EndpointBrokerState};
use crate::broker::rules_engine::{clean_name, compose_json_values};

use futures::future::try_join_all;
use ripple_sdk::api::gateway::rpc_gateway_api::{JsonRpcApiError, JsonRpcApiResponse, RpcRequest};
use ripple_sdk::utils::error::RippleError;
use ripple_sdk::{
    log::{debug, error, trace},
    tokio::{self, sync::mpsc},
};
use serde_json::json;
pub struct WorkflowBroker {
    sender: BrokerSender,
}

#[derive(Debug)]
enum SubBrokerErr {
    RpcError(RippleError),
}

async fn subbroker_call(
    endpoint_broker: EndpointBrokerState,
    rpc_request: RpcRequest,
    source: JsonDataSource,
) -> Result<serde_json::Value, SubBrokerErr> {
    let (brokered_tx, mut brokered_rx) = mpsc::channel::<BrokerOutput>(10);
    endpoint_broker.handle_brokerage(
        rpc_request,
        None,
        Some(BrokerCallback {
            sender: brokered_tx,
        }),
    );

    match brokered_rx.recv().await {
        Some(msg) => {
            if msg.is_error() {
                Err(SubBrokerErr::RpcError(RippleError::BrokerError(
                    msg.get_error_string(),
                )))
            } else {
                Ok(json!({clean_name(
                    &source
                        .clone()
                        .namespace
                        .unwrap_or(source.method.to_string()),
                ): msg.data.result.unwrap_or(json!({}))}))
            }
        }
        None => {
            error!("Failed to receive message");
            Err(SubBrokerErr::RpcError(RippleError::BrokerError(
                "Failed to receive message".to_string(),
            )))
        }
    }
}

impl WorkflowBroker {
    pub fn start(callback: BrokerCallback, endpoint_broker: EndpointBrokerState) -> BrokerSender {
        let (tx, mut rx) = mpsc::channel::<BrokerRequest>(10);
        /*
        This is a "meta rule": a rule that composes other rules.
        */
        tokio::spawn(async move {
            loop {
                match rx.recv().await {
                    Some(broker_request) => {
                        trace!("Received message {:?}", broker_request);

                        let mut futures = vec![];
                        for source in broker_request.rule.sources.clone().unwrap_or(vec![]) {
                            trace!("Source {:?}", source.clone());

                            let mut rpc_request = broker_request.clone().rpc;
                            rpc_request.method = source.method.clone();

                            let t = subbroker_call(endpoint_broker.clone(), rpc_request, source);
                            futures.push(t);
                        }
                        /*
                        workflow steps are currently all or nothing/sudden death: if one step fails, the whole workflow fails
                        */
                        match try_join_all(futures).await {
                            Ok(success) => {
                                let mut results = vec![];
                                for r in success {
                                    results.push(r);
                                }
                                let composed: JsonRpcApiResponse = broker_request.clone().into();
                                let composed =
                                    composed.with_result(Some(compose_json_values(results)));
                                debug!("Composed {:?}", composed.result);
                                //.with_result(compose_json_values(results).to_string().as_bytes().to_vec());
                                Self::send_broker_success_response(&callback, composed);
                            }
                            Err(e) => {
                                error!(
                                    "Error {:?} in subbroker call for workflow: {}",
                                    e, broker_request.rpc.method
                                );
                                Self::send_broker_failure_response(
                                    &callback,
                                    JsonRpcApiError::default()
                                        .with_code(-32001)
                                        .with_message(format!(
                                            "workflow error {:?}: for api {}",
                                            e, broker_request.rpc.method
                                        ))
                                        .with_id(broker_request.rpc.ctx.call_id)
                                        .into(),
                                );
                            }
                        }
                    }
                    None => {
                        error!("Failed to receive message");
                        break;
                    }
                }
            }
        });
        BrokerSender { sender: tx }
    }
}

impl EndpointBroker for WorkflowBroker {
    fn get_broker(
        _request: BrokerConnectRequest,
        callback: BrokerCallback,
        broker_state: &mut EndpointBrokerState,
    ) -> Self {
        Self {
            sender: Self::start(callback, broker_state.clone()),
        }
    }

    fn get_sender(&self) -> super::endpoint_broker::BrokerSender {
        self.sender.clone()
    }

    fn get_cleaner(&self) -> super::endpoint_broker::BrokerCleaner {
        BrokerCleaner::default()
    }
}
