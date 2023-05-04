// If not stated otherwise in this file or this component's license file the
// following copyright and licenses apply:
//
// Copyright 2023 RDK Management
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

use std::str::FromStr;

use serde::{Deserialize, Serialize};
use thunder_ripple_sdk::{
    client::thunder_plugin::ThunderPlugin,
    events::thunder_event_processor::{ThunderEventHandler, ThunderEventHandlerProvider},
    ripple_sdk::{
        log::{error,debug},
        serde_json::{self, Value},
        tokio, api::device::device_request::{NetworkResponse, NetworkState, NetworkType, PowerState, SystemPowerState},
    },
    thunder_state::ThunderState,
};

use super::super::thunder_device_info::{
    get_audio, get_dimension_from_resolution, ThunderDeviceInfoRequestProcessor,
};


// -----------------------
// Active Input Changed
// Taken from Ripple
#[derive(Clone)]
struct HDCPEventHandler {}

#[async_trait]
impl DabEventHandler for HDCPEventHandler {
    async fn handle_dab_event(
        &self,
        ps: &PlatformState,
        cur_value: &mut Option<Value>,
        _dab_event: &DabEvent,
    ) {
        let resp = ps
            .services
            .send_dab(DabRequestPayload::Device(DeviceRequest::HdcpSupport))
            .await;

        match resp {
            Ok(dab_payload) => match dab_payload {
                DabResponsePayload::HdcpSupportResponse(value) => {
                    let new_val = json!(value.clone());
                    if !cur_value
                        .clone()
                        .unwrap()
                        .to_string()
                        .eq(&new_val.to_string())
                    {
                        AppEvents::emit(&ps, &HDCP_CHANGED_EVENT.to_string(), &json!(value)).await;
                        *cur_value = Some(new_val);
                    }
                }
                _ => {}
            },
            Err(_e) => {}
        }
    }

    fn generate_dab_event_subscribe_request(
        &self,
        ctx: Option<CallContext>,
        listen: bool,
    ) -> DabRequestPayload {
        let subscribe_message = DabSubscribeMessage {
            subscribe: listen,
            context: ctx.map(|v| serde_json::to_string(&v).unwrap()),
        };

        DabRequestPayload::Device(DeviceRequest::OnActiveInputChanged(subscribe_message))
    }

    fn get_mapped_dab_event_name(&self) -> &str {
        EVENT_ACTIVE_INPUT_CHANGED
    }

    fn dab_event_handler_clone(&self) -> Box<dyn DabEventHandler + Send + Sync> {
        Box::new(self.clone())
    }
}

// Taken from DAB
#[derive(Clone)]
struct ActiveInputChangeEventResolver {}

#[async_trait]
impl ThunderEventResolver for ActiveInputChangeEventResolver {
    fn resolve_dab_event(&self, resp: &ThunderResponseMessage, event_name: &str) -> DabEvent {
        let dab_event = match event_name {
            "activeInputChanged" => DabEvent::Device(DeviceEvent::ActiveInputChangedEvent(
                resp.message["activeInput"].as_bool().unwrap_or(true),
            )),
            _ => DabEvent::None,
        };
        dab_event
    }

    fn parse_subscribe_params(&self, request: &DabRequest) -> DabSubscribeMessage {
        // Parse and collect the subscribe params
        let mut sub_message = DabSubscribeMessage {
            subscribe: false,
            context: None,
        };

        match request.payload {
            DabRequestPayload::Device(DeviceRequest::OnActiveInputChanged(ref s)) => {
                sub_message.subscribe = s.subscribe;
            }
            _ => {}
        };
        sub_message
    }
    fn thunder_event_resolver_clone(&self) -> Box<dyn ThunderEventResolver + Send + Sync> {
        Box::new(self.clone())
    }
}



// -----------------------
// HDR Changed

// Taken from Ripple
#[derive(Clone)]
struct HDREventHandler {}

