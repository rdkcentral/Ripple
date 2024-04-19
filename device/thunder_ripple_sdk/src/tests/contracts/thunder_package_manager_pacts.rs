use crate::tests::contracts::contract_utils::*;
use crate::thunder_state::ThunderConnectionState;
use crate::{
    client::thunder_client_pool::ThunderClientPool,
    get_pact_with_params,
    processors::thunder_package_manager::{
        AppData, AppsOperationType, Operation, ThunderPackageManagerRequestProcessor,
        ThunderPackageManagerState,
    },
    ripple_sdk::{
        api::{
            device::{
                device_apps::{AppsRequest, DeviceAppMetadata, InstalledApp},
                device_request::DeviceRequest,
            },
            firebolt::fb_capabilities::{
                CapabilityRole, FireboltCap, FireboltPermission, FireboltPermissions,
            },
        },
        async_channel::unbounded,
        extn::client::extn_processor::ExtnRequestProcessor,
        extn::extn_client_message::{ExtnPayload, ExtnRequest},
        serde_json::{self},
        tokio,
    },
    thunder_state::ThunderState,
};
use pact_consumer::mock_server::StartMockServerAsync;
use rstest::rstest;
use serde_json::json;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[rstest(
    active_operations_some,
    op_type_progress,
    timeout,
    case(false, AppsOperationType::Install, 360),
    case(false, AppsOperationType::Uninstall, 360),
    case(true, AppsOperationType::Install, 360),
    case(true, AppsOperationType::Uninstall, 360)
)]
#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "contract_tests"), ignore)]
async fn test_install_app(
    active_operations_some: bool,
    op_type_progress: AppsOperationType,
    timeout: u64,
) {
    let mut pact_builder_async = get_pact_builder_async_obj().await;

    if !active_operations_some || op_type_progress == AppsOperationType::Uninstall {
        pact_builder_async
            .synchronous_message_interaction(
                "A request to install app in package manager",
                |mut i| async move {
                    i.contents_from(json!({
                        "pact:content-type": "application/json",
                        "request": {"jsonrpc": "matching(type, '2.0')", "id": "matching(integer, 0)", "method": "org.rdk.PackageManager.1.install", "params": {
                            "type": "",
                            "id": "firecertApp",
                            "version": "2.0",
                            "url": "https://firecertApp.com",
                            "app_name": "firecertApp",
                            "category": ""
                          }},
                          "requestMetadata": {
                            "path": "/jsonrpc",
                        },
                        "response": [{
                            "jsonrpc": "matching(type, '2.0')",
                            "id": "matching(integer, 0)",
                            "result": "some_resp",
                            }
                        ]
                    })).await;
                    i.test_name("install_package_manager_app");
                    i
                },
            )
            .await;
    }

    if timeout == 0 {
        pact_builder_async
        .synchronous_message_interaction(
            "A request to cancel the async request from package manager",
            |mut i| async move {
                i.contents_from(json!({
                    "pact:content-type": "application/json",
                    "request": {"jsonrpc": "matching(type, '2.0')", "id": "matching(integer, 2)", "method": "org.rdk.PackageManager.1.cancel", "params": {"handle": "some_resp"}},
                    "requestMetadata": {
                        "path": "/jsonrpc"
                    },
                    "response": [
                        {
                            "jsonrpc": "matching(type, '2.0')",
                            "id": "matching(integer, 2)",
                            "result": null
                        }
                    ]
                })).await;
                i.test_name("cancel_async_request_package_manager_apps");
                i
            },
        )
        .await;
    }

    let mock_server = pact_builder_async
        .start_mock_server_async(Some("websockets/transport/websockets"))
        .await;

    let app_metadata_params = DeviceAppMetadata {
        id: "firecertApp".to_string(),
        title: "firecertApp".to_string(),
        uri: "https://firecertApp.com".into(),
        version: "2.0".to_string(),
        data: None,
    };

    let payload = ExtnPayload::Request(ExtnRequest::Device(DeviceRequest::Apps(
        AppsRequest::InstallApp(app_metadata_params.clone()),
    )));
    let msg = get_extn_msg(payload);

    let url = url::Url::parse(mock_server.path("/jsonrpc").as_str()).unwrap();
    let thunder_client =
        ThunderClientPool::start(url, None, Arc::new(ThunderConnectionState::new()), 1)
            .await
            .unwrap();

    let (s, r) = unbounded();
    let mut extn_client = get_extn_client(s.clone(), r.clone());
    let state: ThunderState = ThunderState::new(extn_client.clone(), thunder_client);

    let package_manager_processor = ThunderPackageManagerRequestProcessor::new(state.clone());
    extn_client.add_request_processor(package_manager_processor);

    let mut active_operations = HashMap::new();
    if active_operations_some {
        active_operations.insert(
            "handle".into(),
            Some(Operation::new(
                op_type_progress,
                "firecertApp".into(),
                AppData::new("2.0".into()),
            )),
        );
    }
    let ao: HashMap<String, Operation> = active_operations
        .into_iter()
        .map(|(k, v)| (k, v.unwrap()))
        .collect();

    let tps = ThunderPackageManagerState {
        thunder_state: state,
        active_operations: Arc::new(Mutex::new(ao)),
    };

    let resp = ThunderPackageManagerRequestProcessor::process_request(
        tps,
        msg,
        AppsRequest::InstallApp(app_metadata_params),
    )
    .await;
    assert!(resp);
}

