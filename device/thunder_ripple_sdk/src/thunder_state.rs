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

use std::sync::{Arc, RwLock};

use ripple_sdk::{
    extn::{
        client::extn_client::ExtnClient,
        extn_client_message::{ExtnMessage, ExtnPayloadProvider},
    },
    log::warn,
    tokio,
    tokio::sync::mpsc,
    tokio::sync::{Mutex, Notify},
    utils::error::RippleError,
};
use url::Url;

use crate::{
    client::{
        device_operator::{DeviceOperator, DeviceResponseMessage, DeviceUnsubscribeRequest},
        plugin_manager::ThunderPluginBootParam,
        thunder_client::ThunderClient,
    },
    events::thunder_event_processor::{ThunderEventHandler, ThunderEventProcessor},
};

#[derive(Debug)]
pub struct ThunderConnectionState {
    pub conn_status_mutex: Mutex<bool>,
    pub conn_status_notify: Notify,
}

impl Default for ThunderConnectionState {
    fn default() -> Self {
        Self::new()
    }
}

impl ThunderConnectionState {
    pub fn new() -> Self {
        ThunderConnectionState {
            conn_status_mutex: Mutex::new(false),
            conn_status_notify: Notify::new(),
        }
    }
}
#[derive(Debug, Clone)]
pub struct ThunderBootstrapStateWithConfig {
    pub extn_client: ExtnClient,
    pub url: Url,
    pub pool_size: Option<u32>,
    pub plugin_param: Option<ThunderPluginBootParam>,
    pub thunder_connection_state: Option<Arc<ThunderConnectionState>>,
}

#[derive(Debug, Clone)]
pub struct ThunderBootstrapStateWithClient {
    pub prev: ThunderBootstrapStateWithConfig,
    pub state: ThunderState,
}

#[derive(Debug, Clone)]
pub struct ThunderState {
    extn_client: ExtnClient,
    thunder_client: ThunderClient,
    pub event_processor: ThunderEventProcessor,
    sender: mpsc::Sender<DeviceResponseMessage>,
    receiver: Arc<RwLock<Option<mpsc::Receiver<DeviceResponseMessage>>>>,
}

impl ThunderState {
    pub fn new(extn_client: ExtnClient, thunder_client: ThunderClient) -> ThunderState {
        let (tx, rx) = mpsc::channel(10);
        ThunderState {
            extn_client,
            thunder_client,
            event_processor: ThunderEventProcessor::new(),
            sender: tx,
            receiver: Arc::new(RwLock::new(Some(rx))),
        }
    }

    pub fn get_thunder_client(&self) -> ThunderClient {
        self.thunder_client.clone()
    }

    pub fn get_client(&self) -> ExtnClient {
        self.extn_client.clone()
    }

    pub async fn send_payload(
        &self,
        payload: impl ExtnPayloadProvider,
    ) -> Result<ExtnMessage, RippleError> {
        self.extn_client.clone().request(payload).await
    }

    pub async fn handle_listener(
        &self,
        listen: bool,
        app_id: String,
        handler: ThunderEventHandler,
    ) {
        if self
            .event_processor
            .handle_listener(listen, app_id.clone(), handler.clone())
        {
            if listen {
                self.subscribe(handler).await
            } else {
                self.unsubscribe(handler).await
            }
        }
    }

    async fn subscribe(&self, handler: ThunderEventHandler) {
        let client = self.get_thunder_client();
        let sender = self.sender.clone();
        let _ = client.subscribe(handler.request, sender).await;
    }

    async fn unsubscribe(&self, handler: ThunderEventHandler) {
        let client = self.get_thunder_client();
        let request = DeviceUnsubscribeRequest {
            module: handler.request.module,
            event_name: handler.request.event_name,
        };
        client.unsubscribe(request).await;
    }

    pub fn start_event_thread(&self) {
        let rx = {
            let mut guard = self.receiver.write().unwrap();
            let rx = guard.take();
            drop(guard);
            rx
        };

        if let Some(mut r) = rx {
            let state_c = self.clone();
            tokio::spawn(async move {
                while let Some(request) = r.recv().await {
                    if let Some(id) = request.sub_id {
                        //check the back_off value
                        if state_c.event_processor.get_backoff(&id).is_some() {
                            warn!("back_off is not None. can't process event of ID:{}", id);
                        } else if let Some(handler) = state_c.event_processor.get_handler(&id) {
                            //back_off is None, process the event
                            handler.process(
                                state_c.clone(),
                                &id,
                                request.message.clone(),
                                handler.callback_type.clone(),
                            );
                        }
                    }
                }
            });
        }
    }
}
