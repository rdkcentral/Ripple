use dab::core::{
    message::{DabRequestPayload, DabResponsePayload},
    model::persistent_store::{
        GetStorageProperty, SetStorageProperty, StorageData, StorageRequest,
    },
};
use jsonrpsee::{core::RpcResult, types::error::CallError};
use serde_json::{json, Value};
use tracing::{debug, log::warn};

use chrono::Utc;

use crate::{
    apps::app_events::AppEvents,
    helpers::{
        error_util::{RippleError, CAPABILITY_NOT_AVAILABLE},
        ripple_helper::IRippleHelper,
    },
    managers::storage::storage_manager_utils::{
        storage_to_bool_rpc_result, storage_to_f32_rpc_result, storage_to_string_rpc_result,
        storage_to_u32_rpc_result,
    },
    platform_state::PlatformState,
};

use super::{
    default_storage_properties::DefaultStorageProperties, storage_property::StorageProperty,
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
        let data = property.as_data();
        match StorageManager::get_bool_from_namespace(state, data.namespace.to_string(), data.key)
            .await
        {
            Ok(resp) => Ok(resp.as_value()),
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
        if let Err(_) = StorageManager::set_in_namespace(
            state,
            data.namespace.to_string(),
            data.key.to_string(),
            json!(value),
            data.event_names,
            context,
        )
        .await
        {
            return Err(StorageManager::get_firebolt_error(&property));
        }
        Ok(())
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

    pub async fn set_string(
        state: &PlatformState,
        property: StorageProperty,
        value: String,
        context: Option<Value>,
    ) -> RpcResult<()> {
        let data = property.as_data();
        if let Err(_) = StorageManager::set_in_namespace(
            state,
            data.namespace.to_string(),
            data.key.to_string(),
            json!(value),
            data.event_names,
            context,
        )
        .await
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
        if let Err(_) = StorageManager::set_in_namespace(
            state,
            data.namespace.to_string(),
            data.key.to_string(),
            json!(value),
            data.event_names,
            context,
        )
        .await
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
        if let Err(_) = StorageManager::set_in_namespace(
            state,
            data.namespace.to_string(),
            data.key.to_string(),
            json!(value),
            data.event_names,
            context,
        )
        .await
        {
            return Err(StorageManager::get_firebolt_error(&property));
        }
        Ok(())
    }

    /*
    Used internally or when a custom namespace is required
     */
    pub async fn get_bool_from_namespace(
        state: &PlatformState,
        namespace: String,
        key: &'static str,
    ) -> Result<StorageManagerResponse<bool>, StorageManagerError> {
        debug!("get_bool: namespace={}, key={}", namespace, key);
        let resp = StorageManager::get(state, &namespace, &key.to_string()).await;
        match storage_to_bool_rpc_result(resp) {
            Ok(value) => Ok(StorageManagerResponse::Ok(value)),
            Err(_) => {
                if let Ok(value) = DefaultStorageProperties::get_bool(state, &namespace, &key) {
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
        if let Ok(payload) = StorageManager::get(state, &namespace, &key).await {
            if let DabResponsePayload::StorageData(storage_data) = payload {
                if storage_data.value.eq(&value) {
                    return Ok(StorageManagerResponse::NoChange(()));
                }
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
            .services
            .send_dab(DabRequestPayload::Storage(StorageRequest::Set(ssp)))
            .await
        {
            Ok(_) => {
                if let Some(events) = event_names {
                    let val = Value::from(value.clone());
                    for event in events.iter() {
                        let state_for_event = state.clone();
                        let result = val.clone();
                        let ctx = context.clone();
                        let evt = event.clone();
                        tokio::spawn(async move {
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
                if let Ok(value) = DefaultStorageProperties::get_string(state, &namespace, &key) {
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
                    DefaultStorageProperties::get_number_as_u32(state, &namespace, &key)
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
                DefaultStorageProperties::get_number_as_f32(state, &namespace, &key)
                    .map_or(Err(StorageManagerError::NotFound), |val| {
                        Ok(StorageManagerResponse::Ok(val))
                    })
            },
            |val| Ok(StorageManagerResponse::Ok(val)),
        )
    }

    async fn get(
        state: &PlatformState,
        namespace: &String,
        key: &String,
    ) -> Result<DabResponsePayload, RippleError> {
        debug!("get: namespace={}, key={}", namespace, key);
        let data = GetStorageProperty {
            namespace: namespace.clone(),
            key: key.clone(),
        };
        state
            .services
            .send_dab(DabRequestPayload::Storage(StorageRequest::Get(data)))
            .await
    }

    pub fn get_firebolt_error(property: &StorageProperty) -> jsonrpsee::core::Error {
        let data = property.as_data();
        jsonrpsee::core::Error::Call(CallError::Custom {
            code: CAPABILITY_NOT_AVAILABLE,
            message: format!("{}.{} is not available", data.namespace, data.key),
            data: None,
        })
    }
}

<<<<<<< HEAD
=======
#[cfg(test)]
mod tests {
    use chrono::Utc;
    use dab::core::{
        message::{DabError, DabRequest, DabResponsePayload},
        model::persistent_store::{SetStorageProperty, StorageData},
    };
    use serde_json::{json, Value};
    use tokio::sync::mpsc::channel;

    use crate::{
        helpers::ripple_helper::RippleHelper,
        managers::storage::{
            storage_manager::{StorageManager, StorageManagerError, StorageManagerResponse},
            storage_property::{
                StorageProperty, KEY_ENABLE_RECOMMENDATIONS, KEY_FONT_OPACITY, KEY_FONT_SIZE,
                KEY_NAME, NAMESPACE_PRIVACY,
            },
        },
        platform_state::PlatformState,
    };

    #[tokio::test]
    async fn test_get_bool_default() {
        let mut rh = Box::new(RippleHelper::default());
        let (dab_tx, mut dab_rx) = channel::<DabRequest>(1);
        rh.sender_hub.dab_tx = Some(dab_tx.clone());
        tokio::spawn(async move {
            while let Some(req) = dab_rx.recv().await {
                match &req.payload {
                    dab::core::message::DabRequestPayload::Storage(sreq) => match &sreq {
                        dab::core::model::persistent_store::StorageRequest::Get(key) => {
                            match key.key.as_str() {
                                _ => req.respond_and_log(Err(DabError::NotSupported)),
                            }
                        }
                        dab::core::model::persistent_store::StorageRequest::Set(_) => todo!(),
                    },
                    _ => panic!(),
                }
            }
        });

        let mut state = PlatformState::default();
        state.services.sender_hub.dab_tx = Some(dab_tx);
        let resp = StorageManager::get_bool(&state, StorageProperty::EnableRecommendations).await;
        assert!(matches!(resp, Ok(false)));
    }

    #[tokio::test]
    async fn test_get_bool() {
        let mut rh = Box::new(RippleHelper::default());
        let (dab_tx, mut dab_rx) = channel::<DabRequest>(1);
        rh.sender_hub.dab_tx = Some(dab_tx.clone());
        tokio::spawn(async move {
            while let Some(req) = dab_rx.recv().await {
                match &req.payload {
                    dab::core::message::DabRequestPayload::Storage(sreq) => match &sreq {
                        dab::core::model::persistent_store::StorageRequest::Get(key) => {
                            match key.key.as_str() {
                                KEY_ENABLE_RECOMMENDATIONS => {
                                    req.respond_and_log(Ok(DabResponsePayload::StorageData(
                                        StorageData::new(Value::Bool(true)),
                                    )))
                                }
                                _ => req.respond_and_log(Err(DabError::NotSupported)),
                            }
                        }
                        dab::core::model::persistent_store::StorageRequest::Set(_) => todo!(),
                    },
                    _ => panic!(),
                }
            }
        });

        let mut state = PlatformState::default();
        state.services.sender_hub.dab_tx = Some(dab_tx);
        let resp = StorageManager::get_bool(&state, StorageProperty::EnableRecommendations).await;
        assert!(matches!(resp, Ok(true)));
    }

    #[tokio::test]
    async fn test_get_bool_no_change() {
        let mut rh = Box::new(RippleHelper::default());
        let (dab_tx, mut dab_rx) = channel::<DabRequest>(1);
        rh.sender_hub.dab_tx = Some(dab_tx.clone());
        tokio::spawn(async move {
            let mut set_storage_property = SetStorageProperty {
                namespace: String::from("SomeNamespace"),
                key: String::from("SomeKey"),
                data: StorageData::new(Value::Bool(false)),
            };

            while let Some(req) = dab_rx.recv().await {
                match &req.payload {
                    dab::core::message::DabRequestPayload::Storage(sreq) => match &sreq {
                        dab::core::model::persistent_store::StorageRequest::Get(key) => {
                            if key.namespace.eq(&set_storage_property.namespace) {
                                if key.key.eq(&set_storage_property.key) {
                                    req.respond_and_log(Ok(DabResponsePayload::StorageData(
                                        set_storage_property.data.clone(),
                                    )))
                                } else {
                                    req.respond_and_log(Err(DabError::NotSupported))
                                }
                            }
                        }
                        dab::core::model::persistent_store::StorageRequest::Set(sreq) => {
                            set_storage_property = sreq.clone();
                            req.respond_and_log(Ok(DabResponsePayload::None))
                        }
                    },
                    _ => panic!(),
                }
            }
        });

        let mut state = PlatformState::default();
        state.services.sender_hub.dab_tx = Some(dab_tx);
        let resp = StorageManager::set_in_namespace(
            &state,
            String::from("SomeNamespace"),
            String::from("SomeKey"),
            Value::Bool(true),
            None,
            None,
        )
        .await;

        assert!(matches!(resp, Ok(_)));

        let resp = StorageManager::set_in_namespace(
            &state,
            String::from("SomeNamespace"),
            String::from("SomeKey"),
            Value::Bool(true),
            None,
            None,
        )
        .await;
        assert!(matches!(resp, Ok(StorageManagerResponse::NoChange(()))));
    }

    #[tokio::test]
    async fn test_get_bool_from_namespace_invalid_namespace() {
        let mut rh = Box::new(RippleHelper::default());
        let (dab_tx, mut dab_rx) = channel::<DabRequest>(1);
        rh.sender_hub.dab_tx = Some(dab_tx.clone());
        tokio::spawn(async move {
            while let Some(req) = dab_rx.recv().await {
                match &req.payload {
                    dab::core::message::DabRequestPayload::Storage(sreq) => match &sreq {
                        dab::core::model::persistent_store::StorageRequest::Get(key) => {
                            if key.namespace.eq(NAMESPACE_PRIVACY) {
                                match key.key.as_str() {
                                    KEY_ENABLE_RECOMMENDATIONS => {
                                        req.respond_and_log(Ok(DabResponsePayload::JsonValue(
                                            json!(StorageData::new(Value::Bool(true))),
                                        )))
                                    }
                                    _ => req.respond_and_log(Err(DabError::NotSupported)),
                                }
                            } else {
                                req.respond_and_log(Err(DabError::NotSupported))
                            }
                        }
                        dab::core::model::persistent_store::StorageRequest::Set(_) => todo!(),
                    },
                    _ => panic!(),
                }
            }
        });

        let mut state = PlatformState::default();
        state.services.sender_hub.dab_tx = Some(dab_tx);
        let resp = StorageManager::get_bool_from_namespace(
            &state,
            String::from("SomeInvalidNamespace"),
            KEY_ENABLE_RECOMMENDATIONS,
        )
        .await;
        assert!(matches!(resp, Err(StorageManagerError::NotFound)));
    }

    #[tokio::test]
    async fn test_get_bool_from_namespace_invalid_key() {
        let mut rh = Box::new(RippleHelper::default());
        let (dab_tx, mut dab_rx) = channel::<DabRequest>(1);
        rh.sender_hub.dab_tx = Some(dab_tx.clone());
        tokio::spawn(async move {
            while let Some(req) = dab_rx.recv().await {
                match &req.payload {
                    dab::core::message::DabRequestPayload::Storage(sreq) => match &sreq {
                        dab::core::model::persistent_store::StorageRequest::Get(key) => {
                            if key.namespace.eq(NAMESPACE_PRIVACY) {
                                match key.key.as_str() {
                                    KEY_ENABLE_RECOMMENDATIONS => {
                                        req.respond_and_log(Ok(DabResponsePayload::JsonValue(
                                            json!(StorageData::new(Value::Bool(true))),
                                        )))
                                    }
                                    _ => req.respond_and_log(Err(DabError::NotSupported)),
                                }
                            } else {
                                req.respond_and_log(Err(DabError::NotSupported))
                            }
                        }
                        dab::core::model::persistent_store::StorageRequest::Set(_) => todo!(),
                    },
                    _ => panic!(),
                }
            }
        });

        let mut state = PlatformState::default();
        state.services.sender_hub.dab_tx = Some(dab_tx);
        let resp = StorageManager::get_bool_from_namespace(
            &state,
            String::from(NAMESPACE_PRIVACY),
            "SomeInvalidKey",
        )
        .await;
        assert!(matches!(resp, Err(StorageManagerError::NotFound)));
    }

    #[tokio::test]
    async fn test_set_bool_in_namespace_changed() {
        let mut rh = Box::new(RippleHelper::default());
        let (dab_tx, mut dab_rx) = channel::<DabRequest>(1);
        rh.sender_hub.dab_tx = Some(dab_tx.clone());
        tokio::spawn(async move {
            let mut set_storage_property = SetStorageProperty {
                namespace: String::from("SomeNamespace"),
                key: String::from("SomeKey"),
                data: StorageData::new(Value::Bool(false)),
            };

            while let Some(req) = dab_rx.recv().await {
                match &req.payload {
                    dab::core::message::DabRequestPayload::Storage(sreq) => match &sreq {
                        dab::core::model::persistent_store::StorageRequest::Get(key) => {
                            if key.namespace.eq(&set_storage_property.namespace) {
                                if key.key.eq(&set_storage_property.key) {
                                    req.respond_and_log(Ok(DabResponsePayload::StorageData(
                                        set_storage_property.data.clone(),
                                    )))
                                } else {
                                    req.respond_and_log(Err(DabError::NotSupported))
                                }
                            }
                        }
                        dab::core::model::persistent_store::StorageRequest::Set(sreq) => {
                            set_storage_property = sreq.clone();
                            req.respond_and_log(Ok(DabResponsePayload::None))
                        }
                    },
                    _ => panic!(),
                }
            }
        });

        let mut state = PlatformState::default();
        state.services.sender_hub.dab_tx = Some(dab_tx);
        let resp = StorageManager::set_in_namespace(
            &state,
            String::from("SomeNamespace"),
            String::from("SomeKey"),
            Value::Bool(true),
            None,
            None,
        )
        .await;

        assert!(matches!(resp, Ok(StorageManagerResponse::Ok(_))));

        let resp = StorageManager::get_bool_from_namespace(
            &state,
            String::from("SomeNamespace"),
            "SomeKey",
        )
        .await;
        assert!(matches!(resp, Ok(StorageManagerResponse::Ok(true))));
    }

    #[tokio::test]
    async fn test_set_bool_in_namespace_unchanged() {
        let mut rh = Box::new(RippleHelper::default());
        let (dab_tx, mut dab_rx) = channel::<DabRequest>(1);
        rh.sender_hub.dab_tx = Some(dab_tx.clone());
        tokio::spawn(async move {
            let mut set_storage_property = SetStorageProperty {
                namespace: String::from("SomeNamespace"),
                key: String::from("SomeKey"),
                data: StorageData::new(Value::Bool(true)),
            };

            while let Some(req) = dab_rx.recv().await {
                match &req.payload {
                    dab::core::message::DabRequestPayload::Storage(sreq) => match &sreq {
                        dab::core::model::persistent_store::StorageRequest::Get(key) => {
                            if key.namespace.eq(&set_storage_property.namespace) {
                                if key.key.eq(&set_storage_property.key) {
                                    req.respond_and_log(Ok(DabResponsePayload::StorageData(
                                        set_storage_property.data.clone(),
                                    )))
                                } else {
                                    req.respond_and_log(Err(DabError::NotSupported))
                                }
                            }
                        }
                        dab::core::model::persistent_store::StorageRequest::Set(sreq) => {
                            set_storage_property = sreq.clone();
                            req.respond_and_log(Ok(DabResponsePayload::None))
                        }
                    },
                    _ => panic!(),
                }
            }
        });

        let mut state = PlatformState::default();
        state.services.sender_hub.dab_tx = Some(dab_tx);
        let resp = StorageManager::set_in_namespace(
            &state,
            String::from("SomeNamespace"),
            String::from("SomeKey"),
            Value::Bool(true),
            None,
            None,
        )
        .await;

        assert!(matches!(resp, Ok(StorageManagerResponse::NoChange(_))));

        let resp = StorageManager::get_bool_from_namespace(
            &state,
            String::from("SomeNamespace"),
            "SomeKey",
        )
        .await;
        assert!(matches!(resp, Ok(StorageManagerResponse::Ok(true))));
    }

    #[tokio::test]
    async fn test_set_string_in_namespace_changed() {
        let mut rh = Box::new(RippleHelper::default());
        let (dab_tx, mut dab_rx) = channel::<DabRequest>(1);
        rh.sender_hub.dab_tx = Some(dab_tx.clone());
        tokio::spawn(async move {
            let mut set_storage_property = SetStorageProperty {
                namespace: String::from("SomeNamespace"),
                key: String::from("SomeKey"),
                data: StorageData::new(Value::Null),
            };

            while let Some(req) = dab_rx.recv().await {
                match &req.payload {
                    dab::core::message::DabRequestPayload::Storage(sreq) => match &sreq {
                        dab::core::model::persistent_store::StorageRequest::Get(key) => {
                            if key.namespace.eq(&set_storage_property.namespace) {
                                if key.key.eq(&set_storage_property.key) {
                                    req.respond_and_log(Ok(DabResponsePayload::StorageData(
                                        set_storage_property.data.clone(),
                                    )))
                                } else {
                                    req.respond_and_log(Err(DabError::NotSupported))
                                }
                            }
                        }
                        dab::core::model::persistent_store::StorageRequest::Set(sreq) => {
                            set_storage_property = sreq.clone();
                            req.respond_and_log(Ok(DabResponsePayload::None))
                        }
                    },
                    _ => panic!(),
                }
            }
        });

        let value = String::from("SomeValue");

        let mut state = PlatformState::default();
        state.services.sender_hub.dab_tx = Some(dab_tx);
        let resp = StorageManager::set_in_namespace(
            &state,
            String::from("SomeNamespace"),
            String::from("SomeKey"),
            Value::String(value),
            None,
            None,
        )
        .await;

        assert!(matches!(resp, Ok(StorageManagerResponse::Ok(_))));

        let resp = StorageManager::get_string_from_namespace(
            &state,
            String::from("SomeNamespace"),
            "SomeKey",
        )
        .await;
        assert!(matches!(resp, Ok(StorageManagerResponse::Ok(value))));
    }

    #[tokio::test]
    async fn test_set_string_in_namespace_unchanged() {
        let mut rh = Box::new(RippleHelper::default());
        let (dab_tx, mut dab_rx) = channel::<DabRequest>(1);
        rh.sender_hub.dab_tx = Some(dab_tx.clone());
        tokio::spawn(async move {
            let mut set_storage_property = SetStorageProperty {
                namespace: String::from("SomeNamespace"),
                key: String::from("SomeKey"),
                data: StorageData::new(Value::String(String::from("SomeValue"))),
            };

            while let Some(req) = dab_rx.recv().await {
                match &req.payload {
                    dab::core::message::DabRequestPayload::Storage(sreq) => match &sreq {
                        dab::core::model::persistent_store::StorageRequest::Get(key) => {
                            if key.namespace.eq(&set_storage_property.namespace) {
                                if key.key.eq(&set_storage_property.key) {
                                    req.respond_and_log(Ok(DabResponsePayload::StorageData(
                                        set_storage_property.data.clone(),
                                    )))
                                } else {
                                    req.respond_and_log(Err(DabError::NotSupported))
                                }
                            }
                        }
                        dab::core::model::persistent_store::StorageRequest::Set(sreq) => {
                            set_storage_property = sreq.clone();
                            req.respond_and_log(Ok(DabResponsePayload::None))
                        }
                    },
                    _ => panic!(),
                }
            }
        });

        let value = String::from("SomeValue");

        let mut state = PlatformState::default();
        state.services.sender_hub.dab_tx = Some(dab_tx);
        let resp = StorageManager::set_in_namespace(
            &state,
            String::from("SomeNamespace"),
            String::from("SomeKey"),
            Value::String(value),
            None,
            None,
        )
        .await;

        assert!(matches!(resp, Ok(StorageManagerResponse::NoChange(_))));

        let resp = StorageManager::get_string_from_namespace(
            &state,
            String::from("SomeNamespace"),
            "SomeKey",
        )
        .await;
        assert!(matches!(resp, Ok(StorageManagerResponse::Ok(value))));
    }

    #[tokio::test]
    async fn test_set_number_in_namespace_changed() {
        let mut rh = Box::new(RippleHelper::default());
        let (dab_tx, mut dab_rx) = channel::<DabRequest>(1);
        rh.sender_hub.dab_tx = Some(dab_tx.clone());
        tokio::spawn(async move {
            let mut set_storage_property = SetStorageProperty {
                namespace: String::from("SomeNamespace"),
                key: String::from("SomeKey"),
                data: StorageData::new(Value::Null),
            };

            while let Some(req) = dab_rx.recv().await {
                match &req.payload {
                    dab::core::message::DabRequestPayload::Storage(sreq) => match &sreq {
                        dab::core::model::persistent_store::StorageRequest::Get(key) => {
                            if key.namespace.eq(&set_storage_property.namespace) {
                                if key.key.eq(&set_storage_property.key) {
                                    req.respond_and_log(Ok(DabResponsePayload::StorageData(
                                        set_storage_property.data.clone(),
                                    )))
                                } else {
                                    req.respond_and_log(Err(DabError::NotSupported))
                                }
                            }
                        }
                        dab::core::model::persistent_store::StorageRequest::Set(sreq) => {
                            set_storage_property = sreq.clone();
                            req.respond_and_log(Ok(DabResponsePayload::None))
                        }
                    },
                    _ => panic!(),
                }
            }
        });

        let mut state = PlatformState::default();
        state.services.sender_hub.dab_tx = Some(dab_tx);
        let resp = StorageManager::set_in_namespace(
            &state,
            String::from("SomeNamespace"),
            String::from("SomeKey"),
            json!(1),
            None,
            None,
        )
        .await;

        assert!(matches!(resp, Ok(StorageManagerResponse::Ok(_))));

        let resp = StorageManager::get_number_as_u32_from_namespace(
            &state,
            String::from("SomeNamespace"),
            "SomeKey",
        )
        .await;
        assert!(matches!(resp, Ok(StorageManagerResponse::Ok(1))));
    }

    #[tokio::test]
    async fn test_set_number_in_namespace_unchanged() {
        let mut rh = Box::new(RippleHelper::default());
        let (dab_tx, mut dab_rx) = channel::<DabRequest>(1);
        rh.sender_hub.dab_tx = Some(dab_tx.clone());
        tokio::spawn(async move {
            let mut set_storage_property = SetStorageProperty {
                namespace: String::from("SomeNamespace"),
                key: String::from("SomeKey"),
                data: StorageData::new(json!(1 as u32)),
            };

            while let Some(req) = dab_rx.recv().await {
                match &req.payload {
                    dab::core::message::DabRequestPayload::Storage(sreq) => match &sreq {
                        dab::core::model::persistent_store::StorageRequest::Get(key) => {
                            if key.namespace.eq(&set_storage_property.namespace) {
                                if key.key.eq(&set_storage_property.key) {
                                    req.respond_and_log(Ok(DabResponsePayload::StorageData(
                                        set_storage_property.data.clone(),
                                    )))
                                } else {
                                    req.respond_and_log(Err(DabError::NotSupported))
                                }
                            }
                        }
                        dab::core::model::persistent_store::StorageRequest::Set(sreq) => {
                            set_storage_property = sreq.clone();
                            req.respond_and_log(Ok(DabResponsePayload::None))
                        }
                    },
                    _ => panic!(),
                }
            }
        });

        let mut state = PlatformState::default();
        state.services.sender_hub.dab_tx = Some(dab_tx);
        let resp = StorageManager::set_in_namespace(
            &state,
            String::from("SomeNamespace"),
            String::from("SomeKey"),
            //value: Value::Number(serde_json::Number::from_f64(1.0).unwrap())
            json!(1 as u32),
            None,
            None,
        )
        .await;

        assert!(matches!(resp, Ok(StorageManagerResponse::NoChange(_))));

        let resp = StorageManager::get_number_as_u32_from_namespace(
            &state,
            String::from("SomeNamespace"),
            "SomeKey",
        )
        .await;
        assert!(matches!(resp, Ok(StorageManagerResponse::Ok(1))));
    }

    #[tokio::test]
    async fn test_get_string_default() {
        let mut rh = Box::new(RippleHelper::default());
        let (dab_tx, mut dab_rx) = channel::<DabRequest>(1);
        rh.sender_hub.dab_tx = Some(dab_tx.clone());
        tokio::spawn(async move {
            while let Some(req) = dab_rx.recv().await {
                match &req.payload {
                    dab::core::message::DabRequestPayload::Storage(sreq) => match &sreq {
                        dab::core::model::persistent_store::StorageRequest::Get(key) => {
                            match key.key.as_str() {
                                _ => req.respond_and_log(Err(DabError::NotSupported)),
                            }
                        }
                        dab::core::model::persistent_store::StorageRequest::Set(_) => todo!(),
                    },
                    _ => panic!(),
                }
            }
        });

        let mut state = PlatformState::default();
        state.services.sender_hub.dab_tx = Some(dab_tx);
        let resp = StorageManager::get_string(&state, StorageProperty::DeviceName).await;
        let expected = String::from("Living Room");
        assert!(matches!(resp, Ok(expected)));
    }

    #[tokio::test]
    async fn test_get_string() {
        let mut rh = Box::new(RippleHelper::default());
        let (dab_tx, mut dab_rx) = channel::<DabRequest>(1);
        rh.sender_hub.dab_tx = Some(dab_tx.clone());
        tokio::spawn(async move {
            while let Some(req) = dab_rx.recv().await {
                match &req.payload {
                    dab::core::message::DabRequestPayload::Storage(sreq) => match &sreq {
                        dab::core::model::persistent_store::StorageRequest::Get(key) => {
                            match key.key.as_str() {
                                KEY_NAME => req.respond_and_log(Ok(DabResponsePayload::JsonValue(
                                    Value::String(String::from("SomeDeviceName")),
                                ))),
                                _ => req.respond_and_log(Err(DabError::NotSupported)),
                            }
                        }
                        dab::core::model::persistent_store::StorageRequest::Set(_) => todo!(),
                    },
                    _ => panic!(),
                }
            }
        });

        let mut state = PlatformState::default();
        state.services.sender_hub.dab_tx = Some(dab_tx);
        let resp = StorageManager::get_string(&state, StorageProperty::DeviceName).await;
        let expected = String::from("SomeDeviceName");
        assert!(matches!(resp, Ok(expected)));
    }

    #[tokio::test]
    async fn test_get_string_from_namespace_invalid_namespace() {
        let mut rh = Box::new(RippleHelper::default());
        let (dab_tx, mut dab_rx) = channel::<DabRequest>(1);
        rh.sender_hub.dab_tx = Some(dab_tx.clone());
        tokio::spawn(async move {
            while let Some(req) = dab_rx.recv().await {
                match &req.payload {
                    dab::core::message::DabRequestPayload::Storage(sreq) => match &sreq {
                        dab::core::model::persistent_store::StorageRequest::Get(key) => {
                            if key.namespace.eq(NAMESPACE_PRIVACY) {
                                match key.key.as_str() {
                                    KEY_ENABLE_RECOMMENDATIONS => {
                                        req.respond_and_log(Ok(DabResponsePayload::JsonValue(
                                            Value::String(String::from("SomeString")),
                                        )))
                                    }
                                    _ => req.respond_and_log(Err(DabError::NotSupported)),
                                }
                            } else {
                                req.respond_and_log(Err(DabError::NotSupported))
                            }
                        }
                        dab::core::model::persistent_store::StorageRequest::Set(_) => todo!(),
                    },
                    _ => panic!(),
                }
            }
        });

        let mut state = PlatformState::default();
        state.services.sender_hub.dab_tx = Some(dab_tx);
        let resp = StorageManager::get_string_from_namespace(
            &state,
            String::from("SomeInvalidNamespace"),
            KEY_NAME,
        )
        .await;
        assert!(matches!(resp, Err(StorageManagerError::NotFound)));
    }

    #[tokio::test]
    async fn test_get_string_from_namespace_invalid_key() {
        let mut rh = Box::new(RippleHelper::default());
        let (dab_tx, mut dab_rx) = channel::<DabRequest>(1);
        rh.sender_hub.dab_tx = Some(dab_tx.clone());
        tokio::spawn(async move {
            while let Some(req) = dab_rx.recv().await {
                match &req.payload {
                    dab::core::message::DabRequestPayload::Storage(sreq) => match &sreq {
                        dab::core::model::persistent_store::StorageRequest::Get(key) => {
                            if key.namespace.eq(NAMESPACE_PRIVACY) {
                                match key.key.as_str() {
                                    KEY_ENABLE_RECOMMENDATIONS => {
                                        req.respond_and_log(Ok(DabResponsePayload::JsonValue(
                                            Value::String(String::from("SomeString")),
                                        )))
                                    }
                                    _ => req.respond_and_log(Err(DabError::NotSupported)),
                                }
                            } else {
                                req.respond_and_log(Err(DabError::NotSupported))
                            }
                        }
                        dab::core::model::persistent_store::StorageRequest::Set(_) => todo!(),
                    },
                    _ => panic!(),
                }
            }
        });
        let mut state = PlatformState::default();
        state.services.sender_hub.dab_tx = Some(dab_tx);
        let resp = StorageManager::get_string_from_namespace(
            &state,
            String::from(NAMESPACE_PRIVACY),
            "SomeInvalidKey",
        )
        .await;
        assert!(matches!(resp, Err(StorageManagerError::NotFound)));
    }

    #[tokio::test]
    async fn test_get_number_default() {
        let mut rh = Box::new(RippleHelper::default());
        let (dab_tx, mut dab_rx) = channel::<DabRequest>(1);
        rh.sender_hub.dab_tx = Some(dab_tx.clone());
        tokio::spawn(async move {
            while let Some(req) = dab_rx.recv().await {
                match &req.payload {
                    dab::core::message::DabRequestPayload::Storage(sreq) => match &sreq {
                        dab::core::model::persistent_store::StorageRequest::Get(key) => {
                            match key.key.as_str() {
                                _ => req.respond_and_log(Err(DabError::NotSupported)),
                            }
                        }
                        dab::core::model::persistent_store::StorageRequest::Set(_) => todo!(),
                    },
                    _ => panic!(),
                }
            }
        });

        let mut state = PlatformState::default();
        state.services.sender_hub.dab_tx = Some(dab_tx);
        let resp =
            StorageManager::get_number_as_u32(&state, StorageProperty::ClosedCaptionsFontOpacity)
                .await;
        assert!(matches!(resp, Ok(val) if val == 100));
    }

    #[tokio::test]
    async fn test_get_number_from_namespace_invalid_namespace() {
        let mut rh = Box::new(RippleHelper::default());
        let (dab_tx, mut dab_rx) = channel::<DabRequest>(1);
        rh.sender_hub.dab_tx = Some(dab_tx.clone());
        tokio::spawn(async move {
            while let Some(req) = dab_rx.recv().await {
                match &req.payload {
                    dab::core::message::DabRequestPayload::Storage(sreq) => match &sreq {
                        dab::core::model::persistent_store::StorageRequest::Get(key) => {
                            if key.namespace.eq(NAMESPACE_PRIVACY) {
                                match key.key.as_str() {
                                    KEY_ENABLE_RECOMMENDATIONS => {
                                        req.respond_and_log(Ok(DabResponsePayload::JsonValue(
                                            Value::String(String::from("SomeString")),
                                        )))
                                    }
                                    _ => req.respond_and_log(Err(DabError::NotSupported)),
                                }
                            } else {
                                req.respond_and_log(Err(DabError::NotSupported))
                            }
                        }
                        dab::core::model::persistent_store::StorageRequest::Set(_) => todo!(),
                    },
                    _ => panic!(),
                }
            }
        });

        let mut state = PlatformState::default();
        state.services.sender_hub.dab_tx = Some(dab_tx);
        let resp = StorageManager::get_string_from_namespace(
            &state,
            String::from("SomeInvalidNamespace"),
            KEY_NAME,
        )
        .await;
        assert!(matches!(resp, Err(StorageManagerError::NotFound)));
    }

    #[tokio::test]
    async fn test_get_number_from_namespace_invalid_key() {
        let mut rh = Box::new(RippleHelper::default());
        let (dab_tx, mut dab_rx) = channel::<DabRequest>(1);
        rh.sender_hub.dab_tx = Some(dab_tx.clone());
        tokio::spawn(async move {
            while let Some(req) = dab_rx.recv().await {
                match &req.payload {
                    dab::core::message::DabRequestPayload::Storage(sreq) => match &sreq {
                        dab::core::model::persistent_store::StorageRequest::Get(key) => {
                            if key.namespace.eq(NAMESPACE_PRIVACY) {
                                match key.key.as_str() {
                                    _ => req.respond_and_log(Err(DabError::NotSupported)),
                                }
                            } else {
                                req.respond_and_log(Err(DabError::NotSupported))
                            }
                        }
                        dab::core::model::persistent_store::StorageRequest::Set(_) => todo!(),
                    },
                    _ => panic!(),
                }
            }
        });
        let mut state = PlatformState::default();
        state.services.sender_hub.dab_tx = Some(dab_tx);
        let resp = StorageManager::get_number_as_u32_from_namespace(
            &state,
            String::from(NAMESPACE_PRIVACY),
            "SomeInvalidKey",
        )
        .await;
        assert!(matches!(resp, Err(StorageManagerError::NotFound)));
    }

    #[tokio::test]
    async fn test_get_number() {
        let mut rh = Box::new(RippleHelper::default());
        let (dab_tx, mut dab_rx) = channel::<DabRequest>(1);
        rh.sender_hub.dab_tx = Some(dab_tx.clone());
        tokio::spawn(async move {
            while let Some(req) = dab_rx.recv().await {
                match &req.payload {
                    dab::core::message::DabRequestPayload::Storage(sreq) => match &sreq {
                        dab::core::model::persistent_store::StorageRequest::Get(key) => {
                            match key.key.as_str() {
                                KEY_FONT_OPACITY => {
                                    let data = StorageData::new(json!(20));
                                    req.respond_and_log(Ok(DabResponsePayload::StorageData(data)))
                                }
                                _ => req.respond_and_log(Err(DabError::NotSupported)),
                            }
                        }
                        dab::core::model::persistent_store::StorageRequest::Set(_) => todo!(),
                    },
                    _ => panic!(),
                }
            }
        });

        let mut state = PlatformState::default();
        state.services.sender_hub.dab_tx = Some(dab_tx);
        let resp =
            StorageManager::get_number_as_u32(&state, StorageProperty::ClosedCaptionsFontOpacity)
                .await;
        assert!(matches!(resp, Ok(val) if val == 20));
    }

    #[tokio::test]
    async fn test_get_number_as_f32_value_stored() {
        let mut rh = Box::new(RippleHelper::default());
        let (dab_tx, mut dab_rx) = channel::<DabRequest>(1);
        rh.sender_hub.dab_tx = Some(dab_tx.clone());
        tokio::spawn(async move {
            while let Some(req) = dab_rx.recv().await {
                match &req.payload {
                    dab::core::message::DabRequestPayload::Storage(sreq) => match &sreq {
                        dab::core::model::persistent_store::StorageRequest::Get(key) => {
                            match key.key.as_str() {
                                KEY_FONT_SIZE => req.respond_and_log(Ok(
                                    DabResponsePayload::StorageData(StorageData::new(json!(1))),
                                )),
                                _ => req.respond_and_log(Err(DabError::NotSupported)),
                            }
                        }
                        dab::core::model::persistent_store::StorageRequest::Set(_) => todo!(),
                    },
                    _ => panic!(),
                }
            }
        });

        let mut state = PlatformState::default();
        state.services.sender_hub.dab_tx = Some(dab_tx);
        let resp =
            StorageManager::get_number_as_f32(&state, StorageProperty::ClosedCaptionsFontSize)
                .await;
        let expected_val = 1.0;
        assert!(matches!(resp, Ok(val) if val == expected_val));
    }

    #[tokio::test]
    async fn test_get_number_as_f32_value_not_stored() {
        let mut rh = Box::new(RippleHelper::default());
        let (dab_tx, mut dab_rx) = channel::<DabRequest>(1);
        rh.sender_hub.dab_tx = Some(dab_tx.clone());
        tokio::spawn(async move {
            while let Some(req) = dab_rx.recv().await {
                match &req.payload {
                    dab::core::message::DabRequestPayload::Storage(sreq) => match &sreq {
                        dab::core::model::persistent_store::StorageRequest::Get(key) => {
                            match key.key.as_str() {
                                _ => req.respond_and_log(Err(DabError::NotSupported)),
                            }
                        }
                        dab::core::model::persistent_store::StorageRequest::Set(_) => todo!(),
                    },
                    _ => panic!(),
                }
            }
        });

        let mut state = PlatformState::default();
        state.services.sender_hub.dab_tx = Some(dab_tx);
        let resp =
            StorageManager::get_number_as_f32(&state, StorageProperty::ClosedCaptionsFontSize)
                .await;
        let expected_val = 1.0;
        assert!(matches!(resp, Ok(val) if val == expected_val));
    }

    #[tokio::test]
    async fn test_invalid_type_request() {
        let mut rh = Box::new(RippleHelper::default());
        let (dab_tx, mut dab_rx) = channel::<DabRequest>(1);
        rh.sender_hub.dab_tx = Some(dab_tx.clone());
        tokio::spawn(async move {
            while let Some(req) = dab_rx.recv().await {
                match &req.payload {
                    dab::core::message::DabRequestPayload::Storage(sreq) => match &sreq {
                        dab::core::model::persistent_store::StorageRequest::Get(key) => {
                            match key.key.as_str() {
                                KEY_NAME => {
                                    req.respond_and_log(Ok(DabResponsePayload::JsonValue(
                                        Value::Bool(true),
                                    )));
                                }
                                _ => req.respond_and_log(Err(DabError::NotSupported)),
                            }
                        }
                        dab::core::model::persistent_store::StorageRequest::Set(_) => todo!(),
                    },
                    _ => panic!(),
                }
            }
        });

        let mut state = PlatformState::default();
        state.services.sender_hub.dab_tx = Some(dab_tx);
        let resp = StorageManager::get_string(&state, StorageProperty::DeviceName).await;
        assert!(matches!(resp, Ok(val) if val == "Living Room"));
    }
}
>>>>>>> 7d3d9b6 (Base for accessibility.)
