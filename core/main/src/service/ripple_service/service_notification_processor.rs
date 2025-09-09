use crate::state::platform_state::PlatformState;
use jsonrpsee::core::async_trait;
use ripple_sdk::api::context::RippleContextUpdateType;
use ripple_sdk::api::{context::RippleContextUpdateRequest, device::device_request::AccountToken};
use ripple_sdk::{
    log::{debug, error, trace},
    service::service_message::{JsonRpcMessage, JsonRpcNotification, ServiceMessage},
    tokio,
    tokio::sync::Mutex,
    tokio_tungstenite::tungstenite::Message,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::HashMap, sync::Arc};

#[derive(Serialize, Deserialize, Debug, Clone, Eq, Hash, PartialEq)]
pub enum NotificationEvent {
    RippleContextEvent,
    RippleContextUpdateRequest,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, Hash, PartialEq)]
pub enum NotificationUpdateType {
    Token,
    Activation,
    InternetStatus,
    PowerState,
    TimeZone,
    UpdateFeatures,
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

// Implement strategies for each notification type
#[derive(Debug, Default)]
pub struct ContextEventNotificationStrategy;
impl NotificationEventStrategy for ContextEventNotificationStrategy {
    fn handle_notification(
        &self,
        json_rpc_notification: &JsonRpcNotification,
        platform_state: &PlatformState,
        context: Option<Value>,
    ) {
        if let Some((_context_update, update_type)) = json_rpc_notification.method.split_once(".") {
            platform_state
                .service_controller_state
                .service_event_state
                .subscribe_context_event(update_type, context.clone());
        } else {
            error!(
                "Invalid method format in notification: {}",
                json_rpc_notification.method
            );
        }
    }
}

#[derive(Debug, Default)]
pub struct ContextUpdateEventNotificationStrategy;

impl NotificationEventStrategy for ContextUpdateEventNotificationStrategy {
    fn handle_notification(
        &self,
        json_rpc_notification: &JsonRpcNotification,
        platform_state: &PlatformState,
        context: Option<Value>,
    ) {
        if let Some((_context_update, update_type)) = json_rpc_notification.method.split_once(".") {
            platform_state
                .service_controller_state
                .service_notification_processor
                .clone()
                .process_event_notification(
                    platform_state,
                    update_type,
                    json_rpc_notification,
                    context.clone(),
                );
        } else {
            error!(
                "Invalid method format in notification: {}",
                json_rpc_notification.method
            );
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
            NotificationEvent::RippleContextEvent,
            Box::new(ContextEventNotificationStrategy) as Box<dyn NotificationEventStrategy>,
        );
        strategies.insert(
            NotificationEvent::RippleContextUpdateRequest,
            Box::new(ContextUpdateEventNotificationStrategy) as Box<dyn NotificationEventStrategy>,
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
        if let Some((context_update, _update_type)) = json_rpc_notification.method.split_once(".") {
            let context_update: String = format!("\"{}\"", context_update);

            // json_rpc_notifiction method is like "RippleContextEvent.TokenChanged"
            // We need to extract "RippleContextEvent" and convert it to NotificationEvent enum
            // Then we can use it to get the appropriate strategy from the map

            let notification_event = serde_json::from_str::<NotificationEvent>(&context_update);
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

    fn process_event_notification(
        self,
        platform_state: &PlatformState,
        update_type: &str,
        json_rpc_notification: &JsonRpcNotification,
        context: Option<Value>,
    ) {
        debug!("process event notification: {:?}", json_rpc_notification);
        let params = json_rpc_notification.params.clone();
        let update_type_str = format!("\"{}\"", update_type);
        let notification_update_type =
            serde_json::from_str::<NotificationUpdateType>(&update_type_str);
        if let Some(params) = params {
            match notification_update_type {
                Ok(NotificationUpdateType::Token) => {
                    if let Ok(token) = serde_json::from_value::<AccountToken>(params.clone()) {
                        let request = RippleContextUpdateRequest::Token(token);
                        Self::context_update(update_type, request, platform_state, context);
                    } else {
                        error!("Failed to parse token parameters: {:?}", params);
                    }
                }
                Ok(NotificationUpdateType::Activation) => {
                    todo!()
                    // if let Ok(status) = serde_json::from_value::<ActivationStatus>(params.clone()) {
                    //     request = RippleContextUpdateRequest::Activation(status);
                    //     Self::context_update(update_type, request, platform_state, sm);
                    // } else {
                    //     error!("Failed to parse token parameters: {:?}", params);
                    // }
                }
                Ok(NotificationUpdateType::InternetStatus) => {
                    todo!()
                    // if let Ok(status) = serde_json::from_value::<InternetConnectionStatus>(params.clone()) {
                    //     request = RippleContextUpdateRequest::InternetStatus(status);
                    //     Self::context_update(update_type, request, platform_state, sm);
                    // } else {
                    //     error!("Failed to parse token parameters: {:?}", params);
                    // }
                }
                Ok(NotificationUpdateType::PowerState) => {
                    todo!()
                    // if let Ok(state) = serde_json::from_value::<PowerState>(params.clone()) {
                    //         request = RippleContextUpdateRequest::PowerState(state);
                    // } else {
                    //     error!("Failed to parse token parameters: {:?}", params);
                    // }
                }
                Ok(NotificationUpdateType::TimeZone) => {
                    todo!()
                    // if let Ok(tz) = serde_json::from_value::<TimeZone>(params.clone()) {
                    //         request = RippleContextUpdateRequest::TimeZone(tz);
                    // } else {
                    //     error!("Failed to parse token parameters: {:?}", params);
                    // }
                }
                Ok(NotificationUpdateType::UpdateFeatures) => {
                    todo!()
                    // if let Ok(features) = serde_json::from_value::<FeatureUpdate>(params.clone()) {
                    //         request = RippleContextUpdateRequest::UpdateFeatures(features);
                    // } else {
                    //     error!("Failed to parse token parameters: {:?}", params);
                    // }
                }
                Err(e) => {
                    error!("Invalid update type error: {}", e);
                }
            }
        } else {
            error!("parameters are missing in the message");
        }
    }

    pub fn context_update(
        update_type: &str,
        request: RippleContextUpdateRequest,
        platform_state: &PlatformState,
        _context: Option<Value>,
    ) {
        let propagate = {
            let mut ripple_context = platform_state
                .service_controller_state
                .service_event_state
                .ripple_context
                .write()
                .unwrap();
            debug!(
                "Received context update request: {:?} current ripple_context: {:?}",
                request, ripple_context
            );
            ripple_context.update(request.clone())
        };
        let new_ripple_context = {
            platform_state
                .service_controller_state
                .service_event_state
                .ripple_context
                .read()
                .unwrap()
                .clone()
        };
        if propagate {
            let update_type_str = format!("\"{}Changed\"", update_type);
            debug!(
                "Context update. Propagating updated type: {}",
                update_type_str
            );
            let ripple_context_update_type =
                serde_json::from_str::<RippleContextUpdateType>(&update_type_str);
            match ripple_context_update_type {
                Ok(context_update_type) => {
                    let processors = platform_state
                        .service_controller_state
                        .service_event_state
                        .get_event_processors(Some(context_update_type.clone()));
                    for processor in processors {
                        debug!(
                            "Subscriber found for update type: {:?} subscriber: {:?}",
                            update_type, processor
                        );

                        let processor = processor.clone();
                        let collect = processor.split('&').collect::<Vec<&str>>();
                        let processor_arr = collect
                            .iter()
                            .map(|s| s.to_string())
                            .collect::<Vec<String>>();
                        let sender_id = processor_arr.first().unwrap().to_string();
                        let service_id = processor_arr.get(1).unwrap().to_string();
                        let request_type = processor_arr.get(2).unwrap().to_string();

                        let service_controller_state =
                            platform_state.service_controller_state.clone();

                        let new_ripple_context =
                            serde_json::to_string(&new_ripple_context).unwrap();

                        tokio::spawn(async move {
                            if let Some(sender) =
                                service_controller_state.get_sender(&service_id).await
                            {
                                let context = vec![sender_id, service_id, request_type];
                                let service_message = ServiceMessage {
                                    message: JsonRpcMessage::Notification(JsonRpcNotification {
                                        jsonrpc: "2.0".to_string(),
                                        method: "service.eventNotification".to_string(),
                                        params: Some(new_ripple_context.into()),
                                    }),
                                    context: Some(context.into()),
                                };
                                let msg_str = serde_json::to_string(&service_message).unwrap();
                                let mes = Message::Text(msg_str.clone());
                                let send_res = sender.try_send(mes);
                                trace!("Send to processor result: {:?}", send_res);
                            }
                        });
                    }
                }
                Err(e) => {
                    error!("Failed to parse update type: {}", e);
                }
            }
        } else {
            trace!("Context information is already updated. Hence not propagating");
        }
    }
}
