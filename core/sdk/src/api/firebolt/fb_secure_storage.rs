use serde::{Deserialize, Serialize};

use crate::{
    api::session::AccountSession,
    extn::extn_client_message::{ExtnPayload, ExtnPayloadProvider, ExtnRequest, ExtnResponse},
    framework::ripple_contract::RippleContract,
};
#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum StorageScope {
    Device,
    Account,
}
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StorageSetOptions {
    pub ttl: i32,
}
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SecureStorageGetRequest {
    pub app_id: String,
    pub scope: StorageScope,
    pub key: String,
    pub distributor_session: AccountSession,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SecureStorageSetRequest {
    pub app_id: String,
    pub scope: StorageScope,
    pub key: String,
    pub value: String,
    pub options: StorageSetOptions,
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
    pub options: StorageOptions,
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

impl ExtnPayloadProvider for SecureStorageRequest {
    fn get_extn_payload(&self) -> ExtnPayload {
        ExtnPayload::Request(ExtnRequest::SecureStorage(self.clone()))
    }

    fn get_from_payload(payload: ExtnPayload) -> Option<SecureStorageRequest> {
        match payload {
            ExtnPayload::Request(request) => match request {
                ExtnRequest::SecureStorage(r) => return Some(r),
                _ => {}
            },
            _ => {}
        }
        None
    }

    fn contract() -> RippleContract {
        RippleContract::SecureStorage
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
        ExtnPayload::Response(ExtnResponse::Value(
            serde_json::to_value(self.clone()).unwrap(),
        ))
    }

    fn get_from_payload(payload: ExtnPayload) -> Option<Self> {
        match payload {
            ExtnPayload::Response(response) => match response {
                ExtnResponse::Value(value) => {
                    if let Ok(v) = serde_json::from_value(value) {
                        return Some(v);
                    }
                }
                _ => {}
            },
            _ => {}
        }
        None
    }

    fn contract() -> RippleContract {
        RippleContract::SecureStorage
    }
}
