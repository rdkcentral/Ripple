use crate::api::context::RippleContext;
use crate::log::{debug, error};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

#[derive(Debug, Clone, Default)]
pub struct ServiceEventState {
    pub ripple_context: Arc<RwLock<RippleContext>>,
    pub event_subscribers: Arc<RwLock<HashMap<String, Vec<String>>>>,
}

impl ServiceEventState {
    pub fn new() -> Self {
        ServiceEventState {
            ripple_context: Arc::new(RwLock::new(RippleContext::default())),
            event_subscribers: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn get_ripple_context(&self) -> Arc<RwLock<RippleContext>> {
        Arc::clone(&self.ripple_context)
    }

    pub fn update_ripple_context(&self, context: RippleContext) {
        let mut ripple_context = self.ripple_context.write().unwrap();
        *ripple_context = context;
    }

    pub fn get_event_processors(&self, event: Option<String>) -> Vec<String> {
        let event_subscribers: Arc<RwLock<HashMap<String, Vec<String>>>> =
            Arc::clone(&self.event_subscribers);
        let read_lock = event_subscribers.read().unwrap();
        match event {
            Some(event) => read_lock.get(&event).cloned().unwrap_or_default(),
            None => Vec::new(),
        }
    }

    pub fn add_event_processor(&self, event: String, processor: String) {
        let mut event_subscribers = self.event_subscribers.write().unwrap();
        event_subscribers.entry(event).or_default().push(processor);
    }

    pub fn subscribe_context_event(&self, event: &str, context: Option<Value>) {
        let event = event.to_string();
        debug!(
            "subscribe_context_event context: {:?} event {:?}",
            context, event
        );

        if let Some(ctx) = context {
            if !ctx.is_array() {
                error!("Context is not an array of strings");
                return;
            }
            //Add context[sender_id, service_id] in event processors as string split by "&"
            let sender_tx = ctx.get(0);
            let subscriber = ctx.get(1);
            let new_event_processor = format!(
                "{}&{}",
                sender_tx.and_then(|v| v.as_str()).unwrap_or(""),
                subscriber.and_then(|v| v.as_str()).unwrap_or("")
            );
            if let Some(_s) = subscriber {
                debug!(
                    "Subscribing to context event: {:?} with processor {}",
                    event, new_event_processor
                );
                self.add_event_processor(event, new_event_processor);
            } else {
                error!("Subscriber not found in context");
            }
        } else {
            error!("Context is None event: {}", event);
        }
    }
}
