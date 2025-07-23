use crate::api::context::RippleContext;
use crate::api::context::RippleContextUpdateRequest;
use crate::api::context::RippleContextUpdateType;
use crate::api::gateway::rpc_gateway_api::CallContext;
use crate::log::{debug, error, info};
use crate::service::service_message::ServiceMessage;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

#[derive(Debug, Clone)]
pub struct ServiceEventState {
    pub ripple_context: Arc<RwLock<RippleContext>>,
    pub event_subscribers: Arc<RwLock<HashMap<RippleContextUpdateType, Vec<String>>>>,
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

    pub fn get_event_processors(
        &self,
        context_update_type: Option<RippleContextUpdateType>,
    ) -> Vec<String> {
        let event_subscribers: Arc<RwLock<HashMap<RippleContextUpdateType, Vec<String>>>> =
            Arc::clone(&self.event_subscribers);
        let read_lock = event_subscribers.read().unwrap();
        match context_update_type {
            Some(update_type) => read_lock.get(&update_type).cloned().unwrap_or_default(),
            None => read_lock.values().cloned().flatten().collect(),
        }
    }

    pub fn add_event_processor(&self, update_type: RippleContextUpdateType, processor: String) {
        let mut event_subscribers = self.event_subscribers.write().unwrap();
        event_subscribers
            .entry(update_type)
            .or_default()
            .push(processor);
    }

    pub fn process_event_notification(&self, update_type: &str, sm: ServiceMessage) {
        let update_type = format!("\"{}\"", update_type);
        let update_type = serde_json::from_str::<RippleContextUpdateType>(&update_type);
        match update_type {
            Ok(update_type) => {
                info!("^^^ Parsed update type: {:?}", update_type);
                let ctx = &sm.context.as_ref().map_or_else(CallContext::default, |v| {
                    serde_json::from_value(v.clone()).unwrap_or_default()
                });

                info!("^^^ Context: {:#?}", ctx);

                //TODO we need to store context[sender_id, service_id, request_type] in event processors as string split by "&"
                let sender_tx = ctx.context.get(0);
                let subscriber = ctx.context.get(1);
                let request_type = ctx.context.get(2);
                let new_event_processor = format!(
                    "{}&{}&{}",
                    sender_tx.unwrap_or(&"".to_string()),
                    subscriber.unwrap_or(&"".to_string()),
                    request_type.unwrap_or(&"".to_string())
                );
                if let Some(s) = subscriber {
                    self.add_event_processor(update_type, new_event_processor);
                    info!("^^^ Adding event processor for update type");
                } else {
                    error!("^^^ Subscriber not found in context");
                }
            }
            Err(e) => {
                error!("^^^ Failed to parse update type: {}", e);
            }
        }
    }
}
