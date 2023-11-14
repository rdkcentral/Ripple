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
    async_channel::Receiver,
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
    tokio,
    utils::{error::RippleError, extn_utils::ExtnUtils, logger::init_logger},
};

use crate::{
    launcher_lifecycle_processor::LauncherLifecycleEventProcessor, launcher_state::LauncherState,
};

fn init_library() -> CExtnMetadata {
    let _ = init_logger("launcher".into());

    let launcher_meta = ExtnSymbolMetadata::get(
        ExtnId::new_channel(ExtnClassId::Launcher, "internal".into()),
        ContractFulfiller::new(vec![RippleContract::Launcher]),
        Version::new(1, 1, 0),
    );

    debug!("Returning launcher builder");
    let extn_metadata = ExtnMetadata {
        name: "launcher".into(),
        symbols: vec![launcher_meta],
    };
    extn_metadata.into()
}

export_extn_metadata!(CExtnMetadata, init_library);

fn start_launcher(sender: ExtnSender, receiver: Receiver<CExtnMessage>) {
    let _ = init_logger("launcher".into());
    info!("Starting launcher channel");
    let client = ExtnClient::new(receiver, sender);
    let runtime = ExtnUtils::get_runtime("e-l".to_owned(), client.get_stack_size());
    let client_for_receiver = client.clone();
    runtime.block_on(async move {
        tokio::spawn(async move {
            // create state
            let state = LauncherState::new(client.clone())
                .await
                .expect("state initialization to succeed");
            // Create a client for processors
            let mut client_for_processor = client.clone();

            // All Lifecyclemanagement events will come through this processor
            client_for_processor.add_event_processor(LauncherLifecycleEventProcessor::new(state));

            // Lets Main know that the launcher is ready
            let _ = client_for_processor.event(ExtnStatus::Ready);
        });
        client_for_receiver.initialize().await;
    });
}

fn build(extn_id: String) -> Result<Box<ExtnChannel>, RippleError> {
    if let Ok(id) = ExtnId::try_from(extn_id) {
        let current_id = ExtnId::new_channel(ExtnClassId::Launcher, "internal".into());

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
        service: "launcher".into(),
    }
}

export_channel_builder!(ExtnChannelBuilder, init_extn_builder);
