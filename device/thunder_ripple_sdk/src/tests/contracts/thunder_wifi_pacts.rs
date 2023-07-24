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

use crate::get_pact;
use crate::get_pact_with_params;
use crate::processors::thunder_wifi::ThunderWifiRequestProcessor;
use crate::ripple_sdk::api::device::device_wifi::WifiRequest;
use crate::ripple_sdk::extn::client::extn_processor::ExtnRequestProcessor;
use crate::ripple_sdk::{
    api::device::{
            device_request::DeviceRequest,
            device_wifi::{AccessPointRequest, WifiSecurityMode},
    },
    crossbeam::channel::unbounded,
    extn::extn_client_message::{ExtnPayload, ExtnRequest},
    serde_json, tokio,
};
use crate::tests::contracts::contract_utils::*;
use crate::{client::thunder_client_pool::ThunderClientPool, thunder_state::ThunderState};
use pact_consumer::mock_server::StartMockServerAsync;
use serde_json::json;
use std::collections::HashMap;

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "contract_tests"), ignore)]
async fn test_device_scan_wifi() {
    let mut pact_builder_async = get_pact_builder_async_obj().await;

    let mut result = HashMap::new();
    result.insert("success".into(), ContractMatcher::MatchBool(true));

    pact_builder_async
        .synchronous_message_interaction("A request to start wifi scan", |mut i| async move {
            i.contents_from(get_pact!(
                "org.rdk.Wifi.1.startScan",
                ContractResult { result }
            ))
            .await;
            i.test_name("start_scan_of_wifi");
            i
        })
        .await;

    let mock_server = pact_builder_async
        .start_mock_server_async(Some("websockets/transport/websockets"))
        .await;

    let payload = ExtnPayload::Request(ExtnRequest::Device(DeviceRequest::Wifi(
        WifiRequest::Scan(5),
    )));
    let msg = get_extn_msg(payload);

    let url = url::Url::parse(mock_server.path("/jsonrpc").as_str()).unwrap();
    let thunder_client = ThunderClientPool::start(url, None, 1).await.unwrap();

    let (s, r) = unbounded();
    let extn_client = get_extn_client(s.clone(), r.clone());

    let state: ThunderState = ThunderState::new(extn_client, thunder_client);

    let _ = ThunderWifiRequestProcessor::process_request(state, msg, WifiRequest::Scan(5)).await;
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "contract_tests"), ignore)]
async fn test_device_connect_wifi() {
    let mut pact_builder_async = get_pact_builder_async_obj().await;

    let mut params = HashMap::new();
    params.insert(
        "ssid".into(),
        ContractMatcher::MatchType("test-wifi".into()),
    );
    params.insert(
        "passphrase".into(),
        ContractMatcher::MatchType("test-passphrase".into()),
    );
    params.insert("securityMode".into(), ContractMatcher::MatchInt(2));

    let mut result = HashMap::new();
    result.insert("success".into(), ContractMatcher::MatchBool(true));

    pact_builder_async
        .synchronous_message_interaction("A request to connect wifi", |mut i| async move {
            i.contents_from(get_pact_with_params!(
                "org.rdk.Wifi.1.connect",
                ContractResult { result },
                ContractParams { params }
            ))
            .await;
            i.test_name("connect_to_wifi");
            i
        })
        .await;

    let mock_server = pact_builder_async
        .start_mock_server_async(Some("websockets/transport/websockets"))
        .await;

    let ssid = "test-wifi";
    let passphrase = "test-passphrase";
    let connect_wifi_params = AccessPointRequest {
        ssid: ssid.to_string(),
        passphrase: passphrase.to_string(),
        security: WifiSecurityMode::Wpa2Psk,
    };
    let payload = ExtnPayload::Request(ExtnRequest::Device(DeviceRequest::Wifi(
        WifiRequest::Connect(connect_wifi_params.clone()),
    )));
    let msg = get_extn_msg(payload);

    let url = url::Url::parse(mock_server.path("/jsonrpc").as_str()).unwrap();
    let thunder_client = ThunderClientPool::start(url, None, 1).await.unwrap();

    let (s, r) = unbounded();
    let extn_client = get_extn_client(s.clone(), r.clone());

    let state: ThunderState = ThunderState::new(extn_client, thunder_client);

    let _ = ThunderWifiRequestProcessor::process_request(
        state,
        msg,
        WifiRequest::Connect(connect_wifi_params.clone()),
    )
    .await;
}