#[async_trait]
impl DabEventHandler for HDREventHandler {
    async fn handle_dab_event(
        &self,
        ps: &PlatformState,
        cur_value: &mut Option<Value>,
        _dab_event: &DabEvent,
    ) {
        let resp = ps
            .services
            .send_dab(DabRequestPayload::Device(DeviceRequest::Hdr))
            .await;

        match resp {
            Ok(dab_payload) => match dab_payload {
                DabResponsePayload::HdrResponse(value) => {
                    let new_val = json!(value.clone());
                    if !cur_value
                        .clone()
                        .unwrap()
                        .to_string()
                        .eq(&new_val.to_string())
                    {
                        AppEvents::emit(&ps, &HDR_CHANGED_EVENT.to_string(), &json!(value)).await;
                        *cur_value = Some(new_val);
                    }
                }
                _ => {}
            },
            Err(_e) => {}
        }
    }

    fn generate_dab_event_subscribe_request(
        &self,
        ctx: Option<CallContext>,
        listen: bool,
    ) -> DabRequestPayload {
        let subscribe_message = DabSubscribeMessage {
            subscribe: listen,
            context: ctx.map(|v| serde_json::to_string(&v).unwrap()),
        };

        DabRequestPayload::Device(DeviceRequest::OnActiveInputChanged(subscribe_message))
    }

    fn get_mapped_dab_event_name(&self) -> &str {
        EVENT_ACTIVE_INPUT_CHANGED
    }

    fn dab_event_handler_clone(&self) -> Box<dyn DabEventHandler + Send + Sync> {
        Box::new(self.clone())
    }
}

// -----------------------
// ScreenResolution Changed

// Taken from Ripple

#[derive(Clone)]
struct ScreenResolutionEventHandler {}

#[async_trait]
impl DabEventHandler for ScreenResolutionEventHandler {
    async fn handle_dab_event(
        &self,
        ps: &PlatformState,
        _cur_value: &mut Option<Value>,
        dab_event: &DabEvent,
    ) {
        match dab_event {
            DabEvent::Device(DeviceEvent::ResolutionChangedEvent(event_data)) => {
                let screen_resolution = vec![event_data.width, event_data.height];
                AppEvents::emit(
                    &ps,
                    &SCREEN_RESOLUTION_CHANGED_EVENT.to_string(),
                    &serde_json::to_value(screen_resolution).unwrap(),
                )
                .await;
            }

            _ => {
                error!("Invalid dab_event received");
            }
        }
    }

    fn generate_dab_event_subscribe_request(
        &self,
        ctx: Option<CallContext>,
        listen: bool,
    ) -> DabRequestPayload {
        let subscribe_message = DabSubscribeMessage {
            subscribe: listen,
            context: ctx.map(|v| serde_json::to_string(&v).unwrap()),
        };

        DabRequestPayload::Device(DeviceRequest::OnResolutionChanged(subscribe_message))
    }

    fn get_mapped_dab_event_name(&self) -> &str {
        EVENT_RESOLUTION_CHANGED
    }

    fn dab_event_handler_clone(&self) -> Box<dyn DabEventHandler + Send + Sync> {
        Box::new(self.clone())
    }
}

// Taken from Dab

#[derive(Clone)]
struct ResolutionChangeEventResolver {}

#[async_trait]
impl ThunderEventResolver for ResolutionChangeEventResolver {
    fn resolve_dab_event(&self, resp: &ThunderResponseMessage, event_name: &str) -> DabEvent {
        let dab_event = match event_name {
            "resolutionChanged" => DabEvent::Device(DeviceEvent::ResolutionChangedEvent(
                ResolutionChangedEventData {
                    width: resp.message["width"].as_u64().unwrap_or_default(),
                    height: resp.message["height"].as_u64().unwrap_or_default(),
                    video_display_type: resp.message["videoDisplayType"]
                        .as_str()
                        .unwrap_or_default()
                        .to_owned(),
                    resolution: resp.message["resolution"]
                        .as_str()
                        .unwrap_or_default()
                        .to_owned(),
                },
            )),
            _ => DabEvent::None,
        };
        dab_event
    }

