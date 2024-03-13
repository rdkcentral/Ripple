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

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    extn::extn_client_message::{
        ExtnEvent, ExtnPayload, ExtnPayloadProvider, ExtnRequest, ExtnResponse,
    },
    framework::ripple_contract::RippleContract,
};

use super::session::PubSubAdjective;

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub enum PubSubRequest {
    Connect(PubSubConnectRequest),
    UpdateConnection(PubSubUpdateConnectionCredentialsRequest),
    Subscribe(PubSubSubscribeRequest),
    UnSubscribe(PubSubUnSubscribeRequest),
    Publish(PubSubPublishRequest),
    Disconnect(String),
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub enum PubSubResponse {
    Connect(PubSubConnectResponse),
    Publish(PubSubPublishResponse),
    Subscribe(PubSubSubscribeResponse),
    UnSubscribe(PubSubUnSubscribeResponse),
    UpdateConnection(PubSubConnectResponse),
    Disconnect(bool),
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct PubSubNotifyTopic {
    pub connection_id: String,
    pub topic: String,
    pub value: Value,
}
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub enum PubSubConnectionStatus {
    Connected(String),
    Disconnected(String),
    Reconnecting(String),
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub enum PubSubEvents {
    PubSubConnectionEvent(PubSubConnectionStatus),
    PubSubValueChangeEvent(PubSubNotifyTopic),
}

impl PubSubEvents {
    pub fn as_pub_sub_value_change_event(&self) -> Option<&PubSubNotifyTopic> {
        if let Self::PubSubValueChangeEvent(v) = self {
            Some(v)
        } else {
            None
        }
    }

    pub fn as_pub_sub_connection_event(&self) -> Option<&PubSubConnectionStatus> {
        if let Self::PubSubConnectionEvent(v) = self {
            Some(v)
        } else {
            None
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PubSubSubscribedParam {
    pub connection_id: String,
    pub topic: String,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct PubSubPublishRequest {
    pub connection_id: String,
    pub topic: String,
    pub message: String,
    pub message_type: String,
}
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct PubSubPublishResponse {
    pub result: bool,
    // pub status_code: String,
    // pub connection_status: String,
}
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct Credentials {
    pub id: String,
    pub secret: String,
}
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct PubSubUpdateConnectionCredentialsRequest {
    pub connection_id: String,
    pub updated_secret: String,
}
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct PubSubConnectRequest {
    pub endpoint_url: String,
    pub client_name: Option<String>,
    pub credentials: Credentials,
    // pub retry_interval_in_msec: u32,
    pub operation_timeout_in_msec: u32,
    // pub max_retry_interval: u32,
}
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct PubSubConnectResponse {
    pub connection_status: String,
    // status_code: String,
    pub connection_id: String,
    // result: bool,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct PubSubSubscribeRequest {
    pub connection_id: String,
    pub topic: String,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct PubSubUnSubscribeRequest {
    pub context: String,
    pub topic: String,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct PubSubSubscribeResponse {
    // pub connection_status: String,
    // pub status_code: String,
    pub result: bool,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct PubSubUnSubscribeResponse {
    // pub connection_status: String,
    // pub status_code: String,
    pub result: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PubSubUnSubscriberResponse {
    pub connection_status: String,
    pub status_code: String,
    pub result: bool,
}
impl ExtnPayloadProvider for PubSubNotifyTopic {
    fn get_from_payload(payload: ExtnPayload) -> Option<Self> {
        if let ExtnPayload::Event(ExtnEvent::PubSubEvent(PubSubEvents::PubSubValueChangeEvent(
            topic,
        ))) = payload
        {
            return Some(topic);
        }

        None
    }

    fn get_extn_payload(&self) -> ExtnPayload {
        ExtnPayload::Event(ExtnEvent::PubSubEvent(
            PubSubEvents::PubSubValueChangeEvent(self.to_owned()),
        ))
    }

    fn contract() -> RippleContract {
        RippleContract::PubSub(PubSubAdjective::Listener)
    }
}

impl ExtnPayloadProvider for PubSubConnectionStatus {
    fn get_from_payload(payload: ExtnPayload) -> Option<Self> {
        if let ExtnPayload::Event(ExtnEvent::PubSubEvent(PubSubEvents::PubSubConnectionEvent(
            connection_status,
        ))) = payload
        {
            return Some(connection_status);
        }

        None
    }

    fn get_extn_payload(&self) -> ExtnPayload {
        ExtnPayload::Event(ExtnEvent::PubSubEvent(PubSubEvents::PubSubConnectionEvent(
            self.to_owned(),
        )))
    }

    fn contract() -> RippleContract {
        RippleContract::PubSub(PubSubAdjective::Listener)
    }
}

impl ExtnPayloadProvider for PubSubEvents {
    fn get_from_payload(payload: ExtnPayload) -> Option<Self> {
        if let ExtnPayload::Event(ExtnEvent::PubSubEvent(event)) = payload {
            return Some(event);
        }

        None
    }

    fn get_extn_payload(&self) -> ExtnPayload {
        ExtnPayload::Event(ExtnEvent::PubSubEvent(self.to_owned()))
    }

    fn contract() -> RippleContract {
        RippleContract::PubSub(PubSubAdjective::Listener)
    }
}

impl ExtnPayloadProvider for PubSubConnectRequest {
    fn get_from_payload(payload: ExtnPayload) -> Option<Self> {
        if let ExtnPayload::Request(ExtnRequest::PubSub(PubSubRequest::Connect(connect_request))) =
            payload
        {
            Some(connect_request)
        } else {
            None
        }
    }

    fn get_extn_payload(&self) -> ExtnPayload {
        ExtnPayload::Request(ExtnRequest::PubSub(PubSubRequest::Connect(self.clone())))
    }

    fn contract() -> RippleContract {
        RippleContract::PubSub(PubSubAdjective::Provider)
    }
}

impl ExtnPayloadProvider for PubSubSubscribeRequest {
    fn get_from_payload(payload: ExtnPayload) -> Option<Self> {
        if let ExtnPayload::Request(ExtnRequest::PubSub(PubSubRequest::Subscribe(
            subscribe_request,
        ))) = payload
        {
            Some(subscribe_request)
        } else {
            None
        }
    }

    fn get_extn_payload(&self) -> ExtnPayload {
        ExtnPayload::Request(ExtnRequest::PubSub(PubSubRequest::Subscribe(self.clone())))
    }

    fn contract() -> RippleContract {
        RippleContract::PubSub(PubSubAdjective::Provider)
    }
}

impl ExtnPayloadProvider for PubSubUnSubscribeRequest {
    fn get_from_payload(payload: ExtnPayload) -> Option<Self> {
        if let ExtnPayload::Request(ExtnRequest::PubSub(PubSubRequest::UnSubscribe(
            unsubscribe_request,
        ))) = payload
        {
            Some(unsubscribe_request)
        } else {
            None
        }
    }

    fn get_extn_payload(&self) -> ExtnPayload {
        ExtnPayload::Request(ExtnRequest::PubSub(PubSubRequest::UnSubscribe(
            self.clone(),
        )))
    }

    fn contract() -> RippleContract {
        RippleContract::PubSub(PubSubAdjective::Provider)
    }
}

impl ExtnPayloadProvider for PubSubPublishRequest {
    fn get_from_payload(payload: ExtnPayload) -> Option<Self> {
        if let ExtnPayload::Request(ExtnRequest::PubSub(PubSubRequest::Publish(publish_request))) =
            payload
        {
            Some(publish_request)
        } else {
            None
        }
    }

    fn get_extn_payload(&self) -> ExtnPayload {
        ExtnPayload::Request(ExtnRequest::PubSub(PubSubRequest::Publish(self.clone())))
    }

    fn contract() -> RippleContract {
        RippleContract::PubSub(PubSubAdjective::Publish)
    }
}

impl ExtnPayloadProvider for PubSubConnectResponse {
    fn get_extn_payload(&self) -> ExtnPayload {
        ExtnPayload::Response(ExtnResponse::PubSub(PubSubResponse::Connect(self.clone())))
    }

    fn get_from_payload(payload: ExtnPayload) -> Option<Self> {
        if let ExtnPayload::Response(ExtnResponse::PubSub(PubSubResponse::Connect(
            connect_response,
        ))) = payload
        {
            Some(connect_response)
        } else {
            None
        }
    }

    fn contract() -> RippleContract {
        RippleContract::PubSub(PubSubAdjective::Listener)
    }
}

impl ExtnPayloadProvider for PubSubSubscribeResponse {
    fn get_extn_payload(&self) -> ExtnPayload {
        ExtnPayload::Response(ExtnResponse::PubSub(PubSubResponse::Subscribe(
            self.clone(),
        )))
    }

    fn get_from_payload(payload: ExtnPayload) -> Option<Self> {
        if let ExtnPayload::Response(ExtnResponse::PubSub(PubSubResponse::Subscribe(
            subscribe_response,
        ))) = payload
        {
            Some(subscribe_response)
        } else {
            None
        }
    }

    fn contract() -> RippleContract {
        RippleContract::PubSub(PubSubAdjective::Listener)
    }
}

impl ExtnPayloadProvider for PubSubPublishResponse {
    fn get_extn_payload(&self) -> ExtnPayload {
        ExtnPayload::Response(ExtnResponse::PubSub(PubSubResponse::Publish(self.clone())))
    }

    fn get_from_payload(payload: ExtnPayload) -> Option<Self> {
        if let ExtnPayload::Response(ExtnResponse::PubSub(PubSubResponse::Publish(
            publish_response,
        ))) = payload
        {
            Some(publish_response)
        } else {
            None
        }
    }

    fn contract() -> RippleContract {
        RippleContract::PubSub(PubSubAdjective::Listener)
    }
}

impl ExtnPayloadProvider for PubSubUnSubscribeResponse {
    fn get_extn_payload(&self) -> ExtnPayload {
        ExtnPayload::Response(ExtnResponse::PubSub(PubSubResponse::UnSubscribe(
            self.clone(),
        )))
    }

    fn get_from_payload(payload: ExtnPayload) -> Option<Self> {
        if let ExtnPayload::Response(ExtnResponse::PubSub(PubSubResponse::UnSubscribe(
            unsub_response,
        ))) = payload
        {
            Some(unsub_response)
        } else {
            None
        }
    }

    fn contract() -> RippleContract {
        RippleContract::PubSub(PubSubAdjective::Listener)
    }
}

impl ExtnPayloadProvider for PubSubRequest {
    fn get_extn_payload(&self) -> ExtnPayload {
        ExtnPayload::Request(ExtnRequest::PubSub(self.clone()))
    }

    fn get_from_payload(payload: ExtnPayload) -> Option<Self> {
        if let ExtnPayload::Request(ExtnRequest::PubSub(pubsub_request)) = payload {
            Some(pubsub_request)
        } else {
            None
        }
    }

    fn contract() -> RippleContract {
        RippleContract::PubSub(PubSubAdjective::Provider)
    }
}

impl ExtnPayloadProvider for PubSubResponse {
    fn get_extn_payload(&self) -> ExtnPayload {
        ExtnPayload::Response(ExtnResponse::PubSub(self.clone()))
    }

    fn get_from_payload(payload: ExtnPayload) -> Option<Self> {
        if let ExtnPayload::Response(ExtnResponse::PubSub(pubsub_request)) = payload {
            Some(pubsub_request)
        } else {
            None
        }
    }

    fn contract() -> RippleContract {
        RippleContract::PubSub(PubSubAdjective::Listener)
    }
}

impl ExtnPayloadProvider for PubSubUpdateConnectionCredentialsRequest {
    fn get_from_payload(payload: ExtnPayload) -> Option<Self> {
        if let ExtnPayload::Request(ExtnRequest::PubSub(PubSubRequest::UpdateConnection(
            update_request,
        ))) = payload
        {
            Some(update_request)
        } else {
            None
        }
    }

    fn get_extn_payload(&self) -> ExtnPayload {
        ExtnPayload::Request(ExtnRequest::PubSub(PubSubRequest::UpdateConnection(
            self.clone(),
        )))
    }

    fn contract() -> RippleContract {
        RippleContract::PubSub(PubSubAdjective::Provider)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::test_utils::test_extn_payload_provider;

    #[test]
    fn test_extn_payload_provider_for_pub_sub_notify_topic() {
        let pub_sub_notify_topic = PubSubNotifyTopic {
            topic: String::from("your_topic"),
            connection_id: "test_connection_id".to_string(),
            value: Value::String(String::from("your_value")),
        };
        let contract_type: RippleContract = RippleContract::PubSub(PubSubAdjective::Listener);
        test_extn_payload_provider(pub_sub_notify_topic, contract_type);
    }

    #[test]
    fn test_extn_payload_provider_for_pub_sub_connection_status() {
        let pub_sub_connection_status =
            PubSubConnectionStatus::Connected("test_connected".to_string());
        let contract_type: RippleContract = RippleContract::PubSub(PubSubAdjective::Listener);
        test_extn_payload_provider(pub_sub_connection_status, contract_type);
    }

    #[test]
    fn test_extn_payload_provider_for_pub_sub_events() {
        let pub_sub_events = PubSubEvents::PubSubConnectionEvent(
            PubSubConnectionStatus::Connected("test_connected".to_string()),
        );

        let contract_type: RippleContract = RippleContract::PubSub(PubSubAdjective::Listener);
        test_extn_payload_provider(pub_sub_events, contract_type);
    }

    #[test]
    fn test_pub_sub_connect_request() {
        let pub_sub_connect_request = PubSubConnectRequest {
            endpoint_url: "https://example.com/pubsub".to_string(),
            client_name: Some("test_client".to_string()),
            credentials: Credentials {
                id: "test_id".to_string(),
                secret: "test_secret".to_string(),
            },
            operation_timeout_in_msec: 5000,
        };
        let contract_type: RippleContract = RippleContract::PubSub(PubSubAdjective::Provider);
        test_extn_payload_provider(pub_sub_connect_request, contract_type);
    }

    #[test]
    fn test_pub_sub_subscribe_request() {
        let pub_sub_subscribe_request = PubSubSubscribeRequest {
            connection_id: "test_connection_id".to_string(),
            topic: "test_topic".to_string(),
        };
        let contract_type: RippleContract = RippleContract::PubSub(PubSubAdjective::Provider);
        test_extn_payload_provider(pub_sub_subscribe_request, contract_type);
    }

    #[test]
    fn test_pub_sub_unsubscribe_request() {
        let pub_sub_unsubscribe_request = PubSubUnSubscribeRequest {
            context: "test_context".to_string(),
            topic: "test_topic".to_string(),
        };
        let contract_type: RippleContract = RippleContract::PubSub(PubSubAdjective::Provider);
        test_extn_payload_provider(pub_sub_unsubscribe_request, contract_type);
    }

    #[test]
    fn test_pub_sub_publish_request() {
        let pub_sub_publish_request = PubSubPublishRequest {
            connection_id: "test_connection_id".to_string(),
            topic: "test_topic".to_string(),
            message: "test_message".to_string(),
            message_type: "test_message_type".to_string(),
        };
        let contract_type: RippleContract = RippleContract::PubSub(PubSubAdjective::Publish);
        test_extn_payload_provider(pub_sub_publish_request, contract_type);
    }

    #[test]
    fn test_extn_response_pub_sub_connect_response() {
        let connect_response = PubSubConnectResponse {
            connection_status: "connected".to_string(),
            connection_id: "test_connection_id".to_string(),
        };
        let contract_type: RippleContract = RippleContract::PubSub(PubSubAdjective::Listener);
        test_extn_payload_provider(connect_response, contract_type);
    }

    #[test]
    fn test_pub_sub_subscribe_response() {
        let pub_sub_subscribe_response = PubSubSubscribeResponse { result: true };
        let contract_type: RippleContract = RippleContract::PubSub(PubSubAdjective::Listener);
        test_extn_payload_provider(pub_sub_subscribe_response, contract_type);
    }

    #[test]
    fn test_pub_sub_publish_response() {
        let pub_sub_publish_response = PubSubPublishResponse { result: true };
        let contract_type: RippleContract = RippleContract::PubSub(PubSubAdjective::Listener);
        test_extn_payload_provider(pub_sub_publish_response, contract_type);
    }

    #[test]
    fn test_pub_sub_unsubscribe_response() {
        let pub_sub_unsubscribe_response = PubSubUnSubscribeResponse { result: true };
        let contract_type: RippleContract = RippleContract::PubSub(PubSubAdjective::Listener);
        test_extn_payload_provider(pub_sub_unsubscribe_response, contract_type);
    }

    #[test]
    fn test_pub_sub_request() {
        let pub_sub_request = PubSubRequest::Connect(PubSubConnectRequest {
            endpoint_url: "test_endpoint".to_string(),
            client_name: Some("test_client".to_string()),
            credentials: Credentials {
                id: "test_id".to_string(),
                secret: "test_secret".to_string(),
            },
            operation_timeout_in_msec: 1000,
        });
        let contract_type: RippleContract = RippleContract::PubSub(PubSubAdjective::Provider);
        test_extn_payload_provider(pub_sub_request, contract_type);
    }

    #[test]
    fn test_pub_sub_response() {
        let pub_sub_response = PubSubResponse::Connect(PubSubConnectResponse {
            connection_status: "Connected".to_string(),
            connection_id: "test_connection_id".to_string(),
        });
        let contract_type: RippleContract = RippleContract::PubSub(PubSubAdjective::Listener);
        test_extn_payload_provider(pub_sub_response, contract_type);
    }

    #[test]
    fn test_pub_sub_update_connection_credentials_request() {
        let pub_sub_update_request = PubSubUpdateConnectionCredentialsRequest {
            connection_id: "test_connection_id".to_string(),
            updated_secret: "new_secret".to_string(),
        };
        let contract_type: RippleContract = RippleContract::PubSub(PubSubAdjective::Provider);
        test_extn_payload_provider(pub_sub_update_request, contract_type);
    }
}
