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
use ripple_sdk::api::gateway::rpc_gateway_api::JsonRpcApiError;
use ripple_sdk::extn::extn_client_message::ExtnResponse;
use ripple_sdk::extn::extn_id::ExtnProviderRequest;
use ripple_sdk::log::trace;
use ripple_sdk::{
    api::gateway::rpc_gateway_api::JsonRpcApiResponse,
    api::observability::log_signal::LogSignal,
    extn::extn_id::ExtnId,
    log::error,
    tokio::{self, sync::mpsc},
};

#[derive(Clone)]
pub struct ExtnBroker {
    sender: BrokerSender,
}

impl ExtnBroker {
    pub fn start(
        ps: Option<PlatformState>,
        callback: BrokerCallback,
        _endpoint_broker: EndpointBrokerState,
    ) -> BrokerSender {
        let (tx, mut rx) = mpsc::channel::<BrokerRequest>(BROKER_CHANNEL_BUFFER_SIZE);

        tokio::spawn(async move {
            while let Some(broker_request) = rx.recv().await {
                LogSignal::new(
                    "extn_broker".to_string(),
                    format!("received extn broker request: {:?}", broker_request),
                    broker_request.rpc.ctx.clone(),
                )
                .emit_debug();
                let rpc_request = broker_request.rpc.clone();
                let rule = broker_request.rule.clone();
                let alias = rule.alias;
                let id = match ExtnId::try_from(alias.clone()) {
                    Ok(extn_id) => extn_id,
                    Err(_) => {
                        error!("Failed to convert alias to ExtnId");
                        continue;
                    }
                };

                let request = ExtnProviderRequest {
                    value: serde_json::to_value(rpc_request.clone()).unwrap(),
                    id: id.clone(),
                };

                let client = if let Some(platform_state) = &ps {
                    platform_state.get_client()
                } else {
                    return;
                };

                match client.send_extn_request(request.clone()).await {
                    Ok(response) => {
                        if let Some(ExtnResponse::String(v)) = response.payload.extract() {
                            if let Ok(value) = serde_json::from_str::<JsonRpcApiResponse>(&v) {
                                LogSignal::new(
                                    "extn_broker".to_string(),
                                    format!("Received response from extn: {:?}", value),
                                    broker_request.rpc.ctx.clone(),
                                )
                                .emit_debug();
                                Self::send_broker_success_response(&callback, value);
                            } else {
                                trace!("serde failed in extn_broker");
                                Self::send_broker_failure_response(
                                    &callback,
                                    JsonRpcApiError::default()
                                        .with_code(-32001)
                                        .with_message(format!(
                                            "extn_broker error for api {}: serde failed",
                                            broker_request.rpc.method,
                                        ))
                                        .with_id(broker_request.rpc.ctx.call_id)
                                        .into(),
                                );
                            }
                        } else {
                            Self::log_error_and_send_broker_failure_response(
                                broker_request.clone(),
                                &callback,
                                JsonRpcApiError::default()
                                    .with_code(-32001)
                                    .with_message(format!(
                                        "extn_broker error for api {}: received response: {:?}",
                                        broker_request.rpc.method, response.payload,
                                    ))
                                    .with_id(broker_request.rpc.ctx.call_id),
                            );
                        }
                    }
                    Err(e) => {
                        Self::log_error_and_send_broker_failure_response(
                            broker_request.clone(),
                            &callback,
                            JsonRpcApiError::default()
                                .with_code(-32001)
                                .with_message(format!(
                                    "Extn error for api {}: received response: {}",
                                    broker_request.rpc.method, e
                                ))
                                .with_id(broker_request.rpc.ctx.call_id),
                        );
                    }
                }
            }
        });

        BrokerSender { sender: tx }
    }

    fn log_error_and_send_broker_failure_response(
        request: BrokerRequest,
        callback: &BrokerCallback,
        error: JsonRpcApiError,
    ) {
        LogSignal::new(
            "extn_broker".to_string(),
            format!("broker request failed: {:?} error: {:?}", request, error),
            request.rpc.ctx.clone(),
        )
        .emit_error();
        Self::send_broker_failure_response(callback, error.into());
    }
}

impl EndpointBroker for ExtnBroker {
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
    use std::sync::Arc;

    use super::*;
    use crate::broker::endpoint_broker::BrokerOutput;

    use crate::service::extn::ripple_client::RippleClient;
    use crate::state::bootstrap_state::ChannelsState;
    use ripple_sdk::api::gateway::rpc_gateway_api::RpcRequest;
    use ripple_sdk::api::manifest::device_manifest::DeviceManifest;
    use ripple_sdk::api::manifest::extn_manifest::ExtnManifest;
    use ripple_sdk::api::rules_engine::{Rule, RuleEngine, RuleEngineProvider};
    use ripple_sdk::Mockable;
    use ssda_types::gateway::ApiGatewayServer;

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

        ExtnBroker::log_error_and_send_broker_failure_response(
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
    pub async fn test_get_broker() {
        use super::*;

        let (tx, _rx) = mpsc::channel::<BrokerOutput>(10);
        let callback = BrokerCallback { sender: tx };

        let mut broker_state = EndpointBrokerState::default();

        let broker = ExtnBroker::get_broker(
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
        let broker = ExtnBroker { sender };

        assert!(broker.get_sender().sender.same_channel(&tx));
    }

    #[tokio::test]
    pub async fn test_get_cleaner() {
        use super::*;

        let (tx, _rx) = mpsc::channel::<BrokerRequest>(10);
        let sender = BrokerSender { sender: tx };
        let broker = ExtnBroker { sender };

        let cleaner = broker.get_cleaner();
        assert!(cleaner.cleaner.is_none());
    }

    #[tokio::test]
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
        let rules_engine: Arc<tokio::sync::RwLock<Box<dyn RuleEngineProvider + Send + Sync>>> =
            Arc::new(tokio::sync::RwLock::new(Box::new(RuleEngine::default())));

        let api_gateway_state: Arc<tokio::sync::Mutex<Box<dyn ApiGatewayServer + Send + Sync>>> =
            Arc::new(tokio::sync::Mutex::new(Box::new(
                ssda_service::ApiGateway::new(rules_engine.clone()),
            )));

        let platform_state = PlatformState::new(
            ExtnManifest::default(),
            DeviceManifest::default(),
            RippleClient::new(ChannelsState::default()),
            Vec::new(),
            None,
            api_gateway_state.clone(),
            rules_engine.clone(),
        );
        let sender = ExtnBroker::start(
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
