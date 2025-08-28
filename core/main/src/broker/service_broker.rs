// Copyright 2023 Comcast Cable Communications Management, LLC
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
//
// SPDX-License-Identifier: Apache-2.0
//
use super::endpoint_broker::{
    BrokerCallback, BrokerCleaner, BrokerConnectRequest, BrokerRequest, BrokerSender,
    EndpointBroker, EndpointBrokerState, BROKER_CHANNEL_BUFFER_SIZE,
};
use crate::state::platform_state::PlatformState;
use ripple_sdk::{
    api::{gateway::rpc_gateway_api::JsonRpcApiError, observability::log_signal::LogSignal},
    log::{error, info},
    service::service_message::{Id, ServiceMessage},
    tokio::{self, sync::mpsc},
    tokio_tungstenite::tungstenite::Message,
    utils::error::RippleError,
};

#[derive(Clone)]
pub struct ServiceBroker {
    sender: BrokerSender,
}

impl ServiceBroker {
    pub fn start(
        ps: Option<PlatformState>,
        callback: BrokerCallback,
        _endpoint_broker: EndpointBrokerState,
    ) -> BrokerSender {
        let (broker_request_tx, mut broker_request_rx) = mpsc::channel(BROKER_CHANNEL_BUFFER_SIZE);

        // ps should be valid, otherwise we cannot proceed
        let ps_c = if let Some(ps) = ps {
            ps.clone()
        } else {
            error!("PlatformState is None, cannot start ServiceBroker");
            return BrokerSender {
                sender: broker_request_tx,
            };
        };

        tokio::spawn(async move {
            while let Some(broker_request) = broker_request_rx.recv().await {
                LogSignal::new(
                    "service_broker".to_string(),
                    format!("received service broker request: {:?}", broker_request),
                    broker_request.rpc.ctx.clone(),
                )
                .emit_debug();

                let service_id = broker_request.rule.alias.clone();

                // get the ws sender for the service from service_controller_state
                let service_sender =
                    match ps_c.service_controller_state.get_sender(&service_id).await {
                        Some(sender) => sender,
                        None => {
                            error!("Service sender not found for service id: {}", service_id);
                            Self::log_error_and_send_broker_failure_response(
                                broker_request.clone(),
                                &callback,
                                JsonRpcApiError::default()
                                    .with_code(-32001)
                                    .with_message(format!(
                                        "Service sender not found for service id: {}",
                                        service_id
                                    ))
                                    .with_id(broker_request.rpc.ctx.call_id),
                            );
                            continue;
                        }
                    };

                let request = match Self::update_service_request(&broker_request) {
                    Ok(req) => req,
                    Err(e) => {
                        error!("Failed to update request: {:?}", e);
                        Self::log_error_and_send_broker_failure_response(
                            broker_request.clone(),
                            &callback,
                            JsonRpcApiError::default()
                                .with_code(-32001)
                                .with_message(format!("Failed to update request: {}", e))
                                .with_id(broker_request.rpc.ctx.call_id),
                        );
                        continue;
                    }
                };

                LogSignal::new(
                    "service_broker".to_string(),
                    format!("Sending request to service: {:?}", request),
                    broker_request.rpc.ctx.clone(),
                )
                .emit_debug();

                let request_id = broker_request.rpc.ctx.call_id;
                // set the Broker callback in service controller for sending broker response
                if let Some(workflow_callback) = broker_request.workflow_callback.clone() {
                    let _ = ps_c
                        .service_controller_state
                        .set_broker_callback(&service_id, request_id, workflow_callback)
                        .await;
                } else {
                    let _ = ps_c
                        .service_controller_state
                        .set_broker_callback(&service_id, request_id, callback.clone())
                        .await;
                }

                let message = Message::Text(request.clone());
                info!("Sending request to service {}: {:#?}", service_id, message);

                if let Err(err) = service_sender.try_send(message) {
                    error!(
                        "Failed to send request to service {}: {:?}",
                        service_id, err
                    );
                    Self::log_error_and_send_broker_failure_response(
                        broker_request.clone(),
                        &callback,
                        JsonRpcApiError::default()
                            .with_code(-32001)
                            .with_message(format!(
                                "Failed to send request to service {}: {:?}",
                                service_id, err
                            ))
                            .with_id(broker_request.rpc.ctx.call_id),
                    );
                } else {
                    LogSignal::new(
                        "service_broker".to_string(),
                        format!("Request sent to service: {}", service_id),
                        broker_request.rpc.ctx.clone(),
                    )
                    .emit_debug();
                }
            }
        });

        BrokerSender {
            sender: broker_request_tx,
        }
    }

    fn log_error_and_send_broker_failure_response(
        request: BrokerRequest,
        callback: &BrokerCallback,
        error: JsonRpcApiError,
    ) {
        LogSignal::new(
            "service_broker".to_string(),
            format!("broker request failed: {:?} error: {:?}", request, error),
            request.rpc.ctx.clone(),
        )
        .emit_error();
        Self::send_broker_failure_response(callback, error.into());
    }