#[rstest(
    active_operations_some,
    op_type_progress,
    timeout,
    case(false, AppsOperationType::Install, 360),
    case(true, AppsOperationType::Install, 360),
    case(true, AppsOperationType::Uninstall, 360)
)]
#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "contract_tests"), ignore)]
async fn test_uninstall_app(
    active_operations_some: bool,
    op_type_progress: AppsOperationType,
    timeout: u64,
) {
    let mut pact_builder_async = get_pact_builder_async_obj().await;

    if !active_operations_some || op_type_progress == AppsOperationType::Install {
        pact_builder_async
            .synchronous_message_interaction(
                "A request to uninstall package manager",
                |mut i| async move {
                    i.contents_from(json!({
                        "pact:content-type": "application/json",
                        "request": {"jsonrpc": "matching(type, '2.0')", "id": "matching(integer, 0)", "method": "org.rdk.PackageManager.1.uninstall", "params": {
                            "type": "",
                            "id": "firecertApp",
                            "version": "2.0",
                            "uninstall_type": ""
                          }},
                          "requestMetadata": {
                            "path": "/jsonrpc",
                        },
                        "response": [{
                            "jsonrpc": "matching(type, '2.0')",
                            "id": "matching(integer, 0)",
                            "result": "some_resp",
                            }
                        ]
                    })).await;
                    i.test_name("uninstall_package_manager_app");
                    i
                },
            )
            .await;
    }

    if timeout == 0 {
        pact_builder_async
        .synchronous_message_interaction(
            "A request to cancel the async request from package manager",
            |mut i| async move {
                i.contents_from(json!({
                    "pact:content-type": "application/json",
                    "request": {"jsonrpc": "matching(type, '2.0')", "id": "matching(integer, 1)", "method": "org.rdk.PackageManager.1.cancel", "params": {"handle": "some_resp"}},
                    "requestMetadata": {
                        "path": "/jsonrpc"
                    },
                    "response": [
                        {
                            "jsonrpc": "matching(type, '2.0')",
                            "id": "matching(integer, 1)",
                            "result": null
                        }
                    ]
                })).await;
                i.test_name("cancel_async_request_package_manager_apps");
                i
            },
        )
        .await;
    }

    let mock_server = pact_builder_async
        .start_mock_server_async(Some("websockets/transport/websockets"))
        .await;

    let installed_app = InstalledApp {
        id: "firecertApp".to_string(),
        version: "2.0".to_string(),
    };

    let payload = ExtnPayload::Request(ExtnRequest::Device(DeviceRequest::Apps(
        AppsRequest::UninstallApp(installed_app.clone()),
    )));
    let msg = get_extn_msg(payload);

    let url = url::Url::parse(mock_server.path("/jsonrpc").as_str()).unwrap();
    let thunder_client =
        ThunderClientPool::start(url, None, Arc::new(ThunderConnectionState::new()), 1)
            .await
            .unwrap();

    let (s, r) = unbounded();
    let mut extn_client = get_extn_client(s.clone(), r.clone());
    let state: ThunderState = ThunderState::new(extn_client.clone(), thunder_client);

    let package_manager_processor = ThunderPackageManagerRequestProcessor::new(state.clone());
    extn_client.add_request_processor(package_manager_processor);

    let mut active_operations = HashMap::new();
    if active_operations_some {
        active_operations.insert(
            "handle".into(),
            Some(Operation::new(
                op_type_progress,
                "firecertApp".into(),
                AppData::new("2.0".into()),
            )),
        );
    }
    let ao: HashMap<String, Operation> = active_operations
        .into_iter()
        .map(|(k, v)| (k, v.unwrap()))
        .collect();

    let tps = ThunderPackageManagerState {
        thunder_state: state,
        active_operations: Arc::new(Mutex::new(ao)),
    };

    let resp = ThunderPackageManagerRequestProcessor::process_request(
        tps,
        msg,
        AppsRequest::UninstallApp(installed_app),
    )
    .await;
    assert!(resp);
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "contract_tests"), ignore)]
async fn test_get_installed_apps() {
    let mut pact_builder_async = get_pact_builder_async_obj().await;
    let installed_apps = vec![InstalledApp {
        id: "firecertApp".to_string(),
        version: "1.0".to_string(),
    }];

    pact_builder_async
        .synchronous_message_interaction(
            "A request to list installed package manager apps",
            |mut i| async move {
                i.contents_from(json!({
                    "pact:content-type": "application/json",
                    "request": {"jsonrpc": "matching(type, '2.0')", "id": "matching(integer, 0)", "method": "org.rdk.PackageManager.1.getlist", "params": {"id": ""}},
                    "requestMetadata": {
                        "path": "/jsonrpc"
                    },
                    "response": [
                        {
                            "jsonrpc": "matching(type, '2.0')",
                            "id": "matching(integer, 0)",
                            "result": installed_apps
                        }
                    ]
                })).await;
                i.test_name("get_installed_package_manager_apps");
                i
            },
        )
        .await;

    let mock_server = pact_builder_async
        .start_mock_server_async(Some("websockets/transport/websockets"))
        .await;

    let payload = ExtnPayload::Request(ExtnRequest::Device(DeviceRequest::Apps(
        AppsRequest::GetInstalledApps(None),
    )));
    let msg = get_extn_msg(payload);

    let url = url::Url::parse(mock_server.path("/jsonrpc").as_str()).unwrap();
    let thunder_client =
        ThunderClientPool::start(url, None, Arc::new(ThunderConnectionState::new()), 1)
            .await
            .unwrap();

    let (s, r) = unbounded();
    let mut extn_client = get_extn_client(s.clone(), r.clone());
    let state: ThunderState = ThunderState::new(extn_client.clone(), thunder_client);

    let package_manager_processor = ThunderPackageManagerRequestProcessor::new(state.clone());
    extn_client.add_request_processor(package_manager_processor);

    let tps = ThunderPackageManagerState {
        thunder_state: state,
        active_operations: Arc::new(Mutex::new(HashMap::default())),
    };

    let resp = ThunderPackageManagerRequestProcessor::process_request(
        tps,
        msg,
        AppsRequest::GetInstalledApps(None),
    )
    .await;
    assert!(resp);
}

