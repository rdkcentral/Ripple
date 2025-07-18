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

use std::collections::HashMap;

use chrono::Utc;
#[cfg(not(test))]
use log::error;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fmt::Debug;

#[cfg(test)]
use println as error;

use crate::{
    api::{
        apps::AppEventRequest,
        caps::CapsRequest,
        config::{Config, ConfigResponse},
        context::{RippleContext, RippleContextUpdateRequest},
        device::{
            device_events::DeviceEventRequest,
            device_peristence::StorageData,
            device_request::{DeviceRequest, NetworkResponse, TimeZone},
        },
        distributor::{
            distributor_permissions::{PermissionRequest, PermissionResponse},
            distributor_privacy::{PrivacyCloudRequest, PrivacySettingsStoreRequest},
            distributor_usergrants::UserGrantsCloudStoreRequest,
        },
        firebolt::{
            fb_keyboard::{KeyboardSessionRequest, KeyboardSessionResponse},
            fb_lifecycle_management::LifecycleManagementRequest,
            fb_pin::{PinChallengeRequestWithContext, PinChallengeResponse},
            fb_telemetry::{OperationalMetricRequest, TelemetryPayload},
        },
        gateway::rpc_gateway_api::{ApiMessage, ApiProtocol, JsonRpcApiResponse, RpcRequest},
        manifest::device_manifest::AppLibraryEntry,
        session::{AccountSessionRequest, AccountSessionResponse},
        settings::{SettingValue, SettingsRequest},
        status_update::ExtnStatus,
        storage_property::StorageManagerRequest,
        usergrant_entry::UserGrantsStoreRequest,
    },
    framework::ripple_contract::RippleContract,
    utils::error::RippleError,
};

use super::extn_id::ExtnId;

/// Default Message enum for the Communication Channel
/// Message would be either a request or response or event
///
/// Below fields constitute an [ExtnMessage]
///
/// `id` | String | Usually an UUID to identify a specific message |
///
/// `requestor` | [ExtnCapability]| Used by Clients to identify the requestor for the message. Looks something like `ripple:main:internal:rpc` when converted to String for a request coming from `Main`.  |
///
/// `target` | [ExtnCapability]| Used by Clients to identify the target for the message. Something like `ripple:channel:device:info` for the device channel info request|
///
/// `payload` | [ExtnPayload]| Type of payload could be [ExtnRequest], [ExtnResponse] or [ExtnEvent]
///

#[derive(Debug, Clone, Default)]
pub struct ExtnMessage {
    pub id: String,
    pub requestor: ExtnId,
    pub target: RippleContract,
    pub target_id: Option<ExtnId>,
    pub payload: ExtnPayload,
    pub ts: Option<i64>,
}

impl ExtnMessage {
    /// This method can be used to create [ExtnResponse] payload message from a given [ExtnRequest]
    /// payload.
    ///
    /// Note: If used in a processor this method can be safely unwrapped
    pub fn get_response(&self, response: ExtnResponse) -> Result<ExtnMessage, RippleError> {
        match self.payload {
            ExtnPayload::Request(_) => Ok(ExtnMessage {
                id: self.id.clone(),
                payload: ExtnPayload::Response(response),
                requestor: self.requestor.clone(),
                target: self.target.clone(),
                target_id: self.target_id.clone(),
                ts: Some(Utc::now().timestamp_millis()),
            }),
            _ => {
                error!("can only respond for a request message");
                Err(RippleError::InvalidInput)
            }
        }
    }

    /// This method can be used to create [ExtnEvent] payload message from a given [ExtnRequest]
    /// payload.
    ///
    /// Note: If used in a processor this method can be safely unwrapped
    pub fn get_event(&self, event: ExtnEvent) -> Result<ExtnMessage, RippleError> {
        match self.payload {
            ExtnPayload::Request(_) => Ok(ExtnMessage {
                id: self.id.clone(),
                payload: ExtnPayload::Event(event),
                requestor: self.requestor.clone(),
                target: self.target.clone(),
                target_id: self.target_id.clone(),
                ts: None,
            }),
            _ => {
                error!("can only event for a request message");
                Err(RippleError::InvalidInput)
            }
        }
    }

