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

use jsonrpsee::core::async_trait;
use ripple_sdk::{
    api::{
        apps::{AppEventRequest, EffectiveTransport},
        firebolt::fb_general::ListenRequest,
        gateway::rpc_gateway_api::{ApiMessage, CallContext, JsonRpcApiResponse},
        protocol::BridgeProtocolRequest,
    },
    log::{debug, error},
    serde_json::{json, Value},
    tokio::sync::mpsc,
    utils::channel_utils::mpsc_send_and_log,
};

use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use crate::state::platform_state::PlatformState;

#[derive(Debug)]
pub struct AppEventDecorationError {}
impl From<serde_json::Error> for AppEventDecorationError {
    fn from(_value: serde_json::Error) -> Self {
        AppEventDecorationError {}
    }
}
#[async_trait]
pub trait AppEventDecorator: Send + Sync {
    async fn decorate(
        &self,
        state: &PlatformState,
        ctx: &CallContext,
        event_name: &str,
        val_in: &Value,
    ) -> Result<Value, AppEventDecorationError>;

    fn dec_clone(&self) -> Box<dyn AppEventDecorator + Send + Sync>;
}

impl From<jsonrpsee::core::error::Error> for AppEventDecorationError {
    fn from(_: jsonrpsee::core::error::Error) -> Self {
        AppEventDecorationError {}
    }
}

impl Clone for Box<dyn AppEventDecorator + Send + Sync> {
    fn clone(&self) -> Box<dyn AppEventDecorator + Send + Sync> {
        self.dec_clone()
    }
}

#[derive(Default, Debug)]
pub struct AppEvents {}

type ListenersMap = Arc<RwLock<HashMap<String, HashMap<Option<String>, Vec<EventListener>>>>>;

#[derive(Clone, Default)]
pub struct AppEventsState {
    pub listeners: ListenersMap,
}

impl std::fmt::Debug for AppEventsState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut listeners_debug = HashMap::<String, String>::default();

        for (event_name, context_map) in self.listeners.read().unwrap().iter() {
            for (context, listeners) in context_map.iter() {
                listeners
                    .iter()
                    .map(|x| x.call_ctx.app_id.clone())
                    .for_each(|x| {
                        let cur_value = listeners_debug.get(&event_name.to_owned());
                        let new_value = if let Some(cur_value) = cur_value {
                            if context.is_some() {
                                cur_value.to_string()
                                    + " , ["
                                    + &x
                                    + " with event context "
                                    + &context.clone().unwrap()
                                    + "]"
                            } else {
                                cur_value.to_string() + " , " + &x
                            }
                        } else if context.is_some() {
                            "[".to_string()
                                + &x
                                + " with event context "
                                + &context.clone().unwrap()
                                + "]"
                        } else {
                            x
                        };
                        listeners_debug.insert(event_name.clone(), new_value);
                    })
            }
        }
        f.debug_struct("AppEventsState")
            .field("listeners", &listeners_debug)
            .finish()
    }
}

#[derive(Clone)]
pub struct EventListener {
    pub call_ctx: CallContext,
    // Keep the session_tx package private
    session_tx: Option<mpsc::Sender<ApiMessage>>,
    transport: EffectiveTransport,
    decorator: Option<Box<dyn AppEventDecorator + Send + Sync>>,
}

impl EventListener {
    async fn decorate(
        &self,
        state: &PlatformState,
        event_name: &str,
        result: &Value,
    ) -> Result<Value, AppEventDecorationError> {
        match &self.decorator {
            Some(decorator) => {
                decorator
                    .decorate(state, &self.call_ctx, event_name, result)
                    .await
            }
            None => Ok(result.clone()),
        }
    }
}

impl AppEvents {
    fn get_or_create_listener_vec(
        listeners: &mut HashMap<String, HashMap<Option<String>, Vec<EventListener>>>,
        event_name: String,
        event_context: Option<String>,
    ) -> &mut Vec<EventListener> {
        match listeners.get_mut(&event_name) {
            None => {
                let mut entry = HashMap::new();
                entry.insert(event_context.clone(), Vec::new());
                listeners.insert(event_name.clone(), entry);
            }
            Some(item) => {
                if item.get_mut(&event_context).is_none() {
                    item.insert(event_context.clone(), Vec::new());
                }
            }
        }
        // We just inserted if this was none right before this, so this should always be Some, safe to unwrap
        listeners
            .get_mut(&event_name)
            .unwrap()
            .get_mut(&event_context)
            .unwrap()
    }

