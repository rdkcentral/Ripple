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

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use crate::{client::thunder_client::ThunderClientBuilder, thunder_state::ThunderConnectionState};
use ripple_sdk::{
    api::device::device_operator::DeviceResponseMessage,
    log::{debug, error},
    tokio::sync::{mpsc, oneshot},
    utils::channel_utils::oneshot_send_and_log,
    uuid::Uuid,
};
use ripple_sdk::{tokio, utils::error::RippleError};
use url::Url;

use super::{
    plugin_manager::PluginManagerCommand,
    thunder_client::{ThunderClient, ThunderMessage},
};

#[derive(Debug)]
pub struct ThunderClientPool {
    clients: Vec<PooledThunderClient>,
}

#[derive(Debug)]
struct PooledThunderClient {
    in_use: Arc<AtomicBool>,
    client: ThunderClient,
}

#[derive(Debug)]
pub enum ThunderPoolCommand {
    ThunderMessage(ThunderMessage),
    ResetThunderClient(Uuid),
}

impl ThunderClientPool {
    pub async fn start(
        url: Url,
        plugin_manager_tx: Option<mpsc::Sender<PluginManagerCommand>>,
        thunder_connection_state: Arc<ThunderConnectionState>,
        size: u32,
    ) -> Result<ThunderClient, RippleError> {
        debug!("Starting a Thunder connection pool of size {}", size);
        let (s, mut r) = mpsc::channel::<ThunderPoolCommand>(32);
        let thunder_connection_state = thunder_connection_state.clone();
        let mut clients = Vec::default();
        for _ in 0..size {
            let client = ThunderClientBuilder::get_client(
                url.clone(),
                plugin_manager_tx.clone(),
                Some(s.clone()),
                thunder_connection_state.clone(),
                None,
            )
            .await;
            if let Ok(c) = client {
                clients.push(PooledThunderClient {
                    in_use: Arc::new(AtomicBool::new(false)),
                    client: c,
                });
            }
        }
        if clients.is_empty() {
            return Err(RippleError::BootstrapError);
        }
        let sender_for_thread = s.clone();
        let pmtx_c = plugin_manager_tx.clone();
        tokio::spawn(async move {
            let mut pool = ThunderClientPool { clients };
            while let Some(cmd) = r.recv().await {
                match cmd {
                    ThunderPoolCommand::ThunderMessage(msg) => {
                        let c_opt = pool.get_client(&msg);
                        if c_opt.is_none() {
                            error!("Thunder pool had no clients!");
                            return;
                        }
                        let c = c_opt.unwrap();
                        let (resp_tx, resp_rx) = oneshot::channel::<DeviceResponseMessage>();
                        c.in_use.to_owned().store(true, Ordering::Relaxed);
                        let msg_with_intercept = msg.clone(resp_tx);
                        let in_use = c.in_use.clone();
                        tokio::spawn(async move {
                            let resp = resp_rx.await;
                            in_use.store(false, Ordering::Relaxed);
                            // Intercept the response back from thunder here
                            // so that we can mark the client as not in use
                            match msg {
                                ThunderMessage::ThunderCallMessage(m) => {
                                    if let Ok(r) = resp {
                                        oneshot_send_and_log(m.callback, r, "ThunderReturn")
                                    }
                                }
                                ThunderMessage::ThunderSubscribeMessage(m) => {
                                    if let Ok(r) = resp {
                                        if let Some(cb) = m.callback {
                                            oneshot_send_and_log(cb, r, "ThunderReturn")
                                        }
                                    }
                                }
                                _ => {}
                            }
                        });
                        c.client.send_message(msg_with_intercept).await;
                    }
                    ThunderPoolCommand::ResetThunderClient(client_id) => {
                        // Remove the given client and then start a new one to replace it
                        let mut itr = pool.clients.iter();
                        let i = itr.position(|x| x.client.id == client_id);
                        if let Some(index) = i {
                            let client = ThunderClientBuilder::get_client(
                                url.clone(),
                                plugin_manager_tx.clone(),
                                Some(sender_for_thread.clone()),
                                thunder_connection_state.clone(),
                                pool.clients.get(index).map(|x| x.client.clone()),
                            )
                            .await;
                            if let Ok(client) = client {
                                pool.clients.remove(index);
                                pool.clients.insert(
                                    index,
                                    PooledThunderClient {
                                        in_use: Arc::new(AtomicBool::new(false)),
                                        client,
                                    },
                                );
                            }
                        }
                    }
                }
            }
        });
        Ok(ThunderClient {
            sender: None,
            pooled_sender: Some(s),
            id: Uuid::new_v4(),
            plugin_manager_tx: pmtx_c,
            subscriptions: None,
        })
    }

