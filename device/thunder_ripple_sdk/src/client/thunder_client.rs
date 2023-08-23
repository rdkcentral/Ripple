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

use jsonrpsee::core::client::{Client, ClientT, SubscriptionClientT};
use jsonrpsee::ws_client::WsClientBuilder;

use jsonrpsee::core::async_trait;
use jsonrpsee::types::ParamsSer;
use ripple_sdk::tokio::task::JoinHandle;
use ripple_sdk::{
    api::device::device_operator::DeviceResponseMessage,
    tokio::sync::mpsc::{self, Sender as MpscSender},
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
    log::{error, info, trace, warn},
    utils::channel_utils::{mpsc_send_and_log, oneshot_send_and_log},
};
use ripple_sdk::{
    tokio::sync::oneshot::{self, Sender as OneShotSender},
    utils::error::RippleError,
};
use serde::{Deserialize, Serialize};
use url::Url;

use super::thunder_client_pool::ThunderPoolCommand;
use super::{
    jsonrpc_method_locator::JsonRpcMethodLocator,
    plugin_manager::{PluginActivatedResult, PluginManagerCommand},
};
use std::{env, process::Command};

pub struct ThunderClientBuilder;

#[derive(Debug)]
pub struct ThunderCallMessage {
    pub method: String,
    pub params: Option<DeviceChannelParams>,
    pub callback: OneShotSender<DeviceResponseMessage>,
}

impl ThunderCallMessage {
    pub fn callsign(&self) -> String {
        JsonRpcMethodLocator::from_str(&self.method)
            .unwrap()
            .module
            .unwrap()
    }

