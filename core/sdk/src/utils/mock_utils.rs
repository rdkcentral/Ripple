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
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
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
            if let Ok(v) = serde_json::from_value(v) {
                return Some(v);
            }
        }

        None
    }

    fn contract() -> RippleContract {
        RippleContract::Internal
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MockRequest {
    pub app_id: String,
    pub contract: RippleContract,
    pub expected_response: Option<ExtnResponse>,
}

// TODO - check if we can use macro to generate ExtnPayloadProvider
impl ExtnPayloadProvider for MockRequest {
    fn get_from_payload(payload: ExtnPayload) -> Option<Self> {
        if let ExtnPayload::Request(ExtnRequest::Extn(mock_request)) = payload {
            return Some(serde_json::from_value(mock_request).unwrap());
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