    pub fn ack(&self) -> ExtnMessage {
        ExtnMessage {
            id: self.id.clone(),
            requestor: self.requestor.clone(),
            target: self.target.clone(),
            target_id: self.target_id.clone(),
            payload: ExtnPayload::Response(ExtnResponse::None(())),
            ts: None,
        }
    }
    pub fn as_value(&self) -> Option<Value> {
        if let Some(ExtnResponse::Value(val)) = self.payload.extract::<ExtnResponse>() {
            return Some(val);
        }
        None
    }
}

impl TryFrom<String> for ExtnPayload {
    type Error = RippleError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        if let Ok(r) = serde_json::from_str(&value) {
            return Ok(r);
        }
        Err(RippleError::ParseError)
    }
}

impl From<ExtnPayload> for String {
    fn from(val: ExtnPayload) -> Self {
        serde_json::to_string(&val).unwrap()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(test, derive(PartialEq))]
pub enum ExtnPayload {
    Request(ExtnRequest),
    Response(ExtnResponse),
    Event(ExtnEvent),
}
impl Default for ExtnPayload {
    fn default() -> Self {
        ExtnPayload::Request(ExtnRequest::Config(Config::DefaultName))
    }
}

impl From<ExtnMessage> for ApiMessage {
    fn from(val: ExtnMessage) -> Self {
        let mut message = JsonRpcApiResponse::default();
        let request_id = val.id.clone();
        let value = json!({
            "id": val.id,
            "requestor": val.requestor.to_string(),
            "target": serde_json::to_string(&val.target).unwrap(),
            "target_id": match val.target_id {
                Some(id) => id.to_string(),
                None => "".to_owned(),
            },
            "ts": if let Some(v) = val.ts {
                v
            } else {
                chrono::Utc::now().timestamp_millis()
            },
            "payload": match &val.payload {
                ExtnPayload::Request(r) => serde_json::to_value(r).unwrap(),
                ExtnPayload::Response(r) => serde_json::to_value(r).unwrap(),
                ExtnPayload::Event(r) => serde_json::to_value(r).unwrap(),
            },
        });
        match val.payload {
            ExtnPayload::Request(_) => {
                message.params = Some(value);
                message.method = Some("service.request".to_string());
            }
            ExtnPayload::Response(_) => {
                message.result = Some(value);
                message.method = Some("service.response".to_string());
            }
            ExtnPayload::Event(_) => {
                message.params = Some(value);
                message.method = Some("service.event".to_string());
            }
        }

        ApiMessage {
            protocol: ApiProtocol::JsonRpc,
            jsonrpc_msg: serde_json::to_string(&message).unwrap(),
            request_id,
            stats: None,
        }
    }
}

impl TryFrom<String> for ExtnMessage {
    type Error = RippleError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        if let Ok(message) = serde_json::from_str::<JsonRpcApiResponse>(&value) {
            let mut extn_message = ExtnMessage::default();
            let mut json_payload = None;
            let mut data_is_valid = false;
            if let Some(method) = &message.method {
                match method.as_str() {
                    "service.request" => {
                        if let Some(params) = message.params {
                            if let Some(payload) = params.get("payload").cloned() {
                                if let Ok(request) = serde_json::from_value::<ExtnRequest>(payload)
                                {
                                    extn_message.payload = ExtnPayload::Request(request);
                                    data_is_valid = true;
                                }
                            }
                            let _ = json_payload.insert(params);
                        }
                    }
                    "service.response" => {
                        if let Some(params) = message.result {
                            if let Some(payload) = params.get("payload").cloned() {
                                if let Ok(request) = serde_json::from_value::<ExtnResponse>(payload)
                                {
                                    extn_message.payload = ExtnPayload::Response(request);
                                    data_is_valid = true;
                                }
                            }
                            let _ = json_payload.insert(params);
                        }
                    }
                    "service.event" => {
                        if let Some(params) = message.params {
                            if let Some(payload) = params.get("payload").cloned() {
                                if let Ok(request) = serde_json::from_value::<ExtnEvent>(payload) {
                                    extn_message.payload = ExtnPayload::Event(request);
                                    data_is_valid = true;
                                }
                            }
                            let _ = json_payload.insert(params);
                        }
                    }
                    _ => {}
                }
            }

            if let Some(payload) = json_payload {
                if let Some(id) = payload.get("id") {
                    if let Some(id) = id.as_str() {
                        extn_message.id = id.to_owned();
                        data_is_valid = true
                    }
                }

                if !data_is_valid {
                    error!("Payload parsing failed for {} ", value);
                    return Err(RippleError::BrokerError(extn_message.id));
                }

                if data_is_valid {
                    data_is_valid = false;
                    if let Some(requestor) = payload.get("requestor") {
                        if let Some(requestor) = requestor.as_str() {
                            if let Ok(id) = ExtnId::try_from(requestor.to_owned()) {
                                extn_message.requestor = id;
                                data_is_valid = true
                            }
                        }
                    }
                } else {
                    error!("requestor not found in {} ", payload);
                }

                if data_is_valid {
                    data_is_valid = false;
                    if let Some(target) = payload.get("target") {
                        if let Some(requestor) = target.as_str() {
                            if let Ok(id) = RippleContract::try_from(requestor.to_owned()) {
                                extn_message.target = id;
                                data_is_valid = true
                            }
                        }
                    }
                } else {
                    error!("target not found in {} ", payload);
                }

                if data_is_valid {
                    if let Some(target) = payload.get("target_id") {
                        if let Some(requestor) = target.as_str() {
                            if let Ok(id) = ExtnId::try_from(requestor.to_owned()) {
                                extn_message.target_id = Some(id);
                            }
                        }
                    }
                }

                if data_is_valid {
                    if let Some(ts) = payload.get("ts") {
                        if let Some(ts) = ts.as_i64() {
                            extn_message.ts = Some(ts);
                        }
                    }
                }
                return Ok(extn_message);
            } else {
                error!("payload not found in {:?} ", value);
            }
        }
        Err(RippleError::ParseError)
    }
}

