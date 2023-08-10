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

use serde::{Deserialize, Serialize};

use crate::{
    extn::extn_client_message::{
        ExtnEvent, ExtnPayload, ExtnPayloadProvider, ExtnRequest, ExtnResponse,
    },
    framework::ripple_contract::RippleContract,
};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum PubSubRequest {
    Connect(PubSubConnectParam),
    Subscribe(PubSubSubscribeParam),
    UnSubscribe(PubSubSubscribeParam),
    Publish(PubSubPublishParams),
    GetSubscribedMessage(PubSubSubscribedParam),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum PubSubResponse {
    Connect(PubSubConnectResponse),
    Publish(PubSubPublishResponse),
    Subscribe(PubSubSubscribeResponse),
    GetSubscribedMessage(PubSubSubscribedResponse),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PubSubNotifyTopic {
    pub topic: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PubSubSubscribedParam {
    pub context: String,
    pub topic: String,
}
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PubSubSubscribedResponse {
    pub payload: Option<String>,
    pub payload_type: Option<String>,
    pub headers: Option<String>,
    pub result: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PubSubPublishParams {
    pub context: String,
    pub topic: String,
    pub message: String,
    pub message_type: String,
}
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PubSubPublishResponse {
    pub result: bool,
    pub status_code: String,
    pub connection_status: String,
}
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PubSubConnectParam {
    pub endpoint_url: String,
    pub client_name: String,
    pub credentials: Option<String>,
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
pub struct PubSubSubscribeParam {
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
pub struct PubSubUnSubscriberResponse {
    pub connection_status: String,
    pub status_code: String,
    pub result: bool,
}
impl ExtnPayloadProvider for PubSubNotifyTopic {
    fn get_from_payload(payload: ExtnPayload) -> Option<Self> {
        if let ExtnPayload::Event(ExtnEvent::String(topic)) = payload {
            return Some(PubSubNotifyTopic { topic });
        }

        None
    }

    fn get_extn_payload(&self) -> ExtnPayload {
        ExtnPayload::Event(ExtnEvent::String(self.topic.to_owned()))
    }

    fn contract() -> RippleContract {
        RippleContract::PubSub
    }
}

impl ExtnPayloadProvider for PubSubRequest {
    fn get_from_payload(payload: ExtnPayload) -> Option<Self> {
        if let ExtnPayload::Request(ExtnRequest::PubSub(v)) = payload {
            return Some(v);
        }

        None
    }

    fn get_extn_payload(&self) -> ExtnPayload {
        ExtnPayload::Request(ExtnRequest::PubSub(self.clone()))
    }

    fn contract() -> RippleContract {
        RippleContract::PubSub
    }
}

impl ExtnPayloadProvider for PubSubResponse {
    fn get_from_payload(payload: ExtnPayload) -> Option<Self> {
        if let ExtnPayload::Response(ExtnResponse::PubSub(v)) = payload {
            return Some(v);
        }

        None
    }

    fn get_extn_payload(&self) -> ExtnPayload {
        ExtnPayload::Response(ExtnResponse::PubSub(self.clone()))
    }

    fn contract() -> RippleContract {
        RippleContract::PubSub
    }
}
