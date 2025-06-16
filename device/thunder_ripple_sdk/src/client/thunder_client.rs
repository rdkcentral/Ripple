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

use super::device_operator::{
    DeviceCallRequest, DeviceChannelRequest, DeviceOperator, DeviceResponseMessage,
    DeviceResponseSubscription, DeviceSubscribeRequest,
};
use super::thunder_async_client::{ThunderAsyncClient, ThunderAsyncRequest, ThunderAsyncResponse};
use super::thunder_async_client_plugins_status_mgr::{AsyncCallback, AsyncSender};
use jsonrpsee::core::async_trait;

use ripple_sdk::{
    log::error,
    serde_json::Value,
    tokio,
    tokio::sync::mpsc::{self, Receiver, Sender as MpscSender},
    tokio::sync::oneshot::{self, error::RecvError, Sender as OneShotSender},
    utils::channel_utils::{mpsc_send_and_log, oneshot_send_and_log},
    utils::error::RippleError,
    uuid::Uuid,
    Mockable,
};
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::RwLock;
use tokio::sync::oneshot::Sender;
use url::Url;

pub type BrokerSubMap = HashMap<String, DeviceResponseSubscription>;
pub type BrokerCallbackMap = HashMap<u64, Option<OneShotSender<DeviceResponseMessage>>>;

#[derive(Debug)]
pub struct ThunderClientManager;

impl ThunderClientManager {
    fn start(
        client: ThunderClient,
        request_tr: Receiver<ThunderAsyncRequest>,
        mut response_tr: Receiver<ThunderAsyncResponse>,
        thndr_endpoint_url: String,
        status_check: bool,
    ) {
        if let Some(ref thunder_async_client) = client.thunder_async_client {
            let mut tac = thunder_async_client.clone();

            println!("*** @@@@_DEBUG: ThunderClientManager: ThunderAsyncRequest received");

            tokio::spawn(async move {
                tac.start(&thndr_endpoint_url, request_tr, status_check)
                    .await;
            });
        }

        /*thunder async response will get here */
        tokio::spawn(async move {
            while let Some(response) = response_tr.recv().await {
                println!(
                    "*** _DEBUG: ThunderClientManager: ThunderAsyncResponse received : {:?}",
                    response
                );
                if let Some(id) = response.get_id() {
                    if let Some(thunder_async_callbacks) = client.clone().thunder_async_callbacks {
                        let mut callbacks = thunder_async_callbacks.write().unwrap();
                        if let Some(Some(callback)) = callbacks.remove(&id) {
                            if let Some(resp) = response.get_device_resp_msg(None) {
                                oneshot_send_and_log(callback, resp, "ThunderResponse");
                            };
                        }
                    }
                } else if let Some(event_name) = response.get_method() {
                    if let Some(broker_subs) = client.clone().thunder_async_subscriptions {
                        let subs = {
                            let mut br_subs = broker_subs.write().unwrap();
                            br_subs.get_mut(&event_name).cloned()
                        };

                        if let Some(dev_resp_sub) = subs {
                            //let subc = subs;
                            for s in &dev_resp_sub.handlers {
                                if let Some(resp_msg) =
                                    response.get_device_resp_msg(dev_resp_sub.clone().sub_id)
                                {
                                    mpsc_send_and_log(s, resp_msg, "ThunderResponse").await;
                                }
                            }
                        }
                    }
                }
            }
        });
    }
}

pub struct ThunderClientBuilder;

#[derive(Debug, Clone)]
pub struct ThunderClient {
    pub id: Uuid,
    pub thunder_async_client: Option<ThunderAsyncClient>,
    pub thunder_async_subscriptions: Option<Arc<RwLock<BrokerSubMap>>>,
    pub thunder_async_callbacks: Option<Arc<RwLock<BrokerCallbackMap>>>,
}

