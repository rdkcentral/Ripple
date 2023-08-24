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

use ripple_sdk::{
    api::status_update::ExtnStatus,
    crossbeam::channel::Receiver as CReceiver,
    export_channel_builder, export_extn_metadata,
    extn::{
        client::{extn_client::ExtnClient, extn_sender::ExtnSender},
        extn_id::{ExtnClassId, ExtnId},
        ffi::{
            ffi_channel::{ExtnChannel, ExtnChannelBuilder},
            ffi_library::{CExtnMetadata, ExtnMetadata, ExtnSymbolMetadata},
            ffi_message::CExtnMessage,
        },
    },
    framework::ripple_contract::{ContractFulfiller, RippleContract},
    log::{debug, info},
    semver::Version,
    tokio::{self, runtime::Runtime},
    utils::{error::RippleError, logger::init_logger},
};

use crate::{
    boot_ws_server::boot_ws_server,
    mock_device_ws_server_processor::MockDeviceMockWebsocketServerProcessor,
};

const EXTN_NAME: &str = "mock_device";

fn init_library() -> CExtnMetadata {
    let _ = init_logger(EXTN_NAME.into());

    let dist_meta = ExtnSymbolMetadata::get(
        ExtnId::new_channel(ExtnClassId::Device, EXTN_NAME.into()),
        ContractFulfiller::new(vec![RippleContract::MockWebsocketServer]),
        Version::new(1, 0, 0),
    );

    debug!("Returning distributor builder");
    let extn_metadata = ExtnMetadata {
        name: EXTN_NAME.into(),
        symbols: vec![dist_meta],
    };
    extn_metadata.into()
}

export_extn_metadata!(CExtnMetadata, init_library);

fn start_launcher(sender: ExtnSender, receiver: CReceiver<CExtnMessage>) {
    let _ = init_logger(EXTN_NAME.into());
    info!("Starting mock device channel");
    let runtime = Runtime::new().unwrap();
    let mut client = ExtnClient::new(receiver, sender);
    runtime.block_on(async move {
        let client_c = client.clone();
        tokio::spawn(async move {
            if let Ok(server) = boot_ws_server(client.clone()).await {
                client.add_request_processor(MockDeviceMockWebsocketServerProcessor::new(
                    client.clone(),
                    server,
                ));
            } else {
                panic!("Mock Device can only be used with platform using a WebSocket gateway")
            }

            // Lets Main know that the distributor channel is ready
            let _ = client.event(ExtnStatus::Ready);
        });
        client_c.initialize().await;
    });
}

fn build(extn_id: String) -> Result<Box<ExtnChannel>, RippleError> {
    if let Ok(id) = ExtnId::try_from(extn_id) {
        let current_id = ExtnId::new_channel(ExtnClassId::Device, EXTN_NAME.into());

        if id.eq(&current_id) {
            Ok(Box::new(ExtnChannel {
                start: start_launcher,
            }))
        } else {
            Err(RippleError::ExtnError)
        }
    } else {
        Err(RippleError::InvalidInput)
    }
}

fn init_extn_builder() -> ExtnChannelBuilder {
    ExtnChannelBuilder {
        build,
        service: EXTN_NAME.into(),
    }
}

export_channel_builder!(ExtnChannelBuilder, init_extn_builder);
