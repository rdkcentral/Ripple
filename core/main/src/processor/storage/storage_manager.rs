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

use jsonrpsee::{core::RpcResult, types::error::CallError};
use ripple_sdk::{
    api::{
        device::device_peristence::{
            DeleteStorageProperty, DevicePersistenceRequest, GetStorageProperty,
            SetStorageProperty, StorageData,
        },
        firebolt::fb_capabilities::CAPABILITY_NOT_AVAILABLE,
        storage_property::StorageProperty,
    },
    extn::extn_client_message::ExtnResponse,
    log::debug,
    serde_json::{json, Value},
    tokio,
    utils::error::RippleError,
};
use std::collections::HashMap;

use crate::{
    processor::storage::storage_manager_utils::{
        storage_to_bool_rpc_result, storage_to_f32_rpc_result, storage_to_string_rpc_result,
        storage_to_u32_rpc_result,
    },
    service::apps::app_events::AppEvents,
    state::platform_state::PlatformState,
};

use super::{
    default_storage_properties::DefaultStorageProperties,
    storage_manager_utils::storage_to_vec_string_rpc_result,
};

#[derive(Debug)]
pub enum StorageManagerResponse<T> {
    Ok(T),
    Default(T),
    NoChange(T),
}

impl<T: Clone> StorageManagerResponse<T> {
    pub fn as_value(&self) -> T {
        match self {
            StorageManagerResponse::Ok(value) => value.clone(),
            StorageManagerResponse::Default(value) => value.clone(),
            StorageManagerResponse::NoChange(value) => value.clone(),
        }
    }
}

#[derive(Debug)]
pub enum StorageManagerError {
    NotFound,
    WriteError,
    DataTypeMisMatch,
}

#[derive(Clone)]
pub struct StorageManager;

impl StorageManager {
    pub async fn get_bool(state: &PlatformState, property: StorageProperty) -> RpcResult<bool> {
        if let Some(val) = state
            .ripple_cache
            .get_cached_bool_storage_property(&property)
        {
            return Ok(val);
        }
        let data = property.as_data();
        match StorageManager::get_bool_from_namespace(state, data.namespace.to_string(), data.key)
            .await
        {
            Ok(StorageManagerResponse::Ok(value)) | Ok(StorageManagerResponse::NoChange(value)) => {
                state
                    .ripple_cache
                    .update_cached_bool_storage_property(&property, value);
                Ok(value)
            }
            Ok(StorageManagerResponse::Default(value)) => Ok(value),
            Err(_) => Err(StorageManager::get_firebolt_error(&property)),
        }
    }

    pub async fn set_bool(
        state: &PlatformState,
        property: StorageProperty,
        value: bool,
        context: Option<Value>,
    ) -> RpcResult<()> {
        let data = property.as_data();
        debug!("Storage property: {:?} as data: {:?}", property, data);
        if let Some(val) = state
            .ripple_cache
            .get_cached_bool_storage_property(&property)
        {
            if val == value {
                return Ok(());
            }
        }
        match StorageManager::set_in_namespace(
            state,
            data.namespace.to_string(),
            data.key.to_string(),
            json!(value),
            data.event_names,
            context,
        )
        .await
        {
            Ok(StorageManagerResponse::Ok(_)) | Ok(StorageManagerResponse::NoChange(_)) => {
                state
                    .ripple_cache
                    .update_cached_bool_storage_property(&property, value);
                Ok(())
            }
            Ok(StorageManagerResponse::Default(_)) => Ok(()),
            Err(_) => Err(StorageManager::get_firebolt_error(&property)),
        }
    }

    pub async fn get_string(state: &PlatformState, property: StorageProperty) -> RpcResult<String> {
        let data = property.as_data();
        match StorageManager::get_string_from_namespace(state, data.namespace.to_string(), data.key)
            .await
        {
            Ok(resp) => Ok(resp.as_value()),
            Err(_) => Err(StorageManager::get_firebolt_error(&property)),
        }
    }

