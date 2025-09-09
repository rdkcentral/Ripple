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

use jsonrpsee::core::server::rpc_module::Methods;
use ripple_sdk::service::service_client::ServiceClient;
use ripple_sdk::{
    api::manifest::ripple_manifest_loader::RippleManifestLoader,
    export_extn_channel,
    extn::{
        extn_id::{ExtnClassId, ExtnId},
        ffi::ffi_channel::ExtnChannel,
    },
    log::{error, info},
    processor::rpc_request_processor::RPCRequestProcessor,
    tokio::{self, runtime::Runtime},
    utils::logger::init_and_configure_logger,
};

use crate::{
    mock_data::MockDeviceState,
    mock_device_controller::{MockDeviceController, MockDeviceControllerServer},
    utils::boot_ws_server,
};

pub const EXTN_NAME: &str = "mock_device";

pub async fn start_service() {
    let log_lev = ripple_sdk::log::LevelFilter::Debug;
    let _ = init_and_configure_logger(
        "some_version",
        EXTN_NAME.into(),
        Some(vec![
            ("extn_manifest".to_string(), log_lev),
            ("device_manifest".to_string(), log_lev),
        ]),
    );
    info!("Starting mock device channel");
    let Ok((extn_manifest, _device_manifest)) = RippleManifestLoader::initialize() else {
        error!("Error initializing manifests");
        return;
    };

    let id = ExtnId::new_channel(ExtnClassId::Device, EXTN_NAME.to_string()).to_string();
    let symbol = extn_manifest.get_extn_symbol(&id);
    if symbol.is_none() {
        error!("Error getting symbol");
        return;
    }
    let service_client = if let Some(symbol) = symbol {
        ServiceClient::builder().with_extension(symbol).build()
    } else {
        ServiceClient::builder().build()
    };

    init(service_client.clone()).await;
}

async fn init(client: ServiceClient) {
    info!("@@@NNA: Entered init function");
    if let Some(mut extn_client) = client.get_extn_client() {
        info!("@@@NNA: extn_client found, spawning websocket server");
        let client_c_for_init = client.clone();
        tokio::spawn(async move {
            match boot_ws_server(extn_client.clone()).await {
                Ok(server) => {
                    info!("@@@NNA: Websocket server started successfully");
                    let state = MockDeviceState::new(server);

                    let mut methods = Methods::new();
                    let _ = methods.merge(MockDeviceController::new(state).into_rpc());
                    info!("@@@NNA: RPC methods merged");
                    let processor = RPCRequestProcessor::new(
                        extn_client.clone(),
                        methods,
                        ExtnId::new_channel(ExtnClassId::Device, "mock_device".into()),
                    );
                    info!("@@@NNA: RPCRequestProcessor created, adding to extn_client");
                    extn_client.add_request_processor(processor);
                }
                Err(err) => {
                    error!("@@@NNA: websocket server failed to start. {}", err);
                    panic!("websocket server failed to start. {}", err);
                }
            };
        });

        info!("@@@NNA: Initializing client_c_for_init");
        client_c_for_init.initialize().await;
        info!("@@@NNA: client_c_for_init initialized");
    } else {
        error!("@@@NNA: Service client does not hold an extn client. Cannot start eos extension.");
    }
}

fn start() {
    info!("@@@NNA: Entered start function");
    let Ok((extn_manifest, _device_manifest)) = RippleManifestLoader::initialize() else {
        error!("@@@NNA: Error initializing manifests");
        return;
    };
    info!("@@@NNA: Manifests initialized successfully");

    let runtime = match Runtime::new() {
        Ok(r) => {
            info!("@@@NNA: Tokio runtime created successfully");
            r
        }
        Err(err) => {
            error!("@@@NNA: Error creating runtime: {}", err);
            return;
        }
    };

    let id = ExtnId::new_channel(ExtnClassId::Device, EXTN_NAME.to_string()).to_string();
    info!("@@@NNA: Generated ExtnId: {}", id);

    let symbol = extn_manifest.get_extn_symbol(&id);
    if symbol.is_none() {
        error!("@@@NNA: Error getting symbol");
        return;
    }
    info!("@@@NNA: Extension symbol found");

    let service_client = if let Some(symbol) = symbol {
        info!("@@@NNA: Building ServiceClient with extension symbol");
        ServiceClient::builder().with_extension(symbol).build()
    } else {
        info!("@@@NNA: Building ServiceClient without extension symbol");
        ServiceClient::builder().build()
    };

    info!("@@@NNA: Starting async init in runtime");
    runtime.block_on(async move {
        init(service_client.clone()).await;
    });
    info!("@@@NNA: Exiting start function");
}

fn init_extn_channel() -> ExtnChannel {
    let log_lev = ripple_sdk::log::LevelFilter::Debug;
    let _ = init_and_configure_logger(
        "some_version",
        EXTN_NAME.into(),
        Some(vec![
            ("extn_manifest".to_string(), log_lev),
            ("device_manifest".to_string(), log_lev),
        ]),
    );

    ExtnChannel { start }
}

export_extn_channel!(ExtnChannel, init_extn_channel);
