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
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub enum StorageScope {
    Device,
    Account,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SecureStorageGetRequest {
    pub app_id: String,
    pub scope: StorageScope,
    pub key: String,
    pub distributor_session: AccountSession,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StorageSetOptions {
    pub ttl: i32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SecureStorageSetRequest {
    pub app_id: String,
    pub scope: StorageScope,
    pub key: String,
    pub value: String,
    pub options: Option<StorageSetOptions>,
    pub distributor_session: AccountSession,
}
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SecureStorageRemoveRequest {
    pub scope: StorageScope,
    pub key: String,
    pub app_id: String,
    pub distributor_session: AccountSession,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SecureStorageGetResponse {
    pub value: Option<String>,
}
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SecureStorageSetResponse {}
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SecureStorageRemoveResponse {}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GetRequest {
    pub key: String,
    pub scope: StorageScope,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StorageOptions {
    pub ttl: i32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SetRequest {
    pub scope: StorageScope,
    pub key: String,
    pub value: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<StorageOptions>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RemoveRequest {
    pub key: String,
    pub scope: StorageScope,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum SecureStorageRequest {
    Get(SecureStorageGetRequest),
    Set(SecureStorageSetRequest),
    Remove(SecureStorageRemoveRequest),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SetAppRequest {
    pub scope: StorageScope,
    pub key: String,
    pub value: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<StorageOptions>,
    pub app_id: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RemoveAppRequest {
    pub key: String,
    pub scope: StorageScope,
    pub app_id: String,
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

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum SecureStorageResponse {
    Get(SecureStorageGetResponse),
    Set(SecureStorageSetResponse),
    Remove(SecureStorageRemoveResponse),
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
