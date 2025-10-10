use crate::state::platform_state::PlatformState;
use jsonrpsee::core::async_trait;
use jsonrpsee::tracing::warn;
use ripple_sdk::api::context::{ActivationStatus, FeatureUpdate};
use ripple_sdk::api::device::device_request::{
    InternetConnectionStatus, SystemPowerState, TimeZone,
};
use ripple_sdk::api::{context::RippleContextUpdateRequest, device::device_request::AccountToken};
use ripple_sdk::{
    log::{debug, error},
    service::service_event_state::{Event, EventSubscriber},
    service::service_message::{JsonRpcMessage, JsonRpcNotification, ServiceMessage},
    tokio,
    tokio::sync::Mutex,
    tokio_tungstenite::tungstenite::Message,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use serde_json::Value;
use std::{collections::HashMap, sync::Arc};

#[derive(Serialize, Deserialize, Debug, Clone, Eq, Hash, PartialEq)]
pub enum NotificationEvent {
    RippleContextUpdateTokenRequest,
    RippleContextUpdateActivationRequest,
    RippleContextUpdateInternetStatusRequest,
    RippleContextUpdatePowerStateRequest,
    RippleContextUpdateTimeZoneRequest,
    RippleContextUpdateFeaturesRequest,
}

#[async_trait]
pub trait NotificationEventStrategy: Send + Sync + std::fmt::Debug {
    fn handle_notification(
        &self,
        json_rpc_notification: &JsonRpcNotification,
        platform_state: &PlatformState,
        context: Option<Value>,
    );
}

#[derive(Debug, Default)]
pub struct RippleContextUpdateTokenRequest;

impl NotificationEventStrategy for RippleContextUpdateTokenRequest {
    fn handle_notification(
        &self,
        json_rpc_notification: &JsonRpcNotification,
        platform_state: &PlatformState,
        context: Option<Value>,
    ) {
        if let Some(params) = json_rpc_notification.params.clone() {
            if let Ok(token) = serde_json::from_value::<AccountToken>(params) {
                let request = RippleContextUpdateRequest::Token(token);
                ServiceNotificationProcessor::context_update(
                    Event::RippleContextTokenChangedEvent.to_string().as_str(),
                    request,
                    platform_state,
                    context,
                );
            } else {
                error!("Failed to parse token parameters");
            }
        }
    }
}

#[derive(Debug, Default)]
pub struct RippleContextUpdateActivationRequest;

impl NotificationEventStrategy for RippleContextUpdateActivationRequest {
    fn handle_notification(
        &self,
        json_rpc_notification: &JsonRpcNotification,
        platform_state: &PlatformState,
        context: Option<Value>,
    ) {
        if let Some(params) = json_rpc_notification.params.clone() {
            if let Ok(status) = serde_json::from_value::<ActivationStatus>(params) {
                let request = RippleContextUpdateRequest::Activation(status.into());
                ServiceNotificationProcessor::context_update(
                    "RippleContextActivationChangedEvent",
                    request,
                    platform_state,
                    context,
                );
            } else {
                error!("Failed to parse activation parameters");
            }
        }
    }
}

#[derive(Debug, Default)]
pub struct RippleContextUpdateInternetStatusRequest;

impl NotificationEventStrategy for RippleContextUpdateInternetStatusRequest {
    fn handle_notification(
        &self,
        json_rpc_notification: &JsonRpcNotification,
        platform_state: &PlatformState,
        context: Option<Value>,
    ) {
        if let Some(params) = json_rpc_notification.params.clone() {
            if let Ok(status) = serde_json::from_value::<InternetConnectionStatus>(params) {
                let request = RippleContextUpdateRequest::InternetStatus(status);
                ServiceNotificationProcessor::context_update(
                    "RippleContextInternetStatusChangedEvent",
                    request,
                    platform_state,
                    context,
                );
            } else {
                error!("Failed to parse internet_status parameters");
            }
        }
    }
}

#[derive(Debug, Default)]
pub struct RippleContextUpdatePowerStateRequest;

impl NotificationEventStrategy for RippleContextUpdatePowerStateRequest {
    fn handle_notification(
        &self,
        json_rpc_notification: &JsonRpcNotification,
        platform_state: &PlatformState,
        context: Option<Value>,
    ) {
        if let Some(params) = json_rpc_notification.params.clone() {
            if let Ok(power_state) = serde_json::from_value::<SystemPowerState>(params) {
                let request = RippleContextUpdateRequest::PowerState(power_state);
                ServiceNotificationProcessor::context_update(
                    "RippleContextPowerStateChangedEvent",
                    request,
                    platform_state,
                    context,
                );
            } else {
                error!("Failed to parse power_state parameters");
            }
        }
    }
}

#[derive(Debug, Default)]
pub struct RippleContextUpdateTimeZoneRequest;

impl NotificationEventStrategy for RippleContextUpdateTimeZoneRequest {
    fn handle_notification(
        &self,
        json_rpc_notification: &JsonRpcNotification,
        platform_state: &PlatformState,
        context: Option<Value>,
    ) {
        if let Some(params) = json_rpc_notification.params.clone() {
            if let Ok(time_zone) = serde_json::from_value::<TimeZone>(params) {
                let request = RippleContextUpdateRequest::TimeZone(time_zone);
                ServiceNotificationProcessor::context_update(
                    "RippleContextTimeZoneChangedEvent",
                    request,
                    platform_state,
                    context,
                );
            } else {
                error!("Failed to parse time_zone parameters");
            }
        }
    }
}

#[derive(Debug, Default)]
pub struct RippleContextUpdateFeaturesRequest;

impl NotificationEventStrategy for RippleContextUpdateFeaturesRequest {
    fn handle_notification(
        &self,
        json_rpc_notification: &JsonRpcNotification,
        platform_state: &PlatformState,
        context: Option<Value>,
    ) {
        if let Some(params) = json_rpc_notification.params.clone() {
            if let Ok(features) = serde_json::from_value::<Vec<FeatureUpdate>>(params) {
                let request = RippleContextUpdateRequest::UpdateFeatures(features);
                ServiceNotificationProcessor::context_update(
                    "RippleContextUpdateFeaturesChangedEvent",
                    request,
                    platform_state,
                    context,
                );
            } else {
                error!("Failed to parse features parameters");
            }
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct ServiceNotificationProcessor {
    pub notification_strategies:
        Arc<Mutex<HashMap<NotificationEvent, Box<dyn NotificationEventStrategy>>>>,
}

impl ServiceNotificationProcessor {
    pub fn new() -> Self {
        let mut strategies = HashMap::new();

        strategies.insert(
            NotificationEvent::RippleContextUpdateTokenRequest,
            Box::new(RippleContextUpdateTokenRequest) as Box<dyn NotificationEventStrategy>,
        );
        strategies.insert(
            NotificationEvent::RippleContextUpdateActivationRequest,
            Box::new(RippleContextUpdateActivationRequest) as Box<dyn NotificationEventStrategy>,
        );
        strategies.insert(
            NotificationEvent::RippleContextUpdateInternetStatusRequest,
            Box::new(RippleContextUpdateInternetStatusRequest)
                as Box<dyn NotificationEventStrategy>,
        );
        strategies.insert(
            NotificationEvent::RippleContextUpdatePowerStateRequest,
            Box::new(RippleContextUpdatePowerStateRequest) as Box<dyn NotificationEventStrategy>,
        );
        strategies.insert(
            NotificationEvent::RippleContextUpdateTimeZoneRequest,
            Box::new(RippleContextUpdateTimeZoneRequest) as Box<dyn NotificationEventStrategy>,
        );
        strategies.insert(
            NotificationEvent::RippleContextUpdateFeaturesRequest,
            Box::new(RippleContextUpdateFeaturesRequest) as Box<dyn NotificationEventStrategy>,
        );

        ServiceNotificationProcessor {
            notification_strategies: Arc::new(Mutex::new(strategies)),
        }
    }

    pub async fn process_service_notification(
        self,
        json_rpc_notification: &JsonRpcNotification,
        platform_state: &PlatformState,
        context: Option<Value>,
    ) {
        debug!(
            "Received service notification: {:#?}",
            json_rpc_notification
        );
        if let Some((_context_update, event)) = json_rpc_notification.method.split_once(".") {
            let event: String = format!("\"{}\"", event);

            // json_rpc_notifiction method is like "ripple.RippleContextTokenChangedEvent"
            // We need to extract "RippleContextTokenChangedEvent" and convert it to NotificationEvent enum
            // Then we can use it to get the appropriate strategy from the map

            let notification_event = serde_json::from_str::<NotificationEvent>(&event);
            match notification_event {
                Err(e) => {
                    error!("Invalid event type error: {}", e);
                }
                Ok(event) => {
                    let strategies = self.notification_strategies.lock().await;
                    if let Some(strategy) = strategies.get(&event) {
                        strategy.handle_notification(
                            json_rpc_notification,
                            platform_state,
                            context,
                        );
                    } else {
                        error!("No strategy found for event: {:?}", event.clone());
                    }
                }
            };
        }
    }

    pub fn context_update(
        event: &str,
        request: RippleContextUpdateRequest,
        platform_state: &PlatformState,
        _context: Option<Value>,
    ) {
        let propagate = {
            if let Ok(mut ripple_context) = platform_state
                .service_controller_state
                .service_event_state
                .ripple_context
                .write()
            {
                debug!(
                    "Received context update request: {:?} current ripple_context: {:?} event: {}",
                    request, ripple_context, event
                );
                ripple_context.update(request.clone())
            } else {
                error!("Failed to acquire write lock for ripple_context");
                false
            }
        };
        let new_ripple_context = {
            if let Ok(r) = platform_state
                .service_controller_state
                .service_event_state
                .ripple_context
                .read()
            {
                r.clone()
            } else {
                error!("Failed to acquire read lock for ripple_context");
                return;
            }
        };
        if propagate {
            //let event_str = event.to_string();
            let processors = match platform_state
                .service_controller_state
                .service_event_state
                .get_event_processors(&event)
            {
                Ok(p) if !p.is_empty() => {
                    debug!(
                        "Found {} subscribers for event: {}",
                        p.len(),
                        event.to_string()
                    );
                    p
                }
                _ => {
                    warn!("No subscribers found for event: {}", event.to_string());
                    return;
                }
            };
            for processor in processors {
                let processor = processor.clone();
                let event_str = event.to_string();
                match processor {
                    EventSubscriber::MainSubscriber(s) => {
                        if let Ok(new_ripple_context) = serde_json::to_string(&new_ripple_context) {
                            tokio::spawn(async move {
                                let service_message = ServiceMessage {
                                    message: JsonRpcMessage::Notification(JsonRpcNotification {
                                        jsonrpc: "2.0".to_string(),
                                        method: format!("ripple.{}", event_str),
                                        params: Some(new_ripple_context.into()),
                                    }),
                                    context: None,
                                };
                                let _ = s.send(service_message).await;
                            });
                        } else {
                            error!("Failed to serialize new_ripple_context for MainSubscriber");
                        }
                    }
                    EventSubscriber::ServiceSubscriber(s) => {
                        let collect = s.split('&').collect::<Vec<&str>>();
                        let processor_arr = collect
                            .iter()
                            .map(|s| s.to_string())
                            .collect::<Vec<String>>();
                        if let (Some(sender_id), Some(service_id)) =
                            (processor_arr.first(), processor_arr.get(1))
                        {
                            let sender_id = sender_id.to_string();
                            let service_id = service_id.to_string();
                            let service_controller_state =
                                platform_state.service_controller_state.clone();

                            if let Ok(new_ripple_context) =
                                serde_json::to_string(&new_ripple_context)
                            {
                                let event_str = event_str.clone();
                                tokio::spawn(async move {
                                    if let Some(sender) =
                                        service_controller_state.get_sender(&service_id).await
                                    {
                                        let params = json!({"ripple_context": new_ripple_context, "sender_id": sender_id});
                                        let service_message = ServiceMessage {
                                            message: JsonRpcMessage::Notification(
                                                JsonRpcNotification {
                                                    jsonrpc: "2.0".to_string(),
                                                    method: format!("ripple.{}", event_str),
                                                    params: Some(params),
                                                },
                                            ),
                                            context: None,
                                        };
                                        if let Ok(msg_str) = serde_json::to_string(&service_message)
                                        {
                                            let mes = Message::Text(msg_str.clone());
                                            debug!(
                                                "Sending context update to service: {} message: {}",
                                                s, msg_str
                                            );
                                            if let Err(e) = sender.try_send(mes) {
                                                error!(
                                                    "Failed to send service notification: {:?}",
                                                    e
                                                );
                                            }
                                        } else {
                                            error!("Failed to serialize service_message for ServiceSubscriber");
                                        }
                                    } else {
                                        error!("No sender found for service_id: {}", service_id);
                                    }
                                });
                            } else {
                                error!(
                                    "Failed to serialize new_ripple_context for ServiceSubscriber"
                                );
                            }
                        } else {
                            error!("Invalid processor_arr: missing sender_id or service_id");
                        }
                    }
                }
            }
        } else {
            debug!("Context information is already updated. Hence not propagating");
        }
    }
}