impl TryFrom<ApiMessage> for ExtnMessage {
    type Error = RippleError;
    fn try_from(value: ApiMessage) -> Result<Self, Self::Error> {
        ExtnMessage::try_from(value.jsonrpc_msg)
    }
}

impl ExtnPayload {
    pub fn extract<T: ExtnPayloadProvider>(&self) -> Option<T> {
        T::get_from_payload(self.clone())
    }

    pub fn is_request(&self) -> bool {
        matches!(self, ExtnPayload::Request(_))
    }

    pub fn is_response(&self) -> bool {
        matches!(self, ExtnPayload::Response(_))
    }

    pub fn is_event(&self) -> bool {
        matches!(self, ExtnPayload::Event(_))
    }

    pub fn as_response(&self) -> Option<ExtnResponse> {
        if let ExtnPayload::Response(r) = self.clone() {
            Some(r)
        } else {
            None
        }
    }

    pub fn as_request(&self) -> Option<ExtnRequest> {
        if let ExtnPayload::Request(r) = self.clone() {
            Some(r)
        } else {
            None
        }
    }
}

/// Most critical trait used in Inter Extension Communication(IEC). Any message has to conform to this trait specification
/// in order to be used inside the channel.
///
/// Common structs required for general opensource Firebolt Operations will be implemented for this trait in the `sdk`
/// Developers can also extend this trait for their own Extensions and use it with the [crate::extn::client::extn_client::ExtnClient]
/// Defines the type of payload for the owner. The owner has to implement this method for the client to create a
/// payload for the [ExtnMessage]. [ExtnPayload] is an enumeration with types of [ExtnRequest], [ExtnResponse] and [ExtnEvent]
/// # Example
///
/// ```
/// use serde::{Deserialize, Serialize};
/// use ripple_sdk::extn::extn_id::ExtnId;
/// use ripple_sdk::extn::extn_client_message::ExtnPayload;
/// use ripple_sdk::extn::extn_client_message::ExtnRequest;
/// use ripple_sdk::extn::extn_client_message::ExtnPayloadProvider;
/// use ripple_sdk::extn::extn_id::ExtnClassId;
/// use ripple_sdk::framework::ripple_contract::{RippleContract};
/// #[derive(Debug, Clone, Serialize, Deserialize)]
/// pub enum MyCustomEnumRequestPayload {
///     String(String),
///     Bool(bool)
/// }
///
/// impl ExtnPayloadProvider for MyCustomEnumRequestPayload {
///     fn get_extn_payload(&self) -> ExtnPayload {
///     ExtnPayload::Request(ExtnRequest::Extn(serde_json::to_value(self.clone()).unwrap()))
/// }

