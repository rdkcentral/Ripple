use super::endpoint_broker::{
    BrokerCallback, BrokerCleaner, BrokerConnectRequest, BrokerRequest, BrokerSender,
    EndpointBroker,
};
use super::rules_engine::JsonDataSource;
use crate::broker::endpoint_broker::{BrokerOutput, EndpointBrokerState};
use crate::broker::rules_engine::{compose_json_values, make_name_json_safe};
use futures::future::BoxFuture;
use futures::FutureExt;

use futures::future::try_join_all;
use ripple_sdk::api::gateway::rpc_gateway_api::{JsonRpcApiError, JsonRpcApiResponse, RpcRequest};
use ripple_sdk::utils::error::RippleError;
use ripple_sdk::{
    log::{error, trace},
    tokio::{self, sync::mpsc},
};
use serde_json::json;
pub struct WorkflowBroker {
    sender: BrokerSender,
}

#[derive(Debug)]
pub enum SubBrokerErr {
    RpcError(RippleError),

    JsonRpcApiError(JsonRpcApiError),
}
pub type SubBrokerResult = Result<JsonRpcApiResponse, SubBrokerErr>;

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
        Vec::new(),
        None,
    );

    match brokered_rx.recv().await {
        Some(msg) => {
            if msg.is_error() {
                Err(SubBrokerErr::RpcError(RippleError::BrokerError(
                    msg.get_error_string(),
                )))
            } else {
                Ok(json!({make_name_json_safe(
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
    pub fn create_the_futures(
        sources: Vec<JsonDataSource>,
        rpc_request: RpcRequest,
        endpoint_broker: EndpointBrokerState,
    ) -> Vec<BoxFuture<'static, Result<serde_json::Value, SubBrokerErr>>> {
        let mut futures = vec![];

        for source in sources.clone() {
            trace!("Source {:?}", source.clone());

            let mut rpc_request = rpc_request.clone();
            rpc_request.method = source.method.clone();

            // Deserialize the existing params_json
            let mut existing_params =
                match serde_json::from_str::<serde_json::Value>(&rpc_request.params_json) {
                    Ok(params) => params,
                    Err(e) => {
                        error!("Failed to parse existing params_json: {:?}", e);
                        continue;
                    }
                };

            // Handle new params from the rule source
            if let Some(ref params) = source.params {
                // Ensure params is valid JSON
                match serde_json::from_str::<serde_json::Value>(params) {
                    Ok(new_params) => {
                        // Merge the new params with existing params
                        if let Some(existing_array) = existing_params.as_array_mut() {
                            existing_array.push(new_params);
                        } else {
                            error!(
                                "Existing params_json is not an array: {:?}",
                                existing_params
                            );
                        }
                    }
                    Err(e) => {
                        error!("Invalid params JSON string: {:?}", e);
                        continue;
                    }
                }
            }

            // Serialize the merged parameters back into params_json
            rpc_request.params_json = serde_json::to_string(&existing_params).unwrap();
            let t = subbroker_call(endpoint_broker.clone(), rpc_request, source).boxed(); // source is still usable here
            futures.push(t);
        }
        futures
    }

    pub async fn run_workflow(
        broker_request: &BrokerRequest,
        endpoint_broker: EndpointBrokerState,
    ) -> SubBrokerResult {
        let mut futures = Self::create_the_futures(
            broker_request.rule.sources.clone().unwrap_or_default(),
            broker_request.rpc.clone(),
            endpoint_broker.clone(),
        );
        /*
        workflow steps are currently all or nothing/sudden death: if one step fails, the whole workflow fails
        */

        // Define your batch size here
        let batch_size = 10;
        let mut results = vec![];

        for chunk in futures.chunks_mut(batch_size) {
            match try_join_all(chunk.iter_mut().map(|f| f.as_mut()).collect::<Vec<_>>()).await {
                Ok(success) => {
                    results.extend(success);
                }
                Err(e) => {
                    error!(
                        "Error {:?} in subbroker call for workflow: {}",
                        e, broker_request.rpc.method
                    );
                    return Err(SubBrokerErr::JsonRpcApiError(
                        JsonRpcApiError::default()
                            .with_code(-32001)
                            .with_message(format!(
                                "workflow error {:?}: for api {}",
                                e, broker_request.rpc.method
                            ))
                            .with_id(broker_request.rpc.ctx.call_id),
                    ));
                }
            }
        }

        // Return an Ok result if the loop has zero elements to iterate on
        let composed: JsonRpcApiResponse = broker_request.clone().into();
        let composed = composed.with_result(Some(compose_json_values(results)));
        trace!("Composed {:?}", composed.result);
        Ok(composed)
    }
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

                        match Self::run_workflow(&broker_request, endpoint_broker.clone()).await {
                            Ok(yay) => {
                                Self::send_broker_success_response(&callback, yay);
                            }
                            Err(boo) => match boo {
                                SubBrokerErr::JsonRpcApiError(e) => {
                                    Self::send_broker_failure_response(&callback, e.into());
                                }
                                SubBrokerErr::RpcError(ripple_error) => {
                                    let boo = JsonRpcApiError::default()
                                        .with_code(-32001)
                                        .with_message(format!(
                                            "workflow error {:?}: for api {}",
                                            ripple_error, broker_request.rpc.method
                                        ))
                                        .with_id(broker_request.rpc.ctx.call_id)
                                        .into();
                                    Self::send_broker_failure_response(&callback, boo);
                                }
                            },
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
/*
write exhaustive tests for the WorkflowBroker
   */

#[cfg(test)]
pub mod tests {

    use ripple_sdk::{api::gateway::rpc_gateway_api::RpcRequest, tokio, Mockable};
    use serde_json::json;

    use crate::broker::{
        endpoint_broker::{BrokerCallback, BrokerRequest, EndpointBrokerState},
        rules_engine::{JsonDataSource, Rule, RuleEngine},
    };
    pub fn broker_request(callback: BrokerCallback) -> BrokerRequest {
        let mut rule = Rule {
            alias: "module.method".to_string(),
            ..Default::default()
        };
        let source = JsonDataSource {
            method: "module.method".to_string(),
            namespace: Some("module".to_string()),
            ..Default::default()
        };

        rule.sources = Some(vec![source]);
        BrokerRequest {
            rpc: RpcRequest::mock(),
            rule,
            subscription_processed: None,
            workflow_callback: Some(callback),
        }
    }
    pub fn rule_engine() -> RuleEngine {
        let engine = RuleEngine::load_from_string_literal(
            json!({
                    "endpoints": {
                        "workflow" : {
                            "protocol": "workflow",
                            "url": "http://asdf.com",
                            "jsonrpc": false
                          }
                    },
                    "rules": {
                        "static.rule": {
                            "alias": "static",
                            "transform": {
                              "response": "\"Sky\""
                            }
                        },
                        "module.method": {
                            "alias": "workflow",
                            "endpoint": "workflow",
                            "sources": [{
                                "method": "static.rule"
                            }]
                    }
                }
            }
                )
            .to_string(),
        );
        engine.unwrap()
    }
    pub fn endppoint_broker_state() -> EndpointBrokerState {
        EndpointBrokerState::default().with_rules_engine(rule_engine())
    }

    #[tokio::test]
    pub async fn test_run_workflow() {
        /*
        THIS IS A WORK IN PROGRESS... endpoint_broker is highly coupled, making it difficult to test without a full integration test
        */
        use super::*;

        let (tx, mut _rx) = mpsc::channel::<BrokerOutput>(10);
        let callback = BrokerCallback { sender: tx };
        let request = broker_request(callback);
        let broker = endppoint_broker_state();

        let foo = WorkflowBroker::run_workflow(&request, broker);
        let foo = foo.await;
        assert!(foo.is_err());
    }
}
