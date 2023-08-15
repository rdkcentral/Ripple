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
use ripple_sdk::api::{
    firebolt::fb_capabilities::{CapabilityRole, FireboltCap, FireboltPermission},
    gateway::rpc_gateway_api::CallContext,
};
use ripple_tdk::utils::test_utils::Mockable;

use crate::state::platform_state::PlatformState;

pub struct MockRuntime {
    pub platform_state: PlatformState,
    pub call_context: CallContext,
}

impl MockRuntime {
    pub fn new() -> Self {
        Self {
            platform_state: PlatformState::mock(),
            call_context: CallContext::mock(),
        }
    }
}

impl Default for MockRuntime {
    fn default() -> Self {
        Self::new()
    }
}

pub fn fb_perm(cap: &str, role: Option<CapabilityRole>) -> FireboltPermission {
    FireboltPermission {
        cap: FireboltCap::Full(cap.to_owned()),
        role: role.unwrap_or(CapabilityRole::Use),
    }
}
