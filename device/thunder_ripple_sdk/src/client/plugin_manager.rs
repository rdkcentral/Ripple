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

use std::collections::{HashMap, HashSet};
use std::time::Duration;

use ripple_sdk::log::info;
use ripple_sdk::tokio;
use ripple_sdk::{
    api::device::device_operator::{DeviceCallRequest, DeviceSubscribeRequest},
    log::error,
    serde_json,
};
use ripple_sdk::{
    api::device::device_operator::{DeviceChannelParams, DeviceOperator, DeviceResponseMessage},
    tokio::sync::{mpsc, oneshot},
    utils::channel_utils::{mpsc_send_and_log, oneshot_send_and_log},
};
use serde::{Deserialize, Serialize};

use super::thunder_plugin::ThunderPlugin::Controller;
use super::{thunder_client::ThunderClient, thunder_plugin::ThunderPlugin};

pub struct ActivationSubscriber {
    pub callsign: String,
    pub callback: oneshot::Sender<PluginState>,
}

pub struct PluginManager {
    thunder_client: Box<ThunderClient>,
    plugin_states: HashMap<String, PluginState>,
    state_subscribers: Vec<ActivationSubscriber>,
    //caching the plugin activation param so that we can reactivate the plugins on demand
    plugin_request: ThunderPluginBootParam,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct PluginStateChangeEvent {
    pub callsign: String,
    pub state: PluginState,
}

#[derive(Debug, Serialize)]
pub struct ThunderActivatePluginParams {
    callsign: String,
}

#[derive(Debug, Deserialize, PartialEq, Serialize, Clone)]
pub struct PluginStatus {
    pub state: String,
}

#[derive(Debug, Deserialize)]
pub struct ThunderError {
    pub code: i32,
    pub message: String,
}

impl ThunderError {
    pub fn get_plugin_state(&self) -> PluginState {
        match self.message.as_str() {
            "ERROR_INPROGRESS" | "ERROR_PENDING_CONDITIONS" => PluginState::InProgress,
            "ERROR_UNKNOWN_KEY" => PluginState::Missing,
            _ => PluginState::Unknown,
        }
    }
}

impl PluginStatus {
    pub fn to_plugin_state(&self) -> PluginState {
        match self.state.as_str() {
            "activated" | "resumed" | "suspended" => PluginState::Activated,
            "deactivated" => PluginState::Deactivated,
            "deactivation" => PluginState::Deactivation,
            "activation" | "precondition" => PluginState::Activation,
            _ => PluginState::Unavailable,
        }
    }
}

#[derive(Debug, Deserialize, PartialEq, Serialize, Clone)]
pub enum PluginState {
    Activated,
    Activation,
    Deactivated,
    Deactivation,
    Unavailable,
    Precondition,
    Suspended,
    Resumed,
    Missing,
    Error,
    InProgress,
    Unknown,
}

impl PluginState {
    pub fn is_activated(&self) -> bool {
        matches!(self, PluginState::Activated)
    }
    pub fn is_activating(&self) -> bool {
        matches!(self, PluginState::Activation)
    }
    pub fn is_missing(&self) -> bool {
        matches!(self, PluginState::Missing)
    }
}

#[derive(Debug)]
pub enum PluginManagerCommand {
    StateChangeEvent(PluginStateChangeEvent),
    ActivatePluginIfNeeded {
        callsign: String,
        tx: oneshot::Sender<PluginActivatedResult>,
    },
    WaitForActivation {
        callsign: String,
        tx: oneshot::Sender<PluginActivatedResult>,
    },
    ReactivatePluginState {
        tx: oneshot::Sender<PluginActivatedResult>,
    },
    WaitForActivationForDynamicPlugin {
        callsign: String,
        tx: oneshot::Sender<PluginActivatedResult>,
    },
}

#[derive(Debug)]
pub enum PluginActivatedResult {
    Ready,
    Pending(oneshot::Receiver<PluginState>),
    Error,
}

impl PluginActivatedResult {
    pub async fn ready(self) -> bool {
        match self {
            PluginActivatedResult::Ready => true,
            PluginActivatedResult::Error => false,
            PluginActivatedResult::Pending(sub_rx) => {
                // Fail after 30 secs of expecting a Plugin to be activated
                match tokio::time::timeout(Duration::from_millis(30000), sub_rx).await {
                    Ok(Ok(v)) => v.is_activated(),
                    _ => false,
                }
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum ThunderPluginParam {
    None,
    Custom(Vec<String>),
    Default,
}

#[derive(Debug, Clone)]
pub struct ThunderPluginBootParam {
    pub expected: ThunderPluginParam,
    pub activate_on_boot: ThunderPluginParam,
}

impl PluginManager {
    pub async fn start(
        thunder_client: Box<ThunderClient>,
        plugin_request: ThunderPluginBootParam,
    ) -> (mpsc::Sender<PluginManagerCommand>, Vec<String>) {
        let (sub_tx, mut sub_rx) = mpsc::channel::<DeviceResponseMessage>(32);
        let (tx, mut rx) = mpsc::channel::<PluginManagerCommand>(32);
        let mut pm = PluginManager {
            thunder_client: thunder_client.clone(),
            plugin_states: HashMap::default(),
            state_subscribers: Vec::default(),
            plugin_request: plugin_request.clone(),
        };
        let expected = plugin_request.clone().expected;
        match expected {
            ThunderPluginParam::None => {}
            ThunderPluginParam::Custom(p) => {
                for plugin in p {
                    pm.plugin_states.insert(plugin, PluginState::Activated);
                }
            }
            ThunderPluginParam::Default => {
                for p in ThunderPlugin::expect_activated_plugins() {
                    pm.plugin_states
                        .insert(String::from(p.callsign()), PluginState::Activated);
                }
            }
        }
        let tx_for_sub_thread = tx.clone();
        // Spawn statechange subscription thread
        tokio::spawn(async move {
            while let Some(message) = sub_rx.recv().await {
                let event_res: Result<PluginStateChangeEvent, serde_json::Error> =
                    serde_json::from_value(message.message);
                match event_res {
                    Ok(ev) => {
                        // Send the state change to the command thread so the cache can be updated
                        let msg = PluginManagerCommand::StateChangeEvent(ev);
                        mpsc_send_and_log(&tx_for_sub_thread, msg, "StateChangeEvent").await;
                    }
                    Err(_) => {
                        error!("Invalid plugin state change event format");
                    }
                }
            }
        });

        thunder_client
            .clone()
            .subscribe(
                DeviceSubscribeRequest {
                    module: Controller.callsign_and_version(),
                    event_name: "statechange".into(),
                    params: None,
                    sub_id: None,
                },
                sub_tx,
            )
            .await;
        // Spawn command thread
        tokio::spawn(async move {
            while let Some(command) = rx.recv().await {
                match command {
                    PluginManagerCommand::StateChangeEvent(ev) => {
                        pm.handle_state_change(ev).await;
                    }
                    PluginManagerCommand::ActivatePluginIfNeeded { callsign, tx } => {
                        let res = pm.wait_for_activation(callsign, true).await;
                        oneshot_send_and_log(tx, res, "ActivatePluginIfNeededResponse");
                    }
                    PluginManagerCommand::WaitForActivation { callsign, tx } => {
                        let res = pm.wait_for_activation(callsign, false).await;
                        oneshot_send_and_log(tx, res, "WaitForActivation");
                    }
                    PluginManagerCommand::ReactivatePluginState { tx } => {
                        let res = pm.reactivate_plugin_state().await;
                        oneshot_send_and_log(tx, res, "ReactivatePluginState");
                    }
                    PluginManagerCommand::WaitForActivationForDynamicPlugin { callsign, tx } => {
                        let res = pm.wait_for_activation_for_dynamic(callsign).await;
                        oneshot_send_and_log(tx, res, "WaitForActivation");
                    }
                }
            }
        });

        let failed_plugins =
            Self::activate_mandatory_plugins(plugin_request.clone(), tx.clone()).await;
        (tx, failed_plugins)
    }

    pub async fn wait_for_activation_for_dynamic(
        &mut self,
        callsign: String,
    ) -> PluginActivatedResult {
        // Some thunder plugins are related to Apps and they become available when the app is launched.
        let (sub_tx, sub_rx) = oneshot::channel::<PluginState>();

        self.state_subscribers.push(ActivationSubscriber {
            callsign,
            callback: sub_tx,
        });
        PluginActivatedResult::Pending(sub_rx)
    }

    pub async fn activate_mandatory_plugins(
        plugin_request: ThunderPluginBootParam,
        tx: mpsc::Sender<PluginManagerCommand>,
    ) -> Vec<String> {
        info!("Activating Mandatory Thunder Plugins");
        let mut plugins = Vec::new();
        match plugin_request.activate_on_boot {
            ThunderPluginParam::Default => {
                for p in ThunderPlugin::activate_on_boot_plugins() {
                    plugins.push(p.callsign().to_string())
                }
            }
            ThunderPluginParam::Custom(p) => plugins.extend(p),
            ThunderPluginParam::None => {}
        }
        let mut failed_plugins = Vec::new();

        for p in plugins {
            let (plugin_rdy_tx, plugin_rdy_rx) = oneshot::channel::<PluginActivatedResult>();
            mpsc_send_and_log(
                &tx,
                PluginManagerCommand::ActivatePluginIfNeeded {
                    callsign: p.clone(),
                    tx: plugin_rdy_tx,
                },
                "ActivateOnBoot",
            )
            .await;
            if !plugin_rdy_rx.await.unwrap().ready().await {
                error!("{:?} Mandatory Plugin activation failed after timeout", p);
                failed_plugins.push(p)
            }
        }
        failed_plugins
    }

    pub async fn handle_state_change(&mut self, ev: PluginStateChangeEvent) {
        self.plugin_states
            .insert(ev.callsign.clone(), ev.state.clone());
        if !ev.state.is_activated() {
            return;
        };

        // find any listeners that are waiting for a certain callsign to be active, then notify them
        // the listeners are single use, so they are removed after they are notified
        while let Some(s) = self
            .state_subscribers
            .iter()
            .position(|s| s.callsign == ev.callsign)
        {
            let to_notify = self.state_subscribers.remove(s);
            oneshot_send_and_log(
                to_notify.callback,
                ev.state.clone(),
                "NotifyPluginStateListeners",
            );
        }
    }

    pub async fn wait_for_activation(
        &mut self,
        callsign: String,
        trigger_activation: bool,
    ) -> PluginActivatedResult {
        // First check cached state
        let state = match self.plugin_states.get(&callsign) {
            Some(state) => state.clone(),
            None => {
                // No state is known, go fetch the state from thunder controller and store in cache
                let state = self.current_plugin_state(callsign.clone()).await;
                self.plugin_states.insert(callsign.clone(), state.clone());
                state
            }
        };

        if state.is_missing() {
            PluginActivatedResult::Error
        } else if state.is_activated() {
            PluginActivatedResult::Ready
        } else {
            // means plugin needs activation and any call needs to wait for the activation to be successful before proceeding
            // If plugin is not activated, then add a subscriber for state change and then activate
            let (sub_tx, sub_rx) = oneshot::channel::<PluginState>();

            self.state_subscribers.push(ActivationSubscriber {
                callsign: callsign.clone(),
                callback: sub_tx,
            });

            if !state.is_activating() && trigger_activation {
                return match self.activate_plugin(callsign).await {
                    PluginState::Activated => PluginActivatedResult::Ready,
                    PluginState::Missing => PluginActivatedResult::Error,
                    _ => PluginActivatedResult::Pending(sub_rx),
                };
            }
            PluginActivatedResult::Pending(sub_rx)
        }
    }

    async fn activate_plugin(&self, callsign: String) -> PluginState {
        let r = ThunderActivatePluginParams { callsign };
        let resp = self
            .thunder_client
            .clone()
            .call(DeviceCallRequest {
                method: Controller.method("activate"),
                params: Some(DeviceChannelParams::Json(
                    serde_json::to_string(&r).unwrap(),
                )),
            })
            .await;
        if let Ok(plugin_error) = serde_json::from_value::<ThunderError>(resp.message) {
            return plugin_error.get_plugin_state();
        }
        PluginState::Activated
    }

    pub async fn current_plugin_state(&self, callsign: String) -> PluginState {
        let status_meth = Controller.method(format!("status@{}", callsign).as_str());
        let resp = self
            .thunder_client
            .clone()
            .call(DeviceCallRequest {
                method: status_meth,
                params: None,
            })
            .await;

        // For an unavailable plugin Thunder responds with a Code and Message
        if let Ok(plugin_error) = serde_json::from_value::<ThunderError>(resp.message.clone()) {
            return plugin_error.get_plugin_state();
        }

        let status_res: Result<Vec<PluginStatus>, serde_json::Error> =
            serde_json::from_value(resp.message.clone());
        match status_res {
            Ok(status_arr) => match status_arr.first() {
                Some(status) => status.to_plugin_state(),
                None => PluginState::Missing,
            },
            Err(_) => {
                error!(
                    "Invalid response from thunder for plugin status {}",
                    resp.message
                );
                PluginState::Missing
            }
        }
    }
    pub async fn reactivate_plugin_state(&mut self) -> PluginActivatedResult {
        let mut plugins = Vec::new();
        match self.plugin_request.activate_on_boot.clone() {
            ThunderPluginParam::Default => {
                for p in ThunderPlugin::activate_on_boot_plugins() {
                    plugins.push(p.callsign().to_string())
                }
            }
            ThunderPluginParam::Custom(p) => plugins.extend(p),
            ThunderPluginParam::None => {}
        }
        // filter and merge the plugin activation list
        let mut plugin_activate_set: HashSet<String> = HashSet::new();
        for (key, value) in self.plugin_states.iter() {
            if value.is_activated() {
                plugin_activate_set.insert(key.clone());
            }
        }
        // insert all activate_on_boot_plugins here.
        for p in plugins.iter() {
            plugin_activate_set.insert(p.clone());
        }
        // remove all cached plugin states
        self.plugin_states.clear();
        // activate all plugins from the merged list
        for p in plugin_activate_set {
            self.wait_for_activation(p.clone(), true).await;
        }

        PluginActivatedResult::Ready
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::thunder_client_pool::ThunderClientPool;
    use crate::tests::thunder_client_pool_test_utility::{
        CustomMethodHandler, MockWebSocketServer,
    };
    use crate::thunder_state::ThunderConnectionState;
    use ripple_sdk::tokio::time::{sleep, Duration};
    use std::sync::Arc;
    use url::Url;

    #[tokio::test]
    async fn test_plugin_manager_start_and_message_handling() {
        // Using the default method handler from tests::thunder_client_pool_test_utility
        // This can be replaced with a custom method handler, if needed
        let custom_method_handler = Arc::new(CustomMethodHandler);
        let custom_method_handler_c = custom_method_handler.clone();

        let server_task = tokio::spawn(async {
            let mock_server = MockWebSocketServer::new("127.0.0.1:8081", custom_method_handler_c);
            mock_server.start().await;
        });

        // Wait for the server to start
        sleep(Duration::from_secs(1)).await;

        let url = Url::parse("ws://127.0.0.1:8081/jsonrpc").unwrap();

        let controller_pool = ThunderClientPool::start(
            url.clone(),
            None,
            Arc::new(ThunderConnectionState::new()),
            1,
        )
        .await;
        assert!(controller_pool.is_ok());

        let controller_pool = controller_pool.unwrap();

        let expected_plugins = ThunderPluginBootParam {
            expected: ThunderPluginParam::None,
            activate_on_boot: ThunderPluginParam::None,
        };

        // Start the plugin manager
        let plugin_manager_tx =
            PluginManager::start(Box::new(controller_pool), expected_plugins).await;

        let (plugin_manager_tx_clone, _) = plugin_manager_tx.clone();

        // Start the ThunderClientPool
        let client = ThunderClientPool::start(
            url,
            Some(plugin_manager_tx_clone),
            Arc::new(ThunderConnectionState::new()),
            4,
        )
        .await;
        assert!(client.is_ok());

        // 1. test PluginManagerCommand::StateChangeEvent command
        let (plugin_manager_tx_clone, _) = plugin_manager_tx.clone();
        let msg = PluginManagerCommand::StateChangeEvent(PluginStateChangeEvent {
            callsign: "org.rdk.Controller".to_string(),
            state: PluginState::Activated,
        });
        mpsc_send_and_log(&plugin_manager_tx_clone, msg, "StateChangeEvent").await;

        // 2. test PluginManagerCommand::ActivatePluginIfNeeded command
        let (tx, _rx) = oneshot::channel::<PluginActivatedResult>();
        let (plugin_manager_tx_clone, _) = plugin_manager_tx.clone();
        let msg = PluginManagerCommand::ActivatePluginIfNeeded {
            callsign: "org.rdk.Controller".to_string(),
            tx,
        };
        mpsc_send_and_log(&plugin_manager_tx_clone, msg, "ActivatePluginIfNeeded").await;

        // 3. test PluginManagerCommand::WaitForActivation command
        let (tx, _rx) = oneshot::channel::<PluginActivatedResult>();
        let (plugin_manager_tx_clone, _) = plugin_manager_tx.clone();
        let msg = PluginManagerCommand::WaitForActivation {
            callsign: "org.rdk.Controller".to_string(),
            tx,
        };
        mpsc_send_and_log(&plugin_manager_tx_clone, msg, "WaitForActivation").await;

        // 4. test PluginManagerCommand::ReactivatePluginState command
        let (tx, _rx) = oneshot::channel::<PluginActivatedResult>();
        let (plugin_manager_tx_clone, _) = plugin_manager_tx.clone();
        let msg = PluginManagerCommand::ReactivatePluginState { tx };
        mpsc_send_and_log(&plugin_manager_tx_clone, msg, "ReactivatePluginState").await;

        // Wait for a moment and stop the server
        sleep(Duration::from_secs(1)).await;
        server_task.abort();
    }
    // test PluginStatus
    #[test]
    fn test_plugin_status() {
        let status = PluginStatus {
            state: "activated".to_string(),
        };
        assert_eq!(status.to_plugin_state(), PluginState::Activated);
    }

    #[test]
    fn test_plugin_states() {
        fn get_plugin_status(state: &str) -> PluginStatus {
            PluginStatus {
                state: state.to_owned(),
            }
        }

        assert!(matches!(
            get_plugin_status("activated").to_plugin_state(),
            PluginState::Activated
        ));
        assert!(matches!(
            get_plugin_status("resumed").to_plugin_state(),
            PluginState::Activated
        ));
        assert!(matches!(
            get_plugin_status("suspended").to_plugin_state(),
            PluginState::Activated
        ));
        assert!(matches!(
            get_plugin_status("deactivated").to_plugin_state(),
            PluginState::Deactivated
        ));
        assert!(matches!(
            get_plugin_status("deactivation").to_plugin_state(),
            PluginState::Deactivation
        ));
        assert!(matches!(
            get_plugin_status("activation").to_plugin_state(),
            PluginState::Activation
        ));
        assert!(matches!(
            get_plugin_status("precondition").to_plugin_state(),
            PluginState::Activation
        ));
        assert!(matches!(
            get_plugin_status("unavailable").to_plugin_state(),
            PluginState::Unavailable
        ));
        assert!(matches!(
            get_plugin_status("hibernated").to_plugin_state(),
            PluginState::Unavailable
        ));
        assert!(matches!(
            get_plugin_status("").to_plugin_state(),
            PluginState::Unavailable
        ));
    }
}
