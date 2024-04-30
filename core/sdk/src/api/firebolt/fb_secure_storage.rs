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
    api::{session::AccountSession, storage_property::StorageAdjective},
    extn::extn_client_message::{ExtnPayload, ExtnPayloadProvider, ExtnRequest, ExtnResponse},
    framework::ripple_contract::RippleContract,
};
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub enum StorageScope {
    Device,
    Account,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct SecureStorageGetRequest {
    pub scope: StorageScope,
    pub key: String,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct StorageOptions {
    pub ttl: i32,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SecureStorageSetRequest {
    pub app_id: Option<String>,
    pub scope: StorageScope,
    pub key: String,
    pub value: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<StorageOptions>,
}
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SecureStorageRemoveRequest {
    pub scope: StorageScope,
    pub key: String,
    pub app_id: Option<String>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SecureStorageClearRequest {
    pub app_id: Option<String>,
    pub scope: StorageScope,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct SecureStorageGetResponse {
    pub value: Option<String>,
}
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct SecureStorageDefaultResponse {}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub enum SecureStorageRequest {
    Get(String, SecureStorageGetRequest, AccountSession),
    Set(SecureStorageSetRequest, AccountSession),
    Remove(SecureStorageRemoveRequest, AccountSession),
    Clear(SecureStorageClearRequest, AccountSession),
}

impl ExtnPayloadProvider for SecureStorageRequest {
    fn get_extn_payload(&self) -> ExtnPayload {
        ExtnPayload::Request(ExtnRequest::SecureStorage(self.clone()))
    }

    fn get_from_payload(payload: ExtnPayload) -> Option<SecureStorageRequest> {
        if let ExtnPayload::Request(ExtnRequest::SecureStorage(r)) = payload {
            return Some(r);
        }

        None
    }

    fn contract() -> RippleContract {
        RippleContract::Storage(StorageAdjective::Secure)
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub enum SecureStorageResponse {
    Get(SecureStorageGetResponse),
    Set(SecureStorageDefaultResponse),
    Remove(SecureStorageDefaultResponse),
    Clear(SecureStorageDefaultResponse),
}

impl ExtnPayloadProvider for SecureStorageResponse {
    fn get_extn_payload(&self) -> ExtnPayload {
        ExtnPayload::Response(ExtnResponse::SecureStorage(self.clone()))
    }

    fn get_from_payload(payload: ExtnPayload) -> Option<Self> {
        if let ExtnPayload::Response(ExtnResponse::SecureStorage(v)) = payload {
            return Some(v);
        }

        None
    }

    fn contract() -> RippleContract {
        RippleContract::Storage(StorageAdjective::Secure)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::test_utils::test_extn_payload_provider;

    #[test]
    fn test_extn_request_secure_storage() {
        let account_session = AccountSession {
            id: "test_session_id".to_string(),
            token: "test_token".to_string(),
            account_id: "test_account_id".to_string(),
            device_id: "test_device_id".to_string(),
        };

        let get_request = SecureStorageGetRequest {
            scope: StorageScope::Device,
            key: "test_key".to_string(),
        };

        let secure_storage_request = SecureStorageRequest::Get(
            "test_secure_storage".to_string(),
            get_request,
            account_session,
        );

        let contract_type: RippleContract = RippleContract::Storage(StorageAdjective::Secure);
        test_extn_payload_provider(secure_storage_request, contract_type);
    }

    #[test]
    fn test_extn_response_secure_storage() {
        let secure_storage_get_response = SecureStorageGetResponse {
            value: Some("secret_value".to_string()),
        };
        let secure_storage_response = SecureStorageResponse::Get(secure_storage_get_response);
        let contract_type: RippleContract = RippleContract::Storage(StorageAdjective::Secure);

        test_extn_payload_provider(secure_storage_response, contract_type);
    }
}