#[ignore]
#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "contract_tests"), ignore)]
async fn test_init() {
    let mut pact_builder_async = get_pact_builder_async_obj().await;

    pact_builder_async
            .synchronous_message_interaction(
                "A request to install app in package manager",
                |mut i| async move {
                    i.contents_from(json!({
                        "pact:content-type": "application/json",
                        "request": {"jsonrpc": "matching(type, '2.0')", "id": "matching(integer, 0)", "method": "org.rdk.PackageManager.1.install", "params": {
                            "type": "",
                            "id": "firecertApp",
                            "version": "2.0",
                            "url": "https://firecertApp.com",
                            "app_name": "firecertApp",
                            "category": ""
                          }},
                          "requestMetadata": {
                            "path": "/jsonrpc",
                        },
                        "response": [{
                            "jsonrpc": "matching(type, '2.0')",
                            "id": "matching(integer, 0)",
                            "result": "some_resp",
                            }
                        ]
                    })).await;
                    i.test_name("install_package_manager_app");
                    i
                },
            )
            .await;

    let installed_apps = vec![InstalledApp {
        id: "firecertApp".to_string(),
        version: "1.0".to_string(),
    }];

    pact_builder_async
        .synchronous_message_interaction(
            "A request to list installed package manager apps",
            |mut i| async move {
                i.contents_from(json!({
                    "pact:content-type": "application/json",
                    "request": {"jsonrpc": "matching(type, '2.0')", "id": "matching(integer, 0)", "method": "org.rdk.PackageManager.1.getlist", "params": {"id": ""}},
                    "requestMetadata": {
                        "path": "/jsonrpc"
                    },
                    "response": [
                        {
                            "jsonrpc": "matching(type, '2.0')",
                            "id": "matching(integer, 0)",
                            "result": installed_apps
                        }
                    ]
                })).await;
                i.test_name("getlist_installed_package_manager_apps");
                i
            },
        )
        .await;

    let mut register_result = HashMap::new();
    register_result.insert("type".into(), ContractMatcher::MatchType("test".into()));
    register_result.insert(
        "id".into(),
        ContractMatcher::MatchType("firecertApp".into()),
    );
    register_result.insert("version".into(), ContractMatcher::MatchType("2.0".into()));

    register_result.insert("handle".into(), ContractMatcher::MatchType("some".into()));
    register_result.insert(
        "operation".into(),
        ContractMatcher::MatchType("install".into()),
    );
    register_result.insert(
        "status".into(),
        ContractMatcher::MatchType("Succeeded".into()),
    );
    register_result.insert("details".into(), ContractMatcher::MatchType("some".into()));
    register_result.insert(
        "handle".into(),
        ContractMatcher::MatchType("some_str".into()),
    );

    pact_builder_async
        .synchronous_message_interaction(
            "A request to register package manager",
            |mut i| async move {
                i.contents_from(get_pact_with_params!(
                    "org.rdk.PackageManager.1.register",
                    ContractResult {
                        result: register_result
                    },
                    ContractParams {
                        params: HashMap::default()
                    }
                ))
                .await;
                i.test_name("register_events_package_manager_app");
                i
            },
        )
        .await;

    let mock_server = pact_builder_async
        .start_mock_server_async(Some("websockets/transport/websockets"))
        .await;

    let app_metadata_params = DeviceAppMetadata {
        id: "firecertApp".to_string(),
        title: "firecertApp".to_string(),
        uri: "https://firecertApp.com".into(),
        version: "2.0".to_string(),
        data: None,
    };

    let payload = ExtnPayload::Request(ExtnRequest::Device(DeviceRequest::Apps(
        AppsRequest::InstallApp(app_metadata_params.clone()),
    )));
    let msg = get_extn_msg(payload);

    let url = url::Url::parse(mock_server.path("/jsonrpc").as_str()).unwrap();
    let thunder_client =
        ThunderClientPool::start(url, None, Arc::new(ThunderConnectionState::new()), 1)
            .await
            .unwrap();

    let (s, r) = unbounded();
    let mut extn_client = get_extn_client(s.clone(), r.clone());
    let state: ThunderState = ThunderState::new(extn_client.clone(), thunder_client);

    let package_manager_processor = ThunderPackageManagerRequestProcessor::new(state.clone());
    extn_client.add_request_processor(package_manager_processor);

    let tps = ThunderPackageManagerState {
        thunder_state: state,
        active_operations: Arc::new(Mutex::new(HashMap::default())),
    };

    let tps_for_thread = tps.clone();
    let msg_for_thread = msg.clone();

    tokio::spawn(async move {
        let init_resp = ThunderPackageManagerRequestProcessor::process_request(
            tps_for_thread,
            msg_for_thread,
            AppsRequest::Init,
        )
        .await;
        assert!(init_resp);
    });

    let resp = ThunderPackageManagerRequestProcessor::process_request(
        tps,
        msg,
        AppsRequest::InstallApp(app_metadata_params),
    )
    .await;
    assert!(resp);
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "contract_tests"), ignore)]
async fn test_get_firebolt_permissions() {
    let mut pact_builder_async = get_pact_builder_async_obj().await;
    let installed_apps = vec![InstalledApp {
        id: "firecertApp".to_string(),
        version: "1.0".to_string(),
    }];

    pact_builder_async
        .synchronous_message_interaction(
            "A request to list installed package manager apps",
            |mut i| async move {
                i.contents_from(json!({
                    "pact:content-type": "application/json",
                    "request": {"jsonrpc": "matching(type, '2.0')", "id": "matching(integer, 0)", "method": "org.rdk.PackageManager.1.getlist", "params": {"id": "firecertApp"}},
                    "requestMetadata": {
                        "path": "/jsonrpc"
                    },
                    "response": [
                        {
                            "jsonrpc": "matching(type, '2.0')",
                            "id": "matching(integer, 0)",
                            "result": installed_apps
                        }
                    ]
                })).await;
                i.test_name("getlist_installed_package_manager_apps");
                i
            },
        )
        .await;

    let fp = FireboltPermission {
        cap: FireboltCap::Short("test_short_cap".to_string()),
        role: CapabilityRole::Use,
    };
    let firebolt_perms = FireboltPermissions {
        capabilities: vec![fp.clone()],
    };
    let encoded_perms =
        base64::encode(serde_json::to_string(&firebolt_perms).expect("Serialization failed"));
    let metadata_response = json!({
        "metadata": {
            "appname": "firecertApp",
            "type": "test",
            "url": "http://example.com"
        },
        "resources": [
            {
                "key": "firebolt",
                "value": encoded_perms
            }
        ]
    });
    pact_builder_async
        .synchronous_message_interaction(
            "A request to get metadata for package manager app",
            |mut i| async move {
                i.contents_from(json!({
                    "pact:content-type": "application/json",
                    "request": {
                        "jsonrpc": "matching(type, '2.0')",
                        "id": "matching(integer, 1)",
                        "method": "org.rdk.PackageManager.1.getmetadata",
                        "params": {"id": "firecertApp", "version": "1.0"}
                    },
                    "requestMetadata": {
                        "path": "/jsonrpc"
                    },
                    "response": [
                        {
                            "jsonrpc": "matching(type, '2.0')",
                            "id": "matching(integer, 1)",
                            "result": metadata_response
                        }
                    ]
                }))
                .await;
                i.test_name("get_metadata_package_manager_app");
                i
            },
        )
        .await;

    let mock_server = pact_builder_async
        .start_mock_server_async(Some("websockets/transport/websockets"))
        .await;

    let payload = ExtnPayload::Request(ExtnRequest::Device(DeviceRequest::Apps(
        AppsRequest::GetFireboltPermissions("firecertApp".to_string()),
    )));
    let msg = get_extn_msg(payload);

    let url = url::Url::parse(mock_server.path("/jsonrpc").as_str()).unwrap();
    let thunder_client =
        ThunderClientPool::start(url, None, Arc::new(ThunderConnectionState::new()), 1)
            .await
            .unwrap();

    let (s, r) = unbounded();
    let mut extn_client = get_extn_client(s.clone(), r.clone());
    let state: ThunderState = ThunderState::new(extn_client.clone(), thunder_client);

    let package_manager_processor = ThunderPackageManagerRequestProcessor::new(state.clone());
    extn_client.add_request_processor(package_manager_processor);

    let tps = ThunderPackageManagerState {
        thunder_state: state,
        active_operations: Arc::new(Mutex::new(HashMap::default())),
    };

    let resp = ThunderPackageManagerRequestProcessor::process_request(
        tps,
        msg,
        AppsRequest::GetFireboltPermissions("firecertApp".to_string()),
    )
    .await;
    assert!(resp);
}