impl Mockable for ThunderClient {
    fn mock() -> Self {
        let (resp_tx, _resp_rx) = mpsc::channel(32);
        let callback = AsyncCallback { sender: resp_tx };
        let (broker_tx, _broker_rx) = mpsc::channel(32);
        let broker_sender = AsyncSender { sender: broker_tx };
        let client = ThunderAsyncClient::new(callback, broker_sender);

        ThunderClient {
            id: Uuid::new_v4(),
            thunder_async_client: Some(client),
            thunder_async_subscriptions: Some(Arc::new(RwLock::new(HashMap::new()))),
            thunder_async_callbacks: Some(Arc::new(RwLock::new(HashMap::new()))),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct DefaultThunderResult {
    pub success: bool,
}

#[async_trait]
impl DeviceOperator for ThunderClient {
    async fn call(&self, request: DeviceCallRequest) -> DeviceResponseMessage {
        let (tx, rx) = oneshot::channel::<DeviceResponseMessage>();
        let async_request = ThunderAsyncRequest::new(DeviceChannelRequest::Call(request));
        self.add_callback(&async_request, tx);
        if let Some(async_client) = &self.thunder_async_client {
            println!(
                "*** _DEBUG: ThunderClient: call: request= {:?}",
                async_request
            );
            async_client.send(async_request).await;
        }

        // <pca> Naveen: I think you need something to execute the callback (tx), otherwise you'll never get a response here. </pca>

        match rx.await {
            Ok(response) => {
                println!("*** _DEBUG: ThunderClient: call: response= {:?}", response);
                response
            }
            Err(e) => {
                error!("ThunderClient Failed to receive response: {:?}", e);
                DeviceResponseMessage {
                    message: Value::Null,
                    sub_id: None,
                }
            }
        }
    }

    async fn subscribe(
        &self,
        request: DeviceSubscribeRequest,
        handler: mpsc::Sender<DeviceResponseMessage>,
    ) -> Result<DeviceResponseMessage, RecvError> {
        if let Some(subscribe_request) = self.add_subscription_handler(&request, handler.clone()) {
            let (tx, rx) = oneshot::channel::<DeviceResponseMessage>();
            self.add_callback(&subscribe_request, tx);
            if let Some(async_client) = &self.thunder_async_client {
                async_client.send(subscribe_request).await;
            }
            let result = rx.await;
            if let Err(ref e) = result {
                error!("subscribe: e={:?}", e);
            }
            result
        } else {
            Ok(DeviceResponseMessage {
                message: Value::Null,
                sub_id: None,
            })
        }
    }
}

impl ThunderClient {
    fn add_callback(
        &self,
        request: &ThunderAsyncRequest,
        dev_resp_callback: Sender<DeviceResponseMessage>,
    ) {
        println!(
            "*** _DEBUG: add_callback invoked for : ThunderAsyncRequest: {}",
            request
        );
        if let Some(callbacks_arc) = &self.thunder_async_callbacks {
            let mut callbacks = callbacks_arc.write().unwrap();
            callbacks.insert(request.id, Some(dev_resp_callback));
        } else {
            error!("thunder_async_callbacks found None");
        }
    }

    // if already subscribed updated handlers
    fn add_subscription_handler(
        &self,
        request: &DeviceSubscribeRequest,
        handler: MpscSender<DeviceResponseMessage>,
    ) -> Option<ThunderAsyncRequest> {
        let mut thunder_async_subscriptions = self
            .thunder_async_subscriptions
            .as_ref()
            .unwrap()
            .write()
            .unwrap();

        // Create a key for the subscription based on the event name
        let key = format!("client.events.{}", request.event_name);

        // Check if there are existing subscriptions for the given key
        if let Some(subs) = thunder_async_subscriptions.get_mut(&key) {
            // If a subscription exists, add the handler to the list of handlers
            subs.handlers.push(handler);
            None
        } else {
            // If no subscription exists, create a new async request for subscription
            let async_request =
                ThunderAsyncRequest::new(DeviceChannelRequest::Subscribe(request.clone()));

            // Create a new DeviceResponseSubscription with the handler
            let dev_resp_sub = DeviceResponseSubscription {
                sub_id: request.clone().sub_id,
                handlers: vec![handler],
            };

            // Insert the new subscription into the thunder_async_subscriptions map
            thunder_async_subscriptions.insert(key, dev_resp_sub);
            Some(async_request)
        }
    }

    // <pca> TODO: Move to MockThunderClient or similar o it's only used in tests
    // pub fn start_mock(device_channel_request_tx: MpscSender<DeviceChannelRequest>) -> Self {
    //     let (device_response_message_tx, mut device_response_message_rx) =
    //         mpsc::channel::<DeviceResponseMessage>(32);
    //     let (thunder_async_response_tx, mut thunder_async_response_rx) = mpsc::channel(32);
    //     let callback = AsyncCallback {
    //         sender: thunder_async_response_tx,
    //     };

    //     let (thunder_async_request_tx, mut thunder_async_request_rx) = mpsc::channel(32);
    //     let broker_sender = AsyncSender {
    //         sender: thunder_async_request_tx,
    //     };
    //     let client = ThunderAsyncClient::new(callback, broker_sender);

    //     // Handle requests from ThunderAsyncClient and forward to device_channel_request_tx
    //     tokio::spawn(async move {
    //         while let Some(thunder_async_request) = thunder_async_request_rx.recv().await {
    //             println!(
    //                 "*** _DEBUG: ThunderClient: mock: thunder_async_request= {:?}",
    //                 thunder_async_request
    //             );

    //             mpsc_send_and_log(
    //                 &device_channel_request_tx,
    //                 thunder_async_request.request,
    //                 "DeviceChannelRequest",
    //             )
    //             .await;
    //         }
    //     });

    //     // Optionally, you can process responses here if needed
    //     tokio::spawn(async move {
    //         while let Some(thunder_async_response) = thunder_async_response_rx.recv().await {
    //             println!(
    //                 "*** _DEBUG: ThunderClient: mock: thunder_async_response= {:?}",
    //                 thunder_async_response
    //             );
    //             // oneshot_send_and_log(
    //             //     thunder_async_response_tx,
    //             //     thunder_async_response,
    //             //     "thunderasyncresponse",
    //             // );
    //             // You can add logic here to handle the response if required
    //         }
    //     });

    //     // Optionally, you can process responses here if needed
    //     tokio::spawn(async move {
    //         while let Some(thunder_async_response) = device_response_message_rx.recv().await {
    //             println!(
    //                 "*** _DEBUG: ThunderClient: mock: thunder_async_response= {:?}",
    //                 thunder_async_response
    //             );
    //             // oneshot_send_and_log(
    //             //     thunder_async_response_tx,
    //             //     thunder_async_response,
    //             //     "thunderasyncresponse",
    //             // );
    //             // You can add logic here to handle the response if required
    //         }
    //     });

    //     ThunderClient {
    //         id: Uuid::new_v4(),
    //         thunder_async_client: Some(client),
    //         thunder_async_subscriptions: Some(Arc::new(RwLock::new(HashMap::new()))),
    //         thunder_async_callbacks: Some(Arc::new(RwLock::new(HashMap::new()))),
    //     }
    // }

    pub fn start_mock(
        device_channel_request_tx: MpscSender<DeviceChannelRequest>,
        mut thunderasync_resp_rx: mpsc::Receiver<ThunderAsyncResponse>,
    ) -> Self {
        let (thunder_async_response_tx, mut thunder_async_response_rx) = mpsc::channel(32);
        let callback = AsyncCallback {
            sender: thunder_async_response_tx,
        };

        let (thunder_async_request_tx, mut thunder_async_request_rx) = mpsc::channel(32);
        let broker_sender = AsyncSender {
            sender: thunder_async_request_tx,
        };
        let client = ThunderAsyncClient::new(callback, broker_sender);

        // Handle requests from ThunderAsyncClient and forward to device_channel_request_tx
        tokio::spawn(async move {
            while let Some(thunder_async_request) = thunder_async_request_rx.recv().await {
                println!(
                    "*** _DEBUG: ThunderClient: mock: thunder_async_request= {:?}",
                    thunder_async_request
                );

                mpsc_send_and_log(
                    &device_channel_request_tx,
                    thunder_async_request.request,
                    "DeviceChannelRequest",
                )
                .await;
            }
        });

        // Process responses and forward to device_response_message_tx
        tokio::spawn(async move {
            while let Some(thunder_async_response) = thunder_async_response_rx.recv().await {
                println!(
                    "*** _DEBUG: ThunderClient: mock: thunder_async_response= {:?}",
                    thunder_async_response
                );
                let (device_response_message_tx, _device_response_message_rx) =
                    oneshot::channel::<DeviceResponseMessage>();

                if let Some(resp) = thunder_async_response.get_device_resp_msg(None) {
                    let _ = device_response_message_tx.send(resp);
                };
            }
        });

        // Process responses and forward to device_response_message_tx
        tokio::spawn(async move {
            while let Some(thunder_async_response) = thunderasync_resp_rx.recv().await {
                println!(
                    "*** @@@_DEBUG: ThunderClient: mock: thunder_async_response= {:?}",
                    thunder_async_response
                );
                let (device_response_message_tx, _device_response_message_rx) =
                    oneshot::channel::<DeviceResponseMessage>();

                if let Some(resp) = thunder_async_response.get_device_resp_msg(None) {
                    println!(
                        "*** @@@_DEBUG: ThunderClient:mock sending DeviceResponseMessage: {:?}",
                        resp
                    );
                    let _ = device_response_message_tx.send(resp);
                };
            }
        });

        ThunderClient {
            id: Uuid::new_v4(),
            thunder_async_client: Some(client),
            thunder_async_subscriptions: Some(Arc::new(RwLock::new(HashMap::new()))),
            thunder_async_callbacks: Some(Arc::new(RwLock::new(HashMap::new()))),
        }
    }
}

impl ThunderClientBuilder {
    pub async fn start_thunder_client(
        url: Url,
        status_check: bool,
    ) -> Result<ThunderClient, RippleError> {
        let (resp_tx, resp_rx) = mpsc::channel(32);
        let callback = AsyncCallback { sender: resp_tx };
        let (broker_tx, broker_rx) = mpsc::channel(32);
        let broker_sender = AsyncSender { sender: broker_tx };
        let client = ThunderAsyncClient::new(callback, broker_sender);

        let thunder_client = ThunderClient {
            id: Uuid::new_v4(),
            thunder_async_client: Some(client),
            thunder_async_subscriptions: Some(Arc::new(RwLock::new(HashMap::new()))),
            thunder_async_callbacks: Some(Arc::new(RwLock::new(HashMap::new()))),
        };

        ThunderClientManager::start(
            thunder_client.clone(),
            broker_rx,
            resp_rx,
            url.to_string(),
            status_check,
        );
        Ok(thunder_client)
    }

    #[cfg(test)]
    pub fn mock() -> ThunderClient {
        let (resp_tx, _resp_rx) = mpsc::channel(32);
        let callback = AsyncCallback { sender: resp_tx };
        let (broker_tx, _broker_rx) = mpsc::channel(32);
        let broker_sender = AsyncSender { sender: broker_tx };
        let client = ThunderAsyncClient::new(callback, broker_sender);

        ThunderClient {
            id: Uuid::new_v4(),
            thunder_async_client: Some(client),
            thunder_async_subscriptions: Some(Arc::new(RwLock::new(HashMap::new()))),
            thunder_async_callbacks: Some(Arc::new(RwLock::new(HashMap::new()))),
        }
    }
}
