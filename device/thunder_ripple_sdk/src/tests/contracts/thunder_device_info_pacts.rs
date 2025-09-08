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
#[allow(dead_code, unused_imports)]
use crate::get_pact_with_params;
#[allow(dead_code, unused_imports)]
use crate::ripple_sdk::{
    serde_json::{self, json},
    tokio,
};
#[allow(dead_code, unused_imports)]
use crate::tests::contracts::contract_utils::*;
#[allow(dead_code, unused_imports)]
use crate::{get_pact, send_thunder_call_message};
#[allow(dead_code, unused_imports)]
use pact_consumer::mock_server::StartMockServerAsync;
#[allow(dead_code, unused_imports)]
use std::collections::HashMap;

#[allow(dead_code, unused_imports)]
use futures_util::{SinkExt, StreamExt};
#[allow(dead_code, unused_imports)]
use ripple_sdk::tokio_tungstenite::connect_async;
#[allow(dead_code, unused_imports)]
use ripple_sdk::tokio_tungstenite::tungstenite::protocol::Message;
#[allow(dead_code, unused_imports)]
use tokio::time::{timeout, Duration};

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "websocket_contract_tests"), ignore)]
async fn test_device_get_info_mac_address() {
    // Define Pact request and response - Start
    println!("[TEST] Starting test_device_get_info_mac_address");
    let mut pact_builder_async = get_pact_builder_async_obj().await;
    println!("[TEST] Pact builder initialized");

    pact_builder_async
        .synchronous_message_interaction("A request to get the device info", |mut i| async move {
        println!("[TEST] Defining synchronous message interaction for device info");
            i.contents_from(json!({
                "pact:content-type": "application/json",
                "request": {"jsonrpc": "matching(type, '2.0')", "id": "matching(integer, 0)", "method": "org.rdk.System.1.getDeviceInfo", "params": {"params": ["estb_mac"]}},
                "requestMetadata": {
                    "path": "/jsonrpc"
                },
                "response": [{
                    "jsonrpc": "matching(type, '2.0')",
                    "id": "matching(integer, 0)",
                    "result": {
                        "estb_mac": "matching(regex, '^([0-9A-Fa-f]{2}[:-]){5}([0-9A-Fa-f]{2})$', 'e0:e1:e2:e3:e4:e5')",
                        "success": "matching(boolean, true)"
                    }
                }]
            })).await;
        println!("[TEST] Interaction contents defined");
            i.test_name("get_device_info_mac_address");
        println!("[TEST] Test name set: get_device_info_mac_address");

            i
    }).await;
    // Define Pact request and response - End

    // Initialize mock server
    println!("[TEST] Starting mock server for device info");
    let mock_server = pact_builder_async
        .start_mock_server_async(Some("websockets/transport/websockets"))
        .await;
    println!(
        "[TEST] Mock server started at: {}",
        mock_server.path("/jsonrpc")
    );

    send_thunder_call_message!(
        url::Url::parse(mock_server.path("/jsonrpc").as_str())
            .unwrap()
            .to_string(),
        json!({
            "jsonrpc": "2.0",
            "id": 0,
            "method": "org.rdk.System.1.getDeviceInfo",
            "params": {
                "params": ["estb_mac"]
            }
        })
    )
    .await;
    println!("[TEST] Thunder call message sent for device info");
}

// #[tokio::test(flavor = "multi_thread")]
// #[cfg_attr(not(feature = "websocket_contract_tests"), ignore)]
// async fn test_device_get_model() {
//     println!("@@@NNA Starting test_device_get_model");
//     let mut pact_builder_async = get_pact_builder_async_obj().await;
//     println!("@@@NNA Pact builder initialized");

//     let mut result = HashMap::new();

//     println!("@@@NNA Preparing expected result matchers...");
//     result.insert(
//         "stbVersion".into(),
//         ContractMatcher::MatchRegex(
//             r"^[A-Z0-9]+_VBN_[A-Za-z0-9]+_sprint_\d{14}sdy(_[A-Z0-9_]+)*$".into(),
//             "AX061AEI_VBN_1911_sprint_20200109040424sdy".into(),
//         ),
//     );

//     result.insert(
//         "receiverVersion".into(),
//         ContractMatcher::MatchRegex(r"^\d+\.\d+\.\d+\.\d+$".into(), "3.14.0.0".into()),
//     );

