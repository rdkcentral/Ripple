#[allow(dead_code, unused_imports)]
use crate::ripple_sdk::{
    serde_json::{self, json},
    tokio,
};
#[allow(dead_code, unused_imports)]
use crate::{mock_websocket_server, send_thunder_call_message};
#[allow(dead_code, unused_imports)]
use pact_consumer::mock_server::StartMockServerAsync;
#[allow(dead_code, unused_imports)]
use pact_consumer::prelude::*;

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

    send_thunder_call_message!(
        server_url.to_string(),
        json!({
            "jsonrpc": "2.0",
            "id": 0,
            "method": "Controller.1.register",
            "params": {
                "event": "statechange",
                "id": "client.Controller.1.events"
            }
        })
    )
    .await;
}

//added to avoid RESPONSE: {"jsonrpc":"2.0","error":{"code":-32600,"message":"Cannot find a matching response for the received request","data":""}}
#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "websocket_contract_tests"), ignore)]
async fn test_controller_status_no_callsign() {
    mock_websocket_server!(
        builder,
        server,
        server_url,
        "controller_status_no_callsign",
        json!({
            "pact:content-type": "application/json",
            "request": {"jsonrpc": "matching(type, '2.0')", "id": "matching(integer, 3)", "method": "Controller.1.status"},
            "requestMetadata": {
                "path": "/jsonrpc"
            },
            "response": [{
                "jsonrpc": "matching(type, '2.0')",
                "id": "matching(integer, 3)",
                "result": [{
                    "state": "activated"
                }]
            }]
        })
    );

    send_thunder_call_message!(
        server_url.to_string(),
        json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "Controller.1.status"
        })
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "websocket_contract_tests"), ignore)]
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

    send_thunder_call_message!(
        server_url.to_string(),
        json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "Controller.1.status@DeviceInfo",
            "params": json!({})
        })
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "websocket_contract_tests"), ignore)]
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

    send_thunder_call_message!(
        server_url.to_string(),
        json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "Controller.1.activate",
            "params": json!({
                "callsign": "DeviceInfo",
            })
        })
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "websocket_contract_tests"), ignore)]
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

    send_thunder_call_message!(
        server_url.to_string(),
        json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "Controller.1.status@org.rdk.DisplaySettings",
        })
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "websocket_contract_tests"), ignore)]
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

    send_thunder_call_message!(
        server_url.to_string(),
        json!({
            "jsonrpc": "2.0",
            "id": 4,
            "method": "Controller.1.activate",
            "params": json!({
                "callsign": "org.rdk.DisplaySettings",
            })
        })
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "websocket_contract_tests"), ignore)]
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

    send_thunder_call_message!(
        server_url.to_string(),
        json!({
            "jsonrpc": "2.0",
            "id": 5,
            "method": "Controller.1.status@org.rdk.System",
        })
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "websocket_contract_tests"), ignore)]
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

    send_thunder_call_message!(
        server_url.to_string(),
        json!({
            "jsonrpc": "2.0",
            "id": 6,
            "method": "Controller.1.activate",
            "params": json!({
                "callsign": "org.rdk.System",
            })
        })
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "websocket_contract_tests"), ignore)]
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

    send_thunder_call_message!(
        server_url.to_string(),
        json!({
            "jsonrpc": "2.0",
            "id": 7,
            "method": "Controller.1.status@org.rdk.HdcpProfile",
        })
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "websocket_contract_tests"), ignore)]
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

    send_thunder_call_message!(
        server_url.to_string(),
        json!({
            "jsonrpc": "2.0",
            "id": 8,
            "method": "Controller.1.activate",
            "params": json!({
                "callsign": "org.rdk.HdcpProfile",
            })
        })
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "websocket_contract_tests"), ignore)]
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

    send_thunder_call_message!(
        server_url.to_string(),
        json!({
            "jsonrpc": "2.0",
            "id": 9,
            "method": "Controller.1.status@org.rdk.Telemetry",
        })
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "websocket_contract_tests"), ignore)]
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

    send_thunder_call_message!(
        server_url.to_string(),
        json!({
            "jsonrpc": "2.0",
            "id": 10,
            "method": "Controller.1.activate",
            "params": json!({
                "callsign": "org.rdk.Telemetry",
            })
        })
    )
    .await;
}
