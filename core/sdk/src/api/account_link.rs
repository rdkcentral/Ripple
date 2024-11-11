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
    extn::extn_client_message::{ExtnPayload, ExtnPayloadProvider, ExtnRequest},
    framework::ripple_contract::RippleContract,
};

use super::{
    firebolt::fb_discovery::{ContentAccessRequest, ProgressUnit, WatchedInfo},
    gateway::rpc_gateway_api::CallContext,
};

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub enum AccountLinkRequest {
    SignIn(CallContext),
    SignOut(CallContext),
    ContentAccess(CallContext, ContentAccessRequest),
    ClearContentAccess(CallContext),
    Watched(WatchedRequest),
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct WatchedRequest {
    pub context: CallContext,
    pub info: WatchedInfo,
    pub unit: Option<ProgressUnit>,
}

impl ExtnPayloadProvider for AccountLinkRequest {
    fn get_extn_payload(&self) -> ExtnPayload {
        ExtnPayload::Request(ExtnRequest::AccountLink(self.clone()))
    }

    fn get_from_payload(payload: ExtnPayload) -> Option<Self> {
        if let ExtnPayload::Request(ExtnRequest::AccountLink(value)) = payload {
            return Some(value);
        }

        None
    }

    fn contract() -> RippleContract {
        RippleContract::AccountLink
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::account_link::AccountLinkRequest;
    use crate::api::gateway::rpc_gateway_api::{ApiProtocol, CallContext};
    use crate::utils::test_utils::test_extn_payload_provider;

    #[test]
    fn test_extn_request_account_link() {
        let call_context = CallContext {
            session_id: "test_session_id".to_string(),
            request_id: "test_request_id".to_string(),
            app_id: "test_app_id".to_string(),
            call_id: 123,
            protocol: ApiProtocol::Bridge,
            method: "some_method".to_string(),
            cid: Some("test_cid".to_string()),
            gateway_secure: true,
            context: Vec::new(),
        };

        let account_link_request = AccountLinkRequest::SignIn(call_context);
        let contract_type: RippleContract = RippleContract::AccountLink;

        test_extn_payload_provider(account_link_request, contract_type);
    }
}
