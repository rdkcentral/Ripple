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

use crate::{
    client::thunder_plugin::ThunderPlugin,
    client::thunder_client_pool::ThunderClientPool,
    ripple_sdk::{
        api::device::{
            device_operator::{DeviceCallRequest, DeviceChannelParams, DeviceOperator},
            device_peristence::{
                DevicePersistenceRequest, GetStorageProperty, SetStorageProperty, StorageData,
            },
            device_request::DeviceRequest,
        },
        async_trait::async_trait,
        extn::{
            client::extn_client::ExtnClient,
            client::extn_processor::{
                DefaultExtnStreamer, ExtnRequestProcessor, ExtnStreamProcessor, ExtnStreamer,
            },
            extn_id::{ExtnClassId, ExtnId},
            extn_client_message::{ExtnMessage, ExtnRequest, ExtnResponse, ExtnPayload},
            client::extn_sender::ExtnSender,
        },
        log::{debug, error, info},
        serde_json::{self, json, Value},
        tokio::sync::mpsc,
        utils::error::RippleError,
        framework::{ripple_contract::RippleContract, RippleResponse},
        crossbeam::channel::unbounded,
        tokio::sync::oneshot,
        tokio::sync::oneshot::error::RecvError,
        tokio::sync::oneshot::Sender,
    },
    thunder_state::ThunderState,
};
use crate::processors::thunder_persistent_store::ThunderStorageRequestProcessor;
use crate::tests::contracts::thunder_persistent_store_pacts::chrono::Utc;
use serde::{Deserialize, Serialize};
use expectest::expect;
use expectest::prelude::*;
use pact_consumer::mock_server::StartMockServerAsync;
use pact_consumer::prelude::*;
use pact_consumer::*;
use ripple_sdk::chrono;
use url::Url;

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "contract_tests"), ignore)]
async fn test_device_get_persistent_value() {
    // Define Pact request and response - Start
    let mut pact_builder_async = PactBuilder::new_v4("ripple", "rdk_service")
        .using_plugin("websockets", None)
        .await;

    pact_builder_async
        .synchronous_message_interaction("A request to get the persistent stored value", |mut i| async move {
            i.contents_from(json!({
                "pact:content-type": "application/json",
                "request": {"jsonrpc": "matching(type, '2.0')", "id": "matching(integer, 0)", "method": "org.rdk.PersistentStore.1.getValue", "params": {"namespace": "matching(type, 'testnamespace')", "key": "matching(type, 'testkey')"}},
                "requestMetadata": {
                    "path": "/jsonrpc"
                },
                "response": [{
                    "jsonrpc": "matching(type, '2.0')",
                    "id": "matching(integer, 0)",
                    "result": {
                        "value": "matching(type, 'testvalue1')",
                        "success": "matching(boolean, true)"
                    }
                }]
            })).await;
            i.test_name("get_device_stored_persistent_value");

            i
        }).await;
    // Define Pact request and response - End

    let mock_server = pact_builder_async
        .start_mock_server_async(Some("websockets/transport/websockets"))
        .await;

    // Creating a mock extn message needed for calling the mac address method
    let namespace = "testNameSpace1";
    let key = "testKey1";
    let get_params = GetStorageProperty {namespace:namespace.to_string(), key:key.to_string()};
    let msg: ExtnMessage = ExtnMessage {
        callback: None,
        id: "SomeId".into(),
        payload: ExtnPayload::Request(ExtnRequest::Device(DeviceRequest::Storage(
            DevicePersistenceRequest::Get(get_params.clone()),
        ))),
        requestor: ExtnId::new_channel(ExtnClassId::Device, "pact".into()),
        target: RippleContract::DevicePersistence,
    };

    let url = url::Url::parse(mock_server.path("/jsonrpc").as_str()).unwrap();
    let thunder_client = ThunderClientPool::start(url, None, 1).await.unwrap();

    // creating a cross beam channel used by extn client
    let (s, r) = unbounded();

    // Creating extn client which will send back the data to the receiver(instead of callback)
    let extn_client = ExtnClient::new(
        r.clone(),
        ExtnSender::new(
            s,
            ExtnId::new_channel(ExtnClassId::Device, "pact".into()),
            Vec::new(),
            Vec::new(),
        ),
    );

    // Create  Thunderstate which contains the extn_client for callback and thunder client for making thunder requests
    let state: ThunderState = ThunderState::new(extn_client, thunder_client);

    // Make call to method
    let _ = ThunderStorageRequestProcessor::process_request(state, msg, DevicePersistenceRequest::Get(get_params.clone())).await;
    if let Ok(value) = r.recv() {
        if let Ok(message) = value.try_into() {
            let extn_message: ExtnMessage = message;
            if let Some(ExtnResponse::String(v)) = extn_message.payload.extract() {
                println!("Thunder Response from Plugin ::: {:?}", v);
            }
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "contract_tests"), ignore)]
async fn test_device_set_persistent_value() {
    // Define Pact request and response - Start
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
                        "value": {"update_time":r"matching(datetime, 'YYYY-MM-DDThh:mm:ss.SSSXXXZ', '2023-07-20T14:20:06.477058+00:00')","value":"matching(type, 'testValue1')"}
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
    // Define Pact request and response - End

    let mock_server = pact_builder_async
        .start_mock_server_async(Some("websockets/transport/websockets"))
        .await;

    // Creating a mock extn message needed for calling the mac address method
    let namespace = "testNameSpace1";
    let key = "testKey1";
    let data = StorageData{value: "testValue1".into(), update_time: Utc::now().to_rfc3339()};
    let set_params = SetStorageProperty {namespace:namespace.to_string(), key:key.to_string(), data:data};
    let msg: ExtnMessage = ExtnMessage {
        callback: None,
        id: "SomeId".into(),
        payload: ExtnPayload::Request(ExtnRequest::Device(DeviceRequest::Storage(
            DevicePersistenceRequest::Set(set_params.clone()),
        ))),
        requestor: ExtnId::new_channel(ExtnClassId::Device, "pact".into()),
        target: RippleContract::DevicePersistence,
    };

    let url = url::Url::parse(mock_server.path("/jsonrpc").as_str()).unwrap();
    let thunder_client = ThunderClientPool::start(url, None, 1).await.unwrap();

    // creating a cross beam channel used by extn client
    let (s, r) = unbounded();

    // Creating extn client which will send back the data to the receiver(instead of callback)
    let extn_client = ExtnClient::new(
        r.clone(),
        ExtnSender::new(
            s,
            ExtnId::new_channel(ExtnClassId::Device, "pact".into()),
            Vec::new(),
            Vec::new(),
        ),
    );

    // Create  Thunderstate which contains the extn_client for callback and thunder client for making thunder requests
    let state: ThunderState = ThunderState::new(extn_client, thunder_client);

    // Make call to method
    let _ = ThunderStorageRequestProcessor::process_request(state, msg, DevicePersistenceRequest::Set(set_params.clone())).await;
    if let Ok(value) = r.recv() {
        if let Ok(message) = value.try_into() {
            let extn_message: ExtnMessage = message;
            if let Some(ExtnResponse::String(v)) = extn_message.payload.extract() {
                println!("Thunder Response from Plugin ::: {:?}", v);
            }
        }
    }
}

