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

use async_channel::Sender as CSender;
#[cfg(not(test))]
use log::error;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt::Debug;

#[cfg(test)]
use println as error;

use crate::{
    api::{
        account_link::AccountLinkRequest,
        app_catalog::{AppCatalogRequest, AppMetadata, AppsUpdate},
        apps::AppEventRequest,
        caps::CapsRequest,
        config::{Config, ConfigResponse},
        context::{RippleContext, RippleContextUpdateRequest},
        device::{
            device_apps::InstalledApp,
            device_events::DeviceEventRequest,
            device_peristence::StorageData,
            device_request::{DeviceRequest, NetworkResponse, TimeZone, VoiceGuidanceState},
        },
        distributor::{
            distributor_permissions::{PermissionRequest, PermissionResponse},
            distributor_platform::PlatformTokenRequest,
            distributor_privacy::{PrivacyCloudRequest, PrivacySettingsStoreRequest},
            distributor_request::DistributorRequest,
            distributor_sync::SyncAndMonitorRequest,
            distributor_token::DistributorTokenRequest,
            distributor_usergrants::UserGrantsCloudStoreRequest,
        },
        firebolt::{
            fb_advertising::{AdvertisingRequest, AdvertisingResponse},
            fb_authentication::TokenResult,
            fb_keyboard::{KeyboardSessionRequest, KeyboardSessionResponse},
            fb_lifecycle_management::LifecycleManagementRequest,
            fb_metrics::{BehavioralMetricRequest, MetricsRequest},
            fb_pin::{PinChallengeRequestWithContext, PinChallengeResponse},
            fb_secure_storage::{SecureStorageRequest, SecureStorageResponse},
            fb_telemetry::{OperationalMetricRequest, TelemetryPayload},
        },
        gateway::rpc_gateway_api::RpcRequest,
        manifest::device_manifest::AppLibraryEntry,
        observability::analytics::AnalyticsRequest,
        protocol::BridgeProtocolRequest,
        pubsub::{PubSubEvents, PubSubRequest, PubSubResponse},
        session::{AccountSessionRequest, AccountSessionResponse, SessionTokenRequest},
        settings::{SettingValue, SettingsRequest},
        status_update::ExtnStatus,
        storage_property::StorageManagerRequest,
        usergrant_entry::UserGrantsStoreRequest,
    },
    framework::ripple_contract::RippleContract,
    utils::error::RippleError,
};

use super::{extn_id::ExtnId, ffi::ffi_message::CExtnMessage};

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
/// `callback` |Async Channel [async_channel::Sender<CExtnMessage>] | Usually added by `Main` to the `target` to respond back to the `requestor`|

#[derive(Debug, Clone, Default)]
pub struct ExtnMessage {
    pub id: String,
    pub requestor: ExtnId,
    pub target: RippleContract,
    pub target_id: Option<ExtnId>,
    pub payload: ExtnPayload,
    pub callback: Option<CSender<CExtnMessage>>,
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
                callback: self.callback.clone(),
                id: self.id.clone(),
                payload: ExtnPayload::Response(response),
                requestor: self.requestor.clone(),
                target: self.target.clone(),
                target_id: self.target_id.clone(),
                ts: None,
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
                callback: self.callback.clone(),
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
            callback: self.callback.clone(),
            ts: None,
        }
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
    Distributor(DistributorRequest),
    AccountSession(AccountSessionRequest),
    BridgeProtocolRequest(BridgeProtocolRequest),
    SessionToken(SessionTokenRequest),
    SecureStorage(SecureStorageRequest),
    Advertising(AdvertisingRequest),
    PrivacySettings(PrivacyCloudRequest),
    BehavioralMetric(BehavioralMetricRequest),
    StorageManager(StorageManagerRequest),
    AccountLink(AccountLinkRequest),
    Settings(SettingsRequest),
    PubSub(PubSubRequest),
    CloudSync(SyncAndMonitorRequest),
    UserGrantsCloudStore(UserGrantsCloudStoreRequest),
    UserGrantsStore(UserGrantsStoreRequest),
    PrivacySettingsStore(PrivacySettingsStoreRequest),
    AuthorizedInfo(CapsRequest),
    Metrics(MetricsRequest),
    OperationalMetricsRequest(OperationalMetricRequest),
    PlatformToken(PlatformTokenRequest),
    DistributorToken(DistributorTokenRequest),
    Context(RippleContextUpdateRequest),
    AppCatalog(AppCatalogRequest),
    Analytics(AnalyticsRequest),
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
    AvailableTimezones(Vec<String>),
    TimezoneWithOffset(String, i64),
    Token(TokenResult),
    DefaultApp(AppLibraryEntry),
    Settings(HashMap<String, SettingValue>),
    PubSub(PubSubResponse),
    BoolMap(HashMap<String, bool>),
    Advertising(AdvertisingResponse),
    SecureStorage(SecureStorageResponse),
    AppCatalog(Vec<AppMetadata>),
    InstalledApps(Vec<InstalledApp>),
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
    VoiceGuidanceState(VoiceGuidanceState),
    PubSubEvent(PubSubEvents),
    TimeZone(TimeZone),
    AppsUpdate(AppsUpdate),
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
            callback: None,
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
            callback: None,
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
        assert!(ack_message.callback.is_none());
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
            callback: None,
            ts: Some(1234567890),
        };
        let event_payload = ExtnEvent::Value(json!(1));
        let value = original_message.get_event(event_payload.clone()).unwrap();
        let extn_event_payload = ExtnPayload::Event(event_payload);
        assert!(value.id.eq_ignore_ascii_case("test_id"));
        assert!(value.payload.eq(&extn_event_payload));
    }
}
