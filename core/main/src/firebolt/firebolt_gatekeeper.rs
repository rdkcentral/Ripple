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

use ripple_sdk::{api::gateway::rpc_gateway_api::RpcRequest, utils::error::RippleError};

use crate::state::cap::cap_state::CapState;

pub struct FireboltGatekeeper {}

impl FireboltGatekeeper {
    // TODO return Deny Reason into ripple error
    pub async fn gate(_cap_state: CapState, _request: RpcRequest) -> Result<(), RippleError> {
        Ok(())
    }
}
