use crate::client::plugin_manager::ThunderActivatePluginParams;
use crate::client::thunder_client_pool::ThunderClientPool;
use crate::ripple_sdk::{
    serde_json::{self},
    tokio,
};
use crate::tests::contracts::contract_utils::*;
use crate::thunder_state::ThunderConnectionState;
use pact_consumer::mock_server::StartMockServerAsync;
use ripple_sdk::api::device::device_operator::{
    DeviceCallRequest, DeviceChannelParams, DeviceOperator, DeviceResponseMessage,
};
use serde_json::json;
use std::sync::Arc;

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "contract_tests"), ignore)]
async fn test_register_state_change_event() {
    let mut pact_builder_async = get_pact_builder_async_obj().await;
    pact_builder_async
    .synchronous_message_interaction("A request to register state change event", |mut i| async move {
        i.contents_from(json!({
            "pact:content-type": "application/json",
            "request": {"jsonrpc": "matching(type, '2.0')", "id": "matching(integer, 0)", "method": "Controller.1.register", "params": {"event": "statechange", "id": "client.Controller.1.events"}},
            "requestMetadata": {
                "path": "/jsonrpc"
            },
            "response": [{
                "jsonrpc": "matching(type, '2.0')",
                "id": "matching(integer, 0)",
                "result": 0
            }]
        })).await;
        i.test_name("register_state_change_event");
        i
    }).await;
    let mock_server = pact_builder_async
        .start_mock_server_async(Some("websockets/transport/websockets"))
        .await;

    let url = url::Url::parse(mock_server.path("/jsonrpc").as_str()).unwrap();
    let thunder_client =
        ThunderClientPool::start(url, None, Arc::new(ThunderConnectionState::new()), 1)
            .await
            .unwrap();

    let r = json!({"event": "statechange", "id": "client.Controller.1.events"});
    let _resp = thunder_client
        .clone()
        .call(DeviceCallRequest {
            method: "Controller.1.register".to_string(),
            params: Some(DeviceChannelParams::Json(
                serde_json::to_string(&r).unwrap(),
            )),
        })
        .await;
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "contract_tests"), ignore)]
async fn test_device_info_plugin_status() {
    let mut pact_builder_async = get_pact_builder_async_obj().await;
    pact_builder_async
    .synchronous_message_interaction("A request to get the status of the DeviceInfo plugin", |mut i| async move {
        i.contents_from(json!({
            "pact:content-type": "application/json",
            "request": {"jsonrpc": "matching(type, '2.0')", "id": "matching(integer, 1)", "method": "Controller.1.status@DeviceInfo"},
            "requestMetadata": {
                "path": "/jsonrpc"
            },
            "response": [{
                "jsonrpc": "matching(type, '2.0')",
                "id": "matching(integer, 1)",
                "result": [{
                    "callsign": "matching(type, 'string')",
                    "locator": "matching(type, 'string')",
                    "classname": "matching(type, 'string')",
                    "autostart": "matching(type, 'string')",
                    "configuration": "matching(type, 'string')",
                    "state": "matching(type, 'string')",
                    "observers": "matching(integer, 0)",
                    "module": "matching(type, 'string')",
                    "hash": "matching(type, 'string')",
                    "major": "matching(integer, 0)",
                    "minor": "matching(integer, 0)",
                    "patch":"matching(integer, 0)"
                }]
            }]
        })).await;
        i.test_name("device_info_plugin_status");
        i
    }).await;
    let mock_server = pact_builder_async
        .start_mock_server_async(Some("websockets/transport/websockets"))
        .await;

    let url = url::Url::parse(mock_server.path("/jsonrpc").as_str()).unwrap();
    let thunder_client =
        ThunderClientPool::start(url, None, Arc::new(ThunderConnectionState::new()), 1)
            .await
            .unwrap();

    let _resp: DeviceResponseMessage = thunder_client
        .clone()
        .call(DeviceCallRequest {
            method: "Controller.1.status@DeviceInfo".to_string(),
            params: None,
        })
        .await;
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "contract_tests"), ignore)]
async fn test_device_info_plugin_state() {
    let mut pact_builder_async = get_pact_builder_async_obj().await;
    pact_builder_async
    .synchronous_message_interaction("A request to get the device info plugin activation state ", |mut i| async move {
        i.contents_from(json!({
            "pact:content-type": "application/json",
            "request": {"jsonrpc": "matching(type, '2.0')", "id": "matching(integer, 2)", "method": "Controller.1.activate", "params": {"callsign": "DeviceInfo"}},
            "requestMetadata": {
                "path": "/jsonrpc"
            },
            "response": [{
                "jsonrpc": "matching(type, '2.0')",
                "id": "matching(integer, 42)",
                "result": null
            }]
        })).await;
        i.test_name("activate_device_info_plugin");
        i
    }).await;
    let mock_server = pact_builder_async
        .start_mock_server_async(Some("websockets/transport/websockets"))
        .await;

    let url = url::Url::parse(mock_server.path("/jsonrpc").as_str()).unwrap();
    let thunder_client =
        ThunderClientPool::start(url, None, Arc::new(ThunderConnectionState::new()), 1)
            .await
            .unwrap();

    let r = ThunderActivatePluginParams {
        callsign: "DeviceInfo".to_string(),
    };
    let _resp = thunder_client
        .clone()
        .call(DeviceCallRequest {
            method: "Controller.1.activate".to_string(),
            params: Some(DeviceChannelParams::Json(
                serde_json::to_string(&r).unwrap(),
            )),
        })
        .await;
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "contract_tests"), ignore)]
async fn test_display_settings_plugin_status() {
    let mut pact_builder_async = get_pact_builder_async_obj().await;
    pact_builder_async
    .synchronous_message_interaction("A request to get the status of the DisplaySettings plugin", |mut i| async move {
        i.contents_from(json!({
            "pact:content-type": "application/json",
            "request": {"jsonrpc": "matching(type, '2.0')", "id": "matching(integer, 3)", "method": "Controller.1.status@org.rdk.DisplaySettings"},
            "requestMetadata": {
                "path": "/jsonrpc"
            },
            "response": [{
                "jsonrpc": "matching(type, '2.0')",
                "id": "matching(integer, 3)",
                "result": [{
                    "callsign": "matching(type, 'string')",
                    "locator": "matching(type, 'string')",
                    "classname": "matching(type, 'string')",
                    "autostart": "matching(type, 'string')",
                    "precondition": ["matching(type, 'string')"],
                    "state": "matching(type, 'string')",
                    "observers": "matching(integer, 0)",
                    "module": "matching(type, 'string')",
                    "hash": "matching(type, 'string')",
                    "major": "matching(integer, 0)",
                    "minor": "matching(integer, 0)",
                    "patch":"matching(integer, 0)"
                }]
            }]
        })).await;
        i.test_name("display_settings_plugin_status");
        i
    }).await;
    let mock_server = pact_builder_async
        .start_mock_server_async(Some("websockets/transport/websockets"))
        .await;

    let url = url::Url::parse(mock_server.path("/jsonrpc").as_str()).unwrap();
    let thunder_client =
        ThunderClientPool::start(url, None, Arc::new(ThunderConnectionState::new()), 1)
            .await
            .unwrap();

    let _resp: DeviceResponseMessage = thunder_client
        .clone()
        .call(DeviceCallRequest {
            method: "Controller.1.status@org.rdk.DisplaySettings".to_string(),
            params: None,
        })
        .await;
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "contract_tests"), ignore)]
async fn test_activate_display_settings_plugin() {
    let mut pact_builder_async = get_pact_builder_async_obj().await;
    pact_builder_async
    .synchronous_message_interaction("A request to activate the DisplaySettings plugin", |mut i| async move {
        i.contents_from(json!({
            "pact:content-type": "application/json",
            "request": {"jsonrpc": "matching(type, '2.0')", "id": "matching(integer, 4)", "method": "Controller.1.activate", "params": {"callsign": "org.rdk.DisplaySettings"}},
            "requestMetadata": {
                "path": "/jsonrpc"
            },
            "response": [{
                "jsonrpc": "matching(type, '2.0')",
                "id": "matching(integer, 4)",
                "result": null
            }]
        })).await;
        i.test_name("activate_display_settings_plugin");
        i
    }).await;
    let mock_server = pact_builder_async
        .start_mock_server_async(Some("websockets/transport/websockets"))
        .await;

    let url = url::Url::parse(mock_server.path("/jsonrpc").as_str()).unwrap();
    let thunder_client =
        ThunderClientPool::start(url, None, Arc::new(ThunderConnectionState::new()), 1)
            .await
            .unwrap();

    let r = ThunderActivatePluginParams {
        callsign: "org.rdk.DisplaySettings".to_string(),
    };
    let _resp = thunder_client
        .clone()
        .call(DeviceCallRequest {
            method: "Controller.1.activate".to_string(),
            params: Some(DeviceChannelParams::Json(
                serde_json::to_string(&r).unwrap(),
            )),
        })
        .await;
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "contract_tests"), ignore)]
async fn test_status_org_rdk_system() {
    let mut pact_builder_async = get_pact_builder_async_obj().await;
    pact_builder_async
    .synchronous_message_interaction("A request to get the status of the System plugin", |mut i| async move {
        i.contents_from(json!({
            "pact:content-type": "application/json",
            "request": {"jsonrpc": "matching(type, '2.0')", "id": "matching(integer, 5)", "method": "Controller.1.status@org.rdk.System"},
            "requestMetadata": {
                "path": "/jsonrpc"
            },
            "response": [{
                "jsonrpc": "matching(type, '2.0')",
                "id": "matching(integer, 5)",
                "result": [{
                    "callsign": "matching(type, 'string')",
                    "locator": "matching(type, 'string')",
                    "classname": "matching(type, 'string')",
                    "autostart": "matching(type, 'string')",
                    "precondition": ["matching(type, 'string')"],
                    "state": "matching(type, 'string')",
                    "observers": "matching(integer, 0)",
                    "module": "matching(type, 'string')",
                    "hash": "matching(type, 'string')",
                    "major": "matching(integer, 0)",
                    "minor": "matching(integer, 0)",
                    "patch":"matching(integer, 0)"
                }]
            }]
        })).await;
        i.test_name("status_org_rdk_system");
        i
    }).await;
    let mock_server = pact_builder_async
        .start_mock_server_async(Some("websockets/transport/websockets"))
        .await;

    let url = url::Url::parse(mock_server.path("/jsonrpc").as_str()).unwrap();
    let thunder_client =
        ThunderClientPool::start(url, None, Arc::new(ThunderConnectionState::new()), 1)
            .await
            .unwrap();

    let _resp: DeviceResponseMessage = thunder_client
        .clone()
        .call(DeviceCallRequest {
            method: "Controller.1.status@org.rdk.System".to_string(),
            params: None,
        })
        .await;
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "contract_tests"), ignore)]
async fn test_activate_org_rdk_system() {
    let mut pact_builder_async = get_pact_builder_async_obj().await;
    pact_builder_async
    .synchronous_message_interaction("A request to activate the System plugin", |mut i| async move {
        i.contents_from(json!({
            "pact:content-type": "application/json",
            "request": {"jsonrpc": "matching(type, '2.0')", "id": "matching(integer, 6)", "method": "Controller.1.activate", "params": {"callsign": "org.rdk.System"}},
            "requestMetadata": {
                "path": "/jsonrpc"
            },
            "response": [{
                "jsonrpc": "matching(type, '2.0')",
                "id": "matching(integer, 6)",
                "result": null
            }]
        })).await;
        i.test_name("activate_org_rdk_system");
        i
    }).await;
    let mock_server = pact_builder_async
        .start_mock_server_async(Some("websockets/transport/websockets"))
        .await;

    let url = url::Url::parse(mock_server.path("/jsonrpc").as_str()).unwrap();
    let thunder_client =
        ThunderClientPool::start(url, None, Arc::new(ThunderConnectionState::new()), 1)
            .await
            .unwrap();

    let r = ThunderActivatePluginParams {
        callsign: "org.rdk.System".to_string(),
    };
    let _resp = thunder_client
        .clone()
        .call(DeviceCallRequest {
            method: "Controller.1.activate".to_string(),
            params: Some(DeviceChannelParams::Json(
                serde_json::to_string(&r).unwrap(),
            )),
        })
        .await;
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "contract_tests"), ignore)]
async fn test_status_org_rdk_hdcp_profile() {
    let mut pact_builder_async = get_pact_builder_async_obj().await;
    pact_builder_async
    .synchronous_message_interaction("A request to get the status of the HdcpProfile plugin", |mut i| async move {
        i.contents_from(json!({
            "pact:content-type": "application/json",
            "request": {"jsonrpc": "matching(type, '2.0')", "id": "matching(integer, 7)", "method": "Controller.1.status@org.rdk.HdcpProfile"},
            "requestMetadata": {
                "path": "/jsonrpc"
            },
            "response": [{
                "jsonrpc": "matching(type, '2.0')",
                "id": "matching(integer, 7)",
                "result": [{
                    "callsign": "matching(type, 'string')",
                    "locator": "matching(type, 'string')",
                    "classname": "matching(type, 'string')",
                    "autostart": "matching(type, 'string')",
                    "precondition": ["matching(type, 'string')"],
                    "state": "matching(type, 'string')",
                    "observers": "matching(integer, 0)",
                    "module": "matching(type, 'string')",
                    "hash": "matching(type, 'string')",
                    "major": "matching(integer, 0)",
                    "minor": "matching(integer, 0)",
                    "patch":"matching(integer, 0)"
                }]
            }]
        })).await;
        i.test_name("status_org_rdk_hdcp_profile");
        i
    }).await;
    let mock_server = pact_builder_async
        .start_mock_server_async(Some("websockets/transport/websockets"))
        .await;

    let url = url::Url::parse(mock_server.path("/jsonrpc").as_str()).unwrap();
    let thunder_client =
        ThunderClientPool::start(url, None, Arc::new(ThunderConnectionState::new()), 1)
            .await
            .unwrap();

    let _resp: DeviceResponseMessage = thunder_client
        .clone()
        .call(DeviceCallRequest {
            method: "Controller.1.status@org.rdk.HdcpProfile".to_string(),
            params: None,
        })
        .await;
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "contract_tests"), ignore)]
async fn test_activate_org_rdk_hdcp_profile() {
    let mut pact_builder_async = get_pact_builder_async_obj().await;
    pact_builder_async
    .synchronous_message_interaction("A request to activate the HdcpProfile plugin", |mut i| async move {
        i.contents_from(json!({
            "pact:content-type": "application/json",
            "request": {"jsonrpc": "matching(type, '2.0')", "id": "matching(integer, 8)", "method": "Controller.1.activate", "params": {"callsign": "org.rdk.HdcpProfile"}},
            "requestMetadata": {
                "path": "/jsonrpc"
            },
            "response": [{
                "jsonrpc": "matching(type, '2.0')",
                "id": "matching(integer, 8)",
                "result": null
            }]
        })).await;
        i.test_name("activate_org_rdk_hdcp_profile");
        i
    }).await;
    let mock_server = pact_builder_async
        .start_mock_server_async(Some("websockets/transport/websockets"))
        .await;

    let url = url::Url::parse(mock_server.path("/jsonrpc").as_str()).unwrap();
    let thunder_client =
        ThunderClientPool::start(url, None, Arc::new(ThunderConnectionState::new()), 1)
            .await
            .unwrap();

    let r = ThunderActivatePluginParams {
        callsign: "org.rdk.HdcpProfile".to_string(),
    };
    let _resp = thunder_client
        .clone()
        .call(DeviceCallRequest {
            method: "Controller.1.activate".to_string(),
            params: Some(DeviceChannelParams::Json(
                serde_json::to_string(&r).unwrap(),
            )),
        })
        .await;
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "contract_tests"), ignore)]
async fn test_status_org_rdk_telemetry() {
    let mut pact_builder_async = get_pact_builder_async_obj().await;
    pact_builder_async
    .synchronous_message_interaction("A request to get the status of the Telemetry plugin", |mut i| async move {
        i.contents_from(json!({
            "pact:content-type": "application/json",
            "request": {"jsonrpc": "matching(type, '2.0')", "id": "matching(integer, 9)", "method": "Controller.1.status@org.rdk.Telemetry"},
            "requestMetadata": {
                "path": "/jsonrpc"
            },
            "response": [{
                "jsonrpc": "matching(type, '2.0')",
                "id": "matching(integer, 9)",
                "result": [{
                    "callsign": "matching(type, 'string')",
                    "locator": "matching(type, 'string')",
                    "classname": "matching(type, 'string')",
                    "autostart": "matching(type, 'string')",
                    "precondition": ["matching(type, 'string')"],
                    "configuration": "matching(type, 'string')",
                    "state": "matching(type, 'string')",
                    "observers": "matching(integer, 0)",
                    "module": "matching(type, 'string')",
                    "hash": "matching(type, 'string')",
                    "major": "matching(integer, 0)",
                    "minor": "matching(integer, 0)",
                    "patch":"matching(integer, 0)"
                }]
            }]
        })).await;
        i.test_name("status_org_rdk_telemetry");
        i
    }).await;

    let mock_server = pact_builder_async
        .start_mock_server_async(Some("websockets/transport/websockets"))
        .await;

    let url = url::Url::parse(mock_server.path("/jsonrpc").as_str()).unwrap();
    let thunder_client =
        ThunderClientPool::start(url, None, Arc::new(ThunderConnectionState::new()), 1)
            .await
            .unwrap();

    let _resp: DeviceResponseMessage = thunder_client
        .clone()
        .call(DeviceCallRequest {
            method: "Controller.1.status@org.rdk.Telemetry".to_string(),
            params: None,
        })
        .await;
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "contract_tests"), ignore)]
async fn test_activate_org_rdk_telemetry() {
    let mut pact_builder_async = get_pact_builder_async_obj().await;
    pact_builder_async
    .synchronous_message_interaction("A request to activate the Telemetry plugin", |mut i| async move {
        i.contents_from(json!({
            "pact:content-type": "application/json",
            "request": {"jsonrpc": "matching(type, '2.0')", "id": "matching(integer, 10)", "method": "Controller.1.activate", "params": {"callsign": "org.rdk.Telemetry"}},
            "requestMetadata": {
                "path": "/jsonrpc"
            },
            "response": [{
                "jsonrpc": "matching(type, '2.0')",
                "id": "matching(integer, 10)",
                "result": null
            }]
        })).await;
        i.test_name("activate_org_rdk_telemetry");
        i
    }).await;
    let mock_server = pact_builder_async
        .start_mock_server_async(Some("websockets/transport/websockets"))
        .await;

    let url = url::Url::parse(mock_server.path("/jsonrpc").as_str()).unwrap();
    let thunder_client =
        ThunderClientPool::start(url, None, Arc::new(ThunderConnectionState::new()), 1)
            .await
            .unwrap();

    let r = ThunderActivatePluginParams {
        callsign: "org.rdk.Telemetry".to_string(),
    };
    let _resp = thunder_client
        .clone()
        .call(DeviceCallRequest {
            method: "Controller.1.activate".to_string(),
            params: Some(DeviceChannelParams::Json(
                serde_json::to_string(&r).unwrap(),
            )),
        })
        .await;
}