    pub async fn get_map(
        state: &PlatformState,
        property: StorageProperty,
    ) -> RpcResult<HashMap<String, Value>> {
        match StorageManager::get_string(state, property.clone()).await {
            Ok(raw_value) => match serde_json::from_str(&raw_value) {
                Ok(raw_map) => {
                    let the_map: HashMap<String, serde_json::Value> = raw_map;
                    Ok(the_map)
                }
                Err(_) => Err(StorageManager::get_firebolt_error(&property)),
            },
            Err(_) => Err(StorageManager::get_firebolt_error(&property)),
        }
    }

    pub async fn set_value_in_map(
        state: &PlatformState,
        property: StorageProperty,
        key: String,
        value: String,
    ) -> RpcResult<()> {
        match StorageManager::get_map(state, property.clone()).await {
            Ok(the_map) => {
                let mut mutant: HashMap<String, serde_json::Value> = the_map;
                mutant.insert(key, serde_json::Value::String(value));
                match StorageManager::set_string(
                    state,
                    property.clone(),
                    serde_json::to_string(&mutant).unwrap(),
                    None,
                )
                .await
                {
                    Ok(_) => Ok(()),
                    Err(_) => Err(StorageManager::get_firebolt_error(&property)),
                }
            }
            Err(_) => {
                let mut map: HashMap<String, serde_json::Value> = Default::default();
                map.insert(key, serde_json::Value::String(value));
                match StorageManager::set_string(
                    state,
                    property.clone(),
                    serde_json::to_string(&map).unwrap(),
                    None,
                )
                .await
                {
                    Ok(_) => Ok(()),
                    Err(_) => Err(StorageManager::get_firebolt_error(&property)),
                }
            }
        }
    }

    pub async fn remove_value_in_map(
        state: &PlatformState,
        property: StorageProperty,
        key: String,
    ) -> RpcResult<()> {
        match StorageManager::get_map(state, property.clone()).await {
            Ok(the_map) => {
                let mut mutant: HashMap<String, serde_json::Value> = the_map;
                mutant.remove(&key);
                match StorageManager::set_string(
                    state,
                    property.clone(),
                    serde_json::to_string(&mutant).unwrap(),
                    None,
                )
                .await
                {
                    Ok(_) => Ok(()),
                    Err(_) => Err(StorageManager::get_firebolt_error(&property)),
                }
            }
            Err(_) => Err(StorageManager::get_firebolt_error(&property)),
        }
    }

    pub async fn set_string(
        state: &PlatformState,
        property: StorageProperty,
        value: String,
        context: Option<Value>,
    ) -> RpcResult<()> {
        let data = property.as_data();
        if StorageManager::set_in_namespace(
            state,
            data.namespace.to_string(),
            data.key.to_string(),
            json!(value),
            data.event_names,
            context,
        )
        .await
        .is_err()
        {
            return Err(StorageManager::get_firebolt_error(&property));
        }
        Ok(())
    }

    pub async fn get_number_as_u32(
        state: &PlatformState,
        property: StorageProperty,
    ) -> RpcResult<u32> {
        let data = property.as_data();
        match StorageManager::get_number_as_u32_from_namespace(
            state,
            data.namespace.to_string(),
            data.key,
        )
        .await
        {
            Ok(resp) => Ok(resp.as_value()),
            Err(_) => Err(StorageManager::get_firebolt_error(&property)),
        }
    }

    pub async fn get_number_as_f32(
        state: &PlatformState,
        property: StorageProperty,
    ) -> RpcResult<f32> {
        let data = property.as_data();
        StorageManager::get_number_as_f32_from_namespace(
            state,
            data.namespace.to_string(),
            data.key,
        )
        .await
        .map_or(Err(StorageManager::get_firebolt_error(&property)), |resp| {
            Ok(resp.as_value())
        })
    }

    pub async fn set_number_as_f32(
        state: &PlatformState,
        property: StorageProperty,
        value: f32,
        context: Option<Value>,
    ) -> RpcResult<()> {
        let data = property.as_data();
        if StorageManager::set_in_namespace(
            state,
            data.namespace.to_string(),
            data.key.to_string(),
            json!(value),
            data.event_names,
            context,
        )
        .await
        .is_err()
        {
            return Err(StorageManager::get_firebolt_error(&property));
        }
        Ok(())
    }