    fn parse_subscribe_params(&self, request: &DabRequest) -> DabSubscribeMessage {
        // Parse and collect the subscribe params
        let mut sub_message = DabSubscribeMessage {
            subscribe: false,
            context: None,
        };

        match request.payload {
            DabRequestPayload::Device(DeviceRequest::OnResolutionChanged(ref s)) => {
                sub_message.subscribe = s.subscribe;
            }
            _ => {}
        };
        sub_message
    }
    fn thunder_event_resolver_clone(&self) -> Box<dyn ThunderEventResolver + Send + Sync> {
        Box::new(self.clone())
    }
}


// -----------------------
// VideoResolution Changed

// Taken from Ripple
#[derive(Clone)]
struct VideoResolutionEventHandler {}

#[async_trait]
impl DabEventHandler for VideoResolutionEventHandler {
    async fn handle_dab_event(
        &self,
        ps: &PlatformState,
        _cur_value: &mut Option<Value>,
        dab_event: &DabEvent,
    ) {
        match dab_event {
            DabEvent::Device(DeviceEvent::ResolutionChangedEvent(event_data)) => {
                let video_resolution = get_dimension_from_resolution(&event_data.resolution);
                AppEvents::emit(
                    &ps,
                    &VIDEO_RESOLUTION_CHANGED_EVENT.to_string(),
                    &serde_json::to_value(video_resolution).unwrap(),
                )
                .await;
            }

            _ => {
                error!("Invalid dab_event received");
            }
        }
    }

    fn generate_dab_event_subscribe_request(
        &self,
        ctx: Option<CallContext>,
        listen: bool,
    ) -> DabRequestPayload {
        let subscribe_message = DabSubscribeMessage {
            subscribe: listen,
            context: ctx.map(|v| serde_json::to_string(&v).unwrap()),
        };

        DabRequestPayload::Device(DeviceRequest::OnResolutionChanged(subscribe_message))
    }

    fn get_mapped_dab_event_name(&self) -> &str {
        EVENT_RESOLUTION_CHANGED
    }

    fn dab_event_handler_clone(&self) -> Box<dyn DabEventHandler + Send + Sync> {
        Box::new(self.clone())
    }
}

// -----------------------
// Network Changed

// Taken from Ripple
#[derive(Clone)]
struct NetworkEventHandler {}

#[async_trait]
impl DabEventHandler for NetworkEventHandler {
    async fn handle_dab_event(
        &self,
        ps: &PlatformState,
        _cur_value: &mut Option<Value>,
        dab_event: &DabEvent,
    ) {
        match dab_event {
            DabEvent::Device(DeviceEvent::NetworkChangedEvent(network_response)) => {
                AppEvents::emit(
                    &ps,
                    &NETWORK_CHANGED_EVENT.to_string(),
                    &serde_json::to_value(network_response).unwrap(),
                )
                .await;
            }
            _ => {
                error!("Invalid dab_event received");
            }
        }
    }

    fn generate_dab_event_subscribe_request(
        &self,
        ctx: Option<CallContext>,
        listen: bool,
    ) -> DabRequestPayload {
        let subscribe_message = DabSubscribeMessage {
            subscribe: listen,
            context: ctx.map(|v| serde_json::to_string(&v).unwrap()),
        };

        DabRequestPayload::Device(DeviceRequest::OnNetworkChanged(subscribe_message))
    }

    fn get_mapped_dab_event_name(&self) -> &str {
        EVENT_NETWORK_CHANGED
    }

    fn dab_event_handler_clone(&self) -> Box<dyn DabEventHandler + Send + Sync> {
        Box::new(self.clone())
    }
}

// Taken from Dab
#[derive(Clone)]
struct NetworkChangeEventResolver {}

