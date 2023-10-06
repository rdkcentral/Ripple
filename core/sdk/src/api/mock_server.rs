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

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    extn::extn_client_message::{ExtnPayload, ExtnPayloadProvider, ExtnRequest, ExtnResponse},
    framework::ripple_contract::{ContractAdjective, RippleContract},
};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum PayloadType {
    #[serde(rename = "json")]
    Json,
    #[serde(rename = "jsonrpc")]
    JsonRpc,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RequestPayload {
    /// The type of payload data
    pub payload_type: PayloadType,
    /// The body of the request
    pub body: Value,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ResponsePayload {
    /// The type of payload data
    pub payload_type: PayloadType,
    /// The body of the response
    pub body: Value,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EventPayload {
    /// The body of the event
    pub body: Value,
    /// The number of ms before the event should be emitted
    pub delay: u32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum MockServerRequest {
    AddRequestResponse(AddRequestResponseParams),
    EmitEvent(EmitEventParams),
    RemoveRequest(RemoveRequestParams),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum MockServerResponse {
    AddRequestResponse(AddRequestResponseResponse),
    EmitEvent(EmitEventResponse),
    RemoveRequestResponse(RemoveRequestResponse),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AddRequestResponseParams {
    pub request: RequestPayload,
    pub responses: Vec<ResponsePayload>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AddRequestResponseResponse {
    pub success: bool,
    pub error: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RemoveRequestParams {
    pub request: RequestPayload,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RemoveRequestResponse {
    pub success: bool,
    pub error: Option<String>,
}

// TODO: add a clear all mock data request

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EmitEventParams {
    pub event: EventPayload,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EmitEventResponse {
    pub success: bool,
}

impl ExtnPayloadProvider for MockServerRequest {
    fn get_from_payload(payload: ExtnPayload) -> Option<Self> {
        if let ExtnPayload::Request(ExtnRequest::MockServer(req)) = payload {
            return Some(req);
        }

        None
    }

    fn get_extn_payload(&self) -> ExtnPayload {
        ExtnPayload::Request(ExtnRequest::MockServer(self.clone()))
    }

    fn contract() -> RippleContract {
        RippleContract::MockServer(MockServerAdjective::WebSocket)
    }
}

impl ExtnPayloadProvider for MockServerResponse {
    fn get_from_payload(payload: ExtnPayload) -> Option<Self> {
        if let ExtnPayload::Response(ExtnResponse::MockServer(resp)) = payload {
            return Some(resp);
        }

        None
    }

    fn get_extn_payload(&self) -> ExtnPayload {
        ExtnPayload::Response(ExtnResponse::MockServer(self.clone()))
    }

    fn contract() -> RippleContract {
        RippleContract::MockServer(MockServerAdjective::WebSocket)
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub enum MockServerAdjective {
    WebSocket,
}

impl ContractAdjective for MockServerAdjective {
    fn get_contract(&self) -> RippleContract {
        RippleContract::MockServer(self.clone())
    }
}