    pub async fn set_number_as_u32(
        state: &PlatformState,
        property: StorageProperty,
        value: u32,
        context: Option<Value>,
    ) -> RpcResult<()> {
        let data = property.as_data();
        if StorageManager::set_in_namespace(
            state,
            data.namespace.to_string(),
            data.key.to_string(),
            json!(value),
            data.event_names,
            context,
        )
        .await
        .is_err()
        {
            return Err(StorageManager::get_firebolt_error(&property));
        }
        Ok(())
    }

    /*
    Used internally or when a custom namespace is required
     */
    async fn get_bool_from_namespace(
        state: &PlatformState,
        namespace: String,
        key: &'static str,
    ) -> Result<StorageManagerResponse<bool>, StorageManagerError> {
        debug!("get_bool: namespace={}, key={}", namespace, key);
        let resp = StorageManager::get(state, &namespace, &key.to_string()).await;
        match storage_to_bool_rpc_result(resp) {
            Ok(value) => Ok(StorageManagerResponse::Ok(value)),
            Err(_) => {
                if let Ok(value) = DefaultStorageProperties::get_bool(state, &namespace, key) {
                    return Ok(StorageManagerResponse::Default(value));
                }
                Err(StorageManagerError::NotFound)
            }
        }
    }

    /*
    Used internally or when a custom namespace is required
     */
    pub async fn set_in_namespace(
        state: &PlatformState,
        namespace: String,
        key: String,
        value: Value,
        event_names: Option<&'static [&'static str]>,
        context: Option<Value>,
    ) -> Result<StorageManagerResponse<()>, StorageManagerError> {
        if let Ok(ExtnResponse::StorageData(storage_data)) =
            StorageManager::get(state, &namespace, &key).await
        {
            if storage_data.value.eq(&value) {
                return Ok(StorageManagerResponse::NoChange(()));
            }

            // The stored value may have preceeded StorageData implementation, if so
            // allow the set to occur regardless of whether the values match or not in
            // order to update peristent storage with the new StorageData format.
        }

        let ssp = SetStorageProperty {
            namespace,
            key,
            data: StorageData::new(value.clone()),
        };

        match state
            .get_client()
            .send_extn_request(DevicePersistenceRequest::Set(ssp))
            .await
        {
            Ok(_) => {
                if let Some(events) = event_names {
                    let val = value.clone();
                    for event in events.iter() {
                        let state_for_event = state.clone();
                        let result = val.clone();
                        let ctx = context.clone();
                        let evt = String::from(*event);
                        tokio::spawn(async move {
                            debug!("set_in_namespace: Sending event {:?} ctx {:?}", evt, ctx);
                            AppEvents::emit_with_context(&state_for_event, &evt, &result, ctx)
                                .await;
                        });
                    }
                }
                Ok(StorageManagerResponse::Ok(()))
            }
            Err(_) => Err(StorageManagerError::WriteError),
        }
    }

    /*
    Used internally or when a custom namespace is required
     */
    pub async fn get_string_from_namespace(
        state: &PlatformState,
        namespace: String,
        key: &'static str,
    ) -> Result<StorageManagerResponse<String>, StorageManagerError> {
        debug!("get_string: namespace={}, key={}", namespace, key);
        let resp = StorageManager::get(state, &namespace, &key.to_string()).await;
        match storage_to_string_rpc_result(resp) {
            Ok(value) => Ok(StorageManagerResponse::Ok(value)),
            Err(_) => {
                if let Ok(value) = DefaultStorageProperties::get_string(state, &namespace, key) {
                    return Ok(StorageManagerResponse::Default(value));
                }
                Err(StorageManagerError::NotFound)
            }
        }
    }

    /*
    Used internally or when a custom namespace is required
     */
    pub async fn get_number_as_u32_from_namespace(
        state: &PlatformState,
        namespace: String,
        key: &'static str,
    ) -> Result<StorageManagerResponse<u32>, StorageManagerError> {
        debug!("get_string: namespace={}, key={}", namespace, key);
        let resp = StorageManager::get(state, &namespace, &key.to_string()).await;
        match storage_to_u32_rpc_result(resp) {
            Ok(value) => Ok(StorageManagerResponse::Ok(value)),
            Err(_) => {
                if let Ok(value) =
                    DefaultStorageProperties::get_number_as_u32(state, &namespace, key)
                {
                    return Ok(StorageManagerResponse::Default(value));
                }
                Err(StorageManagerError::NotFound)
            }
        }
    }

    /*
    Used internally or when a custom namespace is required
     */
    pub async fn get_number_as_f32_from_namespace(
        state: &PlatformState,
        namespace: String,
        key: &'static str,
    ) -> Result<StorageManagerResponse<f32>, StorageManagerError> {
        debug!(
            "get_number_as_f32_from_namespace: namespace={}, key={}",
            namespace, key
        );
        let resp = StorageManager::get(state, &namespace, &key.to_string()).await;

        storage_to_f32_rpc_result(resp).map_or_else(
            |_| {
                DefaultStorageProperties::get_number_as_f32(state, &namespace, key)
                    .map_or(Err(StorageManagerError::NotFound), |val| {
                        Ok(StorageManagerResponse::Ok(val))
                    })
            },
            |val| Ok(StorageManagerResponse::Ok(val)),
        )
    }

    pub async fn delete_key(state: &PlatformState, property: StorageProperty) -> RpcResult<()> {
        let data = property.as_data();
        if let Err(_err) =
            StorageManager::delete(state, &data.namespace.to_string(), &data.key.to_string()).await
        {
            return Err(StorageManager::get_firebolt_error(&property));
        }
        Ok(())
    }

    async fn get(
        state: &PlatformState,
        namespace: &String,
        key: &String,
    ) -> Result<ExtnResponse, RippleError> {
        debug!("get: namespace={}, key={}", namespace, key);
        let data = GetStorageProperty {
            namespace: namespace.clone(),
            key: key.clone(),
        };
        let result = state
            .get_client()
            .send_extn_request(DevicePersistenceRequest::Get(data))
            .await;

        match result {
            Ok(msg) => {
                if let Some(m) = msg.payload.extract() {
                    Ok(m)
                } else {
                    Err(RippleError::ParseError)
                }
            }
            Err(e) => Err(e),
        }
    }

    async fn delete(
        state: &PlatformState,
        namespace: &String,
        key: &String,
    ) -> Result<ExtnResponse, RippleError> {
        debug!("delete: namespace={}, key={}", namespace, key);
        let data = DeleteStorageProperty {
            namespace: namespace.clone(),
            key: key.clone(),
        };
        let result = state
            .get_client()
            .send_extn_request(DevicePersistenceRequest::Delete(data))
            .await;
        match result {
            Ok(msg) => {
                if let Some(m) = msg.payload.extract() {
                    Ok(m)
                } else {
                    Err(RippleError::ParseError)
                }
            }
            Err(e) => Err(e),
        }
    }

    pub fn get_firebolt_error(property: &StorageProperty) -> jsonrpsee::core::Error {
        let data = property.as_data();
        jsonrpsee::core::Error::Call(CallError::Custom {
            code: CAPABILITY_NOT_AVAILABLE,
            message: format!("{}.{} is not available", data.namespace, data.key),
            data: None,
        })
    }

    pub async fn set_vec_string(
        state: &PlatformState,
        property: StorageProperty,
        value: Vec<String>,
        context: Option<Value>,
    ) -> RpcResult<()> {
        let data = property.as_data();
        if StorageManager::set_in_namespace(
            state,
            data.namespace.to_string(),
            data.key.to_string(),
            json!(value),
            data.event_names,
            context,
        )
        .await
        .is_err()
        {
            return Err(StorageManager::get_firebolt_error(&property));
        }
        Ok(())
    }

    pub async fn get_vec_string(
        state: &PlatformState,
        property: StorageProperty,
    ) -> RpcResult<Vec<String>> {
        let data = property.as_data();
        storage_to_vec_string_rpc_result(
            StorageManager::get(state, &data.namespace.to_string(), &data.key.to_string()).await,
        )
    }
}
