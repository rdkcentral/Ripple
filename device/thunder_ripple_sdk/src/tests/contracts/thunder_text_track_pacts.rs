// Copyright 2025 Comcast Cable Communications Management, LLC
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
use crate::ripple_sdk::{serde_json::json, tokio};
#[allow(dead_code, unused_imports)]
use crate::tests::contracts::contract_utils::*;
#[allow(dead_code, unused_imports)]
use crate::{get_pact_with_params, send_thunder_call_message};
#[allow(dead_code, unused_imports)]
use pact_consumer::mock_server::StartMockServerAsync;
#[allow(dead_code, unused_imports)]
use pact_consumer::prelude::*;
#[allow(dead_code, unused_imports)]
use ripple_sdk::chrono;
#[allow(dead_code, unused_imports)]
use rstest::rstest;
#[allow(dead_code, unused_imports)]
use std::collections::HashMap;

#[allow(dead_code, unused_imports)]
use futures_util::{SinkExt, StreamExt};
#[allow(dead_code, unused_imports)]
use tokio::time::{timeout, Duration};
#[allow(dead_code, unused_imports)]
use tokio_tungstenite::connect_async;
#[allow(dead_code, unused_imports)]
use tokio_tungstenite::tungstenite::protocol::Message;

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "websocket_contract_tests"), ignore)]
async fn test_text_track_get_font_family() {
    let mut pact_builder_async = get_pact_builder_async_obj().await;

    pact_builder_async
    .synchronous_message_interaction("A request to get the font family", |mut i| async move {
        i.contents_from(json!({
            "pact:content-type": "application/json",
            "request": {"jsonrpc": "matching(type, '2.0')", "id": "matching(integer, 1)", "method": "org.rdk.TextTrack.getFontFamily"},
            "requestMetadata": {
                "path": "/jsonrpc"
            },
            "response": [{
                "jsonrpc": "matching(type, '2.0')",
                "id": "matching(integer, 1)",
                "result": "matching(type, 'Arial')"
            }]
        })).await;
        i.test_name("get_font_family");

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
            "id": 1,
            "method": "org.rdk.TextTrack.getFontFamily"
        })
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "websocket_contract_tests"), ignore)]
async fn test_text_track_set_font_family() {
    let mut pact_builder_async = get_pact_builder_async_obj().await;

    pact_builder_async
        .synchronous_message_interaction("A request to set the font family", |mut i| async move {
            i.contents_from(json!({
                "pact:content-type": "application/json",
                "request": {
                    "jsonrpc": "matching(type, '2.0')",
                    "id": "matching(integer, 2)",
                    "method": "org.rdk.TextTrack.setFontFamily",
                    "params": {
                        "fontFamily": "matching(type, 'Verdana')"
                    }
                },
                "requestMetadata": {
                    "path": "/jsonrpc"
                },
                "response": [{
                    "jsonrpc": "matching(type, '2.0')",
                    "id": "matching(integer, 2)",
                    "result": "matching(type, 'SUCCESS')"
                }]
            }))
            .await;
            i.test_name("set_font_family");

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
            "id": 2,
            "method": "org.rdk.TextTrack.setFontFamily",
            "params": {
                "fontFamily": "Verdana"
            }
        })
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "websocket_contract_tests"), ignore)]
async fn test_text_track_get_font_size() {
    let mut pact_builder_async = get_pact_builder_async_obj().await;

    pact_builder_async
        .synchronous_message_interaction("A request to get the font size", |mut i| async move {
            i.contents_from(json!({
                "pact:content-type": "application/json",
                "request": {"jsonrpc": "matching(type, '2.0')", "id": "matching(integer, 3)", "method": "org.rdk.TextTrack.getFontSize"},
                "requestMetadata": {
                    "path": "/jsonrpc"
                },
                "response": [{
                    "jsonrpc": "matching(type, '2.0')",
                    "id": "matching(integer, 3)",
                    "result": "matching(type, '16px')"
                }]
            })).await;
            i.test_name("get_font_size");

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
            "id": 3,
            "method": "org.rdk.TextTrack.getFontSize"
        })
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "websocket_contract_tests"), ignore)]
async fn test_text_track_set_font_size() {
    let mut pact_builder_async = get_pact_builder_async_obj().await;

    pact_builder_async
        .synchronous_message_interaction("A request to set the font size", |mut i| async move {
            i.contents_from(json!({
                "pact:content-type": "application/json",
                "request": {
                    "jsonrpc": "matching(type, '2.0')",
                    "id": "matching(integer, 4)",
                    "method": "org.rdk.TextTrack.setFontSize",
                    "params": {
                        "fontSize": "matching(type, '18px')"
                    }
                },
                "requestMetadata": {
                    "path": "/jsonrpc"
                },
                "response": [{
                    "jsonrpc": "matching(type, '2.0')",
                    "id": "matching(integer, 4)",
                    "result": "matching(type, 'SUCCESS')"
                }]
            }))
            .await;
            i.test_name("set_font_size");

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
            "id": 4,
            "method": "org.rdk.TextTrack.setFontSize",
            "params": {
                "fontSize": "18px"
            }
        })
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "websocket_contract_tests"), ignore)]
async fn test_text_track_get_font_color() {
    let mut pact_builder_async = get_pact_builder_async_obj().await;

    pact_builder_async
        .synchronous_message_interaction("A request to get the font color", |mut i| async move {
            i.contents_from(json!({
                "pact:content-type": "application/json",
                "request": {"jsonrpc": "matching(type, '2.0')", "id": "matching(integer, 5)", "method": "org.rdk.TextTrack.getFontColor"},
                "requestMetadata": {
                    "path": "/jsonrpc"
                },
                "response": [{
                    "jsonrpc": "matching(type, '2.0')",
                    "id": "matching(integer, 5)",
                    "result": "matching(type, '#FFFFFF')"
                }]
            })).await;
            i.test_name("get_font_color");

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
            "id": 5,
            "method": "org.rdk.TextTrack.getFontColor"
        })
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "websocket_contract_tests"), ignore)]
async fn test_text_track_set_font_color() {
    let mut pact_builder_async = get_pact_builder_async_obj().await;

    pact_builder_async
        .synchronous_message_interaction("A request to set the font color", |mut i| async move {
            i.contents_from(json!({
                "pact:content-type": "application/json",
                "request": {
                    "jsonrpc": "matching(type, '2.0')",
                    "id": "matching(integer, 6)",
                    "method": "org.rdk.TextTrack.setFontColor",
                    "params": {
                        "fontColor": "matching(type, '#FF0000')"
                    }
                },
                "requestMetadata": {
                    "path": "/jsonrpc"
                },
                "response": [{
                    "jsonrpc": "matching(type, '2.0')",
                    "id": "matching(integer, 6)",
                    "result": "matching(type, 'SUCCESS')"
                }]
            }))
            .await;
            i.test_name("set_font_color");

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
            "id": 6,
            "method": "org.rdk.TextTrack.setFontColor",
            "params": {
                "fontColor": "#FF0000"
            }
        })
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "websocket_contract_tests"), ignore)]
async fn test_text_track_get_font_edge() {
    let mut pact_builder_async = get_pact_builder_async_obj().await;

    pact_builder_async
        .synchronous_message_interaction("A request to get the font edge", |mut i| async move {
            i.contents_from(json!({
                "pact:content-type": "application/json",
                "request": {"jsonrpc": "matching(type, '2.0')", "id": "matching(integer, 7)", "method": "org.rdk.TextTrack.getFontEdge"},
                "requestMetadata": {
                    "path": "/jsonrpc"
                },
                "response": [{
                    "jsonrpc": "matching(type, '2.0')",
                    "id": "matching(integer, 7)",
                    "result": "matching(type, 'none')"
                }]
            })).await;
            i.test_name("get_font_edge");

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
            "id": 7,
            "method": "org.rdk.TextTrack.getFontEdge"
        })
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "websocket_contract_tests"), ignore)]
async fn test_text_track_set_font_edge() {
    let mut pact_builder_async = get_pact_builder_async_obj().await;

    pact_builder_async
        .synchronous_message_interaction("A request to set the font edge", |mut i| async move {
            i.contents_from(json!({
                "pact:content-type": "application/json",
                "request": {
                    "jsonrpc": "matching(type, '2.0')",
                    "id": "matching(integer, 8)",
                    "method": "org.rdk.TextTrack.setFontEdge",
                    "params": {
                        "fontEdge": "matching(type, 'raised')"
                    }
                },
                "requestMetadata": {
                    "path": "/jsonrpc"
                },
                "response": [{
                    "jsonrpc": "matching(type, '2.0')",
                    "id": "matching(integer, 8)",
                    "result": "matching(type, 'SUCCESS')"
                }]
            }))
            .await;
            i.test_name("set_font_edge");

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
            "id": 8,
            "method": "org.rdk.TextTrack.setFontEdge",
            "params": {
                "fontEdge": "raised"
            }
        })
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "websocket_contract_tests"), ignore)]
async fn test_text_track_get_font_edge_color() {
    let mut pact_builder_async = get_pact_builder_async_obj().await;

    pact_builder_async
        .synchronous_message_interaction("A request to get the font edge color", |mut i| async move {
            i.contents_from(json!({
                "pact:content-type": "application/json",
                "request": {"jsonrpc": "matching(type, '2.0')", "id": "matching(integer, 9)", "method": "org.rdk.TextTrack.getFontEdgeColor"},
                "requestMetadata": {
                    "path": "/jsonrpc"
                },
                "response": [{
                    "jsonrpc": "matching(type, '2.0')",
                    "id": "matching(integer, 9)",
                    "result": "matching(type, '#000000')"
                }]
            })).await;
            i.test_name("get_font_edge_color");

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
            "id": 9,
            "method": "org.rdk.TextTrack.getFontEdgeColor"
        })
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "websocket_contract_tests"), ignore)]
async fn test_text_track_set_font_edge_color() {
    let mut pact_builder_async = get_pact_builder_async_obj().await;

    pact_builder_async
        .synchronous_message_interaction(
            "A request to set the font edge color",
            |mut i| async move {
                i.contents_from(json!({
                    "pact:content-type": "application/json",
                    "request": {
                        "jsonrpc": "matching(type, '2.0')",
                        "id": "matching(integer, 10)",
                        "method": "org.rdk.TextTrack.setFontEdgeColor",
                        "params": {
                            "fontEdgeColor": "matching(type, '#FF00FF')"
                        }
                    },
                    "requestMetadata": {
                        "path": "/jsonrpc"
                    },
                    "response": [{
                        "jsonrpc": "matching(type, '2.0')",
                        "id": "matching(integer, 10)",
                        "result": "matching(type, 'SUCCESS')"
                    }]
                }))
                .await;
                i.test_name("set_font_edge_color");

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
            "id": 10,
            "method": "org.rdk.TextTrack.setFontEdgeColor",
            "params": {
                "fontEdgeColor": "#FF00FF"
            }
        })
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "websocket_contract_tests"), ignore)]
async fn test_text_track_get_font_opacity() {
    let mut pact_builder_async = get_pact_builder_async_obj().await;

    pact_builder_async
        .synchronous_message_interaction("A request to get the font opacity", |mut i| async move {
            i.contents_from(json!({
                "pact:content-type": "application/json",
                "request": {"jsonrpc": "matching(type, '2.0')", "id": "matching(integer, 11)", "method": "org.rdk.TextTrack.getFontOpacity"},
                "requestMetadata": {
                    "path": "/jsonrpc"
                },
                "response": [{
                    "jsonrpc": "matching(type, '2.0')",
                    "id": "matching(integer, 11)",
                    "result": "matching(type, '0.8')"
                }]
            })).await;
            i.test_name("get_font_opacity");

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
            "id": 11,
            "method": "org.rdk.TextTrack.getFontOpacity"
        })
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "websocket_contract_tests"), ignore)]
async fn test_text_track_set_font_opacity() {
    let mut pact_builder_async = get_pact_builder_async_obj().await;

    pact_builder_async
        .synchronous_message_interaction("A request to set the font opacity", |mut i| async move {
            i.contents_from(json!({
                "pact:content-type": "application/json",
                "request": {
                    "jsonrpc": "matching(type, '2.0')",
                    "id": "matching(integer, 12)",
                    "method": "org.rdk.TextTrack.setFontOpacity",
                    "params": {
                        "fontOpacity": "matching(type, '0.9')"
                    }
                },
                "requestMetadata": {
                    "path": "/jsonrpc"
                },
                "response": [{
                    "jsonrpc": "matching(type, '2.0')",
                    "id": "matching(integer, 12)",
                    "result": "matching(type, 'SUCCESS')"
                }]
            }))
            .await;
            i.test_name("set_font_opacity");

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
            "id": 12,
            "method": "org.rdk.TextTrack.setFontOpacity",
            "params": {
                "fontOpacity": "0.9"
            }
        })
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "websocket_contract_tests"), ignore)]
async fn test_text_track_get_background_color() {
    let mut pact_builder_async = get_pact_builder_async_obj().await;

    pact_builder_async
        .synchronous_message_interaction("A request to get the background color", |mut i| async move {
            i.contents_from(json!({
                "pact:content-type": "application/json",
                "request": {"jsonrpc": "matching(type, '2.0')", "id": "matching(integer, 13)", "method": "org.rdk.TextTrack.getBackgroundColor"},
                "requestMetadata": {
                    "path": "/jsonrpc"
                },
                "response": [{
                    "jsonrpc": "matching(type, '2.0')",
                    "id": "matching(integer, 13)",
                    "result": "matching(type, '#000000')"
                }]
            })).await;
            i.test_name("get_background_color");

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
            "id": 13,
            "method": "org.rdk.TextTrack.getBackgroundColor"
        })
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "websocket_contract_tests"), ignore)]
async fn test_text_track_set_background_color() {
    let mut pact_builder_async = get_pact_builder_async_obj().await;

    pact_builder_async
        .synchronous_message_interaction(
            "A request to set the background color",
            |mut i| async move {
                i.contents_from(json!({
                    "pact:content-type": "application/json",
                    "request": {
                        "jsonrpc": "matching(type, '2.0')",
                        "id": "matching(integer, 14)",
                        "method": "org.rdk.TextTrack.setBackgroundColor",
                        "params": {
                            "backgroundColor": "matching(type, '#FFFFFF')"
                        }
                    },
                    "requestMetadata": {
                        "path": "/jsonrpc"
                    },
                    "response": [{
                        "jsonrpc": "matching(type, '2.0')",
                        "id": "matching(integer, 14)",
                        "result": "matching(type, 'SUCCESS')"
                    }]
                }))
                .await;
                i.test_name("set_background_color");

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
            "id": 14,
            "method": "org.rdk.TextTrack.setBackgroundColor",
            "params": {
                "backgroundColor": "#FFFFFF"
            }
        })
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "websocket_contract_tests"), ignore)]
async fn test_text_track_get_background_opacity() {
    let mut pact_builder_async = get_pact_builder_async_obj().await;

    pact_builder_async
        .synchronous_message_interaction("A request to get the background opacity", |mut i| async move {
            i.contents_from(json!({
                "pact:content-type": "application/json",
                "request": {"jsonrpc": "matching(type, '2.0')", "id": "matching(integer, 15)", "method": "org.rdk.TextTrack.getBackgroundOpacity"},
                "requestMetadata": {
                    "path": "/jsonrpc"
                },
                "response": [{
                    "jsonrpc": "matching(type, '2.0')",
                    "id": "matching(integer, 15)",
                    "result": "matching(type, '0.7')"
                }]
            })).await;
            i.test_name("get_background_opacity");

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
            "id": 15,
            "method": "org.rdk.TextTrack.getBackgroundOpacity"
        })
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "websocket_contract_tests"), ignore)]
async fn test_text_track_set_background_opacity() {
    let mut pact_builder_async = get_pact_builder_async_obj().await;

    pact_builder_async
        .synchronous_message_interaction(
            "A request to set the background opacity",
            |mut i| async move {
                i.contents_from(json!({
                    "pact:content-type": "application/json",
                    "request": {
                        "jsonrpc": "matching(type, '2.0')",
                        "id": "matching(integer, 16)",
                        "method": "org.rdk.TextTrack.setBackgroundOpacity",
                        "params": {
                            "backgroundOpacity": "matching(type, '0.85')"
                        }
                    },
                    "requestMetadata": {
                        "path": "/jsonrpc"
                    },
                    "response": [{
                        "jsonrpc": "matching(type, '2.0')",
                        "id": "matching(integer, 16)",
                        "result": "matching(type, 'SUCCESS')"
                    }]
                }))
                .await;
                i.test_name("set_background_opacity");

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
            "id": 16,
            "method": "org.rdk.TextTrack.setBackgroundOpacity",
            "params": {
                "backgroundOpacity": "0.85"
            }
        })
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "websocket_contract_tests"), ignore)]
async fn test_text_track_get_window_color() {
    let mut pact_builder_async = get_pact_builder_async_obj().await;

    pact_builder_async
        .synchronous_message_interaction("A request to get the window color", |mut i| async move {
            i.contents_from(json!({
                "pact:content-type": "application/json",
                "request": {"jsonrpc": "matching(type, '2.0')", "id": "matching(integer, 17)", "method": "org.rdk.TextTrack.getWindowColor"},
                "requestMetadata": {
                    "path": "/jsonrpc"
                },
                "response": [{
                    "jsonrpc": "matching(type, '2.0')",
                    "id": "matching(integer, 17)",
                    "result": "matching(type, '#CCCCCC')"
                }]
            })).await;
            i.test_name("get_window_color");

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
            "id": 17,
            "method": "org.rdk.TextTrack.getWindowColor"
        })
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "websocket_contract_tests"), ignore)]
async fn test_text_track_set_window_color() {
    let mut pact_builder_async = get_pact_builder_async_obj().await;

    pact_builder_async
        .synchronous_message_interaction("A request to set the window color", |mut i| async move {
            i.contents_from(json!({
                "pact:content-type": "application/json",
                "request": {
                    "jsonrpc": "matching(type, '2.0')",
                    "id": "matching(integer, 18)",
                    "method": "org.rdk.TextTrack.setWindowColor",
                    "params": {
                        "windowColor": "matching(type, '#00FF00')"
                    }
                },
                "requestMetadata": {
                    "path": "/jsonrpc"
                },
                "response": [{
                    "jsonrpc": "matching(type, '2.0')",
                    "id": "matching(integer, 18)",
                    "result": "matching(type, 'SUCCESS')"
                }]
            }))
            .await;
            i.test_name("set_window_color");

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
            "id": 18,
            "method": "org.rdk.TextTrack.setWindowColor",
            "params": {
                "windowColor": "#00FF00"
            }
        })
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "websocket_contract_tests"), ignore)]
async fn test_text_track_get_window_opacity() {
    let mut pact_builder_async = get_pact_builder_async_obj().await;

    pact_builder_async
        .synchronous_message_interaction("A request to get the window opacity", |mut i| async move {
            i.contents_from(json!({
                "pact:content-type": "application/json",
                "request": {"jsonrpc": "matching(type, '2.0')", "id": "matching(integer, 19)", "method": "org.rdk.TextTrack.getWindowOpacity"},
                "requestMetadata": {
                    "path": "/jsonrpc"
                },
                "response": [{
                    "jsonrpc": "matching(type, '2.0')",
                    "id": "matching(integer, 19)",
                    "result": "matching(type, '0.6')"
                }]
            })).await;
            i.test_name("get_window_opacity");

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
            "id": 19,
            "method": "org.rdk.TextTrack.getWindowOpacity"
        })
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "websocket_contract_tests"), ignore)]
async fn test_text_track_set_window_opacity() {
    let mut pact_builder_async = get_pact_builder_async_obj().await;

    pact_builder_async
        .synchronous_message_interaction(
            "A request to set the window opacity",
            |mut i| async move {
                i.contents_from(json!({
                    "pact:content-type": "application/json",
                    "request": {
                        "jsonrpc": "matching(type, '2.0')",
                        "id": "matching(integer, 20)",
                        "method": "org.rdk.TextTrack.setWindowOpacity",
                        "params": {
                            "windowOpacity": "matching(type, '0.75')"
                        }
                    },
                    "requestMetadata": {
                        "path": "/jsonrpc"
                    },
                    "response": [{
                        "jsonrpc": "matching(type, '2.0')",
                        "id": "matching(integer, 20)",
                        "result": "matching(type, 'SUCCESS')"
                    }]
                }))
                .await;
                i.test_name("set_window_opacity");

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
            "id": 20,
            "method": "org.rdk.TextTrack.setWindowOpacity",
            "params": {
                "windowOpacity": "0.75"
            }
        })
    )
    .await;
}
