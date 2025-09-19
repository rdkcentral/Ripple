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

// use ripple_sdk::tokio;
// use ripple_sdk::tokio::sync::oneshot;
//use serde::{Deserialize, Serialize};
// use std::time::Duration;

// pub struct ActivationSubscriber {
//     pub callsign: String,
//     pub callback: oneshot::Sender<PluginState>,
// }

// #[derive(Debug, Deserialize, Clone)]
// #[cfg_attr(any(test, feature = "mock"), derive(Serialize))]
// pub struct PluginStateChangeEvent {
//     pub callsign: String,
//     pub state: PluginState,
// }

// #[derive(Debug, PartialEq, Deserialize, Clone)]
// #[cfg_attr(any(test, feature = "mock"), derive(Serialize))]
// pub struct PluginStatus {
//     pub state: String,
// }

// impl PluginStatus {
//     pub fn to_plugin_state(&self) -> PluginState {
//         match self.state.as_str() {
//             "activated" | "resumed" | "suspended" => PluginState::Activated,
//             "deactivated" => PluginState::Deactivated,
//             "deactivation" => PluginState::Deactivation,
//             "activation" | "precondition" => PluginState::Activation,
//             _ => PluginState::Unavailable,
//         }
//     }
// }

// #[derive(Debug, Deserialize, PartialEq, Clone)]
// #[cfg_attr(any(test, feature = "mock"), derive(Serialize))]
// pub enum PluginState {
//     Activated,
//     Activation,
//     Deactivated,
//     Deactivation,
//     Unavailable,
//     Precondition,
//     Suspended,
//     Resumed,
//     Missing,
//     Error,
//     InProgress,
//     Unknown,
// }

// impl PluginState {
//     pub fn is_activated(&self) -> bool {
//         matches!(self, PluginState::Activated)
//     }
//     pub fn is_activating(&self) -> bool {
//         matches!(self, PluginState::Activation)
//     }
//     pub fn is_missing(&self) -> bool {
//         matches!(self, PluginState::Missing)
//     }
// }

// #[derive(Debug)]
// pub enum PluginManagerCommand {
//     StateChangeEvent(PluginStateChangeEvent),
//     ActivatePluginIfNeeded {
//         callsign: String,
//         tx: oneshot::Sender<PluginActivatedResult>,
//     },
//     WaitForActivation {
//         callsign: String,
//         tx: oneshot::Sender<PluginActivatedResult>,
//     },
//     ReactivatePluginState {
//         tx: oneshot::Sender<PluginActivatedResult>,
//     },
//     WaitForActivationForDynamicPlugin {
//         callsign: String,
//         tx: oneshot::Sender<PluginActivatedResult>,
//     },
// }

// #[derive(Debug)]
// pub enum PluginActivatedResult {
//     Ready,
//     Pending(oneshot::Receiver<PluginState>),
//     Error,
// }

// #[cfg(test)]
// mod tests {
//     use super::*;

//     // test PluginStatus
//     #[test]
//     fn test_plugin_status() {
//         let status = PluginStatus {
//             state: "activated".to_string(),
//         };
//         assert_eq!(status.to_plugin_state(), PluginState::Activated);
//     }

//     #[test]
//     fn test_plugin_states() {
//         fn get_plugin_status(state: &str) -> PluginStatus {
//             PluginStatus {
//                 state: state.to_owned(),
//             }
//         }

//         assert!(matches!(
//             get_plugin_status("activated").to_plugin_state(),
//             PluginState::Activated
//         ));
//         assert!(matches!(
//             get_plugin_status("resumed").to_plugin_state(),
//             PluginState::Activated
//         ));
//         assert!(matches!(
//             get_plugin_status("suspended").to_plugin_state(),
//             PluginState::Activated
//         ));
//         assert!(matches!(
//             get_plugin_status("deactivated").to_plugin_state(),
//             PluginState::Deactivated
//         ));
//         assert!(matches!(
//             get_plugin_status("deactivation").to_plugin_state(),
//             PluginState::Deactivation
//         ));
//         assert!(matches!(
//             get_plugin_status("activation").to_plugin_state(),
//             PluginState::Activation
//         ));
//         assert!(matches!(
//             get_plugin_status("precondition").to_plugin_state(),
//             PluginState::Activation
//         ));
//         assert!(matches!(
//             get_plugin_status("unavailable").to_plugin_state(),
//             PluginState::Unavailable
//         ));
//         assert!(matches!(
//             get_plugin_status("hibernated").to_plugin_state(),
//             PluginState::Unavailable
//         ));
//         assert!(matches!(
//             get_plugin_status("").to_plugin_state(),
//             PluginState::Unavailable
//         ));
//     }
// }
