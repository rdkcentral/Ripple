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
    if let Ok(service_client) =
        ServiceClient::builder(EXTN_NAME.into(), ExtnClassId::Gateway).build()
    {
        info!("Service client built successfully");

        init(service_client).await
    } else {
        error!("Failed to build service client");
    }
}

async fn init(client: ServiceClient) {
    if let Some(mut extn_client) = client.get_extn_client() {
        let client_c_for_init = client.clone();
        tokio::spawn(async move {
            match boot_ws_server(extn_client.clone()).await {
                Ok(server) => {
                    let state = MockDeviceState::new(server);

                    let mut methods = Methods::new();
                    let _ = methods.merge(MockDeviceController::new(state).into_rpc());
                    let processor = RPCRequestProcessor::new(
                        extn_client.clone(),
                        methods,
                        ExtnId::new_channel(ExtnClassId::Device, "mock_device".into()),
                    );
                    extn_client.add_request_processor(processor);
                }
                Err(err) => panic!("websocket server failed to start. {}", err),
            };
        });

        client_c_for_init.initialize().await;
    } else {
        error!("Service client does not hold an extn client. Cannot start eos extension.");
    }
}

fn start() {
    let Ok((_extn_manifest, _device_manifest)) = RippleManifestLoader::initialize() else {
        error!("Error initializing manifests");
        return;
    };

    if let Ok(service_client) =
        ServiceClient::builder(EXTN_NAME.into(), ExtnClassId::Gateway).build()
    {
        info!("Service client built successfully");
        match Runtime::new() {
            Ok(runtime) => {
                runtime.block_on(async move {
                    init(service_client.clone()).await;
                });
            }
            Err(err) => {
                error!("Error creating runtime: {}", err);
            }
        };
    } else {
        error!("Failed to build service client");
    }
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
