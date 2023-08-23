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

use ripple_sdk::{
    api::gateway::rpc_gateway_api::{ApiProtocol, CallContext},
    uuid::Uuid,
};

use crate::utils::test_utils::Mockable;

impl Mockable for CallContext {
    fn mock() -> Self {
        Self::new(
            Uuid::new_v4().to_string(),
            Uuid::new_v4().to_string(),
            "app_id".to_owned(),
            1,
            ApiProtocol::Extn,
            "method".to_owned(),
            None,
            false,
        )
    }
}
