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

use crate::extn::client::extn_client::ExtnClient;
use crate::extn::client::extn_sender::ExtnSender;
use crate::extn::extn_client_message::{
    ExtnEvent, ExtnMessage, ExtnPayload, ExtnPayloadProvider, ExtnRequest, ExtnResponse,
};
use crate::extn::extn_id::ExtnId;
use crate::framework::ripple_contract::RippleContract;
use async_channel::unbounded;
use chrono::Utc;
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize)]
pub struct MockEvent {
    pub event_name: String,
    pub result: Value,
    pub context: Option<Value>,
    pub app_id: Option<String>,
    pub expected_response: Option<ExtnResponse>,
}

impl ExtnPayloadProvider for MockEvent {
    fn get_extn_payload(&self) -> ExtnPayload {
        ExtnPayload::Event(ExtnEvent::Value(
            serde_json::to_value(self.clone()).unwrap(),
        ))
    }

    fn get_from_payload(payload: ExtnPayload) -> Option<Self> {
        if let ExtnPayload::Event(ExtnEvent::Value(v)) = payload {
            if let Ok(mock_event_value) = serde_json::from_value::<serde_json::Value>(v) {
                if let Some(event_name) =
                    mock_event_value.get("event_name").and_then(|v| v.as_str())
                {
                    if let Some(result) = mock_event_value.get("result").cloned() {
                        let context = mock_event_value.get("context").cloned();
                        let app_id = mock_event_value
                            .get("app_id")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string());
                        let expected_response =
                            mock_event_value.get("expected_response").map(|v| {
                                if let Some(response_str) = v.as_str() {
                                    ExtnResponse::String(response_str.to_string())
                                } else if let Some(response_bool) = v.as_bool() {
                                    ExtnResponse::Boolean(response_bool)
                                } else if let Some(response_number) = v.as_u64() {
                                    ExtnResponse::Number(response_number as u32)
                                } else {
                                    ExtnResponse::None(())
                                }
                            });

                        return Some(MockEvent {
                            event_name: event_name.to_string(),
                            result,
                            context,
                            app_id,
                            expected_response,
                        });
                    }
                }
            }
        }

        None
    }

    fn contract() -> RippleContract {
        RippleContract::Internal
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct MockRequest {
    pub app_id: String,
    pub contract: RippleContract,
    pub expected_response: Option<ExtnResponse>,
}

// TODO - check if we can use macro to generate ExtnPayloadProvider
impl ExtnPayloadProvider for MockRequest {
    fn get_from_payload(payload: ExtnPayload) -> Option<Self> {
        if let ExtnPayload::Request(ExtnRequest::Extn(mock_request)) = payload {
            if let Ok(mock_request_value) =
                serde_json::from_value::<serde_json::Value>(mock_request)
            {
                if let Some(app_id) = mock_request_value.get("app_id").and_then(|v| v.as_str()) {
                    if let Some(contract) = mock_request_value
                        .get("contract")
                        .and_then(|v| serde_json::from_value(v.clone()).ok())
                    {
                        let expected_response =
                            mock_request_value.get("expected_response").map(|v| {
                                if let Some(response_str) = v.as_str() {
                                    ExtnResponse::String(response_str.to_string())
                                } else if let Some(response_bool) = v.as_bool() {
                                    ExtnResponse::Boolean(response_bool)
                                } else if let Some(response_number) = v.as_u64() {
                                    ExtnResponse::Number(response_number as u32)
                                } else {
                                    ExtnResponse::None(())
                                }
                            });

                        return Some(MockRequest {
                            app_id: app_id.to_string(),
                            contract,
                            expected_response,
                        });
                    }
                }
            }
        }
        None
    }

    fn get_extn_payload(&self) -> ExtnPayload {
        ExtnPayload::Request(ExtnRequest::Extn(
            serde_json::to_value(self.clone()).unwrap(),
        ))
    }

    fn get_contract(&self) -> RippleContract {
        self.contract.clone()
    }

    fn contract() -> RippleContract {
        RippleContract::Internal
    }
}

pub fn get_mock_extn_client(id: ExtnId) -> ExtnClient {
    let (s, receiver) = unbounded();
    let mock_sender = ExtnSender::new(
        s,
        id,
        vec!["context".to_string()],
        vec!["fulfills".to_string()],
        Some(HashMap::new()),
    );

    ExtnClient::new(receiver, mock_sender)
}

pub fn get_mock_message(payload_type: PayloadType) -> ExtnMessage {
    ExtnMessage {
        id: "test_id".to_string(),
        requestor: ExtnId::get_main_target("main".into()),
        target: RippleContract::Internal,
        target_id: Some(ExtnId::get_main_target("main".into())),
        payload: match payload_type {
            PayloadType::Event => get_mock_event_payload(),
            PayloadType::Request => get_mock_request_payload(),
        },
        callback: None,
        ts: Some(Utc::now().timestamp_millis()),
    }
}

pub fn get_mock_event_payload() -> ExtnPayload {
    ExtnPayload::Event(ExtnEvent::Value(serde_json::to_value(true).unwrap()))
}

pub fn get_mock_request_payload() -> ExtnPayload {
    ExtnPayload::Request(ExtnRequest::Extn(
        serde_json::to_value(MockRequest {
            app_id: "test_app_id".to_string(),
            contract: RippleContract::Internal,
            expected_response: Some(ExtnResponse::Boolean(true)),
        })
        .unwrap(),
    ))
}

pub enum PayloadType {
    Event,
    Request,
}

pub fn get_mock_event() -> MockEvent {
    MockEvent {
        event_name: "test_event".to_string(),
        result: serde_json::json!({"result": "success"}),
        context: None,
        app_id: Some("test_app_id".to_string()),
        expected_response: Some(ExtnResponse::Boolean(true)),
    }
}

pub fn get_mock_request() -> MockRequest {
    MockRequest {
        app_id: "test_app_id".to_string(),
        contract: RippleContract::Internal,
        expected_response: Some(ExtnResponse::Boolean(true)),
    }
}
