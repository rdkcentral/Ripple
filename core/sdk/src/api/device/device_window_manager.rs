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
    api::apps::Dimensions,
    extn::extn_client_message::{ExtnPayload, ExtnPayloadProvider, ExtnRequest},
    framework::ripple_contract::RippleContract,
};

use super::device_request::DeviceRequest;

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub enum WindowManagerRequest {
    Visibility(String, bool),
    MoveToFront(String),
    MoveToBack(String),
    Focus(String),
    Dimensions(String, Dimensions),
}

impl WindowManagerRequest {
    pub fn window_name(&self) -> String {
        match self {
            WindowManagerRequest::Visibility(wn, _) => wn,
            WindowManagerRequest::MoveToFront(wn) => wn,
            WindowManagerRequest::MoveToBack(wn) => wn,
            WindowManagerRequest::Focus(wn) => wn,
            WindowManagerRequest::Dimensions(wn, _) => wn,
        }
        .clone()
    }

    pub fn visible(&self) -> Option<bool> {
        if let WindowManagerRequest::Visibility(_, params) = self {
            return Some(*params);
        }
        None
    }

    pub fn dimensions(&self) -> Option<Dimensions> {
        if let WindowManagerRequest::Dimensions(_, params) = self {
            return Some(params.clone());
        }
        None
    }
}

impl ExtnPayloadProvider for WindowManagerRequest {
    fn get_extn_payload(&self) -> ExtnPayload {
        ExtnPayload::Request(ExtnRequest::Device(DeviceRequest::WindowManager(
            self.clone(),
        )))
    }

    fn get_from_payload(payload: ExtnPayload) -> Option<Self> {
        if let ExtnPayload::Request(ExtnRequest::Device(DeviceRequest::WindowManager(d))) = payload
        {
            return Some(d);
        }

        None
    }

    fn contract() -> RippleContract {
        RippleContract::WindowManager
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::test_utils::test_extn_payload_provider;

    #[test]
    fn test_extn_payload_provider_for_visibility_request() {
        let visibility_request =
            WindowManagerRequest::Visibility(String::from("window_id_1"), true);

        let contract_type: RippleContract = RippleContract::WindowManager;
        test_extn_payload_provider(visibility_request, contract_type);
    }
}