/// fn get_from_payload(payload: ExtnPayload) -> Option<MyCustomEnumRequestPayload> {
///     match payload {
///         ExtnPayload::Request(request) => match request {
///             ExtnRequest::Extn(value) => {
///                 match serde_json::from_value(value) {
///                     Ok(r) => return Some(r),
///                     Err(e) => return None
///                 }
///             },
///             _ => {}
///         },
///         _ => {}
///     }
///     None
/// }
///
/// fn contract() -> RippleContract {
///     RippleContract::DeviceInfo
/// }
/// }
/// ```
pub trait ExtnPayloadProvider: Clone + Send + Sync + Debug
where
    Self: Sized,
{
    fn get_extn_payload(&self) -> ExtnPayload;
    fn get_from_payload(payload: ExtnPayload) -> Option<Self>;
    fn get_contract(&self) -> RippleContract {
        Self::contract()
    }
    fn contract() -> RippleContract;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(test, derive(PartialEq))]
pub enum ExtnRequest {
    Config(Config),
    Rpc(RpcRequest),
    Device(DeviceRequest),
    DeviceEvent(DeviceEventRequest),
    Extn(Value),
    LifecycleManagement(LifecycleManagementRequest),
    PinChallenge(PinChallengeRequestWithContext),
    Keyboard(KeyboardSessionRequest),
    Permission(PermissionRequest),
    AccountSession(AccountSessionRequest),
    PrivacySettings(PrivacyCloudRequest),
    StorageManager(StorageManagerRequest),
    Settings(SettingsRequest),
    UserGrantsCloudStore(UserGrantsCloudStoreRequest),
    UserGrantsStore(UserGrantsStoreRequest),
    PrivacySettingsStore(PrivacySettingsStoreRequest),
    AuthorizedInfo(CapsRequest),
    OperationalMetricsRequest(OperationalMetricRequest),
    Context(RippleContextUpdateRequest),
}

impl ExtnPayloadProvider for ExtnRequest {
    fn get_extn_payload(&self) -> ExtnPayload {
        ExtnPayload::Request(self.clone())
    }

    fn get_from_payload(payload: ExtnPayload) -> Option<Self> {
        if let ExtnPayload::Request(r) = payload {
            return Some(r);
        }

        None
    }

