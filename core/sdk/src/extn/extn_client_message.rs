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

use crossbeam::channel::Sender as CSender;
use log::error;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    api::{
        account_link::AccountLinkRequest,
        apps::AppEventRequest,
        caps::CapsRequest,
        config::{Config, ConfigResponse},
        context::{RippleContext, RippleContextUpdateRequest},
        device::{
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
        protocol::BridgeProtocolRequest,
        pubsub::{PubSubRequest, PubSubResponse},
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
/// `callback` |Crossbeam [crossbeam::channel::Sender<CExtnMessage>] | Usually added by `Main` to the `target` to respond back to the `requestor`|

#[derive(Debug, Clone)]
pub struct ExtnMessage {
    pub id: String,
    pub requestor: ExtnId,
    pub target: RippleContract,
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
        match self.clone().payload {
            ExtnPayload::Request(_) => Ok(ExtnMessage {
                callback: self.callback.clone(),
                id: self.id.clone(),
                payload: ExtnPayload::Response(response),
                requestor: self.requestor.clone(),
                target: self.target.clone(),
                ts: None,
            }),
            _ => {
                error!("can only respond for a request message");
                Err(RippleError::InvalidInput)
            }
        }
    }

    pub fn ack(&self) -> ExtnMessage {
        ExtnMessage {
            id: self.id.clone(),
            requestor: self.requestor.clone(),
            target: self.target.clone(),
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
pub enum ExtnPayload {
    Request(ExtnRequest),
    Response(ExtnResponse),
    Event(ExtnEvent),
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
pub trait ExtnPayloadProvider: Clone + Send + Sync
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
pub enum ExtnEvent {
    String(String),
    Value(Value),
    Status(ExtnStatus),
    AppEvent(AppEventRequest),
    OperationalMetrics(TelemetryPayload),
    Context(RippleContext),
    VoiceGuidanceState(VoiceGuidanceState),
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
