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
        usergrant_entry::UserGrantInfo,
    },
    extn::extn_client_message::{ExtnPayload, ExtnPayloadProvider, ExtnRequest},
    framework::ripple_contract::RippleContract,
};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct UserGrantsCloudGetParams {
    pub app_id: String,
    pub permission: FireboltPermission,
    pub account_session: AccountSession,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct UserGrantsCloudSetParams {
    pub account_session: AccountSession,
    pub user_grant_info: UserGrantInfo,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
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
        RippleContract::UserGrantsCloudStore
    }
}
