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

use jsonrpsee::core::server::rpc_module::Methods;
use ripple_sdk::{
    api::{manifest::extn_manifest::ExtnSymbol, status_update::ExtnStatus},
    export_extn_channel,
    extn::{
        client::extn_client::ExtnClient,
        extn_id::{ExtnClassId, ExtnId},
        ffi::ffi_channel::ExtnChannel,
    },
    log::info,
    processor::rpc_request_processor::RPCRequestProcessor,
    tokio::{self, runtime::Runtime},
    utils::logger::init_logger,
};

use crate::{
    mock_data::MockDeviceState,
    mock_device_controller::{MockDeviceController, MockDeviceControllerServer},
    utils::boot_ws_server,
};

pub const EXTN_NAME: &str = "mock_device";

fn start() {
    let _ = init_logger(EXTN_NAME.into());
    info!("Starting mock device channel");
    let runtime = Runtime::new().unwrap();
    let symbol = ExtnSymbol {
        id: format!(
            "{}",
            ExtnId::new_channel(ExtnClassId::Distributor, EXTN_NAME.to_string())
        ),
        uses: Vec::new(),
        fulfills: Vec::new(),
        config: Some(HashMap::new()),
    };
    let (mut client, tr) = ExtnClient::new_extn(symbol);
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
                        ExtnId::new_channel(ExtnClassId::Gateway, "badger".into()),
                    );
                    client.add_request_processor(processor);
                }
                Err(err) => panic!("websocket server failed to start. {}", err),
            };

            // Lets Main know that the mock_device channel is ready
            let _ = client.event(ExtnStatus::Ready);
        });

        client_c.initialize(tr).await;
    });
}

fn init_extn_channel() -> ExtnChannel {
    ExtnChannel { start }
}

export_extn_channel!(ExtnChannel, init_extn_channel);
