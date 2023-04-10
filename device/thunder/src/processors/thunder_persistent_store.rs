// If not stated otherwise in this file or this component's license file the
// following copyright and licenses apply:
//
// Copyright 2023 RDK Management
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

use serde::{Deserialize, Serialize};
use thunder_ripple_sdk::{
    client::thunder_plugin::ThunderPlugin,
    ripple_sdk::{
        api::device::{
            device_accessibility_data::{GetStorageProperty, SetStorageProperty, StorageRequest},
            device_operator::{DeviceCallRequest, DeviceChannelParams, DeviceOperator},
        },
        async_trait::async_trait,
        extn::{
            client::extn_client::ExtnClient,
            client::extn_processor::{
                DefaultExtnStreamer, ExtnRequestProcessor, ExtnStreamProcessor, ExtnStreamer,
            },
            extn_client_message::{ExtnMessage, ExtnRequest, ExtnResponse},
        },
        log::info,
        serde_json::{self, json},
        tokio::sync::mpsc,
        utils::error::RippleError,
    },
    thunder_state::ThunderState,
};

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
    async fn delete_key(self: Box<Self>, namespace: String, key: String) -> bool;
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

    async fn get_value(state: ThunderState, req: ExtnMessage, data: GetStorageProperty) -> bool {
        let params = Some(DeviceChannelParams::Json(
            serde_json::to_string(&json!({
                "namespace": data.namespace,
                "key": data.key,
            }))
            .unwrap(),
        ));

        let thunder_method = ThunderPlugin::PersistentStorage.method("getValue");
        let response = state
            .get_thunder_client()
            .call(DeviceCallRequest {
                method: thunder_method,
                params: params,
            })
            .await;
        info!("{}", response.message);
        let response = match response.message["value"].as_str() {
            Some(v) => ExtnResponse::String(v.to_string()),
            None => ExtnResponse::Error(RippleError::InvalidOutput),
        };

        Self::respond(state.get_client(), req, response)
            .await
            .is_ok()
    }

    async fn set_value(state: ThunderState, req: ExtnMessage, data: SetStorageProperty) -> bool {
        let params = Some(DeviceChannelParams::Json(
            serde_json::to_string(&json!({
                "namespace": data.namespace,
                "key": data.key,
                "value": data.value,
            }))
            .unwrap(),
        ));

        let thunder_method = ThunderPlugin::PersistentStorage.method("setValue");
        let response = state
            .get_thunder_client()
            .call(DeviceCallRequest {
                method: thunder_method,
                params: params,
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
    type VALUE = StorageRequest;

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
            StorageRequest::Get(get_params) => {
                Self::get_value(state.clone(), msg, get_params).await
            }
            StorageRequest::Set(set_params) => {
                Self::set_value(state.clone(), msg, set_params).await
            }
        }
    }
}
