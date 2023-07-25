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

use crate::get_pact_with_params;
use crate::processors::thunder_persistent_store::ThunderStorageRequestProcessor;
use crate::tests::contracts::contract_utils::*;
use crate::tests::contracts::thunder_persistent_store_pacts::chrono::Utc;
use crate::{
    client::thunder_client_pool::ThunderClientPool,
    ripple_sdk::{
        api::device::{
            device_peristence::{
                DevicePersistenceRequest, GetStorageProperty, SetStorageProperty, StorageData,
            },
            device_request::DeviceRequest,
        },
        crossbeam::channel::unbounded,
        extn::{
            client::extn_processor::ExtnRequestProcessor,
            extn_client_message::{ExtnPayload, ExtnRequest},
        },
        serde_json::json,
    },
    thunder_state::ThunderState,
};
use pact_consumer::mock_server::StartMockServerAsync;
use pact_consumer::prelude::*;
use ripple_sdk::chrono;
use std::collections::HashMap;

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "contract_tests"), ignore)]
async fn test_device_set_persistent_value() {
    let mut pact_builder_async = PactBuilder::new_v4("ripple", "rdk_service")
        .using_plugin("websockets", None)
        .await;

    pact_builder_async
        .synchronous_message_interaction("A request to set the persistent value in device", |mut i| async move {
            i.contents_from(json!({
                "pact:content-type": "application/json",
                "request": {"jsonrpc": "matching(type, '2.0')", 
                    "id": "matching(integer, 0)", 
                    "method": "org.rdk.PersistentStore.1.setValue", 
                    "params": {"namespace": "matching(type, 'testNamespace')", 
                        "key": "matching(type, 'testKey')", 
                        "value": {"update_time":r"matching(type, '2023-07-20T14:20:06.477058+00:00')","value":"matching(type, 'testValue1')"}
                    }
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
            })).await;
            i.test_name("set_device_stored_persistent_value");

            i
        }).await;

    let mock_server = pact_builder_async
        .start_mock_server_async(Some("websockets/transport/websockets"))
        .await;

    let namespace = "testNameSpace";
    let key = "testKey";
    let data = StorageData {
        value: "testValue1".into(),
        update_time: Utc::now().to_rfc3339(),
    };
    let set_params = SetStorageProperty {
        namespace: namespace.to_string(),
        key: key.to_string(),
        data: data,
    };
    let payload = ExtnPayload::Request(ExtnRequest::Device(DeviceRequest::Storage(
        DevicePersistenceRequest::Set(set_params.clone()),
    )));
    let msg = get_extn_msg(payload);

    let url = url::Url::parse(mock_server.path("/jsonrpc").as_str()).unwrap();
    let thunder_client = ThunderClientPool::start(url, None, 1).await.unwrap();

    let (s, r) = unbounded();
    let extn_client = get_extn_client(s, r);

    let state: ThunderState = ThunderState::new(extn_client, thunder_client);

    let _ = ThunderStorageRequestProcessor::process_request(
        state,
        msg,
        DevicePersistenceRequest::Set(set_params.clone()),
    )
    .await;
}


#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "contract_tests"), ignore)]
async fn test_device_get_persistent_value() {
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

    pact_builder_async
        .synchronous_message_interaction(
            "A request to get the persistent stored value",
            |mut i| async move {
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

    let namespace = "testNamespace";
    let key = "testKey";
    let get_params = GetStorageProperty {
        namespace: namespace.to_string(),
        key: key.to_string(),
    };
    let payload = ExtnPayload::Request(ExtnRequest::Device(DeviceRequest::Storage(
        DevicePersistenceRequest::Get(get_params.clone()),
    )));
    let msg = get_extn_msg(payload);

    let url = url::Url::parse(mock_server.path("/jsonrpc").as_str()).unwrap();
    let thunder_client = ThunderClientPool::start(url, None, 1).await.unwrap();

    let (s, r) = unbounded();
    let extn_client = get_extn_client(s, r);

    let state: ThunderState = ThunderState::new(extn_client, thunder_client);

    let _ = ThunderStorageRequestProcessor::process_request(
        state,
        msg,
        DevicePersistenceRequest::Get(get_params.clone()),
    )
    .await;
}