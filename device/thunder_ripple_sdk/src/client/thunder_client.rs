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

use super::thunder_async_client::{ThunderAsyncClient, ThunderAsyncRequest, ThunderAsyncResponse};
use super::thunder_async_client_plugins_status_mgr::{AsyncCallback, AsyncSender};
use super::{
    device_operator::{
        DeviceCallRequest,
        //DeviceChannelParams,
        DeviceChannelRequest,
        DeviceOperator,
        DeviceResponseMessage,
        DeviceResponseSubscription,
        DeviceSubscribeRequest,
    },
    //jsonrpc_method_locator::JsonRpcMethodLocator,
};
use ripple_sdk::async_trait::async_trait;
use ripple_sdk::{
    log::error,
    serde_json::Value,
    tokio,
    tokio::sync::mpsc::{self, Receiver, Sender as MpscSender},
    tokio::sync::oneshot::{self, error::RecvError, Sender as OneShotSender},
    utils::channel_utils::{mpsc_send_and_log, oneshot_send_and_log},
    utils::error::RippleError,
    uuid::Uuid,
};
use serde::{
    Deserialize,
    //Serialize
};
use std::collections::HashMap;
//use std::str::FromStr;
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
            tokio::spawn(async move {
                tac.start(&thndr_endpoint_url, request_tr, status_check)
                    .await;
            });
        }

        /*thunder async response will get here */
        tokio::spawn(async move {
            while let Some(response) = response_tr.recv().await {
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

// #[derive(Debug, Serialize)]
// pub struct ThunderRegisterParams {
//     pub event: String,
//     pub id: String,
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

#[derive(Debug)]
// pub enum ThunderMessage {
//     ThunderCallMessage(ThunderCallMessage),
//     ThunderSubscribeMessage(ThunderSubscribeMessage),
//     ThunderUnsubscribeMessage(ThunderUnsubscribeMessage),
// }

// impl ThunderMessage {
//     // pub fn clone(&self, intercept_tx: OneShotSender<DeviceResponseMessage>) -> ThunderMessage {
//     //     match self {
//     //         ThunderMessage::ThunderCallMessage(m) => {
//     //             ThunderMessage::ThunderCallMessage(ThunderCallMessage {
//     //                 method: m.method.clone(),
//     //                 params: m.params.clone(),
//     //                 callback: intercept_tx,
//     //             })
//     //         }
//     //         ThunderMessage::ThunderSubscribeMessage(m) => {
//     //             ThunderMessage::ThunderSubscribeMessage(ThunderSubscribeMessage {
//     //                 params: m.params.clone(),
//     //                 callback: Some(intercept_tx),
//     //                 module: m.module.clone(),
//     //                 event_name: m.event_name.clone(),
//     //                 handler: m.handler.clone(),
//     //                 sub_id: m.sub_id.clone(),
//     //             })
//     //         }
//     //         ThunderMessage::ThunderUnsubscribeMessage(m) => {
//     //             ThunderMessage::ThunderUnsubscribeMessage(m.clone())
//     //         }
//     //     }
//     // }
// }
#[derive(Clone)]
pub struct ThunderClient {
    pub id: Uuid,
    pub thunder_async_client: Option<ThunderAsyncClient>,
    pub thunder_async_subscriptions: Option<Arc<RwLock<BrokerSubMap>>>,
    pub thunder_async_callbacks: Option<Arc<RwLock<BrokerCallbackMap>>>,
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
            async_client.send(async_request).await;
        }
        match rx.await {
            Ok(response) => response,
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
    // fn add_callback(
    //     &self,
    //     request: &ThunderAsyncRequest,
    //     dev_resp_callback: Sender<DeviceResponseMessage>,
    // ) {
    //     let mut callbacks = self
    //         .thunder_async_callbacks
    //         .as_ref()
    //         .unwrap()
    //         .write()
    //         .unwrap();
    //     callbacks.insert(request.id, Some(dev_resp_callback));
    // }

    fn add_callback(
        &self,
        request: &ThunderAsyncRequest,
        dev_resp_callback: Sender<DeviceResponseMessage>,
    ) {
        if let Some(callbacks_arc) = &self.thunder_async_callbacks {
            let mut callbacks = callbacks_arc.write().unwrap();
            callbacks.insert(request.id, Some(dev_resp_callback));
        } else {
            error!("thunder_async_callbacks is None");
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
    // #[cfg(test)]
    // pub fn mock(sender: MpscSender<ThunderMessage>) -> ThunderClient {
    //     ThunderClient {
    //         id: Uuid::new_v4(),
    //         thunder_async_client: None,
    //         thunder_async_subscriptions: None,
    //         thunder_async_callbacks: None,
    //     }
    // }
}

#[cfg(test)]
mod tests {
    // use jsonrpsee::core::traits::ToRpcParams as _;

    // use super::*;

    // #[tokio::test]
    // async fn test_thunder_call_message() {
    //     let thunder_call_message = ThunderCallMessage {
    //         method: "org.rdk.RDKShell.1.createDisplay".to_string(),
    //         params: Some(DeviceChannelParams::Json("test".to_string())),
    //         callback: oneshot::channel::<DeviceResponseMessage>().0,
    //     };
    //     assert_eq!(thunder_call_message.callsign(), "org.rdk.RDKShell");
    //     assert_eq!(thunder_call_message.method_name(), "createDisplay");
    // }

    // #[test]
    // fn test_extract_callsign_from_register_method() {
    //     let method = "org.rdk.RDKShell.1.register";
    //     let callsign = ThunderClient::extract_callsign_from_register_method(method);
    //     assert_eq!(callsign, Some("org.rdk.RDKShell".to_string()));

    //     let method = "org.rdk.RDKShell.register";
    //     let callsign = ThunderClient::extract_callsign_from_register_method(method);
    //     assert_eq!(callsign, Some("org.rdk.RDKShell".to_string()));

    //     // test method abcd. 1.register
    //     let method = "abcd .1.register";
    //     let callsign = ThunderClient::extract_callsign_from_register_method(method);
    //     assert_eq!(callsign, Some("abcd ".to_string()));
    // }

    // #[test]
    // fn test_extract_callsign_from_register_method_invalid_pattern() {
    //     let method = "abcd.1";
    //     let callsign = ThunderClient::extract_callsign_from_register_method(method);
    //     assert_eq!(callsign, None);

    //     let method = "abcd.1.register.2";
    //     let callsign = ThunderClient::extract_callsign_from_register_method(method);
    //     assert_eq!(callsign, None);
    // }
    // #[test]
    // fn test_get_params_object_params() {
    //     let request = ThunderParamRequest {
    //         method: "test.method",
    //         params: r#"{"key1": "value1", "key2": "value2"}"#,
    //         json_based: true,
    //     };
    //     match request.get_params() {
    //         ParamWrapper::Object(params) => {
    //             let r = params.to_rpc_params();
    //             let r = r.unwrap();
    //             let r = r.unwrap();
    //             let r = r.get();
    //             assert_eq!(r, r#"{"key1":"value1","key2":"value2"}"#);
    //         }
    //         _ => panic!("Expected ObjectParams"),
    //     }
    // }

    // #[test]
    // fn test_get_params_array_param_non_json_based() {
    //     let request = ThunderParamRequest {
    //         method: "test.method",
    //         params: "value1",
    //         json_based: false,
    //     };
    //     match request.get_params() {
    //         ParamWrapper::Array(params) => {
    //             let r = params.to_rpc_params();
    //             let r = r.unwrap();
    //             let r = r.unwrap();
    //             let r = r.get();
    //             assert_eq!(r, r#"["value1"]"#);
    //         }
    //         _ => panic!("Expected ArrayParams"),
    //     }
    // }

    // #[test]
    // fn test_get_params_no_params() {
    //     let request = ThunderParamRequest {
    //         method: "test.method",
    //         params: "",
    //         json_based: true,
    //     };
    //     match request.get_params() {
    //         ParamWrapper::None => {}
    //         _ => panic!("Expected NoParams"),
    //     }
    // }
}
