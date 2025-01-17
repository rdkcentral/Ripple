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
    EndpointBroker, EndpointBrokerState,
};
use crate::state::platform_state::PlatformState;
use ripple_sdk::api::gateway::rpc_gateway_api::JsonRpcApiError;
use ripple_sdk::async_channel::unbounded;
use ripple_sdk::extn::extn_client_message::{ExtnMessage, ExtnPayload, ExtnRequest};
use ripple_sdk::framework::ripple_contract::RippleContract;
use ripple_sdk::log::trace;
use ripple_sdk::{
    api::gateway::rpc_gateway_api::JsonRpcApiResponse,
    extn::extn_id::{ExtnClassId, ExtnId},
    log::error,
    tokio::{self, sync::mpsc},
    chrono::Utc,
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
        let (tx, mut rx) = mpsc::channel::<BrokerRequest>(10);

        tokio::spawn(async move {
            while let Some(broker_request) = rx.recv().await {
                trace!("Received message {:?}", broker_request);
                let rpc_request = broker_request.rpc.clone();
                let rule = broker_request.rule.clone();
                let alias = rule.alias;

                let extn_client = if let Some(platform_state) = &ps {
                    platform_state.get_client().get_extn_client()
                } else {
                    return;
                };

                if let Some(sender) = extn_client.get_extn_sender_with_extn_id(&alias) {
                    let (callback_tx, callback_rx) = unbounded();
                    let msg = ExtnMessage {
                        id: rpc_request.ctx.call_id.to_string(),
                        requestor: ExtnId::get_main_target("main".into()),
                        target: RippleContract::JsonRpsee,
                        target_id: Some(ExtnId::new_extn(ExtnClassId::Gateway, alias)),
                        payload: ExtnPayload::Request(ExtnRequest::Rpc(rpc_request.clone())),
                        callback: Some(callback_tx),
                        ts: Some(Utc::now().timestamp_millis()),
                    };

                    if sender.try_send(msg.into()).is_ok() {
                        match callback_rx.recv().await {
                            Ok(message) => {
                                let value =
                                    serde_json::from_str::<serde_json::Value>(&message.payload)
                                        .ok()
                                        .and_then(|payload| payload.get("Response").cloned())
                                        .and_then(|response| response.get("Value").cloned())
                                        .and_then(|value| {
                                            serde_json::from_str::<serde_json::Value>(
                                                value.as_str().unwrap_or(""),
                                            )
                                            .ok()
                                        })
                                        .and_then(|value_json| value_json.get("result").cloned())
                                        .unwrap_or(serde_json::Value::Null);

                                let composed: JsonRpcApiResponse = broker_request.clone().into();
                                let composed = composed.with_result(Some(value));

                                Self::send_broker_success_response(&callback, composed);
                            }
                            Err(error) => {
                                let error_message = JsonRpcApiError::default()
                                    .with_code(-32001)
                                    .with_message(format!(
                                        "extn_broker error for api {}: {}",
                                        broker_request.rpc.method, error
                                    ))
                                    .with_id(broker_request.rpc.ctx.call_id)
                                    .into();

                                Self::send_broker_failure_response(&callback, error_message);
                            }
                        }
                    } else {
                        error!("No sender available");
                    }
                } else {
                    error!("No sender available");
                }
            }
        });
        BrokerSender { sender: tx }
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
