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
use crate::extn::extn_client_message::{
    ExtnEvent, ExtnMessage, ExtnPayload, ExtnPayloadProvider, ExtnRequest, ExtnResponse,
};
use crate::extn::extn_id::ExtnId;
use crate::framework::ripple_contract::RippleContract;
#[cfg(any(test, feature = "mock"))]
use crate::service::service_message::ServiceMessage;
#[cfg(any(test, feature = "mock"))]
use crate::utils::error::RippleError;
use chrono::Utc;
#[cfg(any(test, feature = "mock"))]
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use serde_json::Value;
#[cfg(any(test, feature = "mock"))]
use std::collections::HashMap;
#[cfg(any(test, feature = "mock"))]
use std::{
    collections::VecDeque,
    sync::{Arc, OnceLock, RwLock},
};

#[cfg(any(test, feature = "mock"))]
type MockResponseQueue = Arc<RwLock<HashMap<String, VecDeque<Result<ExtnMessage, RippleError>>>>>;

#[cfg(any(test, feature = "mock"))]
type MockServiceResponseQueue =
    Arc<RwLock<HashMap<String, VecDeque<Result<ServiceMessage, RippleError>>>>>;

#[cfg(any(test, feature = "mock"))]
static MOCK_RESPONSE_QUEUE: OnceLock<MockResponseQueue> = OnceLock::new();

#[cfg(any(test, feature = "mock"))]
static MOCK_SERVICE_RESPONSE_QUEUE: OnceLock<MockServiceResponseQueue> = OnceLock::new();

#[cfg(any(test, feature = "mock"))]
fn get_mock_queue() -> &'static MockResponseQueue {
    MOCK_RESPONSE_QUEUE.get_or_init(|| Arc::new(RwLock::new(HashMap::new())))
}

#[cfg(any(test, feature = "mock"))]
fn get_mock_service_queue() -> &'static MockServiceResponseQueue {
    MOCK_SERVICE_RESPONSE_QUEUE.get_or_init(|| Arc::new(RwLock::new(HashMap::new())))
}

/// Adds a mock response to the queue for the specified test context
#[cfg(any(test, feature = "mock"))]
pub fn queue_mock_response(test_context: &str, response: Result<ExtnMessage, RippleError>) {
    let mock_queue = get_mock_queue();
    let mut mock_queues = mock_queue.write().unwrap();
    let queue = mock_queues.entry(test_context.to_string()).or_default();
    // Print the size for debugging
    println!("**** Queued mock response for context '{:?}'", response);
    queue.push_back(response);

    // Print the size for debugging
    println!(
        "**** Queued mock response for context '{}'. Queue size: {}",
        test_context,
        queue.len()
    );
}

/// Adds a mock service response to the queue for the specified test context
#[cfg(any(test, feature = "mock"))]
pub fn queue_mock_service_response(
    test_context: &str,
    response: Result<ServiceMessage, RippleError>,
) {
    let mock_queue = get_mock_service_queue();
    let mut mock_queues = mock_queue.write().unwrap();
    let queue = mock_queues.entry(test_context.to_string()).or_default();
    // Print the size for debugging
    println!("**** Queued mock response for context '{:?}'", response);
    queue.push_back(response);

    // Print the size for debugging
    println!(
        "**** Queued mock response for context '{}'. Queue size: {}",
        test_context,
        queue.len()
    );
}

/// Adds a mock response to the queue for the specified test context
#[cfg(any(test, feature = "mock"))]
pub fn get_next_mock_service_response(
    test_context: String,
) -> Option<Result<ServiceMessage, RippleError>> {
    let mock_queue = get_mock_service_queue();
    let mut mock_queues = mock_queue.write().unwrap();
    if let Some(queue) = mock_queues.get_mut(&test_context) {
        queue.pop_front()
        /*
        let response = queue.pop_front();
                // Print the remaining size for debugging
                println!(
                    "Retrieved mock response for context '{}'. Remaining queue size: {}",
                    test_context,
                    queue.len()
                );
        response
        */
    } else {
        //println!("No mock queue found for context '{}'", test_context);
        None
    }
}

/// Retrieves and removes the next mock response for the specified test context
#[cfg(any(test, feature = "mock"))]
pub fn get_next_mock_response(test_context: &str) -> Option<Result<ExtnMessage, RippleError>> {
    let mock_queue = get_mock_queue();
    let mut mock_queues = mock_queue.write().unwrap();

    if let Some(queue) = mock_queues.get_mut(test_context) {
        queue.pop_front()
        /*
        let response = queue.pop_front();
                // Print the remaining size for debugging
                println!(
                    "Retrieved mock response for context '{}'. Remaining queue size: {}",
                    test_context,
                    queue.len()
                );
        response
        */
    } else {
        //println!("No mock queue found for context '{}'", test_context);
        None
    }
}

/// Checks the number of remaining mocks for a test context
#[cfg(any(test, feature = "mock"))]
pub fn get_mock_queue_size(test_context: &str) -> usize {
    let mock_queue = get_mock_queue();
    let mock_queues = mock_queue.read().unwrap();

    if let Some(queue) = mock_queues.get(test_context) {
        queue.len()
    } else {
        0
    }
}

/// Clears all mock responses for a specific test context
#[cfg(any(test, feature = "mock"))]
pub fn clear_test_mocks(test_context: &str) {
    let mock_queue = get_mock_queue();
    let mut mock_queues = mock_queue.write().unwrap();

    if let Some(queue) = mock_queues.get_mut(test_context) {
        println!(
            "Clearing {} mock responses for context '{}'",
            queue.len(),
            test_context
        );
        queue.clear();
    }
}

/// Clears all mock responses for all test contexts
#[cfg(any(test, feature = "mock"))]
pub fn clear_all_test_mocks() {
    let mock_queue = get_mock_queue();
    let mut mock_queues = mock_queue.write().unwrap();

    // Print all queues being cleared
    for (context, queue) in mock_queues.iter() {
        println!(
            "Clearing {} mock responses for context '{}'",
            queue.len(),
            context
        );
    }

    mock_queues.clear();
}

#[cfg(any(test, feature = "mock"))]
pub type MockResponseStore = Arc<RwLock<HashMap<String, Result<ExtnMessage, RippleError>>>>;

#[cfg(any(test, feature = "mock"))]
lazy_static! {
    pub static ref MOCK_RESPONSES: MockResponseStore = Arc::new(RwLock::new(HashMap::new()));
}

#[cfg(any(test, feature = "mock"))]
pub fn set_mock_response(id: String, response: Result<ExtnMessage, RippleError>) {
    let mut mock_responses = MOCK_RESPONSES.write().unwrap();
    mock_responses.insert(id, response);
}

#[cfg(any(test, feature = "mock"))]
pub fn get_mock_response(id: &str) -> Option<Result<ExtnMessage, RippleError>> {
    let mock_responses = MOCK_RESPONSES.read().unwrap();
    mock_responses.get(id).cloned()
}

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

pub fn get_mock_extn_client() -> ExtnClient {
    ExtnClient::new_main()
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