//     result.insert(
//         "stbTimestamp".into(),
//         ContractMatcher::MatchRegex(
//             r"^\w{3} \d{2} \w{3} \d{4} \d{2}:\d{2}:\d{2} [A-Z ]*UTC$".into(),
//             "Thu 09 Jan 2020 04:04:24 AP UTC".into(),
//         ),
//     );
//     result.insert("success".into(), ContractMatcher::MatchBool(true));

//     println!("@@@NNA Setting up Pact interaction...");
//     pact_builder_async
//         .synchronous_message_interaction("A request to get the device model", |mut i| async move {
//             println!("@@@NNA Defining synchronous message interaction for device model");
//             i.given("System Version info is set");
//             i.contents_from(get_pact!(
//                 "org.rdk.System.1.getSystemVersions",
//                 ContractResult { result }
//             ))
//             .await;
//             println!("@@@NNA Interaction contents defined");
//             i.test_name("get_device_model");
//             println!("@@@NNA Test name set: get_device_model");
//             i
//         })
//         .await;

//     println!("@@@NNA Starting Pact mock server...");
//     let mock_server = pact_builder_async
//         .start_mock_server_async(Some("websockets/transport/websockets"))
//         .await;
//     println!(
//         "@@@NNA Mock server started at: {}",
//         mock_server.path("/jsonrpc")
//     );

