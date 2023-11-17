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
    api::session::AccountSession,
    extn::extn_client_message::{ExtnPayload, ExtnPayloadProvider, ExtnRequest},
    framework::ripple_contract::RippleContract,
};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DistributorTokenRequest {
    pub options: Vec<String>,
    pub context: DistributorTokenContext,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DistributorTokenContext {
    pub app_id: String,
    pub dist_session: AccountSession,
}

impl ExtnPayloadProvider for DistributorTokenRequest {
    fn get_extn_payload(&self) -> ExtnPayload {
        ExtnPayload::Request(ExtnRequest::DistributorToken(self.clone()))
    }

    fn get_from_payload(payload: ExtnPayload) -> Option<DistributorTokenRequest> {
        if let ExtnPayload::Request(ExtnRequest::DistributorToken(r)) = payload {
            return Some(r);
        }

        None
    }

    fn contract() -> RippleContract {
        RippleContract::Session(crate::api::session::SessionAdjective::Distributor)
    }
}
