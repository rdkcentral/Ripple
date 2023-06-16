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

use crate::{
    extn::extn_client_message::{ExtnPayload, ExtnPayloadProvider, ExtnResponse},
    framework::ripple_contract::RippleContract,
    utils::error::RippleError,
};

use super::device::device_accessory::{AccessoryDeviceListResponse, AccessoryDeviceResponse};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RemoteAccessoryResponse {
    None(()),
    String(String),
    Boolean(bool),
    Number(u32),
    Error(RippleError),
    RemoteAccessoryListResponse(AccessoryDeviceListResponse),
    AccessoryPairResponse(AccessoryDeviceResponse),
}

impl ExtnPayloadProvider for RemoteAccessoryResponse {
    fn get_extn_payload(&self) -> ExtnPayload {
        ExtnPayload::Response(ExtnResponse::Value(
            serde_json::to_value(self.clone()).unwrap(),
        ))
    }

    fn get_from_payload(payload: ExtnPayload) -> Option<Self> {
        match payload {
            ExtnPayload::Response(response) => match response {
                ExtnResponse::Value(value) => {
                    if let Ok(v) = serde_json::from_value(value) {
                        return Some(v);
                    }
                }
                _ => {}
            },
            _ => {}
        }
        None
    }

    fn contract() -> RippleContract {
        RippleContract::RemoteAccessory
    }
}
