use crate::{
    client::{
        plugin_manager::ThunderActivatePluginParams, thunder_client::ThunderClient,
        thunder_client_pool::ThunderClientPool,
    },
    ripple_sdk::{
        serde_json::{self, json},
        tokio,
    },
    thunder_state::ThunderConnectionState,
};
use ripple_sdk::api::device::device_operator::{
    DeviceCallRequest, DeviceChannelParams, DeviceOperator, DeviceResponseMessage,
};

use crate::mock_websocket_server;
use pact_consumer::mock_server::StartMockServerAsync;
use pact_consumer::prelude::*;
use std::sync::Arc;
use url::Url;

async fn initialize_thunder_client(server_url: Url) -> ThunderClient {
    ThunderClientPool::start(server_url, None, Arc::new(ThunderConnectionState::new()), 1)
        .await
        .unwrap()
}

async fn perform_device_call_request(
    thunder_client: ThunderClient,
    method: &str,
    params: Option<DeviceChannelParams>,
) -> DeviceResponseMessage {
    thunder_client
        .clone()
        .call(DeviceCallRequest {
            method: method.to_string(),
            params,
        })
        .await
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "contract_tests"), ignore)]
async fn test_register_state_change_event() {
    mock_websocket_server!(
        builder,
        server,
        server_url,
        "register_state_change_event",
        json!({
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
        })
    );

    let thunder_client = initialize_thunder_client(server_url.clone()).await;

    let request_payload = json!({"event": "statechange", "id": "client.Controller.1.events"});
    let _resp = perform_device_call_request(
        thunder_client,
        "Controller.1.register",
        Some(DeviceChannelParams::Json(
            serde_json::to_string(&request_payload).unwrap(),
        )),
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "contract_tests"), ignore)]
async fn test_device_info_plugin_status() {
    mock_websocket_server!(
        builder,
        server,
        server_url,
        "device_info_plugin_status",
        json!({
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
        })
    );

    let thunder_client = initialize_thunder_client(server_url.clone()).await;

    let _resp: DeviceResponseMessage =
        perform_device_call_request(thunder_client, "Controller.1.status@DeviceInfo", None).await;
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "contract_tests"), ignore)]
async fn test_device_info_plugin_state() {
    mock_websocket_server!(
        builder,
        server,
        server_url,
        "activate_device_info_plugin",
        json!({
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
        })
    );

    let thunder_client = initialize_thunder_client(server_url.clone()).await;

    let request_payload = ThunderActivatePluginParams {
        callsign: "DeviceInfo".to_string(),
    };
    let _resp = perform_device_call_request(
        thunder_client,
        "Controller.1.activate",
        Some(DeviceChannelParams::Json(
            serde_json::to_string(&request_payload).unwrap(),
        )),
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "contract_tests"), ignore)]
async fn test_display_settings_plugin_status() {
    mock_websocket_server!(
        builder,
        server,
        server_url,
        "display_settings_plugin_status",
        json!({
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
        })
    );

    let thunder_client = initialize_thunder_client(server_url.clone()).await;
    let _resp: DeviceResponseMessage = perform_device_call_request(
        thunder_client,
        "Controller.1.status@org.rdk.DisplaySettings",
        None,
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "contract_tests"), ignore)]
async fn test_activate_display_settings_plugin() {
    mock_websocket_server!(
        builder,
        server,
        server_url,
        "activate_display_settings_plugin",
        json!({
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
        })
    );

    let thunder_client = initialize_thunder_client(server_url.clone()).await;

    let request_payload = ThunderActivatePluginParams {
        callsign: "org.rdk.DisplaySettings".to_string(),
    };
    let _resp = perform_device_call_request(
        thunder_client,
        "Controller.1.activate",
        Some(DeviceChannelParams::Json(
            serde_json::to_string(&request_payload).unwrap(),
        )),
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "contract_tests"), ignore)]
async fn test_status_org_rdk_system() {
    mock_websocket_server!(
        builder,
        server,
        server_url,
        "status_org_rdk_system",
        json!({
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
                    "patch": "matching(integer, 0)"
                }]
            }]
        })
    );

    let thunder_client = initialize_thunder_client(server_url.clone()).await;

    let _resp: DeviceResponseMessage =
        perform_device_call_request(thunder_client, "Controller.1.status@org.rdk.System", None)
            .await;
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "contract_tests"), ignore)]
async fn test_activate_org_rdk_system() {
    mock_websocket_server!(
        builder,
        server,
        server_url,
        "activate_org_rdk_system",
        json!({
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
        })
    );

    let thunder_client = initialize_thunder_client(server_url.clone()).await;

    let request_payload = ThunderActivatePluginParams {
        callsign: "org.rdk.System".to_string(),
    };
    let _resp = perform_device_call_request(
        thunder_client,
        "Controller.1.activate",
        Some(DeviceChannelParams::Json(
            serde_json::to_string(&request_payload).unwrap(),
        )),
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "contract_tests"), ignore)]
async fn test_status_org_rdk_hdcp_profile() {
    mock_websocket_server!(
        builder,
        server,
        server_url,
        "status_org_rdk_hdcp_profile",
        json!({
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
                    "patch": "matching(integer, 0)"
                }]
            }]
        })
    );

    let thunder_client = initialize_thunder_client(server_url.clone()).await;

    let _resp: DeviceResponseMessage = perform_device_call_request(
        thunder_client,
        "Controller.1.status@org.rdk.HdcpProfile",
        None,
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "contract_tests"), ignore)]
async fn test_activate_org_rdk_hdcp_profile() {
    mock_websocket_server!(
        builder,
        server,
        server_url,
        "activate_org_rdk_hdcp_profile",
        json!({
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
        })
    );

    let thunder_client = initialize_thunder_client(server_url.clone()).await;

    let request_payload = ThunderActivatePluginParams {
        callsign: "org.rdk.HdcpProfile".to_string(),
    };
    let _resp = perform_device_call_request(
        thunder_client,
        "Controller.1.activate",
        Some(DeviceChannelParams::Json(
            serde_json::to_string(&request_payload).unwrap(),
        )),
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "contract_tests"), ignore)]
async fn test_status_org_rdk_telemetry() {
    mock_websocket_server!(
        builder,
        server,
        server_url,
        "status_org_rdk_telemetry",
        json!({
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
                    "patch": "matching(integer, 0)"
                }]
            }]
        })
    );

    let thunder_client = initialize_thunder_client(server_url.clone()).await;

    let _resp: DeviceResponseMessage = perform_device_call_request(
        thunder_client,
        "Controller.1.status@org.rdk.Telemetry",
        None,
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "contract_tests"), ignore)]
async fn test_activate_org_rdk_telemetry() {
    mock_websocket_server!(
        builder,
        server,
        server_url,
        "activate_org_rdk_telemetry",
        json!({
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
        })
    );
    let thunder_client = initialize_thunder_client(server_url.clone()).await;
    let request_payload = ThunderActivatePluginParams {
        callsign: "org.rdk.Telemetry".to_string(),
    };
    let _resp = perform_device_call_request(
        thunder_client,
        "Controller.1.activate",
        Some(DeviceChannelParams::Json(
            serde_json::to_string(&request_payload).unwrap(),
        )),
    )
    .await;
}