    fn contract() -> RippleContract {
        RippleContract::Internal
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ExtnResponse {
    None(()),
    String(String),
    Boolean(bool),
    Number(u32),
    Float(f32),
    Value(Value),
    StringMap(HashMap<String, String>),
    List(Vec<String>),
    Error(RippleError),
    Config(ConfigResponse),
    PinChallenge(PinChallengeResponse),
    Keyboard(KeyboardSessionResponse),
    AccountSession(AccountSessionResponse),
    Permission(PermissionResponse),
    StorageData(StorageData),
    NetworkResponse(NetworkResponse),
    TimezoneWithOffset(String, i64),
    DefaultApp(AppLibraryEntry),
    Settings(HashMap<String, SettingValue>),
    BoolMap(HashMap<String, bool>),
}

impl ExtnPayloadProvider for ExtnResponse {
    fn get_extn_payload(&self) -> ExtnPayload {
        ExtnPayload::Response(self.clone())
    }

    fn get_from_payload(payload: ExtnPayload) -> Option<Self> {
        if let ExtnPayload::Response(r) = payload {
            return Some(r);
        }

        None
    }

    fn contract() -> RippleContract {
        RippleContract::Internal
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(test, derive(PartialEq))]
pub enum ExtnEvent {
    String(String),
    Value(Value),
    Status(ExtnStatus),
    AppEvent(AppEventRequest),
    OperationalMetrics(TelemetryPayload),
    Context(RippleContext),
    TimeZone(TimeZone),
}

impl ExtnPayloadProvider for ExtnEvent {
    fn get_extn_payload(&self) -> ExtnPayload {
        ExtnPayload::Event(self.clone())
    }

    fn get_from_payload(payload: ExtnPayload) -> Option<Self> {
        if let ExtnPayload::Event(r) = payload {
            return Some(r);
        }

        None
    }

    fn contract() -> RippleContract {
        RippleContract::Internal
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extn::{
        extn_client_message::{ExtnEvent, ExtnId, ExtnRequest, ExtnResponse},
        extn_id::ExtnClassId,
    };
    use rstest::rstest;
    use serde_json::json;

    #[test]
    fn test_extract_response() {
        let response_payload = ExtnPayload::Response(ExtnResponse::String("Response".to_string()));
        let extracted_response: Option<ExtnResponse> = response_payload.extract();
        assert_eq!(
            extracted_response,
            Some(ExtnResponse::String("Response".to_string()))
        );
    }

    #[test]
    fn test_extract_event() {
        let event_payload = ExtnPayload::Event(ExtnEvent::String("Event".to_string()));
        let extracted_event: Option<ExtnEvent> = event_payload.extract();
        assert_eq!(
            extracted_event,
            Some(ExtnEvent::String("Event".to_string()))
        );
    }

    #[test]
    fn test_is_request() {
        let request_payload = ExtnPayload::Request(ExtnRequest::Config(Config::DefaultName));
        assert!(request_payload.is_request());
        assert!(!request_payload.is_response());
        assert!(!request_payload.is_event());
    }

    #[test]
    fn test_is_response() {
        let response_payload = ExtnPayload::Response(ExtnResponse::String("Response".to_string()));
        assert!(!response_payload.is_request());
        assert!(response_payload.is_response());
        assert!(!response_payload.is_event());
    }

    #[test]
    fn test_is_event() {
        let event_payload = ExtnPayload::Event(ExtnEvent::String("Event".to_string()));
        assert!(!event_payload.is_request());
        assert!(!event_payload.is_response());
        assert!(event_payload.is_event());
    }

    #[test]
    fn test_as_response() {
        let response_payload = ExtnPayload::Response(ExtnResponse::String("Response".to_string()));
        let extracted_response: Option<ExtnResponse> = response_payload.as_response();
        assert_eq!(
            extracted_response,
            Some(ExtnResponse::String("Response".to_string()))
        );
    }

    #[test]
    fn test_as_response_with_non_response_payload() {
        let request_payload = ExtnPayload::Request(ExtnRequest::Config(Config::DefaultName));
        let extracted_response: Option<ExtnResponse> = request_payload.as_response();
        assert_eq!(extracted_response, None);
    }

    #[test]
    fn test_extn_message_get_response() {
        let requestor = ExtnId::new_channel(ExtnClassId::Device, "info".into());
        let target = RippleContract::DeviceInfo;
        let payload = ExtnPayload::Request(ExtnRequest::Config(Config::DefaultName));
        let message = ExtnMessage {
            id: "123".to_string(),
            requestor,
            target,
            target_id: None,
            payload,
            ts: None,
        };

        let response = ExtnResponse::String("Response".to_string());
        let response_message = message.get_response(response).unwrap();

        assert_eq!(
            response_message.payload,
            ExtnPayload::Response(ExtnResponse::String("Response".to_string()))
        );
    }

    #[rstest(
        json_string,
        expected_response,
        case(
            r#"{"Response":{"String":"Response"}}"#,
            Ok(ExtnPayload::Response(ExtnResponse::String("Response".to_string())))
        ),
        case(
            r#"{"Response":"Response"}"#,
            Err(RippleError::ParseError)
        )
    )]
    fn test_extn_payload_try_from(
        json_string: &str,
        expected_response: Result<ExtnPayload, RippleError>,
    ) {
        let payload_result = ExtnPayload::try_from(json_string.to_string());
        assert_eq!(payload_result, expected_response);
    }

    #[tokio::test]
    async fn test_ack() {
        // Create a sample ExtnMessage for testing
        let original_message = ExtnMessage {
            id: "test_id".to_string(),
            requestor: ExtnId::get_main_target("main".into()),
            target: RippleContract::Internal,
            target_id: Some(ExtnId::get_main_target("main".into())),
            payload: ExtnPayload::Request(ExtnRequest::Config(Config::DefaultName)),
            ts: Some(1234567890),
        };

        // Clone the original message and call ack method
        let ack_message = original_message.ack();

        // Check if the generated ack_message has the expected properties
        assert_eq!(ack_message.id, "test_id");
        assert_eq!(
            ack_message.requestor,
            ExtnId::get_main_target("main".into())
        );
        assert_eq!(ack_message.target, RippleContract::Internal);
        assert_eq!(
            ack_message.target_id,
            Some(ExtnId::get_main_target("main".into()))
        );
        assert_eq!(
            ack_message.payload,
            ExtnPayload::Response(ExtnResponse::None(()))
        );
        assert!(ack_message.ts.is_none());
    }

    #[test]
    fn test_get_extn_payload() {
        let event_payload = ExtnEvent::String("Event".to_string());
        let extn_payload = event_payload.get_extn_payload();
        assert_eq!(extn_payload, ExtnPayload::Event(event_payload));
    }

    #[test]
    fn test_get_from_payload_with_valid_payload() {
        let event_payload = ExtnEvent::String("Event".to_string());
        let extn_payload = ExtnPayload::Event(event_payload.clone());
        let extracted_event: Option<ExtnEvent> = ExtnEvent::get_from_payload(extn_payload);
        assert_eq!(extracted_event, Some(event_payload));
    }

    #[test]
    fn test_get_from_payload_with_invalid_payload() {
        let response_payload = ExtnPayload::Response(ExtnResponse::String("Response".to_string()));
        let extracted_event: Option<ExtnEvent> = ExtnEvent::get_from_payload(response_payload);
        assert_eq!(extracted_event, None);
    }

    #[test]
    fn test_contract() {
        let expected_contract = RippleContract::Internal;
        assert_eq!(ExtnEvent::contract(), expected_contract);
    }

    #[test]
    fn test_get_event() {
        // Create a sample ExtnMessage for testing
        let original_message = ExtnMessage {
            id: "test_id".to_string(),
            requestor: ExtnId::get_main_target("main".into()),
            target: RippleContract::Internal,
            target_id: Some(ExtnId::get_main_target("main".into())),
            payload: ExtnPayload::Request(ExtnRequest::Config(Config::DefaultName)),
            ts: Some(1234567890),
        };
        let event_payload = ExtnEvent::Value(json!(1));
        let value = original_message.get_event(event_payload.clone()).unwrap();
        let extn_event_payload = ExtnPayload::Event(event_payload);
        assert!(value.id.eq_ignore_ascii_case("test_id"));
        assert!(value.payload.eq(&extn_event_payload));
    }

    #[test]
    fn test_get_request() {
        let original_message = ExtnMessage {
            id: "test_id".to_string(),
            requestor: ExtnId::get_main_target("main".into()),
            target: RippleContract::Internal,
            target_id: Some(ExtnId::get_main_target("main".into())),
            payload: ExtnPayload::Request(ExtnRequest::Config(Config::DefaultName)),
            ts: Some(1234567890),
        };

        let api_message: ApiMessage = original_message.into();

        let tried_message: ExtnMessage = ExtnMessage::try_from(api_message).unwrap();
        assert!(tried_message.payload.is_request());
        assert!(tried_message.id.eq_ignore_ascii_case("test_id"));
        assert!(tried_message.requestor == ExtnId::get_main_target("main".into()));
        assert!(tried_message.target == RippleContract::Internal);
        assert!(tried_message.target_id == Some(ExtnId::get_main_target("main".into())));
        assert!(
            tried_message.payload == ExtnPayload::Request(ExtnRequest::Config(Config::DefaultName))
        );
        assert!(tried_message.ts == Some(1234567890));
    }
}
