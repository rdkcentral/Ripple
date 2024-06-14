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
    client::thunder_plugin::ThunderPlugin,
    ripple_sdk::{
        api::device::{
            device_operator::{DeviceCallRequest, DeviceChannelParams, DeviceOperator},
            device_peristence::{
                DeleteStorageProperty, DevicePersistenceRequest, GetStorageProperty,
                SetStorageProperty,
            },
        },
        async_trait::async_trait,
        extn::{
            client::extn_client::ExtnClient,
            client::extn_processor::{
                DefaultExtnStreamer, ExtnRequestProcessor, ExtnStreamProcessor, ExtnStreamer,
            },
            extn_client_message::{ExtnMessage, ExtnRequest, ExtnResponse},
        },
        log::{debug, error, info},
        serde_json::{self, json, Value},
        tokio::sync::mpsc,
        utils::error::RippleError,
    },
    thunder_state::ThunderState,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct ThunderGetValueResponse {
    success: bool,
    value: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[allow(non_camel_case_types, non_snake_case)]
struct WifiConnectError {
    code: u32,
}

#[derive(Debug)]
pub struct ThunderStorageRequestProcessor {
    state: ThunderState,
    streamer: DefaultExtnStreamer,
}

#[async_trait]
pub trait StorageService {
    async fn delete_key(state: ThunderState, req: ExtnMessage, data: DeleteStorageProperty)
        -> bool;
    async fn delete_namespace(self: Box<Self>, namespace: String) -> bool;
    async fn flush_cache(self: Box<Self>) -> bool;
    // async fn get_keys(self: Box<Self>, namespace: String) -> (Vec<String>, bool);
    // async fn get_namespaces(self: Box<Self>) -> (Vec<String>, bool);
    // async fn get_storage_size(self: Box<Self>) -> (HashMap<String, u32>, bool);
    async fn get_value(state: ThunderState, req: ExtnMessage, data: GetStorageProperty) -> bool;
    async fn set_value(state: ThunderState, req: ExtnRequest, data: SetStorageProperty) -> bool;
}

impl ThunderStorageRequestProcessor {
    pub fn new(state: ThunderState) -> ThunderStorageRequestProcessor {
        ThunderStorageRequestProcessor {
            state,
            streamer: DefaultExtnStreamer::new(),
        }
    }

    #[allow(dead_code)]
    async fn delete_key(
        state: ThunderState,
        req: ExtnMessage,
        data: DeleteStorageProperty,
    ) -> bool {
        let mut params_json = json!({
            "namespace": data.namespace,
            "key": data.key,
        });
        if let Some(scope) = data.scope {
            params_json
                .as_object_mut()
                .unwrap()
                .insert("scope".to_string(), json!(scope));
        }

        let params = Some(DeviceChannelParams::Json(params_json.to_string()));
        let thunder_method = ThunderPlugin::PersistentStorage.method("deleteKey");
        let response = state
            .get_thunder_client()
            .call(DeviceCallRequest {
                method: thunder_method,
                params,
            })
            .await;

        match response.message["success"].as_bool() {
            Some(success) => {
                let response = ExtnResponse::Boolean(success);
                info!("thunder : {:?}", response);
                Self::respond(state.get_client(), req, response)
                    .await
                    .is_ok()
            }
            None => false,
        }
    }

    #[allow(dead_code)]
    async fn delete_namespace(self, namespace: String) -> bool {
        let thunder_method = ThunderPlugin::PersistentStorage.method("deleteNamespace");
        let client = self.state.clone();
        let params = Some(DeviceChannelParams::Json(
            json!({
                "namespace": namespace,
            })
            .to_string(),
        ));
        let response = client
            .get_thunder_client()
            .call(DeviceCallRequest {
                method: thunder_method,
                params,
            })
            .await;
        if response.message.get("success").is_none()
            || !response.message["success"].as_bool().unwrap_or_default()
        {
            error!("{}", response.message);
            return false;
        }
        true
    }

    #[allow(dead_code)]
    async fn flush_cache(self) -> bool {
        let thunder_method = ThunderPlugin::PersistentStorage.method("flushCache");
        let client = self.state.clone();
        let response = client
            .get_thunder_client()
            .call(DeviceCallRequest {
                method: thunder_method,
                params: None,
            })
            .await;

        if response.message.get("success").is_none()
            || !response.message["success"].as_bool().unwrap_or_default()
        {
            error!("{}", response.message);
            return false;
        }
        true
    }

    async fn get_value(state: ThunderState, req: ExtnMessage, data: GetStorageProperty) -> bool {
        let mut params_json = json!({
            "namespace": data.namespace,
            "key": data.key,
        });
        if let Some(scope) = data.scope {
            params_json
                .as_object_mut()
                .unwrap()
                .insert("scope".to_string(), json!(scope));
        }

        let params = Some(DeviceChannelParams::Json(
            serde_json::to_string(&params_json).unwrap(),
        ));
        let thunder_method = ThunderPlugin::PersistentStorage.method("getValue");
        let response = state
            .get_thunder_client()
            .call(DeviceCallRequest {
                method: thunder_method,
                params,
            })
            .await;
        info!("{}", response.message);

        if let Some(status) = response.message["success"].as_bool() {
            if status {
                let value_resp_res = serde_json::from_value(response.message);
                if let Ok(res) = value_resp_res {
                    debug!("{:?}", res);
                    let value_resp: ThunderGetValueResponse = res;
                    if value_resp.success {
                        if let Ok(v) = serde_json::from_str::<Value>(&value_resp.value) {
                            if let Ok(v) = serde_json::from_value(v.clone()) {
                                return Self::respond(
                                    state.get_client(),
                                    req.clone(),
                                    ExtnResponse::StorageData(v),
                                )
                                .await
                                .is_ok();
                            } else if let Ok(v) = serde_json::from_value(v.clone()) {
                                return Self::respond(
                                    state.get_client(),
                                    req.clone(),
                                    ExtnResponse::Value(v),
                                )
                                .await
                                .is_ok();
                            }
                        } else {
                            return Self::respond(
                                state.get_client(),
                                req.clone(),
                                ExtnResponse::String(value_resp.value),
                            )
                            .await
                            .is_ok();
                        }
                    } else {
                        error!("success failure response from thunder");
                    }
                } else {
                    error!("malformed response from thunder");
                }
            } else {
                return Self::respond(state.get_client(), req.clone(), ExtnResponse::None(()))
                    .await
                    .is_ok();
            }
        }
        Self::handle_error(state.get_client(), req, RippleError::ProcessorError).await
    }

    async fn set_value(state: ThunderState, req: ExtnMessage, data: SetStorageProperty) -> bool {
        let mut params_json = json!({
            "namespace": data.namespace,
            "key": data.key,
            "value": data.data,
        });
        if let Some(scope) = data.scope {
            params_json
                .as_object_mut()
                .unwrap()
                .insert("scope".to_string(), json!(scope));
        }

        let params = Some(DeviceChannelParams::Json(
            serde_json::to_string(&params_json).unwrap(),
        ));
        let thunder_method = ThunderPlugin::PersistentStorage.method("setValue");
        let response = state
            .get_thunder_client()
            .call(DeviceCallRequest {
                method: thunder_method,
                params,
            })
            .await;
        info!("{}", response.message);

        let response = match response.message["success"].as_bool() {
            Some(v) => ExtnResponse::Boolean(v),
            None => ExtnResponse::Error(RippleError::InvalidOutput),
        };
        info!("thunder : {:?}", response);

        Self::respond(state.get_client(), req, response)
            .await
            .is_ok()
    }
}

impl ExtnStreamProcessor for ThunderStorageRequestProcessor {
    type STATE = ThunderState;
    type VALUE = DevicePersistenceRequest;

    fn get_state(&self) -> Self::STATE {
        self.state.clone()
    }

    fn receiver(&mut self) -> mpsc::Receiver<ExtnMessage> {
        self.streamer.receiver()
    }

    fn sender(&self) -> mpsc::Sender<ExtnMessage> {
        self.streamer.sender()
    }
}

#[async_trait]
impl ExtnRequestProcessor for ThunderStorageRequestProcessor {
    fn get_client(&self) -> ExtnClient {
        self.state.get_client()
    }
    async fn process_request(
        state: Self::STATE,
        msg: ExtnMessage,
        extracted_message: Self::VALUE,
    ) -> bool {
        match extracted_message {
            DevicePersistenceRequest::Get(get_params) => {
                Self::get_value(state.clone(), msg, get_params).await
            }
            DevicePersistenceRequest::Set(set_params) => {
                Self::set_value(state.clone(), msg, set_params).await
            }
            DevicePersistenceRequest::Delete(params) => {
                Self::delete_key(state.clone(), msg, params).await
            }
        }
    }
}
