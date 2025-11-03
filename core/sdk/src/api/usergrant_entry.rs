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

use serde::{Deserialize, Serialize};

use super::device::device_user_grants_data::{GrantLifespan, GrantStatus};

#[derive(Clone, Debug, Deserialize)]
pub enum UserGrantsPersistenceType {
    Account,
    Cloud,
}

impl UserGrantsPersistenceType {
    pub fn as_string(&self) -> &'static str {
        match self {
            UserGrantsPersistenceType::Account => "account",
            UserGrantsPersistenceType::Cloud => "cloud",
        }
    }
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct UserGrantInfo {
    pub role: CapabilityRole,
    pub capability: String,
    pub status: Option<GrantStatus>,
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
            status: Some(GrantStatus::Denied),
            last_modified_time: Duration::new(0, 0),
            expiry_time: None,
            app_name: None,
            lifespan: GrantLifespan::Once,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::firebolt::fb_capabilities::CapabilityRole;

    #[test]
    fn test_user_grants_persistence_type_as_string() {
        assert_eq!(UserGrantsPersistenceType::Account.as_string(), "account");
        assert_eq!(UserGrantsPersistenceType::Cloud.as_string(), "cloud");
    }

    #[test]
    fn test_user_grant_info_default() {
        let default_info = UserGrantInfo::default();
        assert_eq!(default_info.role, CapabilityRole::Use);
        assert_eq!(default_info.capability, "");
        assert_eq!(default_info.status, Some(GrantStatus::Denied));
        assert_eq!(default_info.last_modified_time, Duration::new(0, 0));
        assert_eq!(default_info.expiry_time, None);
        assert_eq!(default_info.app_name, None);
        assert_eq!(default_info.lifespan, GrantLifespan::Once);
    }
}
