use crate::state::platform_state::PlatformState;
use crate::tokio;
use ripple_sdk::api::context::{RippleContextUpdateRequest, RippleContextUpdateType};
use ripple_sdk::api::device::device_request::AccountToken;
use ripple_sdk::service::service_message::{JsonRpcMessage, JsonRpcNotification, ServiceMessage};
use ripple_sdk::{
    log::{debug, error, trace},
    tokio_tungstenite::tungstenite::Message,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub enum NotificationEvent {
    RippleContextEvent,
    RippleContextUpdateRequest,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum NotificationUpdateType {
    Token,
    Activation,
    InternetStatus,
    PowerState,
    TimeZone,
    UpdateFeatures,
}

pub struct ServiceNotificationProcessor;

impl Default for ServiceNotificationProcessor {
    fn default() -> Self {
        Self::new()
    }
}

impl ServiceNotificationProcessor {
    pub fn new() -> Self {
        ServiceNotificationProcessor {}
    }

    pub fn process_service_notification(
        json_rpc_notification: &JsonRpcNotification,
        state: &PlatformState,
        context: Option<Value>,
    ) {
        debug!(
            "Received service notification: {:#?}",
            json_rpc_notification
        );
        if let Some((context_update, update_type)) = json_rpc_notification.method.split_once(".") {
            debug!(
                "Received service event notification request event type {:?} update type {:?}",
                context_update, update_type
            );
            let context_update = format!("\"{}\"", context_update);
            let notification_event = serde_json::from_str::<NotificationEvent>(&context_update);

            match notification_event {
                Ok(NotificationEvent::RippleContextEvent) => {
                    state
                        .service_controller_state
                        .service_event_state
                        .subscribe_context_event(update_type, context.clone());
                }
                Ok(NotificationEvent::RippleContextUpdateRequest) => {
                    Self::process_event_notification(
                        state,
                        update_type,
                        json_rpc_notification,
                        context.clone(),
                    );
                }
                Err(e) => {
                    error!(
                        "Invalid context update request: {} error: {}",
                        context_update, e
                    );
                }
            }
            debug!(
                " service event subscribers: {:?}",
                state
                    .service_controller_state
                    .service_event_state
                    .event_subscribers
            );
        } else {
            error!("Invalid service event request format");
        }
    }

    pub fn process_event_notification(
        platform_state: &PlatformState,
        update_type: &str,
        json_rpc_notification: &JsonRpcNotification,
        context: Option<Value>,
    ) {
        debug!("process service notification: {:?}", json_rpc_notification);
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
            println!("&&&& Propagating update_type_str: {}", update_type_str);
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
                        // let context = context.clone();

                        let new_ripple_context =
                            serde_json::to_string(&new_ripple_context).unwrap();

                        tokio::spawn(async move {
                            if let Some(sender) =
                                service_controller_state.get_sender(&service_id).await
                            {
                                //let context = vec![sender_id, Some(&service_id), Some(&request_type)];
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
