use crate::message::{DistributorSession, DpabRequest};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
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
    pub distributor_session: DistributorSession,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SecureStorageSetRequest {
    pub app_id: String,
    pub scope: StorageScope,
    pub key: String,
    pub value: String,
    pub options: StorageSetOptions,
    pub distributor_session: DistributorSession,
}
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SecureStorageRemoveRequest {
    pub scope: StorageScope,
    pub key: String,
    pub app_id: String,
    pub distributor_session: DistributorSession,
}
#[derive(Debug, Clone)]
pub enum SecureStorageRequest {
    Get(SecureStorageGetRequest),
    Set(SecureStorageSetRequest),
    Remove(SecureStorageRemoveRequest),
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
pub enum SecureStorageResponse {
    Get(SecureStorageGetResponse),
    Set(SecureStorageSetResponse),
    Remove(SecureStorageRemoveResponse),
}

impl SecureStorageResponse {
    pub fn as_get_response(&self) -> Option<&SecureStorageGetResponse> {
        match self {
            SecureStorageResponse::Get(get_response) => Some(get_response),
            _ => None,
        }
    }
    pub fn as_set_response(&self) -> Option<&SecureStorageSetResponse> {
        match self {
            SecureStorageResponse::Set(set_response) => Some(set_response),
            _ => None,
        }
    }

    pub fn as_remove_response(&self) -> Option<&SecureStorageRemoveResponse> {
        match self {
            SecureStorageResponse::Remove(remove_response) => Some(remove_response),
            _ => None,
        }
    }
}
