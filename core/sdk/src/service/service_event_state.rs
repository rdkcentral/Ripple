use crate::api::context::RippleContext;
use crate::log::{debug, error};
use crate::service::service_message::ServiceMessage;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::sync::{Arc, RwLock};
use tokio::sync::mpsc::Sender;

#[derive(Debug, Clone, Default)]
pub struct ServiceEventState {
    pub ripple_context: Arc<RwLock<RippleContext>>,
    pub event_subscribers: Arc<RwLock<HashMap<String, Vec<EventSubscriber>>>>,
}

#[derive(Debug, Clone)]
pub enum EventSubscriber {
    ServiceSubscriber(String),
    MainSubscriber(Sender<ServiceMessage>),
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, Hash, PartialEq)]
pub enum Event {
    RippleContextTokenChangedEvent,
    RippleContextActivationChangedEvent,
    RippleContextInternetStatusChangedEvent,
    RippleContextPowerStateChangedEvent,
    RippleContextTimeZoneChangedEvent,
    RippleContextFeaturesChangedEvent,
}

impl Display for Event {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        match self {
            Event::RippleContextTokenChangedEvent => write!(f, "RippleContextTokenChangedEvent"),
            Event::RippleContextActivationChangedEvent => {
                write!(f, "RippleContextActivationChangedEvent")
            }
            Event::RippleContextInternetStatusChangedEvent => {
                write!(f, "RippleContextInternetStatusChangedEvent")
            }
            Event::RippleContextPowerStateChangedEvent => {
                write!(f, "RippleContextPowerStateChangedEvent")
            }
            Event::RippleContextTimeZoneChangedEvent => {
                write!(f, "RippleContextTimeZoneChangedEvent")
            }
            Event::RippleContextFeaturesChangedEvent => {
                write!(f, "RippleContextFeaturesChangedEvent")
            }
        }
    }
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

    pub fn get_event_processors(&self, event: Option<String>) -> Vec<EventSubscriber> {
        let event_subscribers = Arc::clone(&self.event_subscribers);
        let read_lock = event_subscribers.read().unwrap();
        let s = read_lock.get(&event.clone().unwrap_or_default()).cloned();
        if let Some(ref subscribers) = s {
            debug!("Found event subscribers: {:?}", subscribers);
            subscribers.clone()
        } else {
            debug!("No event subscriber found for event {:?}", event);
            Vec::new()
        }
    }

    pub fn add_event_processor(&self, event: String, processor: String) {
        let mut event_subscribers = self.event_subscribers.write().unwrap();
        event_subscribers
            .entry(event)
            .or_default()
            .push(EventSubscriber::ServiceSubscriber(processor));
    }

    pub fn add_main_event_processor(&self, event: String, processor: Sender<ServiceMessage>) {
        let mut event_subscribers = self.event_subscribers.write().unwrap();
        event_subscribers
            .entry(event)
            .or_default()
            .push(EventSubscriber::MainSubscriber(processor));
    }

    pub fn subscribe_context_event(
        &self,
        event: &str,
        context: Option<Value>,
    ) -> Result<(), String> {
        let event = event.to_string();
        debug!(
            "subscribe_context_event context: {:?} event {:?}",
            context, event
        );

        let ctx = context.ok_or_else(|| format!("Context is None event: {}", event))?;
        if !ctx.is_array() {
            error!("Context is not an array of strings");
            return Err("Context is not an array of strings".to_string());
        }
        //Add context[sender_id, service_id] in event processors as string split by "&"
        let sender_tx = ctx.get(0);
        let subscriber = ctx.get(1);
        let new_event_processor = format!(
            "{}&{}",
            sender_tx.and_then(|v| v.as_str()).unwrap_or(""),
            subscriber.and_then(|v| v.as_str()).unwrap_or("")
        );
        if subscriber.is_some() {
            debug!(
                "Subscribing to context event: {:?} with processor {}",
                event, new_event_processor
            );
            self.add_event_processor(event, new_event_processor);
        } else {
            error!("Subscriber not found in context");
            return Err("Subscriber not found in context".to_string());
        }
        Ok(())
    }
}
