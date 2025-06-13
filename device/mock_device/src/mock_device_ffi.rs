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
use ripple_sdk::{
    api::manifest::ripple_manifest_loader::RippleManifestLoader,
    export_extn_channel,
    extn::{
        client::extn_client::ExtnClient,
        extn_id::{ExtnClassId, ExtnId},
        ffi::ffi_channel::ExtnChannel,
    },
    log::error,
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

fn start() {
    let Ok((extn_manifest, _device_manifest)) = RippleManifestLoader::initialize() else {
        error!("Error initializing manifests");
        return;
    };
    let runtime = match Runtime::new() {
        Ok(r) => r,
        Err(err) => {
            error!("Error creating runtime: {}", err);
            return;
        }
    };

    let id = ExtnId::new_channel(ExtnClassId::Device, EXTN_NAME.to_string()).to_string();
    let symbol = extn_manifest.get_extn_symbol(&id);
    if symbol.is_none() {
        error!("Error getting symbol");
        return;
    }
    let symbol = symbol.unwrap();
    let (mut client, tr, trs) = ExtnClient::new_extn(symbol);
    runtime.block_on(async move {
        let client_c = client.clone();
        tokio::spawn(async move {
            match boot_ws_server(client.clone()).await {
                Ok(server) => {
                    let state = MockDeviceState::new(server);

                    let mut methods = Methods::new();
                    let _ = methods.merge(MockDeviceController::new(state).into_rpc());
                    let processor = RPCRequestProcessor::new(
                        client.clone(),
                        methods,
                        ExtnId::new_channel(ExtnClassId::Device, "mock_device".into()),
                    );
                    client.add_request_processor(processor);
                }
                Err(err) => panic!("websocket server failed to start. {}", err),
            };
        });

        client_c.initialize(tr, trs).await;
    });
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