#[async_trait]
impl ThunderEventResolver for NetworkChangeEventResolver {
    fn resolve_dab_event(&self, resp: &ThunderResponseMessage, event_name: &str) -> DabEvent {
        let dab_event = match event_name {
            "onConnectionStatusChanged" => {
                DabEvent::Device(DeviceEvent::NetworkChangedEvent(NetworkResponse {
                    _type: NetworkType::from_str(
                        resp.message["interface"].as_str().unwrap_or_default(),
                    )
                    .unwrap(),
                    state: NetworkState::from_str(
                        resp.message["status"].as_str().unwrap_or_default(),
                    )
                    .unwrap(),
                }))
            }
            _ => DabEvent::None,
        };
        dab_event
    }

    fn parse_subscribe_params(&self, request: &DabRequest) -> DabSubscribeMessage {
        // Parse and collect the subscribe params
        let mut sub_message = DabSubscribeMessage {
            subscribe: false,
            context: None,
        };

        match request.payload {
            DabRequestPayload::Device(DeviceRequest::OnNetworkChanged(ref s)) => {
                sub_message.subscribe = s.subscribe;
            }
            _ => {}
        };
        sub_message
    }
    fn thunder_event_resolver_clone(&self) -> Box<dyn ThunderEventResolver + Send + Sync> {
        Box::new(self.clone())
    }
}

// -----------------------
// SystemPower Changed

// Taken from Ripple
#[derive(Clone)]
struct SystemPowerStateChangeEventHandler {}

#[async_trait]
impl DabEventHandler for SystemPowerStateChangeEventHandler {
    async fn handle_dab_event(
        &self,
        ps: &PlatformState,
        _cur_value: &mut Option<Value>,
        dab_event: &DabEvent,
    ) {
        match dab_event {
            DabEvent::Device(DeviceEvent::SystemPowerStateChangedEvent(
                system_power_state_event_data,
            )) => {
                debug!(
                    "SystemPowerStateChangedEvent Received {:?}",
                    system_power_state_event_data
                );
                if system_power_state_event_data.power_state != PowerState::On {
                    // clear any user Grants with LifeSpan value as PowerActive
                    if UserGrantStateUtils::delete_all_matching_entries(&ps.grant_state, |entry| {
                        entry
                            .lifespan
                            .as_ref()
                            .map_or(false, |lifespan| *lifespan == Lifespan::PowerActive)
                    }) {
                        let _ =
                            UserGrantStateUtils::check_and_update_dab_event_subscription(ps).await;
                    }
                }
            }
            _ => {
                error!("Invalid dab_event received");
            }
        }
    }

    fn generate_dab_event_subscribe_request(
        &self,
        _ctx: Option<CallContext>,
        listen: bool,
    ) -> DabRequestPayload {
        let subscribe_message = DabSubscribeMessage {
            subscribe: listen,
            context: None,
        };

        DabRequestPayload::Device(DeviceRequest::OnSystemPowerStateChanged(subscribe_message))
    }

    fn get_mapped_dab_event_name(&self) -> &str {
        EVENT_SYSTEM_POWER_STATE_CHANGED
    }

    fn dab_event_handler_clone(&self) -> Box<dyn DabEventHandler + Send + Sync> {
        Box::new(self.clone())
    }
}

// Taken from Dab
#[derive(Clone)]
struct SystemPowerStateChangeEventResolver {}

#[async_trait]
impl ThunderEventResolver for SystemPowerStateChangeEventResolver {
    fn resolve_dab_event(&self, resp: &ThunderResponseMessage, event_name: &str) -> DabEvent {
        let dab_event: DabEvent = match event_name {
            "onSystemPowerStateChanged" => DabEvent::Device(
                DeviceEvent::SystemPowerStateChangedEvent(SystemPowerStateEventData {
                    power_state: PowerState::from_str(
                        resp.message["powerState"].as_str().unwrap_or_default(),
                    )
                    .unwrap(),
                    current_power_state: PowerState::from_str(
                        resp.message["currentPowerState"]
                            .as_str()
                            .unwrap_or_default(),
                    )
                    .unwrap(),
                }),
            ),
            _ => DabEvent::None,
        };
        dab_event
    }