    pub fn add_listener(
        state: &PlatformState,
        event_name: String,
        call_ctx: CallContext,
        listen_request: ListenRequest,
    ) {
        AppEvents::add_listener_with_context_and_decorator(
            state,
            event_name,
            call_ctx,
            listen_request,
            None,
            None,
        )
    }

    pub fn add_listener_with_decorator(
        state: &PlatformState,
        event_name: String,
        call_ctx: CallContext,
        listen_request: ListenRequest,
        dec: Option<Box<dyn AppEventDecorator + Send + Sync>>,
    ) {
        AppEvents::add_listener_with_context_and_decorator(
            state,
            event_name,
            call_ctx,
            listen_request,
            None,
            dec,
        )
    }

    pub fn add_listener_with_context(
        state: &PlatformState,
        event_name: String,
        call_ctx: CallContext,
        listen_request: ListenRequest,
        event_context: Option<Value>,
    ) {
        AppEvents::add_listener_with_context_and_decorator(
            state,
            event_name,
            call_ctx,
            listen_request,
            event_context,
            None,
        );
    }

    pub fn add_listener_with_context_and_decorator(
        state: &PlatformState,
        event_name: String,
        call_ctx: CallContext,
        listen_request: ListenRequest,
        event_context: Option<Value>,
        decorator: Option<Box<dyn AppEventDecorator + Send + Sync>>,
    ) {
        let session = match state.session_state.get_session(&call_ctx) {
            Some(session) => session,
            None => {
                error!("No open sessions for id '{:?}'", call_ctx.session_id);
                return;
            }
        };
        let app_events_state = &state.app_events_state;
        let mut listeners = app_events_state.listeners.write().unwrap();
        let event_ctx_string = event_context.map(|x| x.to_string());

        if listen_request.listen {
            let event_listeners =
                AppEvents::get_or_create_listener_vec(&mut listeners, event_name, event_ctx_string);
            //The last listener wins if there is already a listener exists with same session id
            AppEvents::remove_session_from_events(event_listeners, &call_ctx.session_id);
            event_listeners.push(EventListener {
                call_ctx,
                session_tx: session.get_sender(),
                transport: session.get_transport(),
                decorator,
            });
        } else if let Some(entry) = listeners.get_mut(&event_name) {
            if let Some(event_listeners) = entry.get_mut(&event_ctx_string) {
                AppEvents::remove_session_from_events(event_listeners, &call_ctx.session_id);
            }
        }
    }

    pub async fn send_event(state: &PlatformState, listener: &EventListener, data: &Value) {
        let protocol = listener.call_ctx.protocol.clone();
        debug!("Sending event for call context {:?}", listener.call_ctx);
        let mut event = JsonRpcApiResponse::default();
        if listener.call_ctx.is_event_based() {
            event.params = Some(data.clone());
            event.method = Some(format!(
                "{}.{}",
                listener.call_ctx.method, listener.call_ctx.call_id
            ));
        } else {
            event.result = Some(data.clone());
            event.id = Some(listener.call_ctx.call_id);
        }
        // let event = Response {
        //     jsonrpc: TwoPointZero,
        //     result: data,
        //     id: Id::Number(listener.call_ctx.call_id),
        // };

        // Events are pass through no stats
        let api_message = ApiMessage::new(
            protocol,
            json!(event).to_string(),
            listener.call_ctx.request_id.clone(),
        );

        match listener.transport.clone() {
            EffectiveTransport::Websocket => {
                if let Some(session_tx) = listener.session_tx.clone() {
                    mpsc_send_and_log(&session_tx, api_message, "GatewayResponse").await;
                } else {
                    error!("JsonRPC sender missing");
                }
            }
            EffectiveTransport::Bridge(id) => {
                if state.supports_bridge() {
                    let client = state.get_client();
                    let request = BridgeProtocolRequest::Send(id, api_message);
                    if let Err(e) = client.send_extn_request(request).await {
                        error!("Error sending event to bridge {:?}", e);
                    }
                } else {
                    error!("Bridge not supported");
                }
            }
        }
    }

    pub fn get_listeners(
        state: &AppEventsState,
        event_name: &str,
        context: Option<String>,
    ) -> Vec<EventListener> {
        let listeners = state.listeners.read().unwrap();
        let mut vec = Vec::new();

        if let Some(entry) = listeners.get(event_name) {
            if let Some(v) = entry.get(&context) {
                for i in v {
                    vec.push(i.clone());
                }
            }
        }

        vec
    }

    pub async fn emit(state: &PlatformState, event_name: &str, result: &Value) {
        AppEvents::emit_with_context(state, event_name, result, None).await;
    }

