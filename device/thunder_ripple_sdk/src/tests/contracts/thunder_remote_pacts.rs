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

use crate::client::thunder_client_pool::ThunderClientPool;
use crate::get_pact_with_params;
use crate::processors::thunder_remote::ThunderRemoteAccessoryRequestProcessor;
use crate::ripple_sdk::extn::client::extn_processor::ExtnRequestProcessor;
use crate::tests::contracts::contract_utils::*;
use crate::{
    ripple_sdk::{
        api::device::{
            device_accessory::{
                AccessoryPairRequest, AccessoryProtocol, AccessoryType, RemoteAccessoryRequest,
            },
            device_request::DeviceRequest,
        },
        crossbeam::channel::unbounded,
        extn::extn_client_message::{ExtnPayload, ExtnRequest},
        serde_json::{self},
        tokio,
    },
    thunder_state::ThunderState,
};
use pact_consumer::mock_server::StartMockServerAsync;
use serde_json::json;
use std::collections::HashMap;

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "contract_tests"), ignore)]
async fn test_device_remote_start_pairing() {
    let mut pact_builder_async = get_pact_builder_async_obj().await;

    let mut params = HashMap::new();
    params.insert("netType".into(), ContractMatcher::MatchInt(21));
    params.insert("timeout".into(), ContractMatcher::MatchInt(30));

    let mut result = HashMap::new();
    result.insert("success".into(), ContractMatcher::MatchBool(true));

    pact_builder_async
        .synchronous_message_interaction(
            "A request to start pairing remote with the STB",
            |mut i| async move {
                i.contents_from(get_pact_with_params!(
                    "org.rdk.RemoteControl.1.startPairing",
                    ContractResult { result },
                    ContractParams { params }
                ))
                .await;
                i.test_name("start_pairing_remote");
                i
            },
        )
        .await;

    let mock_server = pact_builder_async
        .start_mock_server_async(Some("websockets/transport/websockets"))
        .await;

    let _type = AccessoryType::Remote;
    let timeout = 1;
    let protocol = AccessoryProtocol::BluetoothLE;
    let pair_params = AccessoryPairRequest {
        _type: _type,
        timeout: timeout,
        protocol: protocol,
    };
    let payload = ExtnPayload::Request(ExtnRequest::Device(DeviceRequest::Accessory(
        RemoteAccessoryRequest::Pair(pair_params.clone()),
    )));
    let msg = get_extn_msg(payload);

    let url = url::Url::parse(mock_server.path("/jsonrpc").as_str()).unwrap();
    let thunder_client = ThunderClientPool::start(url, None, 1).await.unwrap();

    let (s, r) = unbounded();
    let extn_client = get_extn_client(s.clone(), r.clone());

    let state: ThunderState = ThunderState::new(extn_client, thunder_client);

    let _ = ThunderRemoteAccessoryRequestProcessor::process_request(
        state,
        msg,
        RemoteAccessoryRequest::Pair(pair_params.clone()),
    )
    .await;
}
