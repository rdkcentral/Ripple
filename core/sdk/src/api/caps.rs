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
    extn::extn_client_message::{ExtnPayload, ExtnPayloadProvider, ExtnRequest},
    framework::ripple_contract::RippleContract,
};

use super::firebolt::fb_capabilities::RoleInfo;

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub enum CapsRequest {
    Permitted(String, Vec<RoleInfo>),
    Supported(Vec<String>),
}

impl ExtnPayloadProvider for CapsRequest {
    fn get_extn_payload(&self) -> ExtnPayload {
        ExtnPayload::Request(ExtnRequest::AuthorizedInfo(self.clone()))
    }

    fn get_from_payload(payload: ExtnPayload) -> Option<Self> {
        if let ExtnPayload::Request(ExtnRequest::AuthorizedInfo(value)) = payload {
            return Some(value);
        }

        None
    }

    fn contract() -> RippleContract {
        RippleContract::Caps
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::firebolt::fb_capabilities::{CapabilityRole, FireboltCap};
    use crate::utils::test_utils::test_extn_payload_provider;

    #[test]
    fn test_extn_request_caps() {
        let app_id = "test_app_id".to_string();
        let role_infos = vec![
            RoleInfo {
                role: Some(CapabilityRole::Use),
                capability: FireboltCap::Short("test_short_cap".to_string()),
            },
            RoleInfo {
                role: Some(CapabilityRole::Manage),
                capability: FireboltCap::Full("test_full_cap".to_string()),
            },
        ];

        let caps_request = CapsRequest::Permitted(app_id, role_infos);

        let contract_type: RippleContract = RippleContract::Caps;
        test_extn_payload_provider(caps_request, contract_type);
    }
}
