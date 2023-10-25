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

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum PubSubRequest {
    Connect(PubSubConnectRequest),
    UpdateConnection(PubSubUpdateConnectionCredentialsRequest),
    Subscribe(PubSubSubscribeRequest),
    UnSubscribe(PubSubUnSubscribeRequest),
    Publish(PubSubPublishRequest),
    Disconnect(String),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum PubSubResponse {
    Connect(PubSubConnectResponse),
    Publish(PubSubPublishResponse),
    Subscribe(PubSubSubscribeResponse),
    UnSubscribe(PubSubUnSubscribeResponse),
    UpdateConnection(PubSubConnectResponse),
    Disconnect(bool),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PubSubNotifyTopic {
    pub topic: String,
    pub value: Value,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PubSubSubscribedParam {
    pub context: String,
    pub topic: String,
}
// #[derive(Debug, Serialize, Deserialize, Clone)]
// pub struct PubSubSubscribedResponse {
//     pub payload: Option<String>,
//     pub payload_type: Option<String>,
//     pub headers: Option<String>,
//     pub result: bool,
// }

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PubSubPublishRequest {
    pub context: String,
    pub topic: String,
    pub message: String,
    pub message_type: String,
}
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PubSubPublishResponse {
    pub result: bool,
    // pub status_code: String,
    // pub connection_status: String,
}
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Credentials {
    pub id: String,
    pub secret: String,
}
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PubSubUpdateConnectionCredentialsRequest {
    pub connection_id: String,
    pub credentials: Credentials,
}
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PubSubConnectRequest {
    pub endpoint_url: String,
    pub client_name: Option<String>,
    pub credentials: Credentials,
    // pub retry_interval_in_msec: u32,
    pub operation_timeout_in_msec: u32,
    // pub max_retry_interval: u32,
}
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PubSubConnectResponse {
    pub connection_status: String,
    // status_code: String,
    pub context: String,
    // result: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PubSubSubscribeRequest {
    pub context: String,
    pub topic: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PubSubUnSubscribeRequest {
    pub context: String,
    pub topic: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PubSubSubscribeResponse {
    // pub connection_status: String,
    // pub status_code: String,
    pub result: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
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
        if let ExtnPayload::Event(ExtnEvent::PubSubEvent(topic)) = payload {
            return Some(topic);
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
