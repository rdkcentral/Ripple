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
    api::{
        firebolt::fb_capabilities::FireboltPermission, session::AccountSession,
        storage_property::StorageAdjective, usergrant_entry::UserGrantInfo,
    },
    extn::extn_client_message::{ExtnPayload, ExtnPayloadProvider, ExtnRequest},
    framework::ripple_contract::RippleContract,
};

#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct UserGrantsCloudGetParams {
    pub app_id: String,
    pub permission: FireboltPermission,
    pub account_session: AccountSession,
}
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct UserGrantsCloudSetParams {
    pub account_session: AccountSession,
    pub user_grant_info: UserGrantInfo,
}
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub enum UserGrantsCloudStoreRequest {
    GetCloudUserGrants(UserGrantsCloudGetParams),
    SetCloudUserGrants(UserGrantsCloudSetParams),
}

impl ExtnPayloadProvider for UserGrantsCloudStoreRequest {
    fn get_extn_payload(&self) -> ExtnPayload {
        ExtnPayload::Request(ExtnRequest::UserGrantsCloudStore(self.clone()))
    }

    fn get_from_payload(payload: ExtnPayload) -> Option<Self> {
        if let ExtnPayload::Request(ExtnRequest::UserGrantsCloudStore(r)) = payload {
            return Some(r);
        }
        None
    }

    fn contract() -> RippleContract {
        RippleContract::Storage(StorageAdjective::UsergrantCloud)
    }
}

pub type UserGrants = Vec<UserGrantInfo>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::firebolt::fb_capabilities::{CapabilityRole, FireboltCap};
    use crate::utils::test_utils::test_extn_payload_provider;

    #[test]
    fn test_extn_request_user_grants_cloud_store() {
        let user_grants_get_params = UserGrantsCloudGetParams {
            app_id: "test_app_id".to_string(),
            permission: FireboltPermission {
                cap: FireboltCap::Short("test_short_cap".to_string()),
                role: CapabilityRole::Use,
            },
            account_session: AccountSession {
                id: "test_session_id".to_string(),
                token: "test_token".to_string(),
                account_id: "test_account_id".to_string(),
                device_id: "test_device_id".to_string(),
            },
        };

        let user_grants_request =
            UserGrantsCloudStoreRequest::GetCloudUserGrants(user_grants_get_params);

        let contract_type = RippleContract::Storage(StorageAdjective::UsergrantCloud);
        test_extn_payload_provider(user_grants_request, contract_type);
    }
}
