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
    api::{firebolt::fb_capabilities::FireboltPermission, session::AccountSession},
    extn::extn_client_message::{ExtnPayload, ExtnPayloadProvider, ExtnRequest, ExtnResponse},
    framework::ripple_contract::RippleContract,
};

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct PermissionRequest {
    pub app_id: String,
    pub session: AccountSession,
}

impl ExtnPayloadProvider for PermissionRequest {
    fn get_from_payload(payload: ExtnPayload) -> Option<Self> {
        if let ExtnPayload::Request(ExtnRequest::Permission(p)) = payload {
            return Some(p);
        }

        None
    }

    fn get_extn_payload(&self) -> ExtnPayload {
        ExtnPayload::Request(ExtnRequest::Permission(self.clone()))
    }

    fn contract() -> crate::framework::ripple_contract::RippleContract {
        RippleContract::Permissions
    }
}

pub type PermissionResponse = Vec<FireboltPermission>;

impl ExtnPayloadProvider for PermissionResponse {
    fn get_from_payload(payload: ExtnPayload) -> Option<Self> {
        if let ExtnPayload::Response(ExtnResponse::Permission(v)) = payload {
            return Some(v);
        }

        None
    }

    fn get_extn_payload(&self) -> ExtnPayload {
        ExtnPayload::Response(ExtnResponse::Permission(self.clone()))
    }

    fn contract() -> crate::framework::ripple_contract::RippleContract {
        RippleContract::Permissions
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::firebolt::fb_capabilities::{CapabilityRole, FireboltCap};
    use crate::utils::test_utils::test_extn_payload_provider;

    #[test]
    fn test_extn_request_permission() {
        let account_session = AccountSession {
            id: "test_session_id".to_string(),
            token: "test_token".to_string(),
            account_id: "test_account_id".to_string(),
            device_id: "test_device_id".to_string(),
        };

        let permission_request = PermissionRequest {
            app_id: "test_app_id".to_string(),
            session: account_session,
        };

        let contract_type: RippleContract = RippleContract::Permissions;

        test_extn_payload_provider(permission_request, contract_type);
    }

    #[test]
    fn test_extn_response_permission() {
        let permission1 = FireboltPermission {
            cap: FireboltCap::Short("test_cap1".to_string()),
            role: CapabilityRole::Use,
        };

        let permission2 = FireboltPermission {
            cap: FireboltCap::Full("test_cap2".to_string()),
            role: CapabilityRole::Manage,
        };

        let permission_response: PermissionResponse = vec![permission1, permission2];
        let contract_type: RippleContract = RippleContract::Permissions;

        test_extn_payload_provider(permission_response, contract_type);
    }
}
