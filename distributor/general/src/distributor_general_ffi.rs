// If not stated otherwise in this file or this component's license file the
// following copyright and licenses apply:
//
// Copyright 2023 RDK Management
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
    framework::ripple_contract::{ContractFulfiller, DistributorContract, RippleContract},
    log::{debug, info},
    semver::Version,
    tokio::runtime::Runtime,
    utils::{error::RippleError, logger::init_logger},
};

use crate::{
    general_permission_processor::DistributorPermissionProcessor,
    general_session_processor::DistributorSessionProcessor,
};

fn init_library() -> CExtnMetadata {
    let _ = init_logger("distributor_general".into());

    let dist_meta = ExtnSymbolMetadata::get(
        ExtnId::new_channel(ExtnClassId::Distributor, "general".into()),
        ContractFulfiller::new(vec![
            RippleContract::Distributor(DistributorContract::Permissions),
            RippleContract::Session,
        ]),
        Version::new(1, 1, 0),
    );

    debug!("Returning distributor builder");
    let extn_metadata = ExtnMetadata {
        name: "distributor_general".into(),
        symbols: vec![dist_meta],
    };
    extn_metadata.into()
}

export_extn_metadata!(CExtnMetadata, init_library);

fn start_launcher(sender: ExtnSender, receiver: CReceiver<CExtnMessage>) {
    let _ = init_logger("distributor_general".into());
    info!("Starting distributor channel");
    let runtime = Runtime::new().unwrap();
    let mut client = ExtnClient::new(receiver.clone(), sender);
    runtime.block_on(async move {
        client.add_request_processor(DistributorSessionProcessor::new(client.clone()));
        client.add_request_processor(DistributorPermissionProcessor::new(client.clone()));
        // Lets Main know that the distributor channel is ready
        let _ = client.event(ExtnStatus::Ready).await;
        client.initialize().await;
    });
}

fn build(extn_id: String) -> Result<Box<ExtnChannel>, RippleError> {
    if let Ok(id) = ExtnId::try_from(extn_id.clone()) {
        let current_id = ExtnId::new_channel(ExtnClassId::Distributor, "general".into());

        if id.eq(&current_id) {
            return Ok(Box::new(ExtnChannel {
                start: start_launcher,
            }));
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
        service: "distributor_general".into(),
    }
}

export_channel_builder!(ExtnChannelBuilder, init_extn_builder);
