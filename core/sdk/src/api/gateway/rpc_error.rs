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

pub trait RpcError {
    type E;
    fn get_rpc_error_code(&self) -> i32;
    fn get_rpc_error_message(&self, error_type: Self::E) -> String;
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum DenyReason {
    Unpermitted,
    Unsupported,
    Disabled,
    Unavailable,
    GrantDenied,
    Ungranted,
}

pub const CAPABILITY_NOT_AVAILABLE: i32 = -50300;

pub const CAPABILITY_NOT_SUPPORTED: i32 = -50100;

pub const CAPABILITY_GET_ERROR: i32 = -50200;

pub const CAPABILITY_NOT_PERMITTED: i32 = -40300;

pub const JSON_RPC_STANDARD_ERROR_INVALID_PARAMS: i32 = -32602;

impl RpcError for DenyReason {
    type E = Vec<String>;
    fn get_rpc_error_code(&self) -> i32 {
        match self {
            Self::Unavailable => CAPABILITY_NOT_AVAILABLE,
            Self::Unsupported => CAPABILITY_NOT_SUPPORTED,
            Self::GrantDenied => CAPABILITY_NOT_PERMITTED,
            Self::Unpermitted => CAPABILITY_NOT_PERMITTED,
            _ => CAPABILITY_GET_ERROR,
        }
    }

    fn get_rpc_error_message(&self, caps: Vec<String>) -> String {
        let caps_disp = caps.clone().join(",");
        match self {
            Self::Unavailable => format!("{} is not available", caps_disp),
            Self::Unsupported => format!("{} is not supported", caps_disp),
            Self::GrantDenied => format!("The user denied access to {}", caps_disp),
            Self::Unpermitted => format!("{} is not permitted", caps_disp),
            _ => format!("Error with {}", caps_disp),
        }
    }
}
