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

use crate::{
    extn::extn_client_message::{
        ExtnEvent, ExtnPayload, ExtnPayloadProvider, ExtnRequest, ExtnResponse,
    },
    framework::ripple_contract::RippleContract,
};

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub enum PubSubRequest {
    Connect(PubSubConnectParam),
    Subscribe(PubSubSubscribeParam),
    UnSubscribe(PubSubSubscribeParam),
    Publish(PubSubPublishParams),
    GetSubscribedMessage(PubSubSubscribedParam),
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
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

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct PubSubSubscribedParam {
    pub context: String,
    pub topic: String,
}
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct PubSubSubscribedResponse {
    pub payload: Option<String>,
    pub payload_type: Option<String>,
    pub headers: Option<String>,
    pub result: bool,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct PubSubPublishParams {
    pub context: String,
    pub topic: String,
    pub message: String,
    pub message_type: String,
}
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct PubSubPublishResponse {
    pub result: bool,
    pub status_code: String,
    pub connection_status: String,
}
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct PubSubConnectParam {
    pub endpoint_url: String,
    pub client_name: String,
    pub credentials: Option<String>,
    // pub retry_interval_in_msec: u32,
    pub operation_timeout_in_msec: u32,
    // pub max_retry_interval: u32,
}
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct PubSubConnectResponse {
    pub connection_status: String,
    // status_code: String,
    pub context: String,
    // result: bool,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct PubSubSubscribeParam {
    pub context: String,
    pub topic: String,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::test_utils::test_extn_payload_provider;

    #[test]
    fn test_extn_request_pub_sub() {
        let connect_param = PubSubConnectParam {
            endpoint_url: "test_endpoint_url".to_string(),
            client_name: "test_client_name".to_string(),
            credentials: Some("test_credentials".to_string()),
            operation_timeout_in_msec: 5000,
        };

        let pub_sub_request = PubSubRequest::Connect(connect_param);

        let contract_type: RippleContract = RippleContract::PubSub;
        test_extn_payload_provider(pub_sub_request, contract_type);
    }

    #[test]
    fn test_extn_response_pub_sub() {
        let connect_response = PubSubConnectResponse {
            connection_status: "connected".to_string(),
            context: "test_context".to_string(),
        };

        let pubsub_response = PubSubResponse::Connect(connect_response);
        let contract_type: RippleContract = RippleContract::PubSub;

        test_extn_payload_provider(pubsub_response, contract_type);
    }
}
