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

use std::collections::{BTreeMap, HashMap};
use std::str::FromStr;
use std::sync::{Arc, RwLock};

use jsonrpsee::core::client::{Client, ClientT, SubscriptionClientT};
use jsonrpsee::ws_client::WsClientBuilder;

use jsonrpsee::core::{async_trait, error::Error as JsonRpcError};
use jsonrpsee::types::ParamsSer;
use regex::Regex;
use ripple_sdk::api::device::device_operator::DeviceChannelRequest;
use ripple_sdk::serde_json::json;
use ripple_sdk::tokio::sync::mpsc::Receiver;
use ripple_sdk::{
    api::device::device_operator::DeviceResponseMessage,
    tokio::sync::mpsc::{self, Sender as MpscSender},
    tokio::{sync::Mutex, task::JoinHandle, time::sleep},
};
use ripple_sdk::{
    api::device::device_operator::{
        DeviceCallRequest, DeviceSubscribeRequest, DeviceUnsubscribeRequest,
    },
    serde_json::{self, Value},
    tokio,
};
use ripple_sdk::{
    api::device::device_operator::{DeviceChannelParams, DeviceOperator},
    uuid::Uuid,
};
use ripple_sdk::{
    log::{error, info, warn},
    utils::channel_utils::{mpsc_send_and_log, oneshot_send_and_log},
};
use ripple_sdk::{
    tokio::sync::oneshot::{self, Sender as OneShotSender},
    utils::error::RippleError,
};
use serde::{Deserialize, Serialize};
use url::Url;

use crate::thunder_state::ThunderConnectionState;
use crate::utils::{get_error_value, get_next_id};

use super::thunder_async_client::{ThunderAsyncClient, ThunderAsyncRequest, ThunderAsyncResponse};
use super::thunder_client_pool::ThunderPoolCommand;

use super::thunder_plugins_status_mgr::{BrokerCallback, BrokerSender};
use super::{
    jsonrpc_method_locator::JsonRpcMethodLocator,
    plugin_manager::{PluginActivatedResult, PluginManagerCommand},
};
use std::{env, process::Command};

#[derive(Debug)]
// pub enum ThunderMessage {
//     ThunderCallMessage(ThunderCallMessage),
//     ThunderSubscribeMessage(ThunderSubscribeMessage),
//     ThunderUnsubscribeMessage(ThunderUnsubscribeMessage),
// }

// impl ThunderMessage {
//     pub fn clone(&self, intercept_tx: OneShotSender<DeviceResponseMessage>) -> ThunderMessage {
//         match self {
//             ThunderMessage::ThunderCallMessage(m) => {
//                 ThunderMessage::ThunderCallMessage(ThunderCallMessage {
//                     method: m.method.clone(),
//                     params: m.params.clone(),
//                     callback: intercept_tx,
//                 })
//             }
//             ThunderMessage::ThunderSubscribeMessage(m) => {
//                 ThunderMessage::ThunderSubscribeMessage(ThunderSubscribeMessage {
//                     params: m.params.clone(),
//                     callback: Some(intercept_tx),
//                     module: m.module.clone(),
//                     event_name: m.event_name.clone(),
//                     handler: m.handler.clone(),
//                     sub_id: m.sub_id.clone(),
//                 })
//             }
//             ThunderMessage::ThunderUnsubscribeMessage(m) => {
//                 ThunderMessage::ThunderUnsubscribeMessage(m.clone())
//             }
//         }
//     }
// }

// #[derive(Debug)]
// pub struct ThunderCallMessage {
//     pub method: String,
//     pub params: Option<DeviceChannelParams>,
//     pub callback: OneShotSender<DeviceResponseMessage>,
// }

// impl ThunderCallMessage {
//     pub fn callsign(&self) -> String {
//         JsonRpcMethodLocator::from_str(&self.method)
//             .unwrap()
//             .module
//             .unwrap()
//     }

//     pub fn method_name(&self) -> String {
//         JsonRpcMethodLocator::from_str(&self.method)
//             .unwrap()
//             .method_name
//     }
// }

// #[derive(Debug)]
// pub struct ThunderSubscribeMessage {
//     pub module: String,
//     pub event_name: String,
//     pub params: Option<String>,
//     pub handler: MpscSender<DeviceResponseMessage>,
//     pub callback: Option<OneShotSender<DeviceResponseMessage>>,
//     pub sub_id: Option<String>,
// }

// impl ThunderSubscribeMessage {
//     pub fn resubscribe(&self) -> ThunderSubscribeMessage {
//         ThunderSubscribeMessage {
//             module: self.module.clone(),
//             event_name: self.event_name.clone(),
//             params: self.params.clone(),
//             handler: self.handler.clone(),
//             callback: None,
//             sub_id: self.sub_id.clone(),
//         }
//     }
// }