//     let url = url::Url::parse(mock_server.path("/jsonrpc").as_str())
//         .unwrap()
//         .to_string();
//     println!("@@@NNA Sending Thunder call message to: {}", url);
//     send_thunder_call_message!(
//         url,
//         json!({
//             "jsonrpc": "2.0",
//             "id": 42,
//             "method": "org.rdk.System.1.getSystemVersions"
//         })
//     )
//     .await;
//     println!("@@@NNA Thunder call message sent for device model");
//     println!("@@@NNA test_device_get_model completed.");
// }

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "websocket_contract_tests"), ignore)]
async fn test_device_get_system_verisons() {
    let mut pact_builder_async = get_pact_builder_async_obj().await;

    let mut result = HashMap::new();
    result.insert(
        "stbVersion".into(),
        ContractMatcher::MatchRegex(
            r"^[A-Z0-9]+_VBN_[A-Za-z0-9]+_sprint_\d{14}sdy(_[A-Z0-9_]+)*$".into(),
            "AX061AEI_VBN_1911_sprint_20200109040424sdy".into(),
        ),
    );
    result.insert(
        "receiverVersion".into(),
        ContractMatcher::MatchRegex(r"^\d+\.\d+\.\d+\.\d+$".into(), "3.14.0.0".into()),
    );
    result.insert(
        "stbTimestamp".into(),
        ContractMatcher::MatchRegex(
            r"^\w{3} \d{2} \w{3} \d{4} \d{2}:\d{2}:\d{2} [A-Z ]*UTC$".into(),
            "Thu 09 Jan 2020 04:04:24 AP UTC".into(),
        ),
    );
    result.insert("success".into(), ContractMatcher::MatchBool(true));

    pact_builder_async
        .synchronous_message_interaction("A request to get system versions", |mut i| async move {
            i.given("System Version info is set");
            i.contents_from(get_pact!(
                "org.rdk.System.1.getSystemVersions",
                ContractResult { result }
            ))
            .await;
            i.test_name("get_system_versions");
            i
        })
        .await;

    let mock_server = pact_builder_async
        .start_mock_server_async(Some("websockets/transport/websockets"))
        .await;

    let url = url::Url::parse(mock_server.path("/jsonrpc").as_str())
        .unwrap()
        .to_string();
    send_thunder_call_message!(
        url,
        json!({
            "jsonrpc": "2.0",
            "id": 42,
            "method": "org.rdk.System.1.getSystemVersions"
        })
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "websocket_contract_tests"), ignore)]
async fn test_device_get_model() {
    let mut pact_builder_async = get_pact_builder_async_obj().await;

    pact_builder_async
        .synchronous_message_interaction("A request to get the device model", |mut i| async move {
            i.given("Device model is set");
            i.contents_from(json!({
                "pact:content-type": "application/json",
                "request": {
                    "jsonrpc": "2.0",
                    "id": 0,
                    "method": "Device.model"
                },
                "requestMetadata": {
                    "path": "/jsonrpc"
                },
                "response": [{
                    "jsonrpc": "2.0",
                    "id": 0,
                    "result": "xi7"
                }]
            }))
            .await;
            i.test_name("get_device_model");
            i
        })
        .await;

    let mock_server = pact_builder_async
        .start_mock_server_async(Some("websockets/transport/websockets"))
        .await;

    let url = url::Url::parse(mock_server.path("/jsonrpc").as_str())
        .unwrap()
        .to_string();
    send_thunder_call_message!(
        url,
        json!({
            "jsonrpc": "2.0",
            "id": 0,
            "method": "Device.model"
        })
    )
    .await;

    send_thunder_call_message!(
        url::Url::parse(mock_server.path("/jsonrpc").as_str())
            .unwrap()
            .to_string(),
        json!({
            "jsonrpc": "2.0",
            "id": 0,
            "method": "device.model"
        })
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "websocket_contract_tests"), ignore)]
async fn test_device_get_interfaces_ethernet() {
    let mut pact_builder_async = get_pact_builder_async_obj().await;

    pact_builder_async
    .synchronous_message_interaction("A request to get the device ethernet interface", |mut i| async move {
        i.contents_from(json!({
            "pact:content-type": "application/json",
            "request": {"jsonrpc": "matching(type, '2.0')", "id": "matching(integer, 0)", "method": "org.rdk.Network.1.getInterfaces"},
            "requestMetadata": {
                "path": "/jsonrpc"
            },
            "response": [{
                "jsonrpc": "matching(type, '2.0')",
                "id": "matching(integer, 0)",
                "result": {
                    "interfaces": [
                        {
                            "interface": "matching(regex, '(WIFI|ETHERNET)', 'ETHERNET')",
                            "macAddress": "matching(regex, '^([0-9A-Fa-f]{2}[:-]){5}([0-9A-Fa-f]{2})$', 'AA:AA:AA:AA:AA:AA')",
                            "enabled": "matching(boolean, true)",
                            "connected": "matching(boolean, true)"
                        },
                        {
                            "interface": "matching(regex, '(WIFI|ETHERNET)', 'WIFI')",
                            "macAddress": "matching(regex, '^([0-9A-Fa-f]{2}[:-]){5}([0-9A-Fa-f]{2})$', 'AA:AA:AA:AA:AA:AA')",
                            "enabled": "matching(boolean, true)",
                            "connected": "matching(boolean, false)"
                        }
                    ],
                    "success": true
                }
            }]
        })).await;
        i.test_name("get_device_interfaces_ethernet");

        i
    }).await;

    let mock_server = pact_builder_async
        .start_mock_server_async(Some("websockets/transport/websockets"))
        .await;

    send_thunder_call_message!(
        url::Url::parse(mock_server.path("/jsonrpc").as_str())
            .unwrap()
            .to_string(),
        json!({
            "jsonrpc": "2.0",
            "id": 0,
            "method": "org.rdk.Network.1.getInterfaces"
        })
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "websocket_contract_tests"), ignore)]
async fn test_device_get_audio() {
    let mut pact_builder_async = get_pact_builder_async_obj().await;

    pact_builder_async
        .synchronous_message_interaction("A request to get the device audio", |mut i| async move {
            i.given("audio format display setting is set");
            i.contents_from(json!({
                "pact:content-type": "application/json",
                "request": {"jsonrpc": "matching(type, '2.0')", "id": "matching(integer, 0)", "method": "org.rdk.DisplaySettings.1.getAudioFormat"},
                "requestMetadata": {
                    "path": "/jsonrpc",
                },
                "response": [{
                    "jsonrpc": "matching(type, '2.0')",
                    "id": "matching(integer, 0)",
                    "result":  {
                        "supportedAudioFormat": [
                            "NONE","PCM","AAC","VORBIS","WMA","DOLBY AC3","DOLBY EAC3","DOLBY AC4","DOLBY MAT","DOLBY TRUEHD","DOLBY EAC3 ATMOS","DOLBY TRUEHD ATMOS","DOLBY MAT ATMOS","DOLBY AC4 ATMOS","UNKNOWN"
                        ],
                        "currentAudioFormat": "matching(type,'PCM')",
                        "success": "matching(boolean, true)"
                    }
                }]
            })).await;
            i.test_name("get_device_audio");
            i
    }).await;

    let mock_server = pact_builder_async
        .start_mock_server_async(Some("websockets/transport/websockets"))
        .await;

    send_thunder_call_message!(
        url::Url::parse(mock_server.path("/jsonrpc").as_str())
            .unwrap()
            .to_string(),
        json!({
            "jsonrpc": "2.0",
            "id": 0,
            "method": "org.rdk.DisplaySettings.1.getAudioFormat"
        })
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "websocket_contract_tests"), ignore)]
async fn test_device_get_hdcp_supported() {
    let mut pact_builder_async = get_pact_builder_async_obj().await;

    let mut result = HashMap::new();
    result.insert(
        "supportedHDCPVersion".into(),
        ContractMatcher::MatchType("2.2".into()),
    );
    result.insert("isHDCPSupported".into(), ContractMatcher::MatchBool(true));
    result.insert("success".into(), ContractMatcher::MatchBool(true));

    pact_builder_async
        .synchronous_message_interaction(
            "A request to get the device hdcp support",
            |mut i| async move {
                i.contents_from(get_pact!(
                    "org.rdk.HdcpProfile.1.getSettopHDCPSupport",
                    ContractResult { result }
                ))
                .await;
                i.test_name("get_device_hdcp_support");
                i
            },
        )
        .await;

    let mock_server = pact_builder_async
        .start_mock_server_async(Some("websockets/transport/websockets"))
        .await;

    send_thunder_call_message!(
        url::Url::parse(mock_server.path("/jsonrpc").as_str())
            .unwrap()
            .to_string(),
        json!({
            "jsonrpc": "2.0",
            "id": 0,
            "method": "org.rdk.HdcpProfile.1.getSettopHDCPSupport"
        })
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "websocket_contract_tests"), ignore)]
async fn test_device_get_hdcp_status() {
    let mut pact_builder_async = get_pact_builder_async_obj().await;

    pact_builder_async
        .synchronous_message_interaction("A request to get the device hdcp status", |mut i| async move {
            i.contents_from(json!({
                "pact:content-type": "application/json",
                "request": {"jsonrpc": "matching(type, '2.0')", "id": "matching(integer, 0)", "method": "org.rdk.HdcpProfile.1.getHDCPStatus"},
                "requestMetadata": {
                    "path": "/jsonrpc",
                },
                "response": [{
                    "jsonrpc": "matching(type, '2.0')",
                    "id": "matching(integer, 0)",
                    "result":  {
                        "HDCPStatus": {
                            "isConnected": "matching(boolean, false)",
                            "isHDCPCompliant": "matching(boolean, false)",
                            "isHDCPEnabled": "matching(boolean, false)",
                            "hdcpReason": "matching(integer, 1)",
                            "supportedHDCPVersion": "matching(type, '2.2')",
                            "receiverHDCPVersion": "matching(type, '1.4')",
                            "currentHDCPVersion": "matching(type, '1.4')"
                        },
                        "success": "matching(boolean, true)"
                    }
                }]
            })).await;
            i.test_name("get_device_hdcp_status");
            i
        }).await;

    let mock_server = pact_builder_async
        .start_mock_server_async(Some("websockets/transport/websockets"))
        .await;

    send_thunder_call_message!(
        url::Url::parse(mock_server.path("/jsonrpc").as_str())
            .unwrap()
            .to_string(),
        json!({
            "jsonrpc": "2.0",
            "id": 0,
            "method": "org.rdk.HdcpProfile.1.getHDCPStatus"
        })
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "websocket_contract_tests"), ignore)]
async fn test_device_get_hdr() {
    let mut pact_builder_async = get_pact_builder_async_obj().await;

    let mut result = HashMap::new();
    result.insert("capabilities".into(), ContractMatcher::MatchInt(3));
    result.insert("success".into(), ContractMatcher::MatchBool(true));

    pact_builder_async
        .synchronous_message_interaction(
            "A request to get the device hdr capabilities",
            |mut i| async move {
                i.contents_from(get_pact!(
                    "org.rdk.DisplaySettings.1.getTVHDRCapabilities",
                    ContractResult { result }
                ))
                .await;
                i.test_name("get_device_hdr_capabilities");
                i
            },
        )
        .await;

    let mock_server = pact_builder_async
        .start_mock_server_async(Some("websockets/transport/websockets"))
        .await;

    send_thunder_call_message!(
        url::Url::parse(mock_server.path("/jsonrpc").as_str())
            .unwrap()
            .to_string(),
        json!({
            "jsonrpc": "2.0",
            "id": 0,
            "method": "org.rdk.DisplaySettings.1.getTVHDRCapabilities"
        })
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "websocket_contract_tests"), ignore)]
async fn test_device_get_screen_resolution_without_port() {
    let mut pact_builder_async = get_pact_builder_async_obj().await;

    let mut result = HashMap::new();
    result.insert(
        "resolution".into(),
        ContractMatcher::MatchType("1080p".into()),
    );
    result.insert("success".into(), ContractMatcher::MatchBool(true));
    pact_builder_async
        .synchronous_message_interaction(
            "A request to get the device current screen resolution without provided port",
            |mut i| async move {
                i.given("current resolution display setting is set");
                i.contents_from(get_pact!(
                    "org.rdk.DisplaySettings.1.getCurrentResolution",
                    ContractResult { result }
                ))
                .await;
                i.test_name("get_device_screen_resolution_without_port");
                i
            },
        )
        .await;

    let mock_server = pact_builder_async
        .start_mock_server_async(Some("websockets/transport/websockets"))
        .await;

    send_thunder_call_message!(
        url::Url::parse(mock_server.path("/jsonrpc").as_str())
            .unwrap()
            .to_string(),
        json!({
            "jsonrpc": "2.0",
            "id": 0,
            "method": "org.rdk.DisplaySettings.1.getCurrentResolution"
        })
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "websocket_contract_tests"), ignore)]
async fn test_device_get_make() {
    let mut pact_builder_async = get_pact_builder_async_obj().await;

    let mut result = HashMap::new();
    result.insert("make".into(), ContractMatcher::MatchType("Arris".into()));
    result.insert("success".into(), ContractMatcher::MatchBool(true));
    pact_builder_async
        .synchronous_message_interaction("A request to get the device make", |mut i| async move {
            i.contents_from(get_pact!(
                "org.rdk.System.1.getDeviceInfo",
                ContractResult { result }
            ))
            .await;
            i.test_name("get_device_make");
            i
        })
        .await;

    let mock_server = pact_builder_async
        .start_mock_server_async(Some("websockets/transport/websockets"))
        .await;

    send_thunder_call_message!(
        url::Url::parse(mock_server.path("/jsonrpc").as_str())
            .unwrap()
            .to_string(),
        json!({
            "jsonrpc": "2.0",
            "id": 0,
            "method": "org.rdk.System.1.getDeviceInfo"
        })
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "websocket_contract_tests"), ignore)]
async fn test_device_get_video_resolution() {
    let mut pact_builder_async = get_pact_builder_async_obj().await;
    pact_builder_async
        .synchronous_message_interaction(
            "A request to get the current resolution on the selected video display port",
            |mut i| async move {
                i.given("current resolution display setting is set");
                i.contents_from(json!({
                    "pact:content-type": "application/json",
                    "request": {
                        "jsonrpc": "matching(type, '2.0')",
                        "id": "matching(integer, 3)",
                        "method": "org.rdk.DisplaySettings.1.getCurrentResolution"
                    },
                    "requestMetadata": {
                        "path": "/jsonrpc"
                    },
                    "response": [{
                        "jsonrpc": "matching(type, '2.0')",
                        "id": "matching(integer, 0)",
                        "result": {
                            "resolution": "matching(type, 'string')",
                            "w": "matching(integer, 1920)",
                            "h": "matching(integer, 1080)",
                            "progressive": "matching(boolean, true)",
                            "success": true
                        }
                    }]
                }))
                .await;
                i.test_name("get_device_current_video_resolution");
                i
            },
        )
        .await;

    let mock_server = pact_builder_async
        .start_mock_server_async(Some("websockets/transport/websockets"))
        .await;

    send_thunder_call_message!(
        url::Url::parse(mock_server.path("/jsonrpc").as_str())
            .unwrap()
            .to_string(),
        json!({
            "jsonrpc": "2.0",
            "id": 0,
            "method": "org.rdk.DisplaySettings.1.getCurrentResolution"
        })
    )
    .await;
}

