use std::collections::HashMap;

use ripple_sdk::tokio;
use ripple_sdk::{
    api::device::device_operator::{DeviceCallRequest, DeviceSubsribeRequest},
    log::error,
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
    pub callback: oneshot::Sender<()>,
}

pub struct PluginManager {
    thunder_client: Box<ThunderClient>,
    plugin_states: HashMap<String, PluginState>,
    state_subscribers: Vec<ActivationSubscriber>,
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

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct PluginStatus {
    pub state: String,
}

impl PluginStatus {
    pub fn to_plugin_state(&self) -> PluginState {
        match self.state.as_str() {
            "activated" => PluginState::Activated,
            "resumed" => PluginState::Activated,
            "suspended" => PluginState::Activated,
            "deactivated" => PluginState::Deactivated,
            _ => PluginState::Missing,
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub enum PluginState {
    Activated,
    Activation,
    Deactivated,
    Deactivation,
    Missing,
}

impl PluginState {
    pub fn is_activated(&self) -> bool {
        match self {
            PluginState::Activated => true,
            _ => false,
        }
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
}

#[derive(Debug)]
pub enum PluginActivatedResult {
    Ready,
    Pending(oneshot::Receiver<()>),
}

impl PluginActivatedResult {
    pub async fn ready(self) {
        match self {
            PluginActivatedResult::Ready => (),
            PluginActivatedResult::Pending(sub_rx) => {
                sub_rx.await.ok();
            }
        }
    }
}

impl PluginManager {
    pub async fn start(thunder_client: Box<ThunderClient>) -> mpsc::Sender<PluginManagerCommand> {
        let (sub_tx, mut sub_rx) = mpsc::channel::<DeviceResponseMessage>(32);
        let (tx, mut rx) = mpsc::channel::<PluginManagerCommand>(32);
        let mut pm = PluginManager {
            thunder_client: thunder_client.clone(),
            plugin_states: HashMap::default(),
            state_subscribers: Vec::default(),
        };
        for p in ThunderPlugin::expect_activated_plugins() {
            pm.plugin_states
                .insert(String::from(p.callsign()), PluginState::Activated);
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
                DeviceSubsribeRequest {
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
                }
            }
        });
        for p in ThunderPlugin::activate_on_boot_plugins() {
            let (plugin_rdy_tx, plugin_rdy_rx) = oneshot::channel::<PluginActivatedResult>();
            mpsc_send_and_log(
                &tx,
                PluginManagerCommand::ActivatePluginIfNeeded {
                    callsign: p.callsign().to_string(),
                    tx: plugin_rdy_tx,
                },
                "ActivateOnBoot",
            )
            .await;
            plugin_rdy_rx.await.unwrap().ready().await;
        }
        tx
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
            oneshot_send_and_log(to_notify.callback, (), "NotifyPluginStateListeners");
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
                // No state is know, go fetch the state from thunder controller and store in cache
                let state = self.current_plugin_state(callsign.clone()).await;
                self.plugin_states.insert(callsign.clone(), state.clone());
                state
            }
        };
        if state.is_activated() {
            return PluginActivatedResult::Ready;
        }
        // If plugin is not activated, then add a subscriber for state change and then activate
        let (sub_tx, sub_rx) = oneshot::channel::<()>();

        self.state_subscribers.push(ActivationSubscriber {
            callsign: callsign.clone(),
            callback: sub_tx,
        });
        if trigger_activation {
            self.activate_plugin(callsign).await;
        }
        PluginActivatedResult::Pending(sub_rx)
    }

    pub async fn activate_plugin(&self, callsign: String) {
        let r = ThunderActivatePluginParams { callsign };
        self.thunder_client
            .clone()
            .call(DeviceCallRequest {
                method: Controller.method("activate"),
                params: Some(DeviceChannelParams::Json(
                    serde_json::to_string(&r).unwrap(),
                )),
            })
            .await;
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
        let status_res: Result<Vec<PluginStatus>, serde_json::Error> =
            serde_json::from_value(resp.message.clone());
        match status_res {
            Ok(status_arr) => match status_arr.get(0) {
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
}
