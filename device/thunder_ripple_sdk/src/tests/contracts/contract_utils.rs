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

use pact_consumer::mock_server::StartMockServerAsync;
use pact_consumer::prelude::PactBuilder;
use pact_consumer::prelude::PactBuilderAsync;
use pact_consumer::prelude::ValidatingMockServer;
use ripple_sdk::async_channel::Receiver;
use ripple_sdk::async_channel::Sender;
use ripple_sdk::extn::client::extn_client::ExtnClient;
use ripple_sdk::extn::client::extn_sender::ExtnSender;
use ripple_sdk::extn::extn_client_message::ExtnMessage;
use ripple_sdk::extn::extn_client_message::ExtnPayload;
use ripple_sdk::extn::extn_id::ExtnClassId;
use ripple_sdk::extn::extn_id::ExtnId;
use ripple_sdk::extn::ffi::ffi_message::CExtnMessage;
use ripple_sdk::framework::ripple_contract::RippleContract;
use std::collections::HashMap;

#[macro_export]
macro_rules! get_pact {
    ($method:expr, $result:expr) => {
        json!({
            "pact:content-type": "application/json",
            "request": {"jsonrpc": "matching(type, '2.0')", "id": "matching(integer, 0)", "method": $method},
            "requestMetadata": {
                "path": "/jsonrpc",
            },
            "response": [{
                "jsonrpc": "matching(type, '2.0')",
                "id": "matching(integer, 0)",
                "result":  $result.get()
            }]
        })
    }
}

#[macro_export]
macro_rules! get_pact_with_params {
    ($method:expr, $result:expr, $params:expr) => {
        json!({
            "pact:content-type": "application/json",
            "request": {"jsonrpc": "matching(type, '2.0')", "id": "matching(integer, 0)", "method": $method, "params": $params.get()},
            "requestMetadata": {
                "path": "/jsonrpc",
            },
            "response": [{
                "jsonrpc": "matching(type, '2.0')",
                "id": "matching(integer, 0)",
                "result":  $result.get()
            }]
        })
    }
}

#[macro_export]
macro_rules! mock_websocket_server {
    ($pact_builder:ident, $server: ident,$server_url: ident, $test_name:expr, $interactions:expr) => {

        let mut $pact_builder = PactBuilder::new_v4("ripple", $test_name) // Define the message consumer and provider by name
        .using_plugin("websockets", Some("0.4.2".to_string()))
        .await;

        let pacts = $interactions.clone();
        let pacts = pacts.as_array();

        if let Some(multis) = pacts {
         for interaction in multis {
            $pact_builder.synchronous_message_interaction($test_name, |mut i| async move {
             i.test_name($test_name);
             i.contents_from(interaction.clone()).await;
             i
            }).await;
         }
        }else {
            $pact_builder.synchronous_message_interaction($test_name, |mut i| async move {
                i.test_name($test_name);
                i.contents_from($interactions.clone()).await;
                i
               }).await;

        }

        let $server = $pact_builder.start_mock_server_async(Some("websockets/transport/websockets")).await;

        let $server_url = reqwest::Url::parse($server.path("/jsonrpc").as_str()).unwrap();


    }
}

pub struct ContractResult {
    pub result: HashMap<String, ContractMatcher>,
}

pub struct ContractParams {
    pub params: HashMap<String, ContractMatcher>,
}

impl ContractResult {
    pub fn get(&self) -> HashMap<String, String> {
        self.result
            .iter()
            .map(|(k, c)| (k.clone(), c.get()))
            .collect()
    }
}

impl ContractParams {
    pub fn get(&self) -> HashMap<String, String> {
        self.params
            .iter()
            .map(|(k, c)| (k.clone(), c.get()))
            .collect()
    }
}

pub enum ContractMatcher {
    MatchType(String),
    MatchBool(bool),
    MatchRegex(String, String),
    MatchInt(i64),
    MatchDecimal(f64),
    MatchDateTime(String, String),
    MatchNumber(i64),
}

impl ContractMatcher {
    pub fn get(&self) -> String {
        match self {
            Self::MatchBool(b) => format!("matching(boolean, {})", b),
            Self::MatchType(s) => format!("matching(type, '{}')", s),
            Self::MatchRegex(r, v) => format!("matching(regex, '{}', '{}')", r, v),
            Self::MatchInt(i) => format!("matching(integer, {})", i),
            Self::MatchDecimal(i) => format!("matching(decimal, {})", i),
            Self::MatchDateTime(r, v) => format!("matching(datetime, '{}', '{}')", r, v),
            Self::MatchNumber(r) => format!("matching(number, {})", r),
        }
    }
}

pub async fn get_pact_builder_async_obj() -> PactBuilderAsync {
    PactBuilder::new_v4("ripple", "rdk_service")
        .using_plugin("websockets", None)
        .await
}

pub async fn get_pact_mock_server(async_build: PactBuilderAsync) -> Box<dyn ValidatingMockServer> {
    async_build
        .start_mock_server_async(Some("websockets/transport/websockets"))
        .await
}

pub fn get_extn_client(s: Sender<CExtnMessage>, r: Receiver<CExtnMessage>) -> ExtnClient {
    // Creating extn client which will send back the data to the receiver(instead of callback)
    let option_map: Option<HashMap<String, String>> = Some(HashMap::new());

    ExtnClient::new(
        r,
        ExtnSender::new(
            s,
            ExtnId::new_channel(ExtnClassId::Device, "pact".into()),
            Vec::new(),
            Vec::new(),
            option_map,
        ),
    )
}

pub fn get_mock_server_url(mock_server: Box<dyn ValidatingMockServer>) -> url::Url {
    url::Url::parse(mock_server.path("/jsonrpc").as_str()).unwrap()
}

pub fn get_extn_msg(payload: ExtnPayload) -> ExtnMessage {
    ExtnMessage {
        callback: None,
        id: "SomeId".into(),
        payload,
        requestor: ExtnId::new_channel(ExtnClassId::Device, "pact".into()),
        target: RippleContract::DeviceInfo,
        target_id: None,
        ts: Some(30),
    }
}
