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

use super::fb_capabilities::{CapabilityRole, FireboltCap, FireboltPermission};

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GetUserGrantsByCapabilityRequest {
    pub capability: String,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GetUserGrantsByAppRequest {
    pub app_id: String,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GrantInfo {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app: Option<AppInfo>, //None in case of device
    pub state: String,
    pub capability: String,
    pub role: String,
    pub lifespan: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires: Option<String>, // Option<u64>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GrantModificationOptions {
    pub app_id: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct GrantRequest {
    pub role: CapabilityRole,
    pub capability: String,
    pub options: Option<GrantModificationOptions>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct UserGrantRequestParam {
    pub app_id: String,
    pub permissions: Vec<CapabilityAndRole>,
    pub options: Option<RequestOptions>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct RequestOptions {
    pub force: bool,
}

#[derive(Debug, Deserialize, Clone)]
pub struct CapabilityAndRole {
    pub capability: FireboltCap,
    pub role: CapabilityRole,
}

impl From<UserGrantRequestParam> for Vec<FireboltPermission> {
    fn from(value: UserGrantRequestParam) -> Self {
        let mut fb_perms = Vec::new();
        for caps in value.permissions {
            fb_perms.push(FireboltPermission {
                cap: caps.capability.clone(),
                role: caps.role,
            })
        }
        fb_perms
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_user_grant_request_param() {
        let param = UserGrantRequestParam {
            app_id: String::from("my_app"),
            permissions: vec![
                CapabilityAndRole {
                    capability: FireboltCap::Short("use_cap".to_string()),
                    role: CapabilityRole::Use,
                },
                CapabilityAndRole {
                    capability: FireboltCap::Short("provide_cap".to_string()),
                    role: CapabilityRole::Provide,
                },
            ],
            options: Some(RequestOptions { force: true }),
        };

        let expected = vec![
            FireboltPermission {
                cap: FireboltCap::Short("use_cap".to_string()),
                role: CapabilityRole::Use,
            },
            FireboltPermission {
                cap: FireboltCap::Short("provide_cap".to_string()),
                role: CapabilityRole::Provide,
            },
        ];

        let result: Vec<FireboltPermission> = param.into();
        assert_eq!(result, expected);
    }
}
