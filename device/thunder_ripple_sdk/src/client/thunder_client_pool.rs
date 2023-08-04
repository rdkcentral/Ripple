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

use crate::client::thunder_client::ThunderClientBuilder;
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
    RemoveFromPool(Uuid),
}

impl ThunderClientPool {
    pub async fn start(
        url: Url,
        plugin_manager_tx: Option<mpsc::Sender<PluginManagerCommand>>,
        size: u32,
    ) -> Result<ThunderClient, RippleError> {
        debug!("Starting a Thunder connection pool of size {}", size);
        let (s, mut r) = mpsc::channel::<ThunderPoolCommand>(32);
        let mut clients = Vec::default();
        for _ in 0..size {
            let client = ThunderClientBuilder::get_client(
                url.clone(),
                plugin_manager_tx.clone(),
                Some(s.clone()),
            )
            .await;
            if client.is_ok() {
                clients.push(PooledThunderClient {
                    in_use: Arc::new(AtomicBool::new(false)),
                    client: client.unwrap(),
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
                        let c_opt = pool.get_client();
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
                    ThunderPoolCommand::RemoveFromPool(client_id) => {
                        // Remove the given client and then start a new one
                        // to replace it
                        let mut itr = pool.clients.iter();
                        let i = itr.position(|x| x.client.id == client_id);
                        if let Some(index) = i {
                            pool.clients.remove(index);
                        }

                        let client = ThunderClientBuilder::get_client(
                            url.clone(),
                            plugin_manager_tx.clone(),
                            Some(sender_for_thread.clone()),
                        )
                        .await;
                        if client.is_ok() {
                            pool.clients.push(PooledThunderClient {
                                in_use: Arc::new(AtomicBool::new(false)),
                                client: client.unwrap(),
                            });
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
        })
    }

    fn get_client(&mut self) -> Option<&mut PooledThunderClient> {
        // First use an unused client, if there are none just use
        // any client and it will queue in the thread
        let len = self.clients.len();
        let itr = self.clients.iter_mut();
        itr.enumerate()
            .find(|(i, client)| *i == (len - 1) || !client.in_use.load(Ordering::Relaxed))
            .map(|x| x.1)
    }
}
