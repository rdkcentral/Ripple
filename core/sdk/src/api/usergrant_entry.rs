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

use std::time::Duration;

use crate::api::firebolt::fb_capabilities::CapabilityRole;
use crate::{
    extn::extn_client_message::{ExtnPayload, ExtnPayloadProvider, ExtnRequest},
    framework::ripple_contract::RippleContract,
};
use serde::{Deserialize, Serialize};

use super::device::device_user_grants_data::{GrantLifespan, GrantStatus};
use super::firebolt::fb_capabilities::FireboltPermission;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum UserGrantsStoreRequest {
    GetUserGrants(String, FireboltPermission),
    SetUserGrants(UserGrantInfo),
}

impl ExtnPayloadProvider for UserGrantsStoreRequest {
    fn get_extn_payload(&self) -> ExtnPayload {
        ExtnPayload::Request(ExtnRequest::UserGrantsStore(self.clone()))
    }

    fn get_from_payload(payload: ExtnPayload) -> Option<Self> {
        match payload {
            ExtnPayload::Request(request) => match request {
                ExtnRequest::UserGrantsStore(r) => return Some(r),
                _ => {}
            },
            _ => {}
        }
        None
    }

    fn contract() -> RippleContract {
        RippleContract::UserGrantsLocalStore
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct UserGrantInfo {
    pub role: CapabilityRole,
    pub capability: String,
    pub status: GrantStatus,
    pub last_modified_time: Duration, // Duration since Unix epoch
    pub expiry_time: Option<Duration>,
    pub app_name: Option<String>,
    pub lifespan: GrantLifespan,
}

impl Default for UserGrantInfo {
    fn default() -> Self {
        UserGrantInfo {
            role: CapabilityRole::Use,
            capability: Default::default(),
            status: GrantStatus::Denied,
            last_modified_time: Duration::new(0, 0),
            expiry_time: None,
            app_name: None,
            lifespan: GrantLifespan::Once,
        }
    }
}