    fn parse_subscribe_params(&self, request: &DabRequest) -> DabSubscribeMessage {
        // Parse and collect the subscribe params
        let mut sub_message = DabSubscribeMessage {
            subscribe: false,
            context: None,
        };

        match request.payload {
            DabRequestPayload::Device(DeviceRequest::OnSystemPowerStateChanged(ref s)) => {
                sub_message.subscribe = s.subscribe;
            }
            _ => {}
        };
        sub_message
    }
    fn thunder_event_resolver_clone(&self) -> Box<dyn ThunderEventResolver + Send + Sync> {
        Box::new(self.clone())
    }
}


// -----------------------
// VoiceGuidance Changed

// Taken from Ripple
#[derive(Clone)]
struct VoiceGuidanceEnabledChangedEventHandler {}

#[async_trait]
impl DabEventHandler for VoiceGuidanceEnabledChangedEventHandler {
    async fn handle_dab_event(
        &self,
        ps: &PlatformState,
        _cur_value: &mut Option<Value>,
        dab_event: &DabEvent,
    ) {
        match dab_event {
            DabEvent::Device(DeviceEvent::VoiceGuidanceEnabledChangedEvent(enabled)) => {
                AppEvents::emit(
                    &ps,
                    &VOICE_GUIDANCE_ENABLED_CHANGED_EVENT.to_string(),
                    &serde_json::to_value(enabled).unwrap(),
                )
                .await;

                AppEvents::emit(
                    &ps,
                    &VOICE_GUIDANCE_SETTINGS_CHANGED_EVENT.to_string(),
                    &serde_json::to_value(enabled).unwrap(),
                )
                .await;
            }

            _ => {
                error!("Invalid dab_event received");
            }
        }
    }

    fn generate_dab_event_subscribe_request(
        &self,
        ctx: Option<CallContext>,
        listen: bool,
    ) -> DabRequestPayload {
        let subscribe_message = DabSubscribeMessage {
            subscribe: listen,
            context: ctx.map(|v| serde_json::to_string(&v).unwrap()),
        };

        DabRequestPayload::Device(DeviceRequest::OnVoiceGuidanceEnabledChanged(
            subscribe_message,
        ))
    }

    fn get_mapped_dab_event_name(&self) -> &str {
        EVENT_VOICE_GUIDANCE_ENABLED_CHANGED
    }

    fn dab_event_handler_clone(&self) -> Box<dyn DabEventHandler + Send + Sync> {
        Box::new(self.clone())
    }
}

// Taken from Dab
#[derive(Clone)]
struct VoiceGuidanceChangedEventResolver {}

#[async_trait]
impl ThunderEventResolver for VoiceGuidanceChangedEventResolver {
    fn resolve_dab_event(&self, resp: &ThunderResponseMessage, event_name: &str) -> DabEvent {
        let dab_event = match event_name {
            "onttsstatechanged" => DabEvent::Device(DeviceEvent::VoiceGuidanceEnabledChangedEvent(
                resp.message["state"].as_bool().unwrap(),
            )),
            _ => DabEvent::None,
        };
        dab_event
    }

    fn parse_subscribe_params(&self, request: &DabRequest) -> DabSubscribeMessage {
        let mut sub_message = DabSubscribeMessage {
            subscribe: false,
            context: None,
        };

        match request.payload {
            DabRequestPayload::Device(DeviceRequest::OnVoiceGuidanceEnabledChanged(ref s)) => {
                sub_message.subscribe = s.subscribe;
            }
            _ => {}
        };
        sub_message
    }

    fn thunder_event_resolver_clone(&self) -> Box<dyn ThunderEventResolver + Send + Sync> {
        Box::new(self.clone())
    }
}
