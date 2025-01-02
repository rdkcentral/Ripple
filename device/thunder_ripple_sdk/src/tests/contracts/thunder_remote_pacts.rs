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
use crate::thunder_state::ThunderConnectionState;
use crate::{
    ripple_sdk::{
        api::device::{
            device_accessory::{
                AccessoryPairRequest, AccessoryProtocol, AccessoryType, RemoteAccessoryRequest,
            },
            device_request::DeviceRequest,
        },
        async_channel::unbounded,
        extn::extn_client_message::{ExtnPayload, ExtnRequest},
        serde_json::{self},
    },
    thunder_state::ThunderState,
};
use pact_consumer::mock_server::StartMockServerAsync;
use ripple_sdk::api::device::device_accessory::AccessoryListRequest;
use ripple_sdk::api::device::device_accessory::AccessoryListType;
use ripple_sdk::api::device::device_accessory::AccessoryProtocolListType;
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;

// #[tokio::test(flavor = "multi_thread")]
// #[cfg_attr(not(feature = "contract_tests"), ignore)]
#[allow(dead_code)]
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
        _type,
        timeout,
        protocol,
    };
    let payload = ExtnPayload::Request(ExtnRequest::Device(DeviceRequest::Accessory(
        RemoteAccessoryRequest::Pair(pair_params.clone()),
    )));
    let msg = get_extn_msg(payload);

    let url = url::Url::parse(mock_server.path("/jsonrpc").as_str()).unwrap();
    let thunder_client =
        ThunderClientPool::start(url, None, Some(Arc::new(ThunderConnectionState::new())), 1)
            .await
            .unwrap();

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

// #[tokio::test(flavor = "multi_thread")]
// #[cfg_attr(not(feature = "contract_tests"), ignore)]
#[allow(dead_code)]
async fn test_device_remote_network_status() {
    let mut pact_builder_async = get_pact_builder_async_obj().await;

    pact_builder_async
        .synchronous_message_interaction("A request to get the device info", |mut i| async move {
            i.contents_from(json!({
                "pact:content-type": "application/json",
                "request": {"jsonrpc": "matching(type, '2.0')", "id": "matching(integer, 0)", "method": "org.rdk.RemoteControl.1.getNetStatus", "params": {"netType": "matching(decimal, 21)"}},
                "requestMetadata": {
                    "path": "/jsonrpc"
                },
                "response": [{
                    "jsonrpc": "matching(type, '2.0')",
                    "id": "matching(integer, 0)",
                    "result": {
                        "status": {
                            "netType": 21,
                            "netTypeSupported": [
                                [
                                    21
                                ]
                            ],
                            "pairingState": "matching(regex, '(INITIALISING|IDLE|SEARCHING|PAIRING|COMPLETE|FAILED)', 'COMPLETE')",
                            "irProgState": "matching(regex, '(IDLE|WAITING|COMPLETE|FAILED)', 'COMPLETE')",
                            "remoteData": [
                                {
                                    "macAddress": "matching(regex, '^([0-9A-Fa-f]{2}[:-]){5}([0-9A-Fa-f]{2})$', 'E8:1C:FD:9A:07:1E')",
                                    "connected": "matching(boolean, true)",
                                    "name": "matching(type, 'P073 SkyQ EC201')",
                                    "remoteId": "matching(integer, 1)",
                                    "deviceId": "matching(integer, 65)",
                                    "make": "matching(type, 'Omni Remotes')",
                                    "model": "matching(type, 'PEC201')",
                                    "hwVersion": "matching(type, '201.2.0.0')",
                                    "swVersion": "matching(type, '1.0.0')",
                                    "btlVersion": "matching(type, '2.0')",
                                    "serialNumber": "matching(type, '18464408B544')",
                                    "batteryPercent": "matching(integer, 85)",
                                    "tvIRCode": "matching(type, '1')",
                                    "ampIRCode": "matching(type, '1')",
                                    "wakeupKeyCode": "matching(integer, 65)",
                                    "wakeupConfig": "matching(type, 'custom')",
                                    "wakeupCustomList": "matching(type, '[3,1]')"
                                }
                            ]
                        },
                        "success": "matching(boolean, true)"
                    }
                }]
            })).await;
            i.test_name("remote_network_status");

            i
    }).await;

    let mock_server = pact_builder_async
        .start_mock_server_async(Some("websockets/transport/websockets"))
        .await;

    let _type: Option<AccessoryListType> = Some(AccessoryListType::All);
    let protocol: Option<AccessoryProtocolListType> = Some(AccessoryProtocolListType::All);
    let list_params = AccessoryListRequest { _type, protocol };
    let payload = ExtnPayload::Request(ExtnRequest::Device(DeviceRequest::Accessory(
        RemoteAccessoryRequest::List(list_params.clone()),
    )));
    let msg = get_extn_msg(payload);

    let url = url::Url::parse(mock_server.path("/jsonrpc").as_str()).unwrap();
    let thunder_client =
        ThunderClientPool::start(url, None, Some(Arc::new(ThunderConnectionState::new())), 1)
            .await
            .unwrap();

    let (s, r) = unbounded();
    let extn_client = get_extn_client(s.clone(), r.clone());

    let state: ThunderState = ThunderState::new(extn_client, thunder_client);

    let _ = ThunderRemoteAccessoryRequestProcessor::process_request(
        state,
        msg,
        RemoteAccessoryRequest::List(list_params.clone()),
    )
    .await;
}
