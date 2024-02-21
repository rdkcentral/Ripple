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

use serde_json::json;
use std::collections::HashMap;

use crate::{
    client::thunder_client_pool::ThunderClientPool,
    get_pact_with_params,
    ripple_sdk::{
        async_channel::unbounded,
        extn::extn_client_message::{ExtnPayload, ExtnRequest},
    },
    thunder_state::ThunderState,
};

use crate::ripple_sdk::{
    api::device::{
        device_info_request::DeviceInfoRequest,
        device_request::{DeviceRequest, OnInternetConnectedRequest},
    },
    extn::client::extn_processor::ExtnRequestProcessor,
    serde_json::{self},
    tokio,
};

use crate::processors::thunder_device_info::{CachedState, ThunderDeviceInfoRequestProcessor};

use crate::get_pact;
use crate::tests::contracts::contract_utils::*;
use pact_consumer::mock_server::StartMockServerAsync;

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "contract_tests"), ignore)]
async fn test_device_get_info_mac_address() {
    // Define Pact request and response - Start
    let mut pact_builder_async = get_pact_builder_async_obj().await;

    pact_builder_async
        .synchronous_message_interaction("A request to get the device info", |mut i| async move {
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
            i.test_name("get_device_info_mac_address");

            i
    }).await;
    // Define Pact request and response - End

    // Initialize mock server
    let mock_server = pact_builder_async
        .start_mock_server_async(Some("websockets/transport/websockets"))
        .await;

    // Creating a mock extn message needed for calling method
    let payload = ExtnPayload::Request(ExtnRequest::Device(DeviceRequest::DeviceInfo(
        DeviceInfoRequest::MacAddress,
    )));
    let msg = get_extn_msg(payload);

    // Creating thunder client with mock server url
    let url = url::Url::parse(mock_server.path("/jsonrpc").as_str()).unwrap();
    let thunder_client = ThunderClientPool::start(url, None, 1).await.unwrap();

    // Creating extn client which will send back the data to the receiver(instead of callback)
    let (s, r) = unbounded();
    let extn_client = get_extn_client(s, r);

    // Create  Thunderstate which contains the extn_client for callback and thunder client for making thunder requests
    let state: CachedState = CachedState::new(ThunderState::new(extn_client, thunder_client));

    // Make call to method
    let _ = ThunderDeviceInfoRequestProcessor::process_request(
        state,
        msg,
        DeviceInfoRequest::MacAddress,
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "contract_tests"), ignore)]
async fn test_device_get_model() {
    let mut pact_builder_async = get_pact_builder_async_obj().await;

    let mut result = HashMap::new();
    result.insert(
        "stbVersion".into(),
        ContractMatcher::MatchType("AX061AEI_VBN_1911_sprint_20200109040424sdy".into()),
    );
    result.insert(
        "receiverVersion".into(),
        ContractMatcher::MatchRegex(
            r"^[0-9]\d*.[0-9]\d*.[0-9]\d*.[0-9]\d*$".into(),
            "3.14.0.0".into(),
        ),
    );
    result.insert(
        "stbTimestamp".into(),
        ContractMatcher::MatchDateTime(
            "EEE dd MMM yyyy HH:mm:ss a z".into(),
            "Thu 09 Jan 2020 04:04:24 AM UTC".into(),
        ),
    );
    result.insert("success".into(), ContractMatcher::MatchBool(true));

    pact_builder_async
        .synchronous_message_interaction("A request to get the device model", |mut i| async move {
            i.contents_from(get_pact!(
                "org.rdk.System.1.getSystemVersions",
                ContractResult { result }
            ))
            .await;
            i.test_name("get_device_model");
            i
        })
        .await;

    let mock_server = pact_builder_async
        .start_mock_server_async(Some("websockets/transport/websockets"))
        .await;

    let payload = ExtnPayload::Request(ExtnRequest::Device(DeviceRequest::DeviceInfo(
        DeviceInfoRequest::Model,
    )));
    let msg = get_extn_msg(payload);

    let url = url::Url::parse(mock_server.path("/jsonrpc").as_str()).unwrap();
    let thunder_client = ThunderClientPool::start(url, None, 1).await.unwrap();

    let (s, r) = unbounded();
    let extn_client = get_extn_client(s.clone(), r.clone());

    let state: CachedState = CachedState::new(ThunderState::new(extn_client, thunder_client));

    let _ =
        ThunderDeviceInfoRequestProcessor::process_request(state, msg, DeviceInfoRequest::Model)
            .await;
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "contract_tests"), ignore)]
async fn test_device_get_interfaces_wifi() {
    let mut pact_builder_async = get_pact_builder_async_obj().await;

    pact_builder_async
    .synchronous_message_interaction("A request to get the device wifi interface", |mut i| async move {
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
                            "interface": "matching(regex, '(WIFI|ETHERNET)', 'WIFI')",
                            "macAddress": "matching(regex, '^([0-9A-Fa-f]{2}[:-]){5}([0-9A-Fa-f]{2})$', 'AA:AA:AA:AA:AA:AA')",
                            "enabled": "matching(boolean, true)",
                            "connected": "matching(boolean, true)"
                        },
                        {
                            "interface": "matching(regex, '(WIFI|ETHERNET)', 'ETHERNET')",
                            "macAddress": "matching(regex, '^([0-9A-Fa-f]{2}[:-]){5}([0-9A-Fa-f]{2})$', 'AA:AA:AA:AA:AA:AA')",
                            "enabled": "matching(boolean, true)",
                            "connected": "matching(boolean, false)"
                        }
                    ],
                    "success": true
                }
            }]
        })).await;
        i.test_name("get_device_interfaces_wifi");

        i
    }).await;

    let mock_server = pact_builder_async
        .start_mock_server_async(Some("websockets/transport/websockets"))
        .await;

    let payload = ExtnPayload::Request(ExtnRequest::Device(DeviceRequest::DeviceInfo(
        DeviceInfoRequest::Network,
    )));
    let msg = get_extn_msg(payload);

    let url = url::Url::parse(mock_server.path("/jsonrpc").as_str()).unwrap();
    let thunder_client = ThunderClientPool::start(url, None, 1).await.unwrap();

    let (s, r) = unbounded();
    let extn_client = get_extn_client(s.clone(), r.clone());

    let state: CachedState = CachedState::new(ThunderState::new(extn_client, thunder_client));

    let _ =
        ThunderDeviceInfoRequestProcessor::process_request(state, msg, DeviceInfoRequest::Network)
            .await;
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "contract_tests"), ignore)]
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

    let payload = ExtnPayload::Request(ExtnRequest::Device(DeviceRequest::DeviceInfo(
        DeviceInfoRequest::Network,
    )));
    let msg = get_extn_msg(payload);

    let url = url::Url::parse(mock_server.path("/jsonrpc").as_str()).unwrap();
    let thunder_client = ThunderClientPool::start(url, None, 1).await.unwrap();

    let (s, r) = unbounded();
    let extn_client = get_extn_client(s, r);

    let state: CachedState = CachedState::new(ThunderState::new(extn_client, thunder_client));

    let _ =
        ThunderDeviceInfoRequestProcessor::process_request(state, msg, DeviceInfoRequest::Network)
            .await;
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "contract_tests"), ignore)]
async fn test_device_get_audio() {
    let mut pact_builder_async = get_pact_builder_async_obj().await;

    pact_builder_async
        .synchronous_message_interaction("A request to get the device audio", |mut i| async move {
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

    let payload = ExtnPayload::Request(ExtnRequest::Device(DeviceRequest::DeviceInfo(
        DeviceInfoRequest::Audio,
    )));
    let msg = get_extn_msg(payload);

    let url = url::Url::parse(mock_server.path("/jsonrpc").as_str()).unwrap();
    let thunder_client = ThunderClientPool::start(url, None, 1).await.unwrap();

    let (s, r) = unbounded();
    let extn_client = get_extn_client(s, r);

    let state: CachedState = CachedState::new(ThunderState::new(extn_client, thunder_client));

    let _ =
        ThunderDeviceInfoRequestProcessor::process_request(state, msg, DeviceInfoRequest::Audio)
            .await;
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "contract_tests"), ignore)]
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

    let payload = ExtnPayload::Request(ExtnRequest::Device(DeviceRequest::DeviceInfo(
        DeviceInfoRequest::HdcpSupport,
    )));
    let msg = get_extn_msg(payload);

    let url = url::Url::parse(mock_server.path("/jsonrpc").as_str()).unwrap();
    let thunder_client = ThunderClientPool::start(url, None, 1).await.unwrap();

    let (s, r) = unbounded();
    let extn_client = get_extn_client(s, r);

    let state: CachedState = CachedState::new(ThunderState::new(extn_client, thunder_client));

    let _ = ThunderDeviceInfoRequestProcessor::process_request(
        state,
        msg,
        DeviceInfoRequest::HdcpSupport,
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "contract_tests"), ignore)]
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

    let payload = ExtnPayload::Request(ExtnRequest::Device(DeviceRequest::DeviceInfo(
        DeviceInfoRequest::HdcpStatus,
    )));
    let msg = get_extn_msg(payload);

    let url = url::Url::parse(mock_server.path("/jsonrpc").as_str()).unwrap();
    let thunder_client = ThunderClientPool::start(url, None, 1).await.unwrap();

    let (s, r) = unbounded();
    let extn_client = get_extn_client(s, r);

    let state: CachedState = CachedState::new(ThunderState::new(extn_client, thunder_client));

    let _ = ThunderDeviceInfoRequestProcessor::process_request(
        state,
        msg,
        DeviceInfoRequest::HdcpStatus,
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "contract_tests"), ignore)]
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

    let payload = ExtnPayload::Request(ExtnRequest::Device(DeviceRequest::DeviceInfo(
        DeviceInfoRequest::Hdr,
    )));
    let msg = get_extn_msg(payload);

    let url = url::Url::parse(mock_server.path("/jsonrpc").as_str()).unwrap();
    let thunder_client = ThunderClientPool::start(url, None, 1).await.unwrap();

    let (s, r) = unbounded();
    let extn_client = get_extn_client(s, r);

    let state: CachedState = CachedState::new(ThunderState::new(extn_client, thunder_client));

    let _ = ThunderDeviceInfoRequestProcessor::process_request(state, msg, DeviceInfoRequest::Hdr)
        .await;
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "contract_tests"), ignore)]
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

    let payload = ExtnPayload::Request(ExtnRequest::Device(DeviceRequest::DeviceInfo(
        DeviceInfoRequest::ScreenResolution,
    )));
    let msg = get_extn_msg(payload);

    let url = url::Url::parse(mock_server.path("/jsonrpc").as_str()).unwrap();
    let thunder_client = ThunderClientPool::start(url, None, 1).await.unwrap();

    let (s, r) = unbounded();
    let extn_client = get_extn_client(s, r);

    let state: CachedState = CachedState::new(ThunderState::new(extn_client, thunder_client));

    let _ = ThunderDeviceInfoRequestProcessor::process_request(
        state,
        msg,
        DeviceInfoRequest::ScreenResolution,
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "contract_tests"), ignore)]
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

    let payload = ExtnPayload::Request(ExtnRequest::Device(DeviceRequest::DeviceInfo(
        DeviceInfoRequest::Make,
    )));
    let msg = get_extn_msg(payload);

    let url = url::Url::parse(mock_server.path("/jsonrpc").as_str()).unwrap();
    let thunder_client = ThunderClientPool::start(url, None, 1).await.unwrap();

    let (s, r) = unbounded();
    let extn_client = get_extn_client(s, r);

    let state: CachedState = CachedState::new(ThunderState::new(extn_client, thunder_client));

    let _ = ThunderDeviceInfoRequestProcessor::process_request(state, msg, DeviceInfoRequest::Make)
        .await;
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "contract_tests"), ignore)]
async fn test_device_get_video_resolution() {
    let mut pact_builder_async = get_pact_builder_async_obj().await;

    let mut result = HashMap::new();
    result.insert(
        "currentResolution".into(),
        ContractMatcher::MatchType("1080p".into()),
    );
    result.insert("success".into(), ContractMatcher::MatchBool(true));
    pact_builder_async
        .synchronous_message_interaction(
            "A request to get the device video resolution",
            |mut i| async move {
                i.contents_from(get_pact!(
                    "org.rdk.DisplaySettings.1.getCurrentResolution",
                    ContractResult { result }
                ))
                .await;
                i.test_name("get_device_video_resolution");
                i
            },
        )
        .await;

    let mock_server = pact_builder_async
        .start_mock_server_async(Some("websockets/transport/websockets"))
        .await;

    let payload = ExtnPayload::Request(ExtnRequest::Device(DeviceRequest::DeviceInfo(
        DeviceInfoRequest::VideoResolution,
    )));
    let msg = get_extn_msg(payload);

    let url = url::Url::parse(mock_server.path("/jsonrpc").as_str()).unwrap();
    let thunder_client = ThunderClientPool::start(url, None, 1).await.unwrap();

    let (s, r) = unbounded();
    let extn_client = get_extn_client(s, r);

    let state: CachedState = CachedState::new(ThunderState::new(extn_client, thunder_client));

    let _ = ThunderDeviceInfoRequestProcessor::process_request(
        state,
        msg,
        DeviceInfoRequest::VideoResolution,
    )
    .await;
}

