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

use std::collections::HashMap;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::{thread, time};

use crate::ripple_sdk::{self};
use crate::{
    client::thunder_plugin::ThunderPlugin,
    ripple_sdk::{
        api::device::device_operator::{DeviceCallRequest, DeviceChannelParams, DeviceOperator},
        async_trait::async_trait,
        extn::{
            client::extn_client::ExtnClient,
            client::extn_processor::{
                DefaultExtnStreamer, ExtnRequestProcessor, ExtnStreamProcessor, ExtnStreamer,
            },
            extn_client_message::{ExtnMessage, ExtnResponse},
        },
        serde_json,
        tokio::sync::mpsc,
    },
    thunder_state::ThunderState,
};
use ripple_sdk::api::app_catalog::{AppCatalogRequest, AppOperationComplete, AppsUpdate};
use ripple_sdk::api::device::device_apps::DeviceAppMetadata;
use ripple_sdk::api::device::device_operator::{DeviceResponseMessage, DeviceSubscribeRequest};
use ripple_sdk::api::firebolt::fb_capabilities::FireboltPermissions;
use ripple_sdk::api::firebolt::fb_metrics::{Timer, TimerType};
use ripple_sdk::api::observability::metrics_util::{
    start_service_metrics_timer, stop_and_send_service_metrics_timer,
};
#[cfg(not(test))]
use ripple_sdk::log::{debug, error, info};
use ripple_sdk::tokio;
use ripple_sdk::{
    api::device::device_apps::{AppsRequest, InstalledApp},
    framework::ripple_contract::RippleContract,
    utils::error::RippleError,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
#[cfg(test)]
use {println as info, println as debug, println as error};

use super::thunder_telemetry::{ThunderMetricsTimerName, ThunderResponseStatus};

// TODO: If/when ripple supports selectable download speeds we'll probably want multiple configurable values or compute this based on throughput.
const DEFAULT_OPERATION_TIMEOUT_SECS: u64 = 12 * 60; // 12 minutes

#[derive(Debug, Clone)]
pub struct ThunderPackageManagerState {
    pub(crate) thunder_state: ThunderState,
    pub(crate) active_operations: Arc<Mutex<HashMap<String, Operation>>>,
    pub(crate) operation_timeout_secs: u64,
}

#[derive(Debug)]
pub struct ThunderPackageManagerRequestProcessor {
    state: ThunderPackageManagerState,
    streamer: DefaultExtnStreamer,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetListRequest {
    pub id: String,
}

impl GetListRequest {
    pub fn new(id: String) -> GetListRequest {
        GetListRequest { id }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UninstallAppRequest {
    pub id: String,
    pub version: String,
    #[serde(rename = "type")]
    pub _type: String,
    pub uninstall_type: String,
}

impl UninstallAppRequest {
    pub fn new(app: InstalledApp) -> UninstallAppRequest {
        UninstallAppRequest {
            id: app.id,
            version: app.version,
            _type: String::default(),
            uninstall_type: String::default(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct InstallAppRequest {
    pub id: String,
    pub version: String,
    pub url: String,
    pub app_name: String,
    #[serde(rename = "type")]
    pub _type: String,
    pub category: String,
}

impl InstallAppRequest {
    pub fn new(app: DeviceAppMetadata) -> InstallAppRequest {
        let mut app_type = String::default();
        let mut category = String::default();

        if let Some(data_json) = app.data {
            if let Ok(data) = serde_json::from_str::<HashMap<String, String>>(&data_json) {
                if let Some(t) = data.get("type") {
                    app_type = t.to_string();
                }
                if let Some(c) = data.get("category") {
                    category = c.to_string();
                }
            }
        }

        InstallAppRequest {
            id: app.id,
            version: app.version,
            url: app.uri,
            app_name: app.title,
            _type: app_type,
            category,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CancelRequest {
    pub handle: String,
}

impl CancelRequest {
    pub fn new(handle: String) -> CancelRequest {
        CancelRequest { handle }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetMetadataRequest {
    pub id: String,
    pub version: String,
}

impl GetMetadataRequest {
    pub fn new(id: String, version: String) -> GetMetadataRequest {
        GetMetadataRequest { id, version }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct AppData {
    pub version: String,
}

impl AppData {
    pub fn new(version: String) -> AppData {
        AppData { version }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct Operation {
    durable_app_id: String,
    operation_type: AppsOperationType,
    app_data: AppData,
    timer: Timer,
}

impl Operation {
    pub fn new(
        operation_type: AppsOperationType,
        durable_app_id: String,
        app_data: AppData,
    ) -> Operation {
        let mut tags: HashMap<String, String> = HashMap::new();
        tags.insert("app_id".to_string(), durable_app_id.clone());
        tags.insert("app_version".to_string(), app_data.clone().version);
        Operation {
            operation_type: operation_type.clone(),
            durable_app_id,
            app_data,
            timer: Timer::start(
                operation_type.to_string(),
                Some(tags),
                Some(TimerType::Remote),
            ),
        }
    }
}

#[derive(Debug)]
enum OperationStatus {
    Succeeded,
    Failed,
    NotStarted,
    Downloading,
    Downloaded,
    DownloadFailed,
    Verifying,
    VerificationFailed,
    InstallationFailed,
    Cancelled,
    NotEnoughStorage,
    Unknown,
}

impl OperationStatus {
    pub fn new(s: &str) -> OperationStatus {
        match s {
            "Succeeded" => OperationStatus::Succeeded,
            "Failed" => OperationStatus::Failed,
            "NotStarted" => OperationStatus::NotStarted,
            "Downloading" => OperationStatus::Downloading,
            "Downloaded" => OperationStatus::Downloaded,
            "DownloadFailed" => OperationStatus::DownloadFailed,
            "Verifying" => OperationStatus::Verifying,
            "VerificationFailed" => OperationStatus::VerificationFailed,
            "InstallationFailed" => OperationStatus::InstallationFailed,
            "Cancelled" => OperationStatus::Cancelled,
            "NotEnoughStorage" => OperationStatus::NotEnoughStorage,
            "Unknown" => OperationStatus::Unknown,
            _ => OperationStatus::Unknown,
        }
    }

    pub fn completed(&self) -> bool {
        match self {
            OperationStatus::Succeeded => true,
            OperationStatus::Failed => true,
            OperationStatus::NotStarted => false,
            OperationStatus::Downloading => false,
            OperationStatus::Downloaded => false,
            OperationStatus::DownloadFailed => true,
            OperationStatus::Verifying => false,
            OperationStatus::VerificationFailed => true,
            OperationStatus::InstallationFailed => true,
            OperationStatus::Cancelled => true,
            OperationStatus::NotEnoughStorage => true,
            OperationStatus::Unknown => false,
        }
    }

    pub fn succeeded(&self) -> bool {
        matches!(self, OperationStatus::Succeeded)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AppsOperationType {
    Install,
    Uninstall,
}

impl FromStr for AppsOperationType {
    type Err = ();

    fn from_str(input: &str) -> Result<AppsOperationType, Self::Err> {
        match input.to_lowercase().as_str() {
            "install" => Ok(AppsOperationType::Install),
            "uninstall" => Ok(AppsOperationType::Uninstall),
            _ => Err(()),
        }
    }
}
impl ToString for AppsOperationType {
    fn to_string(&self) -> String {
        match self {
            AppsOperationType::Install => "install".to_string(),
            AppsOperationType::Uninstall => "uninstall".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppsOperationStatus {
    pub handle: String,
    pub operation: AppsOperationType,
    #[serde(rename = "type")]
    pub app_type: String,
    pub id: String,
    pub version: String,
    pub status: String,
    pub details: String,
}

impl AppsOperationStatus {
    pub fn new(
        handle: String,
        operation: AppsOperationType,
        app_type: String,
        id: String,
        version: String,
        status: String,
        details: String,
    ) -> AppsOperationStatus {
        AppsOperationStatus {
            handle,
            operation,
            app_type,
            id,
            version,
            status,
            details,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Metadata {
    pub appname: String,
    #[serde(rename = "type")]
    pub _type: String,
    pub url: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct KeyValuePair {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ThunderAppMetadata {
    pub metadata: Metadata,
    pub resources: Vec<KeyValuePair>,
}

fn get_string_field(
    status: &serde_json::Map<std::string::String, serde_json::Value>,
    field_name: &str,
) -> String {
    if let Some(Value::String(field_value)) = status.get(field_name) {
        return field_value.clone();
    }
    String::default()
}

impl ThunderPackageManagerRequestProcessor {
    pub fn new(thunder_state: ThunderState) -> ThunderPackageManagerRequestProcessor {
        let operation_timeout_secs = thunder_state
            .get_client()
            .get_uint_config("pm_operation_timeout_secs")
            .unwrap_or(DEFAULT_OPERATION_TIMEOUT_SECS);

        ThunderPackageManagerRequestProcessor {
            state: ThunderPackageManagerState {
                thunder_state,
                active_operations: Arc::new(Mutex::new(HashMap::default())),
                operation_timeout_secs,
            },
            streamer: DefaultExtnStreamer::new(),
        }
    }

    pub async fn init(state: ThunderPackageManagerState, req: ExtnMessage) -> bool {
        let (sub_tx, mut sub_rx) = mpsc::channel::<DeviceResponseMessage>(32);

        debug!("ThunderPackageManagerRequestProcessor::init: Starting listener loop");

        Self::poll_pm(state.thunder_state.clone()).await;
        let state_for_event_handler = state.clone();
        tokio::spawn(async move {
            while let Some(message) = sub_rx.recv().await {
                debug!(
                    "ThunderPackageManagerRequestProcessor::: message={:?}",
                    message
                );
                if let Some(status_map) = message.message.as_object() {
                    let operation_type = AppsOperationType::from_str(
                        get_string_field(status_map, "operation").as_str(),
                    );

                    if let Err(()) = operation_type {
                        error!("ThunderPackageManagerRequestProcessor: Unexpected operation type");
                        continue;
                    }

                    let operation_status = AppsOperationStatus {
                        handle: get_string_field(status_map, "handle"),
                        operation: operation_type.unwrap(),
                        app_type: get_string_field(status_map, "type"),
                        id: get_string_field(status_map, "id"),
                        version: get_string_field(status_map, "version"),
                        status: get_string_field(status_map, "status"),
                        details: get_string_field(status_map, "details"),
                    };

                    let status = OperationStatus::new(&operation_status.status);
                    if status.completed() {
                        let operation = Operation::new(
                            operation_status.operation.clone(),
                            operation_status.id.clone(),
                            AppData::new(operation_status.version.clone()),
                        );
                        Self::add_or_remove_operation(
                            state_for_event_handler.clone(),
                            operation_status.handle,
                            operation,
                            Some(operation_status.status),
                        );
                        let op_comp = AppOperationComplete {
                            id: operation_status.id,
                            version: operation_status.version,
                            success: status.succeeded(),
                        };
                        let cli = state_for_event_handler.thunder_state.get_client();
                        match operation_status.operation {
                            AppsOperationType::Install => {
                                if let Err(e) = cli.event(AppsUpdate::InstallComplete(op_comp)) {
                                    error!("Could not emit install complete event, e={:?}", e)
                                }
                            }
                            AppsOperationType::Uninstall => {
                                if let Err(e) = cli.event(AppsUpdate::UninstallComplete(op_comp)) {
                                    error!("Could not emit uninstall complete event, e={:?}", e)
                                }
                            }
                        }
                    }
                } else {
                    error!("ThunderPackageManagerRequestProcessor: Unexpected message payload");
                }
            }
        });

        state
            .thunder_state
            .get_thunder_client()
            .clone()
            .subscribe(
                DeviceSubscribeRequest {
                    module: ThunderPlugin::PackageManager.callsign_and_version(),
                    event_name: "operationstatus".into(),
                    params: None,
                    sub_id: None,
                },
                sub_tx,
            )
            .await;
        Self::ack(state.thunder_state.get_client(), req)
            .await
            .is_ok()
    }

    pub async fn poll_pm(thunder_state: ThunderState) {
        info!("Checking if PackageManager is ready");
        let mut got_success = false;
        while !got_success {
            let resp = Self::get_apps_list(thunder_state.clone(), None).await;
            got_success = matches!(resp, ExtnResponse::InstalledApps(_));
            if got_success {
                info!("PackageManager is ready");
            } else {
                info!("PackageManager still not ready, checking again in 10 seconds");
                let poll_time = time::Duration::from_secs(10);
                thread::sleep(poll_time);
            }
        }
    }

    // add_or_remove_operation: Adds or removes an active operation to/from the map depending on whether or not it already existed.
    // This is necessary because it's possible for thunder to send an operation status event before the associated thunder call
    // returns. This allows us to track operations by handle regardless of which occurs first, e.g. if the thunder call returns before
    // the event is received, the operation is added to the map upon return and removed when the event arrives. If the event occurs before
    // the thunder call returns, the operation is added when the event occurs and removed when the call returns. The active operation map
    // is used to cancel any operations that haven't completed after some time.
    fn add_or_remove_operation(
        state: ThunderPackageManagerState,
        handle: String,
        operation: Operation,
        status: Option<String>,
    ) {
        /*
        clone active_operations, we want an immutable view
        */
        let active_operations = state.active_operations.lock().unwrap().clone();
        match active_operations.get(&handle) {
            Some(op) => {
                let mut timer = op.timer.clone();
                timer.stop();
                timer.insert_tag("status".to_string(), status.unwrap_or("".to_string()));
                rdk_telemetry_emit(timer);
            }
            None => {
                /*new operation, insert into map */
                state
                    .active_operations
                    .lock()
                    .unwrap()
                    .insert(handle.clone(), operation);
                Self::start_operation_timeout_timer(state, handle);
            }
        }
    }

    fn operation_in_progress(
        state: ThunderPackageManagerState,
        operation_type: AppsOperationType,
        app_id: &str,
        version: &str,
    ) -> Option<String> {
        debug!(
            "operation_in_progress: operation_type={:?}, app_id={}, version={}",
            operation_type, app_id, version
        );
        for (handle, operation) in state.active_operations.lock().unwrap().iter() {
            if operation_type == operation.operation_type
                && app_id.eq(&operation.durable_app_id)
                && version.eq(&operation.app_data.version)
            {
                return Some(handle.to_string());
            }
        }
        None
    }
    /*
    2024 Apr 26 19:31:12.268372 ripple[15590]:  [INFO][thunder_ripple_sdk::processors::thunder_package_manager][thunder_comcast]-install: amazonPrime,0.0.1713210750898,failed,5010
    2024 Apr 26 19:31:12.286119 ripple[15590]:  [INFO][thunder_ripple_sdk::processors::thunder_package_manager][thunder_comcast]-install: amazonPrime,0.0.1713210750898,Cancelled,5029
     */

    fn start_operation_timeout_timer(state: ThunderPackageManagerState, handle: String) {
        let timeout_secs = state.operation_timeout_secs;
        tokio::spawn(async move {
            tokio::time::sleep(tokio::time::Duration::from_secs(timeout_secs)).await;
            /*
            clone active_operations, we want an immutable view, and stop/emit timer if the operation is in the map
            */

            let active_operations = state.active_operations.lock().unwrap().clone();
            /*
            If the handle is still in the map after state.operation_timeout_secs, timeout has occured and cancel is needed
            */
            match active_operations.get(&handle) {
                Some(_operation) => {
                    error!("Detected incomplete operation after {} seconds, attempting to cancel: handle={}", timeout_secs, handle.clone());
                    /*remove from map, this is a needed additional map
                    operation to keep locking consistent and compiling...
                    */
                    let _ = state
                        .active_operations
                        .lock()
                        .unwrap()
                        .clone()
                        .remove(&handle);

                    Self::cancel_operation(state.clone(), handle).await;
                }
                None => {
                    debug!("an attempt was made to cancel an operation that is not currently in progress. Handle {}, timeout {}",handle, timeout_secs);
                }
            };
        });
    }

    async fn get_apps_list(thunder_state: ThunderState, id: Option<String>) -> ExtnResponse {
        let method: String = ThunderPlugin::PackageManager.method("getlist");
        let request = GetListRequest::new(id.unwrap_or_default());

        let metrics_timer = start_service_metrics_timer(
            &thunder_state.get_client(),
            ThunderMetricsTimerName::PackageManagerGetList.to_string(),
        );

        let device_response = thunder_state
            .get_thunder_client()
            .call(DeviceCallRequest {
                method,
                params: Some(DeviceChannelParams::Json(
                    serde_json::to_string(&request).unwrap(),
                )),
            })
            .await;

        let (status, extn_resp) =
            match serde_json::from_value::<Vec<InstalledApp>>(device_response.message) {
                Ok(apps) => (
                    ThunderResponseStatus::Success,
                    ExtnResponse::InstalledApps(apps),
                ),
                Err(_) => (
                    ThunderResponseStatus::Failure,
                    ExtnResponse::Error(RippleError::ProcessorError),
                ),
            };

        stop_and_send_service_metrics_timer(
            thunder_state.get_client().clone(),
            metrics_timer,
            status.to_string(),
        )
        .await;

        extn_resp
    }

    async fn get_apps(thunder_state: ThunderState, req: ExtnMessage, id: Option<String>) -> bool {
        let res = Self::get_apps_list(thunder_state.clone(), id).await;
        Self::respond(thunder_state.get_client(), req, res)
            .await
            .is_ok()
    }

    async fn install_app(
        state: ThunderPackageManagerState,
        req: ExtnMessage,
        app: DeviceAppMetadata,
    ) -> bool {
        let extn_resp = Self::install(state.clone(), app).await;
        Self::respond(state.thunder_state.get_client(), req, extn_resp)
            .await
            .is_ok()
    }

    async fn install(state: ThunderPackageManagerState, app: DeviceAppMetadata) -> ExtnResponse {
        if let Some(handle) = Self::operation_in_progress(
            state.clone(),
            AppsOperationType::Install,
            &app.id,
            &app.version,
        ) {
            info!(
                "install: Installation already in progress: app={}, version={}",
                app.id, app.version
            );

            return ExtnResponse::String(handle);
        }
        /*
        setup operation at higher scope to allow it to time itself
        */
        let operation = Operation::new(
            AppsOperationType::Install,
            app.clone().id,
            AppData::new(app.clone().version),
        );

        let method: String = ThunderPlugin::PackageManager.method("install");
        let request = InstallAppRequest::new(app.clone());

        let metrics_timer = start_service_metrics_timer(
            &state.thunder_state.get_client(),
            ThunderMetricsTimerName::PackageManagerInstall.to_string(),
        );

        let device_response = state
            .thunder_state
            .get_thunder_client()
            .call(DeviceCallRequest {
                method,
                params: Some(DeviceChannelParams::Json(
                    serde_json::to_string(&request).unwrap(),
                )),
            })
            .await;

        let thunder_resp = serde_json::from_value::<String>(device_response.message);

        let status = if thunder_resp.is_ok() {
            ThunderResponseStatus::Success
        } else {
            ThunderResponseStatus::Failure
        };

        stop_and_send_service_metrics_timer(
            state.thunder_state.get_client().clone(),
            metrics_timer,
            status.to_string(),
        )
        .await;

        match thunder_resp {
            Ok(handle) => {
                Self::add_or_remove_operation(
                    state.clone(),
                    handle.clone(),
                    operation,
                    Some(status.to_string()),
                );

                ExtnResponse::String(handle)
            }
            Err(_) => ExtnResponse::Error(RippleError::ProcessorError),
        }
    }

    async fn uninstall_app(
        state: ThunderPackageManagerState,
        req: ExtnMessage,
        app: InstalledApp,
    ) -> bool {
        if let Some(handle) = Self::operation_in_progress(
            state.clone(),
            AppsOperationType::Uninstall,
            &app.id,
            &app.version,
        ) {
            info!(
                "uninstall_app: Uninstallation already in progress: app={}, version={}",
                app.clone().id,
                app.clone().version
            );

            return Self::respond(
                state.clone().thunder_state.get_client(),
                req,
                ExtnResponse::String(handle),
            )
            .await
            .is_ok();
        }
        /*
        instansiate operation here to start timing
        */
        let operation = Operation::new(
            AppsOperationType::Uninstall,
            app.clone().clone().id,
            AppData::new(app.clone().version),
        );

        let method: String = ThunderPlugin::PackageManager.method("uninstall");
        let request = UninstallAppRequest::new(app.clone());

        let metrics_timer = start_service_metrics_timer(
            &state.thunder_state.get_client(),
            ThunderMetricsTimerName::PackageManagerUninstall.to_string(),
        );

        let device_response = state
            .thunder_state
            .get_thunder_client()
            .call(DeviceCallRequest {
                method,
                params: Some(DeviceChannelParams::Json(
                    serde_json::to_string(&request).unwrap(),
                )),
            })
            .await;

        let thunder_resp = serde_json::from_value::<String>(device_response.message);

        let status = if thunder_resp.is_ok() {
            ThunderResponseStatus::Success
        } else {
            ThunderResponseStatus::Failure
        };

        stop_and_send_service_metrics_timer(
            state.thunder_state.get_client().clone(),
            metrics_timer,
            status.to_string(),
        )
        .await;

        let extn_resp = match thunder_resp {
            Ok(handle) => {
                Self::add_or_remove_operation(
                    state.clone(),
                    handle.clone(),
                    operation,
                    Some(status.to_string()),
                );
                ExtnResponse::String(handle)
            }
            Err(_) => ExtnResponse::Error(RippleError::ProcessorError),
        };

        Self::respond(state.thunder_state.get_client(), req, extn_resp)
            .await
            .is_ok()
    }

    fn decode_permissions(perms_encoded: String) -> Result<FireboltPermissions, ()> {
        let perms = base64::decode(perms_encoded);
        if let Err(e) = perms {
            error!(
                "decode_permissions: Could not decode permissions: e={:?}",
                e
            );
            return Err(());
        }

        let perms = perms.unwrap();
        let perms_str = String::from_utf8_lossy(&perms);

        debug!("decode_permissions: perms={}", perms_str.clone());

        let firebolt_perms = serde_json::from_str::<FireboltPermissions>(&perms_str);

        if let Err(e) = firebolt_perms {
            error!(
                "decode_permissions: Could not deserialize permissions: e={:?}",
                e
            );
            return Err(());
        }

        Ok(firebolt_perms.unwrap())
    }

    async fn get_firebolt_permissions(
        state: ThunderPackageManagerState,
        req: ExtnMessage,
        app_id: String,
    ) -> bool {
        let extn_resp = match Self::get_perms(state.clone(), app_id.clone()).await {
            ExtnResponse::Permission(perms) => ExtnResponse::Permission(perms),
            ExtnResponse::Error(RippleError::NotAvailable) => {
                // Could not retrieve permissions from app, attempt to _asynchronously_ reinstall. Upon successful installation
                // permissions may then be available.
                match state
                    .thunder_state
                    .get_client()
                    .standalone_request(AppCatalogRequest::GetCatalog, 2000)
                    .await
                {
                    Ok(ExtnResponse::AppCatalog(apps)) => {
                        let app: Vec<ripple_sdk::api::app_catalog::AppMetadata> =
                            apps.iter().filter(|&a| a.id.eq(&app_id)).cloned().collect();

                        if app.is_empty() {
                            error!(
                                "get_firebolt_permissions: App not found in catalog: app_id={}",
                                app_id
                            );
                        } else {
                            Self::install(state.clone(), app[0].clone().into()).await;
                        }
                    }
                    Ok(_) => {
                        error!("get_firebolt_permissions: Unexpected response");
                    }
                    Err(e) => {
                        error!("get_firebolt_permissions: Failed to get catalog: e={:?}", e);
                    }
                }
                ExtnResponse::Error(RippleError::NotAvailable)
            }
            _ => {
                error!("get_firebolt_permissions: Unexpected response");
                ExtnResponse::Error(RippleError::NotAvailable)
            }
        };

        Self::respond(state.thunder_state.get_client(), req, extn_resp)
            .await
            .is_ok()
    }

    async fn get_perms(state: ThunderPackageManagerState, app_id: String) -> ExtnResponse {
        let installed_apps =
            match Self::get_apps_list(state.thunder_state.clone(), Some(app_id.clone())).await {
                ExtnResponse::InstalledApps(apps) => apps,
                _ => {
                    error!("get_perms: Unexpected extension response");
                    return ExtnResponse::Error(RippleError::ProcessorError);
                }
            };

        let installed_app = installed_apps.iter().find(|app| app.id.eq(&app_id));
        if installed_app.is_none() {
            error!("get_perms: Failed to determine version");
            return ExtnResponse::Error(RippleError::NotAvailable);
        }

        let app = installed_app.unwrap();
        let method: String = ThunderPlugin::PackageManager.method("getmetadata");
        let request = GetMetadataRequest::new(app.id.clone(), app.version.clone());

        let metrics_timer = start_service_metrics_timer(
            &state.thunder_state.get_client(),
            ThunderMetricsTimerName::PackageManagerGetMetadata.to_string(),
        );

        let device_response = state
            .thunder_state
            .get_thunder_client()
            .call(DeviceCallRequest {
                method,
                params: Some(DeviceChannelParams::Json(
                    serde_json::to_string(&request).unwrap(),
                )),
            })
            .await;

        debug!("get_perms: device_response={:?}", device_response);

        let thunder_resp = serde_json::from_value::<ThunderAppMetadata>(device_response.message);

        let status = if thunder_resp.is_ok() {
            ThunderResponseStatus::Success
        } else {
            ThunderResponseStatus::Failure
        };

        stop_and_send_service_metrics_timer(
            state.thunder_state.get_client().clone(),
            metrics_timer,
            status.to_string(),
        )
        .await;

        let extn_resp = match thunder_resp {
            Ok(metadata) => {
                match metadata
                    .resources
                    .iter()
                    .find(|resource| resource.key.eq(&String::from("firebolt")))
                {
                    Some(r) => match Self::decode_permissions(r.value.clone()) {
                        Ok(permissions) => ExtnResponse::Permission(permissions.capabilities),
                        Err(()) => ExtnResponse::Error(RippleError::NotAvailable),
                    },
                    None => {
                        error!("get_perms: No permissions for app");
                        ExtnResponse::Error(RippleError::NotAvailable)
                    }
                }
            }
            Err(e) => {
                error!("get_perms: Failed to deserialize response: e={:?}", e);
                ExtnResponse::Error(RippleError::NotAvailable)
            }
        };

        extn_resp
    }

    async fn cancel_operation(state: ThunderPackageManagerState, handle: String) {
        let method: String = ThunderPlugin::PackageManager.method("cancel");
        let request = CancelRequest::new(handle);

        let metrics_timer = start_service_metrics_timer(
            &state.thunder_state.get_client(),
            ThunderMetricsTimerName::PackageManagerUninstall.to_string(),
        );

        let device_response = state
            .thunder_state
            .get_thunder_client()
            .call(DeviceCallRequest {
                method,
                params: Some(DeviceChannelParams::Json(
                    serde_json::to_string(&request).unwrap(),
                )),
            })
            .await;

        let status = if device_response.message.is_null() {
            ThunderResponseStatus::Success
        } else {
            error!(
                "cancel_operation: Unexpected response: message={:?}",
                device_response.message
            );

            ThunderResponseStatus::Failure
        };

        stop_and_send_service_metrics_timer(
            state.thunder_state.get_client().clone(),
            metrics_timer,
            status.to_string(),
        )
        .await;
    }
}

impl ExtnStreamProcessor for ThunderPackageManagerRequestProcessor {
    type STATE = ThunderPackageManagerState;
    type VALUE = AppsRequest;

    fn get_state(&self) -> Self::STATE {
        self.state.clone()
    }

    fn receiver(&mut self) -> mpsc::Receiver<ExtnMessage> {
        self.streamer.receiver()
    }

    fn sender(&self) -> mpsc::Sender<ExtnMessage> {
        self.streamer.sender()
    }

    fn contract(&self) -> RippleContract {
        RippleContract::Apps
    }
}

#[async_trait]
impl ExtnRequestProcessor for ThunderPackageManagerRequestProcessor {
    fn get_client(&self) -> ExtnClient {
        self.state.thunder_state.get_client()
    }
    async fn process_request(
        state: Self::STATE,
        msg: ExtnMessage,
        extracted_message: Self::VALUE,
    ) -> bool {
        match extracted_message {
            AppsRequest::Init => Self::init(state, msg).await,
            AppsRequest::GetInstalledApps(id) => {
                Self::get_apps(state.thunder_state.clone(), msg, id).await
            }
            AppsRequest::InstallApp(app) => Self::install_app(state.clone(), msg, app).await,
            AppsRequest::UninstallApp(app) => Self::uninstall_app(state.clone(), msg, app).await,
            AppsRequest::GetFireboltPermissions(app_id) => {
                Self::get_firebolt_permissions(state.clone(), msg, app_id).await
            }
        }
    }
}
/*
RDK Telemetry  message processing/emitting
*/
pub fn rdk_telemetry_emit(timer: ripple_sdk::api::firebolt::fb_metrics::Timer) {
    emit(format_timer(timer));
}

fn format_timer(timer: ripple_sdk::api::firebolt::fb_metrics::Timer) -> String {
    /*
    emit - name: <appId>,<appVersion>,<status>,<duration>
    */
    let tags = timer.clone().tags.unwrap_or_default();
    let app_id = tags.get("app_id").unwrap_or(&"".to_string()).to_string();
    let app_version = tags
        .get("app_version")
        .unwrap_or(&"".to_string())
        .to_string();
    let status = match tags.get("status") {
        Some(value) => value,
        None => "",
    };
    format!(
        "{}: {},{},{},{}",
        timer.name,
        app_id,
        app_version,
        status,
        timer.elapsed().as_millis()
    )
    .to_string()
}

fn emit(message: String) {
    ripple_sdk::log::info!("{}", message);
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::{
        client::thunder_client::ThunderClient,
        processors::thunder_package_manager::{
            format_timer, AppData, AppsOperationType, Operation,
        },
    };
    use ripple_sdk::extn::mock_extension_client::*;
    use ripple_sdk::{
        api::firebolt::fb_metrics::Timer,
        tokio::{self, time::sleep},
        uuid::Uuid,
    };
    use std::{collections::HashMap, time::Duration};

    #[tokio::test]
    pub async fn test_format_timer_all_tags() {
        let timer_name = "test_timer".to_string();
        let mut tags: HashMap<String, String> = HashMap::new();
        tags.insert("app_id".to_string(), "xumo".to_string());
        tags.insert("app_version".to_string(), "1.2.3".to_string());
        tags.insert("status".to_string(), "success".to_string());
        let mut timer = Timer::start(timer_name, Some(tags), None);
        sleep(Duration::from_millis(1000)).await;
        timer.stop();
        let rendered = format_timer(timer);
        assert!(rendered.starts_with("test_timer: xumo,1.2.3,success,"));
    }

    #[tokio::test]
    pub async fn test_format_timer_some_tags() {
        let timer_name = "test_timer".to_string();
        let mut tags: HashMap<String, String> = HashMap::new();
        tags.insert("status".to_string(), "success".to_string());
        let mut timer = Timer::start(timer_name, Some(tags), None);
        sleep(Duration::from_millis(1000)).await;
        timer.stop();
        let rendered = format_timer(timer);
        assert!(rendered.starts_with("test_timer: ,,success,"));
    }

    #[tokio::test]
    pub async fn test_stop_operation() {
        let operation = Operation::new(
            AppsOperationType::Uninstall,
            String::from("xumo"),
            AppData::new(String::from("1.2.3")),
        );
        sleep(Duration::from_millis(1000)).await;
        let mut timer = operation.timer.clone();
        timer.stop();
        let rdk_output = format_timer(timer);
        println!("{}", rdk_output);
    }
    #[tokio::test]
    pub async fn test_add_or_remove_operation_existing_handle() {
        let operation = Operation::new(
            AppsOperationType::Install,
            String::from("xumo"),
            AppData::new(String::from("1.2.3")),
        );
        let client = MockExtnClient::main();
        let thunder_client = ThunderClient {
            sender: None,
            pooled_sender: None,
            id: Uuid::new_v4(),
            plugin_manager_tx: None,
            subscriptions: None,
        };
        let mut sessions: HashMap<String, Operation> = HashMap::new();
        sessions.insert("asdf".to_string(), operation.clone());
        let state = ThunderPackageManagerState {
            thunder_state: ThunderState::new(client, thunder_client),
            active_operations: Arc::new(Mutex::new(sessions.clone())),
            operation_timeout_secs: 1,
        };
        sleep(Duration::from_millis(1000)).await;

        ThunderPackageManagerRequestProcessor::add_or_remove_operation(
            state.clone(),
            "asdf".to_string(),
            operation,
            Some("success".to_string()),
        );
        assert!(sessions.eq(&state.active_operations.lock().unwrap().clone()));
    }

    #[tokio::test]
    pub async fn test_add_or_remove_operation_new_handle() {
        let operation = Operation::new(
            AppsOperationType::Install,
            String::from("xumo"),
            AppData::new(String::from("1.2.3")),
        );
        let client = MockExtnClient::main();
        let thunder_client = ThunderClient {
            sender: None,
            pooled_sender: None,
            id: Uuid::new_v4(),
            plugin_manager_tx: None,
            subscriptions: None,
        };
        let mut sessions: HashMap<String, Operation> = HashMap::new();
        sessions.insert("asdf".to_string(), operation.clone());
        let state = ThunderPackageManagerState {
            thunder_state: ThunderState::new(client, thunder_client),
            active_operations: Arc::new(Mutex::new(HashMap::new())),
            operation_timeout_secs: 1,
        };
        sleep(Duration::from_millis(1000)).await;

        ThunderPackageManagerRequestProcessor::add_or_remove_operation(
            state.clone(),
            "asdf".to_string(),
            operation,
            Some("success".to_string()),
        );
        assert!(sessions.eq(&state.active_operations.lock().unwrap().clone()));
    }
}