    fn update_service_request(broker_request: &BrokerRequest) -> Result<String, RippleError> {
        let v = Self::apply_request_rule(broker_request)?;
        info!("transformed request {:?}", v);

        // Create a ServiceMessage
        let mut request = ServiceMessage::new_request(
            broker_request.rpc.method.clone(),
            Some(v.clone()),
            Id::Number(broker_request.rpc.ctx.call_id.try_into().unwrap()),
        );
        request.set_context(Some(serde_json::Value::from(
            broker_request.rpc.ctx.clone(),
        )));
        Ok(request.into())
    }
}

impl EndpointBroker for ServiceBroker {
    fn get_broker(
        ps: Option<PlatformState>,
        _request: BrokerConnectRequest,
        callback: BrokerCallback,
        broker_state: &mut EndpointBrokerState,
    ) -> Self {
        Self {
            sender: Self::start(ps, callback, broker_state.clone()),
        }
    }

    fn get_sender(&self) -> BrokerSender {
        self.sender.clone()
    }

    fn get_cleaner(&self) -> super::endpoint_broker::BrokerCleaner {
        BrokerCleaner::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::broker::endpoint_broker::BrokerOutput;
    use ripple_sdk::api::gateway::rpc_gateway_api::RpcRequest;
    use ripple_sdk::api::rules_engine::Rule;
    use ripple_sdk::Mockable;

    #[tokio::test]
    pub async fn test_log_error_and_send_broker_failure_response() {
        use super::*;
        use tokio::time::{timeout, Duration};

        let (tx, mut rx) = mpsc::channel::<BrokerOutput>(10);
        let callback = BrokerCallback { sender: tx };

        let mut rpc_request = RpcRequest::internal("test_method", None);
        rpc_request.ctx.call_id = 147;

        let broker_request = BrokerRequest {
            rpc: rpc_request,
            rule: Rule {
                alias: "test_rule".to_string(),
                ..Default::default()
            },
            subscription_processed: None,
            workflow_callback: Some(callback.clone()),
            telemetry_response_listeners: vec![],
        };

        let error = JsonRpcApiError::default()
            .with_code(-32001)
            .with_message("Test error message".to_string())
            .with_id(147);

        ServiceBroker::log_error_and_send_broker_failure_response(
            broker_request.clone(),
            &callback,
            error.clone(),
        );

        if let Ok(Some(BrokerOutput { data, .. })) =
            timeout(Duration::from_secs(5), rx.recv()).await
        {
            assert!(data.is_error());
        } else {
            panic!("Timeout or channel closed without receiving data");
        }
    }

    #[tokio::test]
    #[ignore]
    pub async fn test_get_broker() {
        use super::*;

        let (tx, _rx) = mpsc::channel::<BrokerOutput>(10);
        let callback = BrokerCallback { sender: tx };

        let mut broker_state = EndpointBrokerState::default();

        let broker = ServiceBroker::get_broker(
            None,
            BrokerConnectRequest::default(),
            callback.clone(),
            &mut broker_state,
        );

        assert!(!broker.get_sender().sender.is_closed());
    }

    #[tokio::test]
    pub async fn test_get_sender() {
        use super::*;

        let (tx, _rx) = mpsc::channel::<BrokerRequest>(10);
        let sender = BrokerSender { sender: tx.clone() };
        let broker = ServiceBroker { sender };

        assert!(broker.get_sender().sender.same_channel(&tx));
    }

    #[tokio::test]
    pub async fn test_get_cleaner() {
        use super::*;

        let (tx, _rx) = mpsc::channel::<BrokerRequest>(10);
        let sender = BrokerSender { sender: tx };
        let broker = ServiceBroker { sender };

        let cleaner = broker.get_cleaner();
        assert!(cleaner.cleaner.is_none());
    }

    #[tokio::test]
    #[ignore]
    pub async fn test_start_successful_response() {
        use super::*;
        use tokio::time::{timeout, Duration};

        let (tx, mut rx) = mpsc::channel::<BrokerOutput>(10);
        let callback = BrokerCallback { sender: tx };

        let mut rpc_request = RpcRequest::mock();
        rpc_request.ctx.call_id = 11;

        let broker_request = BrokerRequest {
            rpc: rpc_request.clone(),
            rule: Rule {
                alias: "test_rule".to_string(),
                ..Default::default()
            },
            subscription_processed: None,
            workflow_callback: Some(callback.clone()),
            telemetry_response_listeners: vec![],
        };
        let platform_state = PlatformState::default();

        let sender = ServiceBroker::start(
            Some(platform_state),
            callback.clone(),
            EndpointBrokerState::default(),
        );
        sender.sender.send(broker_request.clone()).await.unwrap();

        if let Ok(Some(BrokerOutput { data, .. })) =
            timeout(Duration::from_secs(5), rx.recv()).await
        {
            assert!(data.is_success());
        } else {
            eprintln!("Timeout or channel closed without receiving data, skipping test");
        }
    }
}