// #[tokio::test(flavor = "multi_thread")]
// #[cfg_attr(not(feature = "contract_tests"), ignore)]
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
#[cfg_attr(not(feature = "contract_tests"), ignore)]
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

    let payload = ExtnPayload::Request(ExtnRequest::Device(DeviceRequest::DeviceInfo(
        DeviceInfoRequest::GetTimezone,
    )));
    let msg = get_extn_msg(payload);

    let url = url::Url::parse(mock_server.path("/jsonrpc").as_str()).unwrap();
    let thunder_client = ThunderClientPool::start(url, None, 1).await.unwrap();

    let (s, r) = unbounded();
    let extn_client = get_extn_client(s, r);

    let state: CachedState = CachedState::new(ThunderState::new(extn_client, thunder_client));

    let _ = ThunderDeviceInfoRequestProcessor::process_request(
        state,
        msg,
        DeviceInfoRequest::GetTimezone,
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "contract_tests"), ignore)]
async fn test_device_get_available_timezone() {
    let mut pact_builder_async = get_pact_builder_async_obj().await;

    pact_builder_async
        .synchronous_message_interaction("A request to get the device available timezone", |mut i| async move {
            i.contents_from(json!({
                "pact:content-type": "application/json",
                "request": {"jsonrpc": "matching(type, '2.0')", "id": "matching(integer, 0)", "method": "org.rdk.System.1.getTimeZones"},
                "requestMetadata": {
                    "path": "/jsonrpc",
                },
                "response": [{
                    "jsonrpc": "matching(type, '2.0')",
                    "id": "matching(integer, 0)",
                    "result":  {
                        "zoneinfo": {
                            "EST": "matching(datetime, 'EEE MMM d HH:mm:ss yyyy z', 'Thu Nov 5 15:21:17 2020 EST')",
                            "America": {
                                "New_York": "matching(datetime, 'EEE MMM d HH:mm:ss yyyy z', 'Thu Nov 5 15:21:17 2020 EST')",
                                "Los_Angeles": "matching (datetime, 'EEE MMM d HH:mm:ss yyyy z', 'Thu Nov 5 12:21:17 2020 PST')"
                            },
                            "Europe": {
                                "London": "matching(datetime, 'EEE MMM d HH:mm:ss yyyy z', 'Thu Nov 5 14:21:17 2020 CST')"
                            }
                        },
                        "success": "matching(boolean, true)"
                    }
                }]
            })).await;
            i.test_name("get_device_available_timezone");
            i
        }).await;

    let mock_server = pact_builder_async
        .start_mock_server_async(Some("websockets/transport/websockets"))
        .await;

    let payload = ExtnPayload::Request(ExtnRequest::Device(DeviceRequest::DeviceInfo(
        DeviceInfoRequest::GetAvailableTimezones,
    )));
    let msg = get_extn_msg(payload);

    let url = url::Url::parse(mock_server.path("/jsonrpc").as_str()).unwrap();
    let thunder_client = ThunderClientPool::start(url, None, 1).await.unwrap();

    let (s, r) = unbounded();
    let extn_client = get_extn_client(s, r);

    let state: CachedState = CachedState::new(ThunderState::new(extn_client, thunder_client));

    let _ = ThunderDeviceInfoRequestProcessor::process_request(
        state,
        msg,
        DeviceInfoRequest::GetAvailableTimezones,
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "contract_tests"), ignore)]
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

    let payload = ExtnPayload::Request(ExtnRequest::Device(DeviceRequest::DeviceInfo(
        DeviceInfoRequest::VoiceGuidanceEnabled,
    )));
    let msg = get_extn_msg(payload);

    let url = url::Url::parse(mock_server.path("/jsonrpc").as_str()).unwrap();
    let thunder_client = ThunderClientPool::start(url, None, 1).await.unwrap();

    let (s, r) = unbounded();
    let extn_client = get_extn_client(s, r);

    let state: CachedState = CachedState::new(ThunderState::new(extn_client, thunder_client));
    let _ = ThunderDeviceInfoRequestProcessor::process_request(
        state,
        msg,
        DeviceInfoRequest::VoiceGuidanceEnabled,
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "contract_tests"), ignore)]
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

    let payload = ExtnPayload::Request(ExtnRequest::Device(DeviceRequest::DeviceInfo(
        DeviceInfoRequest::VoiceGuidanceSpeed,
    )));
    let msg = get_extn_msg(payload);

    let url = url::Url::parse(mock_server.path("/jsonrpc").as_str()).unwrap();
    let thunder_client = ThunderClientPool::start(url, None, 1).await.unwrap();

    let (s, r) = unbounded();
    let extn_client = get_extn_client(s, r);

    let state: CachedState = CachedState::new(ThunderState::new(extn_client, thunder_client));

    let _ = ThunderDeviceInfoRequestProcessor::process_request(
        state,
        msg,
        DeviceInfoRequest::VoiceGuidanceSpeed,
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "contract_tests"), ignore)]
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

    let payload = ExtnPayload::Request(ExtnRequest::Device(DeviceRequest::DeviceInfo(
        DeviceInfoRequest::SetTimezone("".to_string()),
    )));
    let msg = get_extn_msg(payload);

    let url = url::Url::parse(mock_server.path("/jsonrpc").as_str()).unwrap();
    let thunder_client = ThunderClientPool::start(url, None, 1).await.unwrap();

    let (s, r) = unbounded();
    let extn_client = get_extn_client(s, r);

    let state: CachedState = CachedState::new(ThunderState::new(extn_client, thunder_client));

    let _ = ThunderDeviceInfoRequestProcessor::process_request(
        state,
        msg,
        DeviceInfoRequest::SetTimezone("America/New_York".to_string()),
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "contract_tests"), ignore)]
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

    let payload = ExtnPayload::Request(ExtnRequest::Device(DeviceRequest::DeviceInfo(
        DeviceInfoRequest::SetVoiceGuidanceEnabled(false),
    )));
    let msg = get_extn_msg(payload);

    let url = url::Url::parse(mock_server.path("/jsonrpc").as_str()).unwrap();
    let thunder_client = ThunderClientPool::start(url, None, 1).await.unwrap();

    let (s, r) = unbounded();
    let extn_client = get_extn_client(s, r);

    let state: CachedState = CachedState::new(ThunderState::new(extn_client, thunder_client));

    let _ = ThunderDeviceInfoRequestProcessor::process_request(
        state,
        msg,
        DeviceInfoRequest::SetVoiceGuidanceEnabled(false),
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "contract_tests"), ignore)]
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

    let payload = ExtnPayload::Request(ExtnRequest::Device(DeviceRequest::DeviceInfo(
        DeviceInfoRequest::SetVoiceGuidanceSpeed(50.0),
    )));
    let msg = get_extn_msg(payload);

    let url = url::Url::parse(mock_server.path("/jsonrpc").as_str()).unwrap();
    let thunder_client = ThunderClientPool::start(url, None, 1).await.unwrap();

    let (s, r) = unbounded();
    let extn_client = get_extn_client(s, r);

    let state: CachedState = CachedState::new(ThunderState::new(extn_client, thunder_client));

    let _ = ThunderDeviceInfoRequestProcessor::process_request(
        state,
        msg,
        DeviceInfoRequest::SetVoiceGuidanceSpeed(50.0),
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "contract_tests"), ignore)]
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

    let timeout = 50;
    let time_out = OnInternetConnectedRequest { timeout };
    let payload = ExtnPayload::Request(ExtnRequest::Device(DeviceRequest::DeviceInfo(
        DeviceInfoRequest::OnInternetConnected(time_out.clone()),
    )));
    let msg = get_extn_msg(payload);

    let url = url::Url::parse(mock_server.path("/jsonrpc").as_str()).unwrap();
    let thunder_client = ThunderClientPool::start(url, None, 1).await.unwrap();

    let (s, r) = unbounded();
    let extn_client = get_extn_client(s, r);

    let state: CachedState = CachedState::new(ThunderState::new(extn_client, thunder_client));

    let _ = ThunderDeviceInfoRequestProcessor::process_request(
        state,
        msg,
        DeviceInfoRequest::OnInternetConnected(time_out.clone()),
    )
    .await;
}
