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
    framework::ripple_contract::RippleContract,
};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum MockWebsocketServerRequest {
    AddRequestResponse(AddRequestResponseParams),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum MockWebsocketServerResponse {
    AddRequestResponse(AddRequestResponseResponse),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AddRequestResponseParams {
    pub request: Value,
    pub responses: Vec<Value>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AddRequestResponseResponse {
    pub success: bool,
}

impl ExtnPayloadProvider for MockWebsocketServerRequest {
    fn get_from_payload(payload: ExtnPayload) -> Option<Self> {
        if let ExtnPayload::Request(ExtnRequest::MockWebsocketServer(req)) = payload {
            return Some(req);
        }

        None
    }

    fn get_extn_payload(&self) -> ExtnPayload {
        ExtnPayload::Request(ExtnRequest::MockWebsocketServer(self.clone()))
    }

    fn contract() -> RippleContract {
        RippleContract::MockWebsocketServer
    }
}

impl ExtnPayloadProvider for MockWebsocketServerResponse {
    fn get_from_payload(payload: ExtnPayload) -> Option<Self> {
        if let ExtnPayload::Response(ExtnResponse::MockWebsocketServer(resp)) = payload {
            return Some(resp);
        }

        None
    }

    fn get_extn_payload(&self) -> ExtnPayload {
        ExtnPayload::Response(ExtnResponse::MockWebsocketServer(self.clone()))
    }

    fn contract() -> RippleContract {
        RippleContract::MockWebsocketServer
    }
}
