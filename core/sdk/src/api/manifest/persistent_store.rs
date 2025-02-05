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
    api::storage_property::StorageAdjective,
    extn::extn_client_message::{ExtnPayload, ExtnPayloadProvider, ExtnResponse},
    framework::ripple_contract::RippleContract,
    utils::error::RippleError,
};

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub enum AccessibilityResponse {
    None(()),
    String(String),
    Boolean(bool),
    Number(u32),
    Error(RippleError),
}

impl ExtnPayloadProvider for AccessibilityResponse {
    fn get_extn_payload(&self) -> ExtnPayload {
        ExtnPayload::Response(ExtnResponse::Value(
            serde_json::to_value(self.clone()).unwrap(),
        ))
    }

    fn get_from_payload(payload: ExtnPayload) -> Option<Self> {
        if let ExtnPayload::Response(ExtnResponse::Value(value)) = payload {
            if let Ok(v) = serde_json::from_value(value) {
                return Some(v);
            }
        }

        None
    }

    fn contract() -> RippleContract {
        RippleContract::Storage(StorageAdjective::Local)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::test_utils::test_extn_payload_provider;

    #[test]
    fn test_extn_payload_provider_for_accessibility_response() {
        let accessibility_response = AccessibilityResponse::None(());
        let contract_type: RippleContract = RippleContract::Storage(StorageAdjective::Local);
        test_extn_payload_provider(accessibility_response, contract_type);
    }
    #[test]
    fn test_extn_payload_provider_for_string_response() {
        let accessibility_response = AccessibilityResponse::String("test".to_string());
        let contract_type: RippleContract = RippleContract::Storage(StorageAdjective::Local);
        test_extn_payload_provider(accessibility_response, contract_type);
    }

    #[test]
    fn test_extn_payload_provider_for_boolean_response() {
        let accessibility_response = AccessibilityResponse::Boolean(true);
        let contract_type: RippleContract = RippleContract::Storage(StorageAdjective::Local);
        test_extn_payload_provider(accessibility_response, contract_type);
    }

    #[test]
    fn test_extn_payload_provider_for_number_response() {
        let accessibility_response = AccessibilityResponse::Number(42);
        let contract_type: RippleContract = RippleContract::Storage(StorageAdjective::Local);
        test_extn_payload_provider(accessibility_response, contract_type);
    }

    #[test]
    fn test_extn_payload_provider_for_error_response() {
        let accessibility_response = AccessibilityResponse::Error(RippleError::MissingInput);
        let contract_type: RippleContract = RippleContract::Storage(StorageAdjective::Local);
        test_extn_payload_provider(accessibility_response, contract_type);
    }
}