    fn get_client(&mut self, msg: &ThunderMessage) -> Option<&mut PooledThunderClient> {
        // For subscribe and Un subscribe use the same client
        match msg {
            ThunderMessage::ThunderSubscribeMessage(_)
            | ThunderMessage::ThunderUnsubscribeMessage(_) => self.clients.get_mut(0),
            _ => {
                // First use an unused client, if there are none just use
                // any client and it will queue in the thread
                let len = self.clients.len();
                let itr = self.clients.iter_mut();
                itr.enumerate()
                    .find(|(i, client)| *i == (len - 1) || !client.in_use.load(Ordering::Relaxed))
                    .map(|x| x.1)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        client::plugin_manager::{PluginActivatedResult, PluginManagerCommand},
        tests::thunder_client_pool_test_utility::{
            CustomMethodHandler, MethodHandler, MockWebSocketServer,
        },
    };
    use ripple_sdk::api::device::device_operator::DeviceUnsubscribeRequest;
    use ripple_sdk::{
        api::device::device_operator::DeviceSubscribeRequest,
        utils::channel_utils::oneshot_send_and_log,
    };
    use ripple_sdk::{
        api::device::device_operator::{DeviceCallRequest, DeviceOperator},
        tokio::time::{sleep, Duration},
    };
    use url::Url;

    #[tokio::test]
    async fn test_thunder_client_pool_start_and_reset() {
        // Using the default method handler from tests::thunder_client_pool_test_utility
        // This can be replaced with a custom method handler, if needed
        let custom_method_handler = Arc::new(CustomMethodHandler);
        let callsign = custom_method_handler.get_callsign();

        // clone to pass to the server
        let custom_method_handler_c = custom_method_handler.clone();

        let server_task = tokio::spawn(async {
            let mock_server = MockWebSocketServer::new("127.0.0.1:8080", custom_method_handler_c);
            mock_server.start().await;
        });

        // Wait for server to start
        sleep(Duration::from_secs(1)).await;

        let url = Url::parse("ws://127.0.0.1:8080/jsonrpc").unwrap();
        let (tx, mut rx) = mpsc::channel(32);

        // Spawn command thread
        tokio::spawn(async move {
            while let Some(command) = rx.recv().await {
                match command {
                    PluginManagerCommand::StateChangeEvent(_ev) => {}
                    PluginManagerCommand::ActivatePluginIfNeeded { callsign: _, tx } => {
                        oneshot_send_and_log(
                            tx,
                            PluginActivatedResult::Ready,
                            "ActivatePluginIfNeededResponse",
                        );
                    }
                    PluginManagerCommand::WaitForActivation { callsign: _, tx } => {
                        oneshot_send_and_log(tx, PluginActivatedResult::Ready, "WaitForActivation");
                    }
                    PluginManagerCommand::ReactivatePluginState { tx } => {
                        oneshot_send_and_log(
                            tx,
                            PluginActivatedResult::Ready,
                            "ReactivatePluginState",
                        );
                    }
                    PluginManagerCommand::WaitForActivationForDynamicPlugin { callsign: _, tx } => {
                        oneshot_send_and_log(
                            tx,
                            PluginActivatedResult::Ready,
                            "WaitForActivationForDynamic",
                        );
                    }
                }
            }
        });

        // Test cases
        // 1. create a client pool of size 4
        let client =
            ThunderClientPool::start(url, Some(tx), Arc::new(ThunderConnectionState::new()), 4)
                .await;
        assert!(client.is_ok());
        let client = client.unwrap();

        // 2. invoke client.call functions 20 times from multple spawned threads
        for _ in 0..20 {
            let client = client.clone();
            let callsign = callsign.clone();
            tokio::spawn(async move {
                let resp = client
                    .call(DeviceCallRequest {
                        method: format!("{}.1.testMethod", callsign),
                        params: None,
                    })
                    .await;
                assert_eq!(resp.message, "testMethod Request Response".to_string());
            });
        }

        // 3. test subscribe. Call this 3 times from a loop
        for _ in 0..3 {
            let (sub_tx, _sub_rx) = mpsc::channel::<DeviceResponseMessage>(32);
            let resp = client
                .subscribe(
                    DeviceSubscribeRequest {
                        module: format!("{}.1", callsign),
                        event_name: "testEvent".into(),
                        params: None,
                        sub_id: None,
                    },
                    sub_tx,
                )
                .await;
            assert_eq!(resp.message, "Subscribed".to_string());
        }

        // 4. Re-start server to test Thunder client reset
        server_task.abort();
        sleep(Duration::from_secs(1)).await;
        let custom_method_handler_c = custom_method_handler.clone();
        let server_task = tokio::spawn(async {
            let mock_server = MockWebSocketServer::new("127.0.0.1:8080", custom_method_handler_c);
            mock_server.start().await;
        });
        // Wait for server to start
        sleep(Duration::from_secs(1)).await;
        // issue client call again
        let resp = client
            .call(DeviceCallRequest {
                method: format!("{}.1.testMethod", callsign),
                params: None,
            })
            .await;
        assert_eq!(resp.message, "testMethod Request Response".to_string());

        // 5. test unsubscribe
        client
            .unsubscribe(DeviceUnsubscribeRequest {
                module: format!("{}.1", callsign),
                event_name: "testEvent".into(),
            })
            .await;
        sleep(Duration::from_secs(1)).await;
        server_task.abort();
    }
}
