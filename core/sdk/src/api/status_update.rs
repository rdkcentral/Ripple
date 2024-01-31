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

use crate::{
    extn::extn_client_message::{ExtnEvent, ExtnPayload, ExtnPayloadProvider},
    framework::ripple_contract::RippleContract,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ExtnStatus {
    Error,
    Ready,
    Interrupted,
}

impl ExtnPayloadProvider for ExtnStatus {
    fn get_extn_payload(&self) -> ExtnPayload {
        ExtnPayload::Event(ExtnEvent::Status(self.clone()))
    }

    fn get_from_payload(payload: ExtnPayload) -> Option<ExtnStatus> {
        if let ExtnPayload::Event(ExtnEvent::Status(r)) = payload {
            return Some(r);
        }

        None
    }

    fn contract() -> RippleContract {
        RippleContract::ExtnStatus
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::test_utils::test_extn_payload_provider;

    #[test]
    fn test_extn_payload_provider_for_extn_status() {
        let extn_status = ExtnStatus::Error;
        let contract_type: RippleContract = RippleContract::ExtnStatus;
        test_extn_payload_provider(extn_status, contract_type);
    }
}
