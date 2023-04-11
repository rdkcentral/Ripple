use crate::{
    client::thunder_client::{ThunderClient, ThunderParams},
    gateway::thunder_plugin::ThunderPlugin,
    service::thunder_service_resolver::{ThunderDelegate, ThunderOperator},
};
use async_trait::async_trait;
use dab_core::{
    message::{DabError, DabRequest, DabResponsePayload},
    model::persistent_store::{
        GetStorageProperty, SetStorageProperty, StorageData, StorageRequest, StorageService,
    },
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tracing::{error, info_span, Instrument};

pub struct ThunderStorageService {
    pub client: Box<ThunderClient>,
}

#[derive(Serialize)]
pub struct ThunderSetValueRequest {
    namespace: String,
    key: String,
    value: String,
}

#[derive(Serialize)]
pub struct ThunderGetValueRequest {
    namespace: String,
    key: String,
}

#[derive(Deserialize)]
pub struct ThunderGetValueResponse {
    success: bool,
    value: String,
}

#[async_trait]
impl StorageService for ThunderStorageService {
    async fn delete_key(self: Box<Self>, namespace: String, key: String) -> bool {
        let thunder_method = ThunderPlugin::PersistentStorage.method("deleteKey");
        let client = self.client.clone();
        let params = Some(ThunderParams::Json(
            json!({
                "namespace": namespace,
                "key": key,
            })
            .to_string(),
        ));
        let span = info_span!("delete_key");
        let response = client
            .call_thunder(&thunder_method, params, Some(span))
            .await;

        if response.message.get("success").is_none()
            || response.message["success"].as_bool().unwrap() == false
        {
            error!("{}", response.message);
            return false;
        }
        true
    }

    async fn delete_namespace(self: Box<Self>, namespace: String) -> bool {
        let thunder_method = ThunderPlugin::PersistentStorage.method("deleteNamespace");
        let client = self.client.clone();
        let params = Some(ThunderParams::Json(
            json!({
                "namespace": namespace,
            })
            .to_string(),
        ));
        let span = info_span!("delete_namespace");
        let response = client
            .call_thunder(&thunder_method, params, Some(span))
            .await;

        if response.message.get("success").is_none()
            || response.message["success"].as_bool().unwrap() == false
        {
            error!("{}", response.message);
            return false;
        }
        true
    }

    async fn flush_cache(self: Box<Self>) -> bool {
        let thunder_method = ThunderPlugin::PersistentStorage.method("flushCache");
        let client = self.client.clone();
        let span = info_span!("flush_cache");
        let response = client.call_thunder(&thunder_method, None, Some(span)).await;

        if response.message.get("success").is_none()
            || response.message["success"].as_bool().unwrap() == false
        {
            error!("{}", response.message);
            return false;
        }
        true
    }

    async fn get_value(self: Box<Self>, request: DabRequest, data: GetStorageProperty) {
        let parent_span = match request.parent_span.clone() {
            Some(s) => s,
            None => info_span!("storage get value"),
        };
        let thunder_method = ThunderPlugin::PersistentStorage.method("getValue");
        let client = self.client.clone();
        let params = Some(ThunderParams::Json(
            json!(ThunderGetValueRequest {
                namespace: data.namespace,
                key: data.key.clone(),
            })
            .to_string(),
        ));
        let response = client
            .call_thunder(&thunder_method, params, Some(parent_span))
            .await;
        let value_resp_res = serde_json::from_value(response.message);
        if let Err(_) = value_resp_res {
            request.respond_and_log(Err(DabError::OsError));
            return;
        }
        let value_resp: ThunderGetValueResponse = value_resp_res.unwrap();
        if !value_resp.success {
            request.respond_and_log(Err(DabError::OsError));
            return;
        }
        let parsed_res: Result<Value, serde_json::Error> = serde_json::from_str(&value_resp.value);
        if let Err(_) = parsed_res {
            error!(
                "Invalid json {} stored at key {}",
                value_resp.value, data.key
            );
            request.respond_and_log(Err(DabError::OsError));
            return;
        }
        let value = parsed_res.unwrap();
        let has_storage_data: Result<StorageData, serde_json::Error> =
            serde_json::from_value(value.clone());
        match has_storage_data {
            Ok(storage_data) => {
                request.respond_and_log(Ok(DabResponsePayload::StorageData(storage_data)))
            }
            Err(_) => {
                // Stored value predates StorageData implementation, return Value.
                request.respond_and_log(Ok(DabResponsePayload::JsonValue(value)))
            }
        }
    }

    async fn set_value(self: Box<Self>, request: DabRequest, ssp: SetStorageProperty) {
        let parent_span = match request.parent_span.clone() {
            Some(s) => s,
            None => info_span!("storage set value"),
        };
        let thunder_method = ThunderPlugin::PersistentStorage.method("setValue");
        let client = self.client.clone();
        let json_opt = serde_json::to_string(&ssp.data);
        if let Err(_) = json_opt {
            request.respond_and_log(Err(DabError::OsError));
            return;
        }
        let params = Some(ThunderParams::Json(
            json!(ThunderSetValueRequest {
                namespace: ssp.namespace,
                key: ssp.key,
                value: json_opt.unwrap(),
            })
            .to_string(),
        ));
        let response = client
            .call_thunder(&thunder_method, params, Some(parent_span))
            .await;

        if response.message.get("success").is_none()
            || response.message["success"].as_bool().unwrap() == false
        {
            request.respond_and_log(Err(DabError::OsError));
            return;
        }
        request.respond_and_log(Ok(DabResponsePayload::JsonValue(json!(true))));
    }
}

pub struct StorageDelegate;

#[async_trait]
impl ThunderDelegate for StorageDelegate {
    async fn handle(&self, request: DabRequest, client: Box<ThunderClient>) {
        let service = Box::new(ThunderStorageService { client: client });
        match request.payload.as_storage_request() {
            Some(req) => match req {
                StorageRequest::Get(data) => {
                    let span = request.req_span("StorageRequest::Get");
                    service.get_value(request, data).instrument(span).await;
                }
                StorageRequest::Set(data) => {
                    let span = request.req_span("StorageRequest::Set");
                    service.set_value(request, data).instrument(span).await;
                }
            },
            None => error!("Invalid DAB payload for Storage module"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        client::thunder_client::{ThunderCallMessage, ThunderResponseMessage},
        test::{mock_thunder_controller::CustomHandler, test_utils::ThunderDelegateTest},
        util::channel_util::oneshot_send_and_log,
    };
    use dab_core::{
        chrono::Utc,
        message::{DabRequestPayload, DabResponse},
        model::persistent_store::StorageData,
    };
    use serde_json::json;
    use std::collections::HashMap;
    use tokio::sync::oneshot::error::RecvError;

    #[tokio::test]
    async fn test_thunder_persistent_store_set() {
        let mut custom_handler = CustomHandler {
            custom_request_handler: HashMap::new(),
            custom_subscription_handler: HashMap::new(),
        };
        let payload = DabRequestPayload::Storage(StorageRequest::Set(SetStorageProperty {
            namespace: String::from("testns"),
            key: String::from("testkey"),
            data: StorageData::new(serde_json::Value::String(String::from("12345"))),
        }));
        let handle_function = |msg: ThunderCallMessage| {
            let response = json!({"success" : true});
            let resp_msg = ThunderResponseMessage::call(response.clone());
            oneshot_send_and_log(msg.callback, resp_msg, "StatusReturn")
        };

        custom_handler.custom_request_handler.insert(
            String::from("org.rdk.PersistentStore".to_owned() + &"setValue".to_owned()),
            handle_function,
        );
        let delegate = Box::new(StorageDelegate);

        let validator = |res: Result<DabResponse, RecvError>| {
            if let Ok(dab_response) = res {
                assert!(matches!(dab_response, Ok { .. }));
                if let Ok(_payload) = dab_response {}
            }
        };

        let delegate_test = ThunderDelegateTest::new(delegate);
        delegate_test
            .start_test(payload, Some(custom_handler), Box::new(validator))
            .await;
    }

    #[tokio::test]
    async fn test_thunder_persistent_store_get() {
        let mut custom_handler = CustomHandler {
            custom_request_handler: HashMap::new(),
            custom_subscription_handler: HashMap::new(),
        };
        let payload = DabRequestPayload::Storage(StorageRequest::Get(GetStorageProperty {
            namespace: String::from("testns"),
            key: String::from("testkey"),
        }));
        let handle_function = |msg: ThunderCallMessage| {
            let response = json!({"success" : true, "value" : serde_json::Value::String(String::from("12345"))});

            let resp_msg = ThunderResponseMessage::call(response.clone());
            oneshot_send_and_log(msg.callback, resp_msg, "StatusReturn")
        };

        custom_handler.custom_request_handler.insert(
            String::from("org.rdk.PersistentStore".to_owned() + &"getValue".to_owned()),
            handle_function,
        );
        let delegate = Box::new(StorageDelegate);

        let validator = |res: Result<DabResponse, RecvError>| {
            if let Ok(dab_response) = res {
                assert!(matches!(dab_response, Ok { .. }));
                if let Ok(_payload) = dab_response {}
            }
        };

        let delegate_test = ThunderDelegateTest::new(delegate);
        delegate_test
            .start_test(payload, Some(custom_handler), Box::new(validator))
            .await;
    }

    #[tokio::test]
    async fn test_thunder_persistent_store_get_none() {
        let mut custom_handler = CustomHandler {
            custom_request_handler: HashMap::new(),
            custom_subscription_handler: HashMap::new(),
        };
        let payload = DabRequestPayload::Storage(StorageRequest::Get(GetStorageProperty {
            namespace: String::from("testns"),
            key: String::from("none"),
        }));
        let handle_function = |msg: ThunderCallMessage| {
            let response = json!({"success" : false});

            let resp_msg = ThunderResponseMessage::call(response.clone());
            oneshot_send_and_log(msg.callback, resp_msg, "StatusReturn")
        };

        custom_handler.custom_request_handler.insert(
            String::from("org.rdk.PersistentStore".to_owned() + &"getValue".to_owned()),
            handle_function,
        );
        let delegate = Box::new(StorageDelegate);

        let validator = |res: Result<DabResponse, RecvError>| {
            if let Ok(dab_response) = res {
                if let Ok(_payload) = dab_response {}
            }
        };

        let delegate_test = ThunderDelegateTest::new(delegate);
        delegate_test
            .start_test(payload, Some(custom_handler), Box::new(validator))
            .await;
    }
}