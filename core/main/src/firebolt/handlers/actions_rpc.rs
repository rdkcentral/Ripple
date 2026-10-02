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

use crate::{
    firebolt::rpc::RippleRPCProvider, service::apps::app_events::AppEvents,
    state::platform_state::PlatformState,
};
use jsonrpsee::{
    core::{async_trait, RpcResult},
    proc_macros::rpc,
    RpcModule,
};
use ripple_sdk::api::{
    apps::ActionsIntentResponse,
    firebolt::{
        fb_discovery::ACTIONS_EVENT_ON_INTENT,
        fb_general::{ListenRequest, ListenerResponse},
    },
    gateway::rpc_gateway_api::CallContext,
};
use serde_json::Value;

use crate::utils::rpc_utils::rpc_err;

#[rpc(server)]
pub trait Actions {
    /// Actions.intent - Getter: returns the most recently received intent
    /// for the calling app, as a JSON document with a monotonic intentId.
    #[method(name = "actions.intent")]
    async fn intent(&self, ctx: CallContext) -> RpcResult<Value>;

    /// Actions.onIntent - Event: subscribe to intent delivery events.
    /// Listeners receive { intentId, intent } whenever a new intent is
    /// delivered to the app by the platform.
    #[method(name = "actions.onIntent")]
    async fn on_intent(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;
}

pub struct ActionsImpl {
    pub state: PlatformState,
}

#[async_trait]
impl ActionsServer for ActionsImpl {
    async fn intent(&self, ctx: CallContext) -> RpcResult<Value> {
        match self
            .state
            .app_manager_state
            .get_current_intent_with_id(&ctx.app_id)
        {
            Some((intent, intent_id)) => {
                let response = ActionsIntentResponse { intent_id, intent };
                serde_json::to_value(response).map_err(|_| rpc_err("serialization error"))
            }
            None => Ok(serde_json::json!({
                "intentId": 0,
                "intent": {}
            })),
        }
    }

    async fn on_intent(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        let listen = request.listen;

        AppEvents::add_listener(&self.state, ACTIONS_EVENT_ON_INTENT.into(), ctx, request);
        Ok(ListenerResponse {
            listening: listen,
            event: ACTIONS_EVENT_ON_INTENT.into(),
        })
    }
}

pub struct ActionsRPCProvider;
impl RippleRPCProvider<ActionsImpl> for ActionsRPCProvider {
    fn provide(state: PlatformState) -> RpcModule<ActionsImpl> {
        (ActionsImpl { state }).into_rpc()
    }
}

#[cfg(test)]
mod tests {
    use ripple_sdk::api::{
        apps::ActionsIntentResponse,
        device::entertainment_data::{HomeIntent, NavigationIntent, NavigationIntentStrict},
        firebolt::fb_discovery::DiscoveryContext,
    };

    #[test]
    fn test_actions_intent_response_serialization() {
        let home_intent = HomeIntent {
            context: DiscoveryContext {
                source: "voice".to_string(),
                age_policy: None,
            },
        };
        let response = ActionsIntentResponse {
            intent_id: 42,
            intent: NavigationIntent::NavigationIntentStrict(NavigationIntentStrict::Home(
                home_intent,
            )),
        };

        let json = serde_json::to_value(&response).unwrap();
        assert_eq!(json["intentId"], 42);
        assert!(json["intent"].is_object());
        assert_eq!(json["intent"]["action"], "home");
    }

    #[test]
    fn test_actions_intent_response_roundtrip() {
        let home_intent = HomeIntent {
            context: DiscoveryContext {
                source: "remote".to_string(),
                age_policy: None,
            },
        };
        let original = ActionsIntentResponse {
            intent_id: 7,
            intent: NavigationIntent::NavigationIntentStrict(NavigationIntentStrict::Home(
                home_intent,
            )),
        };

        let json_str = serde_json::to_string(&original).unwrap();
        let deserialized: ActionsIntentResponse = serde_json::from_str(&json_str).unwrap();
        assert_eq!(deserialized.intent_id, 7);
    }
}
