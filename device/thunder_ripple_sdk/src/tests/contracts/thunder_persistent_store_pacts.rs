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

use crate::ripple_sdk::{serde_json::json, tokio};
use crate::tests::contracts::contract_utils::*;
use crate::tests::contracts::thunder_persistent_store_pacts::chrono::Utc;
use crate::{get_pact_with_params, send_thunder_call_message};
use pact_consumer::mock_server::StartMockServerAsync;
use pact_consumer::prelude::*;
use ripple_sdk::chrono;
use rstest::rstest;
use std::collections::HashMap;

use futures_util::{SinkExt, StreamExt};
use tokio::time::{timeout, Duration};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::protocol::Message;

#[rstest(with_scope, case(true), case(false))]
#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "websocket_contract_tests"), ignore)]
async fn test_device_set_persistent_value(with_scope: bool) {
    let mut pact_builder_async = PactBuilder::new_v4("ripple", "rdk_service")
        .using_plugin("websockets", None)
        .await;

    let scope = if with_scope {
        "matching(type, 'device')"
    } else {
        "matching(type, '')"
    };

    let mut request_params = json!({
        "namespace": "matching(type, 'testNamespace')",
        "key": "matching(type, 'testKey')",
        "value": {
            "update_time": "matching(type, '2023-07-20T14:20:06.477058+00:00')",
            "value": "matching(type, 'testValue1')"
        }
    });

    if with_scope {
        request_params["scope"] = json!(scope);
    }

    pact_builder_async
        .synchronous_message_interaction(
            "A request to set the persistent value in device",
            |mut i| async move {
                i.contents_from(json!({
                    "pact:content-type": "application/json",
                    "request": {"jsonrpc": "matching(type, '2.0')",
                        "id": "matching(integer, 0)",
                        "method": "org.rdk.PersistentStore.1.setValue",
                        "params": request_params,
                    },
                    "requestMetadata": {
                        "path": "/jsonrpc"
                    },
                    "response": [{
                        "jsonrpc": "matching(type, '2.0')",
                        "id": "matching(integer, 0)",
                        "result": {
                            "success": "matching(boolean, true)"
                        }
                    }]
                }))
                .await;
                i.test_name("set_device_stored_persistent_value");

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
            "method": "org.rdk.PersistentStore.1.setValue",
            "params": json!({
                "namespace": "testNameSpace",
                "key": "testKey",
                "value": {
                    "update_time": Utc::now().to_rfc3339(),
                    "value": "testValue1"
                },
                "scope": "device"
            })
        })
    )
    .await;
}

#[rstest(with_scope, case(true), case(false))]
#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "websocket_contract_tests"), ignore)]
async fn test_device_get_persistent_value(with_scope: bool) {
    // Define Pact request and response - Start
    let mut pact_builder_async = PactBuilder::new_v4("ripple", "rdk_service")
        .using_plugin("websockets", None)
        .await;

    let mut result = HashMap::new();
    result.insert(
        "value".into(),
        ContractMatcher::MatchType("testvalue1".into()),
    );
    result.insert("success".into(), ContractMatcher::MatchBool(true));

    let mut params = HashMap::new();
    params.insert(
        "namespace".into(),
        ContractMatcher::MatchType("testNamespace".into()),
    );
    params.insert("key".into(), ContractMatcher::MatchType("testKey".into()));
    if with_scope {
        params.insert("scope".into(), ContractMatcher::MatchType("device".into()));
    }

    let given_statement = format!(
        "\"key:{}\", \"namespace:{}\", \"scope:{}\" is retrieved from persistentStore with value \"{}\"",
        "testKey", "testNamespace", if with_scope { "device" } else { "" }, "testValue1"
    );

    pact_builder_async
        .synchronous_message_interaction(
            "A request to get the persistent stored value",
            |mut i| async move {
                i.given(given_statement);
                i.contents_from(get_pact_with_params!(
                    "org.rdk.PersistentStore.1.getValue",
                    ContractResult { result },
                    ContractParams { params }
                ))
                .await;
                i.test_name("get_device_stored_persistent_value");
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
            "method": "org.rdk.PersistentStore.1.getValue",
            "params": json!({
                "namespace": "testNamespace",
                "key": "testKey",
                "scope": "device"
            })
        })
    )
    .await;
}

#[rstest(with_scope, case(true), case(false))]
#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "websocket_contract_tests"), ignore)]
async fn test_device_delete_persistent_value_by_key(with_scope: bool) {
    // Define Pact request and response - Start
    let mut pact_builder_async = PactBuilder::new_v4("ripple", "rdk_service")
        .using_plugin("websockets", None)
        .await;

    let mut result = HashMap::new();
    result.insert("success".into(), ContractMatcher::MatchBool(true));

    let mut params = HashMap::new();
    params.insert(
        "namespace".into(),
        ContractMatcher::MatchType("testNamespace".into()),
    );
    params.insert("key".into(), ContractMatcher::MatchType("testKey".into()));
    if with_scope {
        params.insert("scope".into(), ContractMatcher::MatchType("device".into()));
    }

    let given_statement = format!(
        "\"key:{}\", \"namespace:{}\", \"scope:{}\" is deleted from persistentStore",
        "testKey",
        "testNamespace",
        if with_scope { "device" } else { "" }
    );

    pact_builder_async
        .synchronous_message_interaction(
            "A request to delete the persistent stored value",
            |mut i| async move {
                i.given(given_statement);
                i.contents_from(get_pact_with_params!(
                    "org.rdk.PersistentStore.1.deleteKey",
                    ContractResult { result },
                    ContractParams { params }
                ))
                .await;
                i.test_name("delete_device_stored_persistent_value");
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
            "method": "org.rdk.PersistentStore.1.deleteKey",
            "params": json!({
                "namespace": "testNamespace",
                "key": "testKey",
                "scope": "device"
            })
        })
    )
    .await;
}
