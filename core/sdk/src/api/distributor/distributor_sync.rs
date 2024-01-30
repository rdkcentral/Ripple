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

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub enum SyncAndMonitorRequest {
    SyncAndMonitor(SyncAndMonitorModule, AccountSession),
    UpdateDistributorToken(String),
}

#[derive(PartialEq, Eq, Clone, Copy, Debug, Serialize, Deserialize)]
pub enum SyncAndMonitorModule {
    Privacy,
    UserGrants,
}

impl ExtnPayloadProvider for SyncAndMonitorRequest {
    fn get_from_payload(payload: ExtnPayload) -> Option<Self> {
        if let ExtnPayload::Request(ExtnRequest::CloudSync(p)) = payload {
            return Some(p);
        }

        None
    }

    fn get_extn_payload(&self) -> ExtnPayload {
        ExtnPayload::Request(ExtnRequest::CloudSync(self.clone()))
    }

    fn contract() -> crate::framework::ripple_contract::RippleContract {
        RippleContract::CloudSync
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::test_utils::test_extn_payload_provider;

    #[test]
    fn test_extn_request_sync_and_monitor_sync_privacy() {
        let sync_and_monitor_module = SyncAndMonitorModule::Privacy;
        let account_session = AccountSession {
            id: "test_session_id".to_string(),
            token: "test_token".to_string(),
            account_id: "test_account_id".to_string(),
            device_id: "test_device_id".to_string(),
        };

        let sync_and_monitor_request =
            SyncAndMonitorRequest::SyncAndMonitor(sync_and_monitor_module, account_session);
        let contract_type: RippleContract = RippleContract::CloudSync;

        test_extn_payload_provider(sync_and_monitor_request, contract_type);
    }
}