// #[derive(Debug, Clone)]
// pub struct ThunderUnsubscribeMessage {
//     pub module: String,
//     pub event_name: String,
//     pub subscription_id: Option<String>,
// }

pub struct ThunderClientBuilder;

pub type BrokerSubMap = HashMap<String, Vec<MpscSender<DeviceResponseMessage>>>;

#[derive(Debug, Clone)]
pub struct ThunderClient {
    client: ThunderAsyncClient,
    subscriptions: Arc<RwLock<BrokerSubMap>>,
    callbacks: Arc<RwLock<HashMap<u64, Option<OneShotSender<DeviceResponseMessage>>>>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DefaultThunderResult {
    pub success: bool,
}

impl ThunderClient {
    fn add_to_callback(&self, request: &ThunderAsyncRequest) {
        let mut callbacks = self.callbacks.write().unwrap();
        callbacks.insert(request.id, None);
    }

    // if already subscibed updated handlers
    fn check_sub(
        &self,
        request: &DeviceSubscribeRequest,
        handler: MpscSender<DeviceResponseMessage>,
    ) -> Option<ThunderAsyncRequest> {
        let mut subscriptions = self.subscriptions.write().unwrap();
        let key = format!("{}_{}", request.module, request.event_name);
        if let Some(handlers) = subscriptions.get_mut(&key) {
            handlers.push(handler);
            return None;
        }
        let id = get_next_id();
        let async_request =
            ThunderAsyncRequest::new(DeviceChannelRequest::Subscribe(request.clone()));
        subscriptions.insert(key, vec![handler]);
        Some(async_request)
    }

    // if only one handler cleanup
    fn check_unsub(&self, request: &DeviceUnsubscribeRequest) -> Option<ThunderAsyncRequest> {
        None
    }
}

#[async_trait]
impl DeviceOperator for ThunderClient {
    async fn call(&self, request: DeviceCallRequest) -> DeviceResponseMessage {
        let (tx, rx) = oneshot::channel::<DeviceResponseMessage>();
        let async_request = ThunderAsyncRequest::new(DeviceChannelRequest::Call(request));
        self.add_to_callback(&async_request);
        self.client.send(async_request).await;
        match rx.await {
            Ok(response) => response,
            Err(_) => DeviceResponseMessage {
                message: Value::Null,
                sub_id: None,
            },
        }
    }

    async fn subscribe(
        &self,
        request: DeviceSubscribeRequest,
        handler: mpsc::Sender<DeviceResponseMessage>,
    ) -> DeviceResponseMessage {
        if let Some(subscribe_request) = self.check_sub(&request, handler.clone()) {
            let (tx, rx) = oneshot::channel::<DeviceResponseMessage>();
            self.add_to_callback(&subscribe_request);
            self.client.send(subscribe_request).await;
            rx.await.unwrap()
        } else {
            warn!(
                "Already subscribed to module: {}, event: {}",
                request.module, request.event_name
            );
            DeviceResponseMessage {
                message: Value::Null,
                sub_id: None,
            }
        }
    }

    async fn unsubscribe(&self, request: DeviceUnsubscribeRequest) {
        // deprecate
    }
}

#[derive(Debug)]
pub struct ThunderClientManager;

impl ThunderClientManager {
    fn manage(
        client: ThunderClient,
        mut request_tr: Receiver<ThunderAsyncRequest>,
        mut response_tr: Receiver<ThunderAsyncResponse>,
    ) {
        let client_c = client.clone();
        tokio::spawn(async move {
            loop {
                request_tr = client_c.client.start("", request_tr).await;
                error!("Thunder disconnected so reconnecting")
            }
        });

        tokio::spawn(async move {
            while let Some(v) = response_tr.recv().await {
                // check with thunder client callbacks and subscriptions
            }
        });
    }
}

impl ThunderClientBuilder {
    pub async fn get_client() -> Result<ThunderClient, RippleError> {
        let (sender, tr) = mpsc::channel(10);
        let callback = BrokerCallback { sender };
        let (broker_tx, broker_rx) = mpsc::channel(10);
        let client = ThunderAsyncClient::new(callback, broker_tx);
        let thunder_client = ThunderClient {
            client,
            subscriptions: Arc::new(RwLock::new(HashMap::new())),
            callbacks: Arc::new(RwLock::new(HashMap::new())),
        };
        ThunderClientManager::manage(thunder_client.clone(), broker_rx, tr);
        Ok(thunder_client)
    }
}
