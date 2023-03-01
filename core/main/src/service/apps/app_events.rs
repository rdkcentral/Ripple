use jsonrpsee::{
    core::async_trait,
    types::{Id, Response, TwoPointZero},
};
use ripple_sdk::{
    api::gateway::rpc_gateway_api::{ApiMessage, CallContext},
    log::error,
    serde_json::{json, Value},
    tokio::sync::mpsc,
    utils::channel_utils::mpsc_send_and_log,
};
use serde::{Deserialize, Serialize};

use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use crate::state::platform_state::PlatformState;

#[derive(Debug)]
pub struct AppEventDecorationError {}

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

#[derive(Clone, Default)]
pub struct AppEventsState {
    pub listeners: Arc<RwLock<HashMap<String, HashMap<Option<String>, Vec<EventListener>>>>>,
    pub sessions: Arc<RwLock<HashMap<String, mpsc::Sender<ApiMessage>>>>,
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
                        let new_value = if cur_value.is_some() {
                            if context.is_some() {
                                cur_value.unwrap().to_string()
                                    + " , [".into()
                                    + &x
                                    + " with event context ".into()
                                    + &context.clone().unwrap()
                                    + "]".into()
                            } else {
                                cur_value.unwrap().to_string() + " , ".into() + &x
                            }
                        } else {
                            if context.is_some() {
                                "[".to_string()
                                    + &x
                                    + " with event context ".into()
                                    + &context.clone().unwrap()
                                    + "]".into()
                            } else {
                                x
                            }
                        };
                        listeners_debug.insert(event_name.clone(), new_value);
                    })
            }
        }
        f.debug_struct("AppEventsState")
            .field("listeners", &listeners_debug)
            .field("sessions", &self.sessions.read().unwrap().keys())
            .finish()
    }
}

#[derive(Clone)]
pub struct EventListener {
    pub call_ctx: CallContext,
    // Keep the session_tx package private
    session_tx: mpsc::Sender<ApiMessage>,
    decorator: Option<Box<dyn AppEventDecorator + Send + Sync>>,
}

impl EventListener {
    async fn decorate(
        &self,
        state: &PlatformState,
        event_name: &str,
        result: &Value,
    ) -> Result<Value, AppEventDecorationError> {
        if let None = self.decorator {
            return Ok(result.clone());
        }
        self.decorator
            .as_ref()
            .unwrap()
            .decorate(&state, &self.call_ctx, event_name, &result)
            .await
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ListenRequest {
    pub listen: bool,
}

#[derive(Serialize, Deserialize)]
pub struct ListenerResponse {
    pub listening: bool,
    pub event: &'static str,
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
            Some(item) => match item.get_mut(&event_context) {
                None => {
                    item.insert(event_context.clone(), Vec::new());
                }
                _ => {}
            },
        }
        // We just inserted if this was none right before this, so this should always be Some, safe to unwrap
        listeners
            .get_mut(&event_name)
            .unwrap()
            .get_mut(&event_context)
            .unwrap()
    }

    pub fn add_listener(
        state: &AppEventsState,
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
        state: &AppEventsState,
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
        state: &AppEventsState,
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
        state: &AppEventsState,
        event_name: String,
        call_ctx: CallContext,
        listen_request: ListenRequest,
        event_context: Option<Value>,
        decorator: Option<Box<dyn AppEventDecorator + Send + Sync>>,
    ) {
        let sessions = state.sessions.read().unwrap();
        let session = sessions.get(&call_ctx.session_id);
        match session {
            Some(session_tx) => {
                let mut listeners = state.listeners.write().unwrap();
                let event_ctx_string = event_context.map(|x| x.to_string());

                if listen_request.listen {
                    let event_listeners = AppEvents::get_or_create_listener_vec(
                        &mut listeners,
                        event_name,
                        event_ctx_string.clone(),
                    );
                    //The last listener wins if there is already a listener exists with same session id
                    AppEvents::remove_session_from_events(event_listeners, &call_ctx.session_id);
                    event_listeners.push(EventListener {
                        call_ctx: call_ctx,
                        session_tx: session_tx.clone(),
                        decorator: decorator,
                    });
                } else if let Some(entry) = listeners.get_mut(&event_name) {
                    if let Some(event_listeners) = entry.get_mut(&event_ctx_string) {
                        AppEvents::remove_session_from_events(
                            event_listeners,
                            &call_ctx.session_id,
                        );
                    }
                }
            }
            None => error!("No open sessions for id '{:?}'", call_ctx.session_id),
        }
    }

    pub async fn send_event(listener: &EventListener, data: &Value) {
        let proto = listener.call_ctx.protocol.clone();
        let event = Response {
            jsonrpc: TwoPointZero,
            result: data,
            id: Id::Number(listener.call_ctx.call_id),
        };
        let api_message = ApiMessage::new(
            proto,
            json!(event).to_string(),
            listener.call_ctx.request_id.clone(),
        );
        mpsc_send_and_log(&listener.session_tx, api_message, "GatewayResponse").await;
    }

    pub fn get_listeners(
        state: &AppEventsState,
        event_name: &str,
        context: Option<String>,
    ) -> Vec<EventListener> {
        let listeners = state.listeners.read().unwrap();
        let mut vec = Vec::new();

        match listeners.get(event_name) {
            Some(entry) => match entry.get(&context) {
                Some(v) => {
                    for i in v {
                        vec.push(i.clone());
                    }
                }
                None => {}
            },
            None => {}
        }
        vec
    }

    pub async fn emit(state: &PlatformState, event_name: &str, result: &Value) {
        AppEvents::emit_with_context(state, event_name, result, None).await;
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
                    &i,
                    &json!({
                        "context": context.clone(),
                        "value"  : &decorated_res.unwrap(),
                    }),
                )
                .await;
            } else {
                AppEvents::send_event(&i, &decorated_res.unwrap()).await;
            }
        }

        // Now Notify events to the context based listeners. Context info is not included as part of the result
        if context.is_some() {
            let event_ctx_string = Some(context.unwrap().to_string());
            let listeners = AppEvents::get_listeners(
                &state.app_events_state,
                event_name,
                event_ctx_string.clone(),
            );
            for i in listeners {
                AppEvents::send_event(&i, result).await;
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
            if decorated_res.is_err() {
                error!("could not generate event for '{}'", event_name);
            } else {
                AppEvents::send_event(&i, &decorated_res.unwrap()).await;
            }
        }
    }

    fn remove_session_from_events(event_listeners: &mut Vec<EventListener>, session_id: &String) {
        let mut itr = event_listeners.iter();
        let i = itr.position(|x| x.call_ctx.session_id == *session_id);
        if let Some(index) = i {
            event_listeners.remove(index);
        }
    }

    pub fn remove_session(state: &AppEventsState, session_id: String) {
        let mut sessions = state.sessions.write().unwrap();
        sessions.remove(&session_id);
        let mut listeners = state.listeners.write().unwrap();
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

    pub fn register_session(
        state: &AppEventsState,
        session_id: String,
        session_tx: mpsc::Sender<ApiMessage>,
    ) {
        let mut sessions = state.sessions.write().unwrap();
        sessions.insert(session_id, session_tx);
    }
}