// #[tokio::test(flavor = "multi_thread")]
// #[cfg_attr(not(feature = "websocket_contract_tests"), ignore)]
// async fn test_device_get_system_memory() {
//     let mut pact_builder_async = get_pact_builder_async_obj().await;

//     let mut result = HashMap::new();
//     result.insert("freeRam".into(), ContractMatcher::MatchNumber(321944));
//     result.insert("swapRam".into(), ContractMatcher::MatchNumber(0));
//     result.insert("totalRam".into(), ContractMatcher::MatchNumber(624644));
//     result.insert("success".into(), ContractMatcher::MatchBool(true));

//     pact_builder_async
//         .synchronous_message_interaction(
//             "A request to get the device available system memory",
//             |mut i| async move {
//                 i.contents_from(get_pact!(
//                     "org.rdk.RDKShell.1.getSystemMemory",
//                     ContractResult { result }
//                 ))
//                 .await;
//                 i.test_name("get_device_system_memory");
//                 i
//             },
//         )
//         .await;

//     let mock_server = pact_builder_async
//         .start_mock_server_async(Some("websockets/transport/websockets"))
//         .await;

//     let payload = ExtnPayload::Request(ExtnRequest::Device(DeviceRequest::DeviceInfo(
//         DeviceInfoRequest::AvailableMemory,
//     )));
//     let msg = get_extn_msg(payload);