    pub async fn emit_with_app_event(state: &PlatformState, event: AppEventRequest) {
        if let AppEventRequest::Emit(app_event) = event {
            AppEvents::emit_with_context(
                state,
                &app_event.event_name,
                &app_event.result,
                app_event.context,
            )
            .await
        }
    }

    pub async fn emit_with_context(
        state: &PlatformState,
        event_name: &str,
        result: &Value,
        context: Option<Value>,
    ) {
        // Notify all the default listners by providing the context data as part of the result when context
        // is present. Otherwise event result without context.
        let listeners = AppEvents::get_listeners(&state.app_events_state, event_name, None);
        for i in listeners {
            let decorated_res = i.decorate(state, event_name, result).await;
            if decorated_res.is_err() {
                error!("could not generate event for '{}'", event_name);
                continue;
            }
            if context.is_some() {
                AppEvents::send_event(
                    state,
                    &i,
                    &json!({
                        "context": context.clone(),
                        "value"  : &decorated_res.unwrap(),
                    }),
                )
                .await;
            } else {
                AppEvents::send_event(state, &i, &decorated_res.unwrap()).await;
            }
        }

        // Now Notify events to the context based listeners. Context info is not included as part of the result
        if let Some(ctx) = context {
            let event_ctx_string = Some(ctx.to_string());
            let listeners = AppEvents::get_listeners(
                &state.app_events_state,
                event_name,
                event_ctx_string.clone(),
            );
            for i in listeners {
                AppEvents::send_event(state, &i, result).await;
            }
        }
    }

    pub async fn emit_to_app(
        state: &PlatformState,
        app_id: String,
        event_name: &str,
        result: &Value,
    ) {
        let listeners_vec = AppEvents::get_listeners(&state.app_events_state, event_name, None)
            .into_iter()
            .filter(|listener| listener.call_ctx.app_id.eq(&app_id))
            .collect::<Vec<_>>();

        for i in listeners_vec {
            let decorated_res = i.decorate(state, event_name, result).await;
            if let Ok(res) = decorated_res {
                AppEvents::send_event(state, &i, &res).await;
            } else {
                error!("could not generate event for '{}'", event_name);
            }
        }
    }

    pub fn is_app_registered_for_event(
        state: &PlatformState,
        app_id: String,
        event_name: &str,
    ) -> bool {
        return AppEvents::get_listeners(&state.app_events_state, event_name, None)
            .iter()
            .any(|listener| listener.call_ctx.app_id.eq(&app_id));
    }

    fn remove_session_from_events(event_listeners: &mut Vec<EventListener>, session_id: &String) {
        let mut itr = event_listeners.iter();
        let i = itr.position(|x| x.call_ctx.session_id == *session_id);
        if let Some(index) = i {
            event_listeners.remove(index);
        }
    }

    pub fn remove_session(state: &PlatformState, session_id: String) {
        state.session_state.clear_session(&session_id);
        let mut listeners = state.app_events_state.listeners.write().unwrap();
        let all_events = listeners.keys().cloned().collect::<Vec<String>>();
        for event_name in all_events {
            if let Some(ctx_map) = listeners.get_mut(&event_name) {
                let all_contexts = ctx_map.keys().cloned().collect::<Vec<Option<String>>>();
                for context in all_contexts {
                    if let Some(event_listener) = ctx_map.get_mut(&context) {
                        AppEvents::remove_session_from_events(event_listener, &session_id);
                    }
                }
            }
        }
    }
}
#[cfg(test)]
pub mod tests {
    use crate::state::session_state::Session;
    use ripple_sdk::tokio;
    use ripple_tdk::utils::test_utils::Mockable;

    use super::*;
    #[tokio::test]
    pub async fn test_add_listener() {
        let platform_state = PlatformState::mock();
        let call_context = CallContext::mock();
        let listen_request = ListenRequest { listen: true };
        Session::new(
            call_context.clone().app_id,
            None,
            EffectiveTransport::Websocket,
        );
        let session = Session::new(
            call_context.clone().app_id,
            None,
            EffectiveTransport::Websocket,
        );
        platform_state
            .session_state
            .add_session(call_context.clone().session_id, session);

        AppEvents::add_listener(
            &platform_state,
            "test_event".to_string(),
            call_context,
            listen_request,
        );
        assert!(
            platform_state
                .app_events_state
                .listeners
                .read()
                .unwrap()
                .len()
                == 1
        );
        let listeners =
            AppEvents::get_listeners(&platform_state.app_events_state, "test_event", None);
        assert!(listeners.len() == 1);
    }
}
