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

use jsonrpsee::core::async_trait;
use ripple_sdk::api::device::device_operator::DeviceChannelRequest;
use ripple_sdk::api::device::device_operator::DeviceOperator;
use ripple_sdk::log::error;
use ripple_sdk::tokio::sync::mpsc::Receiver;

use ripple_sdk::{
    api::device::device_operator::DeviceResponseMessage,
    tokio::sync::mpsc::{self, Sender as MpscSender},
};

use ripple_sdk::{
    api::device::device_operator::{
        DeviceCallRequest, DeviceSubscribeRequest, DeviceUnsubscribeRequest,
    },
    serde_json::Value,
    tokio,
};

use ripple_sdk::{
    tokio::sync::oneshot::{self, Sender as OneShotSender},
    utils::error::RippleError,
};

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use super::thunder_async_client::{ThunderAsyncClient, ThunderAsyncRequest, ThunderAsyncResponse};
use super::thunder_plugins_status_mgr::BrokerCallback;

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
    fn add_to_callback(&self, request: &ThunderAsyncRequest) {}

    // if already subscibed updated handlers
    fn check_sub(&self, request: &DeviceSubscribeRequest) -> Option<ThunderAsyncRequest> {
        None
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
        if let Some(subscribe_request) = self.check_sub(&request) {
            let (tx, rx) = oneshot::channel::<DeviceResponseMessage>();
            self.add_to_callback(&subscribe_request);
            self.client.send(subscribe_request).await;
            rx.await.unwrap()
        } else {
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