//     let url = url::Url::parse(mock_server.path("/jsonrpc").as_str()).unwrap();
//     let thunder_client = ThunderClientPool::start(url, None, 1).await.unwrap();

//     let (s, r) = unbounded();
//     let extn_client = get_extn_client(s, r);

//     let state: CachedState = CachedState::new(ThunderState::new(extn_client, thunder_client));

//     let _ = ThunderDeviceInfoRequestProcessor::process_request(
//         state,
//         msg,
//         DeviceInfoRequest::AvailableMemory,
//     )
//     .await;
// }

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "websocket_contract_tests"), ignore)]
async fn test_device_get_timezone() {
    let mut pact_builder_async = get_pact_builder_async_obj().await;

    let mut result = HashMap::new();
    result.insert(
        "timeZone".into(),
        ContractMatcher::MatchType("America/New_York".into()),
    );
    result.insert(
        "accuracy".into(),
        ContractMatcher::MatchRegex("(INITIAL|INTERIM|FINAL)".into(), "INITIAL".into()),
    );
    result.insert("success".into(), ContractMatcher::MatchBool(true));

    pact_builder_async
        .synchronous_message_interaction(
            "A request to get the device timezone",
            |mut i| async move {
                i.contents_from(get_pact!(
                    "org.rdk.System.1.getTimeZoneDST",
                    ContractResult { result }
                ))
                .await;
                i.test_name("get_device_timezone");
                i
            },
        )
        .await;

    let mock_server = pact_builder_async
        .start_mock_server_async(Some("websockets/transport/websockets"))
        .await;

    send_thunder_call_message!(
        url::Url::parse(mock_server.path("/jsonrpc").as_str())
            .unwrap()
            .to_string(),
        json!({
            "jsonrpc": "2.0",
            "id": 0,
            "method": "org.rdk.System.1.getTimeZoneDST"
        })
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "websocket_contract_tests"), ignore)]
async fn test_device_get_available_timezone() {
    let mut pact_builder_async = get_pact_builder_async_obj().await;

    pact_builder_async
        .synchronous_message_interaction(
            "A request to get the device available timezone",
            |mut i| async move {
                i.given("system time zones is set");
                i.contents_from(json!({
                    "pact:content-type": "application/json",
                    "request": {
                        "jsonrpc": "matching(type, '2.0')",
                        "id": "matching(integer, 1)",
                        "method": "org.rdk.System.1.getTimeZones"
                    },
                    "requestMetadata": {
                        "path": "/jsonrpc"
                    },
                    "response": [{
                        "jsonrpc": "matching(type, '2.0')",
                        "id": "matching(integer, 0)",
                        "result": {
                            "zoneinfo": {
                                "EST": "matching(type, 'string')",
                                "America": {
                                    "New_York": "matching(type, 'string')",
                                    "Los_Angeles": "matching(type, 'string')",
                                },
                                "Europe": {
                                    "London": "matching(type, 'string')",
                                }
                            },
                            "success": "matching(boolean, true)"
                        }
                    }]
                }))
                .await;
                i.test_name("get_device_available_timezone");
                i
            },
        )
        .await;

    let mock_server = pact_builder_async
        .start_mock_server_async(Some("websockets/transport/websockets"))
        .await;

    send_thunder_call_message!(
        url::Url::parse(mock_server.path("/jsonrpc").as_str())
            .unwrap()
            .to_string(),
        json!({
            "jsonrpc": "2.0",
            "id": 0,
            "method": "org.rdk.System.1.getTimeZones"
        })
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "websocket_contract_tests"), ignore)]
async fn test_device_get_voice_guidance_enabled() {
    let mut pact_builder_async = get_pact_builder_async_obj().await;

    let mut result = HashMap::new();
    result.insert("isenabled".into(), ContractMatcher::MatchBool(true));
    result.insert(
        "TTS_Status".into(),
        ContractMatcher::MatchRegex("(0|1|2|3)".into(), "0".into()),
    );
    result.insert("success".into(), ContractMatcher::MatchBool(true));

    pact_builder_async
        .synchronous_message_interaction(
            "A request to get the device voice guidance enabled",
            |mut i| async move {
                i.contents_from(get_pact!(
                    "org.rdk.TextToSpeech.1.isttsenabled",
                    ContractResult { result }
                ))
                .await;
                i.test_name("get_device_voice_guidance_enabled");
                i
            },
        )
        .await;

    let mock_server = pact_builder_async
        .start_mock_server_async(Some("websockets/transport/websockets"))
        .await;

    send_thunder_call_message!(
        url::Url::parse(mock_server.path("/jsonrpc").as_str())
            .unwrap()
            .to_string(),
        json!({
            "jsonrpc": "2.0",
            "id": 0,
            "method": "org.rdk.TextToSpeech.1.isttsenabled"
        })
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "websocket_contract_tests"), ignore)]
async fn test_device_get_voice_guidance_speed() {
    let mut pact_builder_async = get_pact_builder_async_obj().await;

    let mut result = HashMap::new();
    result.insert(
        "ttsendpoint".into(),
        ContractMatcher::MatchRegex(
            r"^(https?://)?([\da-z\.-]+)\.([a-z\.]{2,6})([\/\w \.-]*)*\/?$".into(),
            "http://url_for_the_text_to_speech_processing_unit.com".into(),
        ),
    );
    result.insert(
        "ttsendpointsecured".into(),
        ContractMatcher::MatchRegex(
            r"^(https?://)?([\da-z\.-]+)\.([a-z\.]{2,6})([\/\w \.-]*)*\/?$".into(),
            "https://url_for_the_text_to_speech_processing_unit.com".into(),
        ),
    );
    result.insert(
        "language".into(),
        ContractMatcher::MatchType("en-US".into()),
    );
    result.insert("voice".into(), ContractMatcher::MatchType("carol".into()));
    result.insert(
        "volume".into(),
        ContractMatcher::MatchType("100.000000".into()),
    );
    result.insert("rate".into(), ContractMatcher::MatchNumber(50));
    result.insert(
        "TTS_Status".into(),
        ContractMatcher::MatchRegex("(0|1|2|3)".into(), "0".into()),
    );
    result.insert("success".into(), ContractMatcher::MatchBool(true));

    pact_builder_async
        .synchronous_message_interaction(
            "A request to get the device voice guidance speed",
            |mut i| async move {
                i.contents_from(get_pact!(
                    "org.rdk.TextToSpeech.1.getttsconfiguration",
                    ContractResult { result }
                ))
                .await;
                i.test_name("get_voice_guidance_speed");
                i
            },
        )
        .await;

    let mock_server = pact_builder_async
        .start_mock_server_async(Some("websockets/transport/websockets"))
        .await;

    send_thunder_call_message!(
        url::Url::parse(mock_server.path("/jsonrpc").as_str())
            .unwrap()
            .to_string(),
        json!({
            "jsonrpc": "2.0",
            "id": 0,
            "method": "org.rdk.TextToSpeech.1.getttsconfiguration"
        })
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "websocket_contract_tests"), ignore)]
async fn test_device_set_timezone() {
    let mut pact_builder_async = get_pact_builder_async_obj().await;

    let mut result = HashMap::new();
    result.insert("success".into(), ContractMatcher::MatchBool(true));

    let mut params = HashMap::new();
    params.insert(
        "timeZone".into(),
        ContractMatcher::MatchType("America/New_York".into()),
    );

    pact_builder_async
        .synchronous_message_interaction(
            "A request to set the device timezone",
            |mut i| async move {
                i.contents_from(get_pact_with_params!(
                    "org.rdk.System.1.setTimeZoneDST",
                    ContractResult { result },
                    ContractParams { params }
                ))
                .await;
                i.test_name("set_device_timezone");
                i
            },
        )
        .await;

    let mock_server = pact_builder_async
        .start_mock_server_async(Some("websockets/transport/websockets"))
        .await;

    send_thunder_call_message!(
        url::Url::parse(mock_server.path("/jsonrpc").as_str())
            .unwrap()
            .to_string(),
        json!({
            "jsonrpc": "2.0",
            "id": 0,
            "method": "org.rdk.System.1.setTimeZoneDST",
            "params": json!({
                "timeZone": "America/New_York"
            })
        })
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "websocket_contract_tests"), ignore)]
async fn test_device_set_voice_guidance() {
    let mut pact_builder_async = get_pact_builder_async_obj().await;

    let mut result = HashMap::new();
    result.insert("TTS_Status".into(), ContractMatcher::MatchNumber(2));
    result.insert("success".into(), ContractMatcher::MatchBool(true));

    let mut params = HashMap::new();
    params.insert("enabletts".into(), ContractMatcher::MatchBool(false));

    pact_builder_async
        .synchronous_message_interaction(
            "A request to set the device voice guidance",
            |mut i| async move {
                i.contents_from(get_pact_with_params!(
                    "org.rdk.TextToSpeech.1.enabletts",
                    ContractResult { result },
                    ContractParams { params }
                ))
                .await;
                i.test_name("set_device_voice_guidance");
                i
            },
        )
        .await;

    let mock_server = pact_builder_async
        .start_mock_server_async(Some("websockets/transport/websockets"))
        .await;

    send_thunder_call_message!(
        url::Url::parse(mock_server.path("/jsonrpc").as_str())
            .unwrap()
            .to_string(),
        json!({
            "jsonrpc": "2.0",
            "id": 0,
            "method": "org.rdk.TextToSpeech.1.enabletts",
            "params": json!({
                "enabletts": false
            })
        })
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "websocket_contract_tests"), ignore)]
async fn test_device_set_voice_guidance_speed() {
    let mut pact_builder_async = get_pact_builder_async_obj().await;

    let mut result = HashMap::new();
    result.insert("TTS_Status".into(), ContractMatcher::MatchNumber(0));
    result.insert("success".into(), ContractMatcher::MatchBool(true));

    let mut params = HashMap::new();
    params.insert("rate".into(), ContractMatcher::MatchNumber(50));

    pact_builder_async
        .synchronous_message_interaction(
            "A request to set the device voice guidance speed",
            |mut i| async move {
                i.contents_from(get_pact_with_params!(
                    "org.rdk.TextToSpeech.1.setttsconfiguration",
                    ContractResult { result },
                    ContractParams { params }
                ))
                .await;
                i.test_name("set_device_voice_guidance_speed");
                i
            },
        )
        .await;

    let mock_server = pact_builder_async
        .start_mock_server_async(Some("websockets/transport/websockets"))
        .await;

    send_thunder_call_message!(
        url::Url::parse(mock_server.path("/jsonrpc").as_str())
            .unwrap()
            .to_string(),
        json!({
            "jsonrpc": "2.0",
            "id": 0,
            "method": "org.rdk.TextToSpeech.1.setttsconfiguration",
            "params": json!({
                "rate": 50
            })
        })
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "websocket_contract_tests"), ignore)]
async fn test_device_get_internet() {
    let mut pact_builder_async = get_pact_builder_async_obj().await;

    let mut result = HashMap::new();
    result.insert(
        "connectedToInternet".into(),
        ContractMatcher::MatchBool(true),
    );
    result.insert("success".into(), ContractMatcher::MatchBool(true));

    pact_builder_async
        .synchronous_message_interaction(
            "A request to get the device internet",
            |mut i| async move {
                i.contents_from(get_pact!(
                    "org.rdk.Network.1.isConnectedToInternet",
                    ContractResult { result }
                ))
                .await;
                i.test_name("get_device_internet");
                i
            },
        )
        .await;

    let mock_server = pact_builder_async
        .start_mock_server_async(Some("websockets/transport/websockets"))
        .await;

    send_thunder_call_message!(
        url::Url::parse(mock_server.path("/jsonrpc").as_str())
            .unwrap()
            .to_string(),
        json!({
            "jsonrpc": "2.0",
            "id": 0,
            "method": "org.rdk.Network.1.isConnectedToInternet"
        })
    )
    .await;
}
