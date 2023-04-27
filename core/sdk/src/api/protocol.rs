// If not stated otherwise in this file or this component's license file the
// following copyright and licenses apply:
//
// Copyright 2023 RDK Management
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

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    extn::extn_client_message::{ExtnPayload, ExtnPayloadProvider, ExtnRequest},
    framework::ripple_contract::RippleContract,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BridgeProtocolRequest {
    StartSession(BridgeSessionParams),
    EndSession(String),
    Send(String, Value),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeSessionParams {
    pub container_id: String,
    pub session_id: String,
}

impl ExtnPayloadProvider for BridgeProtocolRequest {
    fn get_extn_payload(&self) -> ExtnPayload {
        ExtnPayload::Request(ExtnRequest::BridgeProtocolRequest(self.clone()))
    }

    fn get_from_payload(payload: ExtnPayload) -> Option<BridgeProtocolRequest> {
        match payload {
            ExtnPayload::Request(request) => match request {
                ExtnRequest::BridgeProtocolRequest(r) => return Some(r),
                _ => {}
            },
            _ => {}
        }
        None
    }

    fn contract() -> RippleContract {
        RippleContract::BridgeProtocol
    }
}
