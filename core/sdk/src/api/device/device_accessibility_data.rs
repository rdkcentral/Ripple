use crate::message::DabRequest;
use async_trait::async_trait;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use strum::AsRefStr;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StorageData {
    pub value: Value,
    pub update_time: String, // ISO 8601/RFC3339 format
}

impl StorageData {
    pub fn new(value: Value) -> StorageData {
        StorageData {
            value: value.clone(),
            update_time: Utc::now().to_rfc3339(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GetStorageProperty {
    pub namespace: String,
    pub key: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SetStorageProperty {
    pub namespace: String,
    pub key: String,
    pub data: StorageData,
}

#[derive(Debug, Serialize, Deserialize, Clone, AsRefStr)]
pub enum StorageRequest {
    Get(GetStorageProperty),
    Set(SetStorageProperty),
}

#[async_trait]
pub trait StorageService {
    async fn delete_key(self: Box<Self>, namespace: String, key: String) -> bool;
    async fn delete_namespace(self: Box<Self>, namespace: String) -> bool;
    async fn flush_cache(self: Box<Self>) -> bool;
    // async fn get_keys(self: Box<Self>, namespace: String) -> (Vec<String>, bool);
    // async fn get_namespaces(self: Box<Self>) -> (Vec<String>, bool);
    // async fn get_storage_size(self: Box<Self>) -> (HashMap<String, u32>, bool);
    async fn get_value(self: Box<Self>, request: DabRequest, data: GetStorageProperty);
    async fn set_value(self: Box<Self>, request: DabRequest, data: SetStorageProperty);
}