    pub fn method_name(&self) -> String {
        JsonRpcMethodLocator::from_str(&self.method)
            .unwrap()
            .method_name
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ThunderRegisterParams {
    pub event: String,
    pub id: String,
}

#[derive(Debug)]
pub struct ThunderSubscribeMessage {
    pub module: String,
    pub event_name: String,
    pub params: Option<String>,
    pub handler: MpscSender<DeviceResponseMessage>,
    pub callback: Option<OneShotSender<DeviceResponseMessage>>,
    pub sub_id: Option<String>,
}

impl ThunderSubscribeMessage {
    pub fn resubscribe(&self) -> ThunderSubscribeMessage {
        ThunderSubscribeMessage {
            module: self.module.clone(),
            event_name: self.event_name.clone(),
            params: self.params.clone(),
            handler: self.handler.clone(),
            callback: None,
            sub_id: self.sub_id.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ThunderUnsubscribeMessage {
    pub module: String,
    pub event_name: String,
    pub subscription_id: Option<String>,
}

#[derive(Debug)]
pub enum ThunderMessage {
    ThunderCallMessage(ThunderCallMessage),
    ThunderSubscribeMessage(ThunderSubscribeMessage),
    ThunderUnsubscribeMessage(ThunderUnsubscribeMessage),
}

impl ThunderMessage {
    pub fn clone(&self, intercept_tx: OneShotSender<DeviceResponseMessage>) -> ThunderMessage {
        match self {
            ThunderMessage::ThunderCallMessage(m) => {
                ThunderMessage::ThunderCallMessage(ThunderCallMessage {
                    method: m.method.clone(),
                    params: m.params.clone(),
                    callback: intercept_tx,
                })
            }
            ThunderMessage::ThunderSubscribeMessage(m) => {
                ThunderMessage::ThunderSubscribeMessage(ThunderSubscribeMessage {
                    params: m.params.clone(),
                    callback: Some(intercept_tx),
                    module: m.module.clone(),
                    event_name: m.event_name.clone(),
                    handler: m.handler.clone(),
                    sub_id: m.sub_id.clone(),
                })
            }
            ThunderMessage::ThunderUnsubscribeMessage(m) => {
                ThunderMessage::ThunderUnsubscribeMessage(m.clone())
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct ThunderClient {
    pub sender: Option<MpscSender<ThunderMessage>>,
    pub pooled_sender: Option<MpscSender<ThunderPoolCommand>>,
    pub id: Uuid,
    pub plugin_manager_tx: Option<MpscSender<PluginManagerCommand>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DefaultThunderResult {
    pub success: bool,
}

impl ThunderClient {
    /// Sends a message to thunder. If this client is pooled
    /// then it will wrap the message in a pool command before sending
    pub async fn send_message(&self, message: ThunderMessage) {
        info!("Client {} sending thunder message {:?}", self.id, message);
        if let Some(s) = &self.pooled_sender {
            mpsc_send_and_log(
                s,
                ThunderPoolCommand::ThunderMessage(message),
                "ThunderMessageToPool",
            )
            .await;
        } else if let Some(s) = &self.sender {
            mpsc_send_and_log(s, message, "ThunderMessage").await;
        }
    }
}

#[async_trait]
impl DeviceOperator for ThunderClient {
    async fn call(&self, request: DeviceCallRequest) -> DeviceResponseMessage {
        let (tx, rx) = oneshot::channel::<DeviceResponseMessage>();
        let message = ThunderMessage::ThunderCallMessage(ThunderCallMessage {
            method: request.method,
            params: request.params,
            callback: tx,
        });
        self.send_message(message).await;

        rx.await.unwrap()
    }

    async fn subscribe(
        &self,
        request: DeviceSubscribeRequest,
        handler: mpsc::Sender<DeviceResponseMessage>,
    ) -> DeviceResponseMessage {
        let (tx, rx) = oneshot::channel::<DeviceResponseMessage>();
        let message = ThunderSubscribeMessage {
            module: request.module,
            event_name: request.event_name,
            params: request.params,
            handler,
            callback: Some(tx),
            sub_id: request.sub_id,
        };
        let msg = ThunderMessage::ThunderSubscribeMessage(message);
        self.send_message(msg).await;
        rx.await.unwrap()
    }

    async fn unsubscribe(&self, request: DeviceUnsubscribeRequest) {
        let message = ThunderUnsubscribeMessage {
            module: request.module,
            event_name: request.event_name,
            subscription_id: None,
        };
        let msg = ThunderMessage::ThunderUnsubscribeMessage(message);
        self.send_message(msg).await;
    }
}

struct ThunderSubscription {
    handle: JoinHandle<()>,
    listeners: HashMap<String, MpscSender<DeviceResponseMessage>>,
    rpc_response: DeviceResponseMessage,
}

impl ThunderClient {
    async fn subscribe(
        client_id: Uuid,
        client: &Client,
        subscriptions: &mut HashMap<String, ThunderSubscription>,
        thunder_message: ThunderSubscribeMessage,
        pool_tx: Option<mpsc::Sender<ThunderPoolCommand>>,
    ) {
        let subscribe_method = format!(
            "client.{}.events.{}",
            thunder_message.module, thunder_message.event_name
        );
        let sub_id = match &thunder_message.sub_id {
            Some(sid) => sid.clone(),
            None => Uuid::new_v4().to_string(),
        };
        if let Some(sub) = subscriptions.get_mut(&subscribe_method) {
            // rpc subscription already exists, just add a listener
            sub.listeners
                .insert(sub_id.clone(), thunder_message.handler);
            if let Some(cb) = thunder_message.callback {
                let resp = DeviceResponseMessage::sub(sub.rpc_response.message.clone(), sub_id);
                oneshot_send_and_log(cb, resp, "ThunderRegisterResponse");
            }
            return;
        }
        // rpc subscription does not exist, set it up
        let subscription_res = client
            .subscribe_to_method::<Value>(subscribe_method.as_str())
            .await;
        if let Err(e) = subscription_res {
            error!("Failed to setup subscriber in jsonrpsee client, {}", e);
            // Maybe this method signature should change to propagate the error up
            return;
        }
        let mut subscription = subscription_res.unwrap();
        let params = ThunderRegisterParams {
            event: thunder_message.event_name.clone(),
            id: format!("client.{}.events", thunder_message.module.clone()),
        };
        let json = serde_json::to_string(&params).unwrap();
        let response = Box::new(ThunderParamRequest {
            method: format!("{}.register", thunder_message.module).as_str(),
            params: &json,
            json_based: true,
        })
        .send_request(client)
        .await;
        let handler_channel = thunder_message.handler.clone();
        let resub_message = ThunderMessage::ThunderSubscribeMessage(thunder_message.resubscribe());
        let sub_id_c = sub_id.clone();
        let handle = ripple_sdk::tokio::spawn(async move {
            trace!("Starting thread to listen for thunder events");
            while let Some(ev_res) = subscription.next().await {
                if let Ok(ev) = ev_res {
                    let msg = DeviceResponseMessage::sub(ev, sub_id_c.clone());
                    mpsc_send_and_log(&thunder_message.handler, msg, "ThunderSubscribeEvent").await;
                }
            }
            if let Some(ptx) = pool_tx {
                warn!(
                    "Client {} became disconnected, resubscribing to events",
                    client_id
                );
                // Remove the client and then resubscribe with a new client
                let pool_msg = ThunderPoolCommand::RemoveFromPool(client_id);
                mpsc_send_and_log(&ptx, pool_msg, "RemoveThunderClient").await;
                let pool_msg = ThunderPoolCommand::ThunderMessage(resub_message);
                mpsc_send_and_log(&ptx, pool_msg, "RetryThunderMessage").await;
            }
        });

        let msg = DeviceResponseMessage::sub(response, sub_id.clone());
        let mut tsub = ThunderSubscription {
            handle,
            listeners: HashMap::default(),
            rpc_response: msg.clone(),
        };
        tsub.listeners.insert(sub_id, handler_channel);
        subscriptions.insert(subscribe_method, tsub);
        if let Some(cb) = thunder_message.callback {
            oneshot_send_and_log(cb, msg, "ThunderRegisterResponse");
        }
    }

    async fn unsubscribe(
        client: &Client,
        subscriptions: &mut HashMap<String, ThunderSubscription>,
        thunder_message: ThunderUnsubscribeMessage,
    ) {
        let subscribe_method = format!(
            "client.{}.events.{}",
            thunder_message.module, thunder_message.event_name
        );
        match thunder_message.subscription_id {
            Some(sub_id) => {
                // Remove the listener for the given sub_id, if there are no more listeners then
                // unsubscribe through rpc
                if let Some(sub) = subscriptions.get_mut(&subscribe_method) {
                    sub.listeners.remove(&sub_id);
                    if sub.listeners.is_empty() {
                        if let Some(s) = subscriptions.remove(&subscribe_method) {
                            s.handle.abort();
                        }
                    }
                }
            }
            None => {
                // removing all subscriptions for a method
                if let Some(sub) = subscriptions.remove(&subscribe_method) {
                    sub.handle.abort();
                }
            }
        }
        let params = ThunderRegisterParams {
            event: thunder_message.event_name,
            id: format!("client.{}.events", thunder_message.module),
        };
        let json = serde_json::to_string(&params).unwrap();
        Box::new(ThunderParamRequest {
            method: format!("{}.unregister", thunder_message.module).as_str(),
            params: &json,
            json_based: true,
        })
        .send_request(client)
        .await;
    }

    async fn call(
        client: &Client,
        thunder_message: ThunderCallMessage,
        plugin_manager_tx: Option<MpscSender<PluginManagerCommand>>,
    ) {
        // First check if the plugin is activated and ready to use
        let (plugin_rdy_tx, plugin_rdy_rx) = oneshot::channel::<PluginActivatedResult>();
        if let Some(tx) = plugin_manager_tx {
            let msg = PluginManagerCommand::ActivatePluginIfNeeded {
                callsign: thunder_message.callsign(),
                tx: plugin_rdy_tx,
            };
            mpsc_send_and_log(&tx, msg, "ActivatePluginIfNeeded").await;
            if let Ok(res) = plugin_rdy_rx.await {
                res.ready().await;
            }
        }
        let params = thunder_message.params;
        match params {
            Some(p) => match p {
                DeviceChannelParams::Bool(b) => {
                    let r = Box::new(ThunderRawBoolRequest {
                        method: thunder_message.method.clone(),
                        v: b,
                    })
                    .send_request()
                    .await;
                    return_message(thunder_message.callback, r);
                }
                _ => {
                    let response = Box::new(ThunderParamRequest {
                        method: &thunder_message.method.clone(),
                        params: &p.as_params(),
                        json_based: p.is_json(),
                    })
                    .send_request(client)
                    .await;
                    return_message(thunder_message.callback, response);
                }
            },
            None => {
                let response: Value = Box::new(ThunderNoParamRequest {
                    method: thunder_message.method.clone(),
                })
                .send_request(client)
                .await;
                return_message(thunder_message.callback, response);
            }
        }
    }
}

impl ThunderClientBuilder {
    pub async fn get_client(
        url: Url,
        plugin_manager_tx: Option<MpscSender<PluginManagerCommand>>,
        pool_tx: Option<mpsc::Sender<ThunderPoolCommand>>,
    ) -> Result<ThunderClient, RippleError> {
        let id = Uuid::new_v4();

        info!("initiating thunder connection {}", url);
        let mut subscriptions = HashMap::<String, ThunderSubscription>::default();
        let (s, mut r) = mpsc::channel::<ThunderMessage>(32);
        let pmtx_c = plugin_manager_tx.clone();
        let url_with_token = if let Ok(token) = env::var("THUNDER_TOKEN") {
            Url::parse_with_params(url.as_str(), &[("token", token)]).unwrap()
        } else {
            url.clone()
        };
        let client = WsClientBuilder::default()
            .build(url_with_token.to_string())
            .await;
        if client.is_err() {
            return Err(RippleError::BootstrapError);
        }

        let client = client.unwrap();
        tokio::spawn(async move {
            while let Some(message) = r.recv().await {
                if !client.is_connected() {
                    if let Some(ptx) = pool_tx {
                        warn!("Client {} became disconnected, removing from pool", id);
                        // Remove the client and then try the message again with a new client
                        let pool_msg = ThunderPoolCommand::RemoveFromPool(id);
                        mpsc_send_and_log(&ptx, pool_msg, "RemoveThunderClient").await;
                        let pool_msg = ThunderPoolCommand::ThunderMessage(message);
                        mpsc_send_and_log(&ptx, pool_msg, "RetryThunderMessage").await;
                        return;
                    }
                }
                match message {
                    ThunderMessage::ThunderCallMessage(thunder_message) => {
                        ThunderClient::call(&client, thunder_message, plugin_manager_tx.clone())
                            .await;
                    }
                    ThunderMessage::ThunderSubscribeMessage(thunder_message) => {
                        ThunderClient::subscribe(
                            id,
                            &client,
                            &mut subscriptions,
                            thunder_message,
                            pool_tx.clone(),
                        )
                        .await;
                    }
                    ThunderMessage::ThunderUnsubscribeMessage(thunder_message) => {
                        ThunderClient::unsubscribe(&client, &mut subscriptions, thunder_message)
                            .await;
                    }
                }
            }
        });

        Ok(ThunderClient {
            sender: Some(s),
            pooled_sender: None,
            id,
            plugin_manager_tx: pmtx_c,
        })
    }

    #[cfg(test)]
    pub fn mock(sender: MpscSender<ThunderMessage>) -> ThunderClient {
        ThunderClient {
            sender: Some(sender),
            pooled_sender: None,
            id: Uuid::new_v4(),
            plugin_manager_tx: None,
        }
    }
}

pub struct ThunderRawBoolRequest {
    method: String,
    v: bool,
}

impl ThunderRawBoolRequest {
    async fn send_request(self: Box<Self>) -> Value {
        let host = match env::var("THUNDER_HOST") {
            Ok(h) => h,
            Err(_) => String::from("127.0.0.1"),
        };

        if let Ok(t) = env::var("THUNDER_TOKEN") {
            let command = format!(
                r#"/usr/bin/curl -H "Authorization: Bearer {}" -d '{{"jsonrpc":"2.0","id":"1","method":"{}","params":{}}}' http://{}:9998/jsonrpc"#,
                t, self.method, self.v, host
            );
            let mut start_ref_app_command = Command::new("sh");
            start_ref_app_command.arg("-c").arg(command);
            if start_ref_app_command.output().is_ok() {
                Value::Bool(true)
            } else {
                Value::Bool(false)
            }
        } else {
            let command = format!(
                r#"/usr/bin/curl -d '{{"jsonrpc":"2.0","id":"1","method":"{}","params":{}}}' http://{}:9998/jsonrpc"#,
                self.method, self.v, host
            );
            let mut start_ref_app_command = Command::new("sh");
            start_ref_app_command.arg("-c").arg(command);
            if start_ref_app_command.output().is_ok() {
                Value::Bool(true)
            } else {
                Value::Bool(false)
            }
        }
    }
}

pub struct ThunderNoParamRequest {
    method: String,
}

impl ThunderNoParamRequest {
    async fn send_request(self: Box<Self>, client: &Client) -> Value {
        let result = client.request(&self.method, None).await;
        if let Err(e) = result {
            error!("send_request: Error: e={}", e);
            return Value::Null;
        }
        result.unwrap()
    }
}

pub struct ThunderParamRequest<'a> {
    method: &'a str,
    params: &'a str,
    json_based: bool,
}

impl<'a> ThunderParamRequest<'a> {
    async fn send_request(self: Box<Self>, client: &Client) -> Value {
        let result = client.request(self.method, self.get_params()).await;
        if let Err(e) = result {
            error!("send_request: Error: e={}", e);
            return Value::Null;
        }
        result.unwrap()
    }

    fn get_params(self) -> Option<ParamsSer<'a>> {
        match self.json_based {
            true => {
                let r: Result<BTreeMap<&'a str, Value>, _> = serde_json::from_str(self.params);
                match r {
                    Ok(v_tree_map) => Some(ParamsSer::from(v_tree_map)),
                    Err(_e) => None,
                }
            }
            false => Some(ParamsSer::Array(
                [Value::String(String::from(self.params))].to_vec(),
            )),
        }
    }
}

fn return_message(callback: OneShotSender<DeviceResponseMessage>, response: Value) {
    let msg = DeviceResponseMessage::call(response);
    oneshot_send_and_log(callback, msg, "returning message");
}
