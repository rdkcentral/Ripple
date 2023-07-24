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

use crate::ripple_sdk::{
    api::device::{device_request::DeviceRequest, device_window_manager::WindowManagerRequest},
    crossbeam::channel::unbounded,
    extn::client::extn_processor::ExtnRequestProcessor,
    extn::extn_client_message::{ExtnPayload, ExtnRequest},
    serde_json,
};

use crate::get_pact_with_params;
use crate::processors::thunder_window_manager::ThunderWindowManagerRequestProcessor;
use crate::tests::contracts::contract_utils::*;
use crate::{client::thunder_client_pool::ThunderClientPool, thunder_state::ThunderState};
use pact_consumer::mock_server::StartMockServerAsync;
use ripple_sdk::api::apps::Dimensions;
use serde_json::json;
use std::collections::HashMap;

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "contract_tests"), ignore)]
async fn test_device_set_window_visibility() {
    let mut pact_builder_async = get_pact_builder_async_obj().await;

    let mut params = HashMap::new();
    params.insert(
        "client".into(),
        ContractMatcher::MatchType("org.rdk.Netflix".into()),
    );
    params.insert(
        "callsign".into(),
        ContractMatcher::MatchType("org.rdk.Netflix".into()),
    );
    params.insert("visible".into(), ContractMatcher::MatchBool(true));

    let mut result = HashMap::new();
    result.insert("success".into(), ContractMatcher::MatchBool(true));

    pact_builder_async
        .synchronous_message_interaction(
            "A request to set visibility of a client",
            |mut i| async move {
                i.contents_from(get_pact_with_params!(
                    "org.rdk.RDKShell.1.setVisibility",
                    ContractResult { result },
                    ContractParams { params }
                ))
                .await;
                i.test_name("set_visibility_of_client");
                i
            },
        )
        .await;

    let mock_server = pact_builder_async
        .start_mock_server_async(Some("websockets/transport/websockets"))
        .await;

    let visibility_params = r#"{"callsign": "org.rdk.Netflix", "client": "org.rdk.Netflix"}"#;
    let payload = ExtnPayload::Request(ExtnRequest::Device(DeviceRequest::WindowManager(
        WindowManagerRequest::Visibility(serde_json::to_string(&visibility_params).unwrap(), true),
    )));
    let msg = get_extn_msg(payload);

    let url = url::Url::parse(mock_server.path("/jsonrpc").as_str()).unwrap();
    let thunder_client = ThunderClientPool::start(url, None, 1).await.unwrap();

    let (s, r) = unbounded();
    let extn_client = get_extn_client(s.clone(), r.clone());

    let state: ThunderState = ThunderState::new(extn_client, thunder_client);

    let _ = ThunderWindowManagerRequestProcessor::process_request(
        state,
        msg,
        WindowManagerRequest::Visibility(serde_json::to_string(&visibility_params).unwrap(), true),
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "contract_tests"), ignore)]
async fn test_device_move_window_to_front() {
    let mut pact_builder_async = get_pact_builder_async_obj().await;

    let mut params = HashMap::new();
    params.insert(
        "client".into(),
        ContractMatcher::MatchType("org.rdk.Netflix".into()),
    );
    params.insert(
        "callsign".into(),
        ContractMatcher::MatchType("org.rdk.Netflix".into()),
    );

    let mut result = HashMap::new();
    result.insert("success".into(), ContractMatcher::MatchBool(true));

    pact_builder_async
        .synchronous_message_interaction("A request to move window to front", |mut i| async move {
            i.contents_from(get_pact_with_params!(
                "org.rdk.RDKShell.1.moveToFront",
                ContractResult { result },
                ContractParams { params }
            ))
            .await;
            i.test_name("move_window_to_front");
            i
        })
        .await;

    let mock_server = pact_builder_async
        .start_mock_server_async(Some("websockets/transport/websockets"))
        .await;

    let move_to_front_params = r#"{"callsign": "org.rdk.Netflix", "client": "org.rdk.Netflix"}"#;
    let payload = ExtnPayload::Request(ExtnRequest::Device(DeviceRequest::WindowManager(
        WindowManagerRequest::MoveToFront(serde_json::to_string(&move_to_front_params).unwrap()),
    )));
    let msg = get_extn_msg(payload);

    let url = url::Url::parse(mock_server.path("/jsonrpc").as_str()).unwrap();
    let thunder_client = ThunderClientPool::start(url, None, 1).await.unwrap();

    let (s, r) = unbounded();
    let extn_client = get_extn_client(s.clone(), r.clone());

    let state: ThunderState = ThunderState::new(extn_client, thunder_client);

    let _ = ThunderWindowManagerRequestProcessor::process_request(
        state,
        msg,
        WindowManagerRequest::MoveToFront(serde_json::to_string(&move_to_front_params).unwrap()),
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "contract_tests"), ignore)]
async fn test_device_move_window_to_back() {
    let mut pact_builder_async = get_pact_builder_async_obj().await;

    let mut params = HashMap::new();
    params.insert(
        "client".into(),
        ContractMatcher::MatchType("org.rdk.Netflix".into()),
    );
    params.insert(
        "callsign".into(),
        ContractMatcher::MatchType("org.rdk.Netflix".into()),
    );

    let mut result = HashMap::new();
    result.insert("success".into(), ContractMatcher::MatchBool(true));

    pact_builder_async
        .synchronous_message_interaction("A request to move window to back", |mut i| async move {
            i.contents_from(get_pact_with_params!(
                "org.rdk.RDKShell.1.moveToBack",
                ContractResult { result },
                ContractParams { params }
            ))
            .await;
            i.test_name("move_window_to_back");
            i
        })
        .await;

    let mock_server = pact_builder_async
        .start_mock_server_async(Some("websockets/transport/websockets"))
        .await;

    let move_to_back_params = r#"{"callsign": "org.rdk.Netflix", "client": "org.rdk.Netflix"}"#;
    let payload = ExtnPayload::Request(ExtnRequest::Device(DeviceRequest::WindowManager(
        WindowManagerRequest::MoveToBack(serde_json::to_string(&move_to_back_params).unwrap()),
    )));
    let msg = get_extn_msg(payload);

    let url = url::Url::parse(mock_server.path("/jsonrpc").as_str()).unwrap();
    let thunder_client = ThunderClientPool::start(url, None, 1).await.unwrap();

    let (s, r) = unbounded();
    let extn_client = get_extn_client(s.clone(), r.clone());

    let state: ThunderState = ThunderState::new(extn_client, thunder_client);

    let _ = ThunderWindowManagerRequestProcessor::process_request(
        state,
        msg,
        WindowManagerRequest::MoveToBack(serde_json::to_string(&move_to_back_params).unwrap()),
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "contract_tests"), ignore)]
async fn test_device_set_window_to_focus() {
    let mut pact_builder_async = get_pact_builder_async_obj().await;

    let mut params = HashMap::new();
    params.insert(
        "client".into(),
        ContractMatcher::MatchType("org.rdk.Netflix".into()),
    );
    params.insert(
        "callsign".into(),
        ContractMatcher::MatchType("org.rdk.Netflix".into()),
    );

    let mut result = HashMap::new();
    result.insert("success".into(), ContractMatcher::MatchBool(true));

    pact_builder_async
        .synchronous_message_interaction("A request to set window to focus", |mut i| async move {
            i.contents_from(get_pact_with_params!(
                "org.rdk.RDKShell.1.setFocus",
                ContractResult { result },
                ContractParams { params }
            ))
            .await;
            i.test_name("set_window_to_focus");
            i
        })
        .await;

    let mock_server = pact_builder_async
        .start_mock_server_async(Some("websockets/transport/websockets"))
        .await;

    let set_to_focus_params = r#"{"callsign": "org.rdk.Netflix", "client": "org.rdk.Netflix"}"#;
    let payload = ExtnPayload::Request(ExtnRequest::Device(DeviceRequest::WindowManager(
        WindowManagerRequest::Focus(serde_json::to_string(&set_to_focus_params).unwrap()),
    )));
    let msg = get_extn_msg(payload);

    let url = url::Url::parse(mock_server.path("/jsonrpc").as_str()).unwrap();
    let thunder_client = ThunderClientPool::start(url, None, 1).await.unwrap();

    let (s, r) = unbounded();
    let extn_client = get_extn_client(s.clone(), r.clone());

    let state: ThunderState = ThunderState::new(extn_client, thunder_client);

    let _ = ThunderWindowManagerRequestProcessor::process_request(
        state,
        msg,
        WindowManagerRequest::Focus(serde_json::to_string(&set_to_focus_params).unwrap()),
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "contract_tests"), ignore)]
async fn test_device_set_window_bounds() {
    let mut pact_builder_async = get_pact_builder_async_obj().await;

    let mut params = HashMap::new();
    params.insert(
        "client".into(),
        ContractMatcher::MatchType("org.rdk.Netflix".into()),
    );
    params.insert(
        "callsign".into(),
        ContractMatcher::MatchType("org.rdk.Netflix".into()),
    );
    params.insert("x".into(), ContractMatcher::MatchNumber(0));
    params.insert("y".into(), ContractMatcher::MatchNumber(0));
    params.insert("w".into(), ContractMatcher::MatchNumber(1980));
    params.insert("h".into(), ContractMatcher::MatchNumber(1080));

    let mut result = HashMap::new();
    result.insert("success".into(), ContractMatcher::MatchBool(true));

    pact_builder_async
        .synchronous_message_interaction("A request to set window bounds", |mut i| async move {
            i.contents_from(get_pact_with_params!(
                "org.rdk.RDKShell.1.setBounds",
                ContractResult { result },
                ContractParams { params }
            ))
            .await;
            i.test_name("set_window_bounds");
            i
        })
        .await;

    let mock_server = pact_builder_async
        .start_mock_server_async(Some("websockets/transport/websockets"))
        .await;

    let set_to_focus_params = r#"{"callsign": "org.rdk.Netflix", "client": "org.rdk.Netflix"}"#;
    let dimenstions = Dimensions {
        x: 0,
        y: 0,
        w: 1920,
        h: 1080,
    };
    let payload = ExtnPayload::Request(ExtnRequest::Device(DeviceRequest::WindowManager(
        WindowManagerRequest::Dimensions(
            serde_json::to_string(&set_to_focus_params).unwrap(),
            dimenstions.clone(),
        ),
    )));
    let msg = get_extn_msg(payload);

    let url = url::Url::parse(mock_server.path("/jsonrpc").as_str()).unwrap();
    let thunder_client = ThunderClientPool::start(url, None, 1).await.unwrap();

    let (s, r) = unbounded();
    let extn_client = get_extn_client(s.clone(), r.clone());

    let state: ThunderState = ThunderState::new(extn_client, thunder_client);

    let _ = ThunderWindowManagerRequestProcessor::process_request(
        state,
        msg,
        WindowManagerRequest::Dimensions(
            serde_json::to_string(&set_to_focus_params).unwrap(),
            dimenstions.clone(),
        ),
    )
    .await;
}
