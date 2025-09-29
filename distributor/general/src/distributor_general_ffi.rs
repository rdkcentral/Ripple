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
    api::{config::Config, status_update::ExtnStatus, storage_property::StorageAdjective},
    async_channel::Receiver as CReceiver,
    export_extn_channel, export_extn_metadata,
    extn::{
        client::{extn_client::ExtnClient, extn_sender::ExtnSender},
        extn_client_message::ExtnResponse,
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

use crate::general_permission_processor::DistributorPermissionProcessor;

fn init_library() -> CExtnMetadata {
    let _ = init_logger("distributor_general".into());

    let dist_meta = ExtnSymbolMetadata::get(
        ExtnId::new_channel(ExtnClassId::Distributor, "general".into()),
        ContractFulfiller::new(vec![
            RippleContract::Permissions,
            RippleContract::Storage(StorageAdjective::Secure),
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
    let mut client: ExtnClient = ExtnClient::new(receiver, sender);
    let runtime = ExtnUtils::get_runtime("e-dg".to_owned(), client.get_stack_size());
    runtime.block_on(async move {
        let client_c = client.clone();
        tokio::spawn(async move {
            if let Ok(response) = client.request(Config::SavedDir).await {
                if let Some(ExtnResponse::String(_value)) = response.payload.extract() {
                    // Stubbed out - DistributorPrivacyProcessor replaced with direct RPC calls
                    // Privacy operations now handled by distributor RPC API
                    debug!("Privacy processor stubbed out - using direct RPC calls instead");
                    // client.add_request_processor(DistributorPrivacyProcessor::new(
                    //     client.clone(),
                    //     value.clone(),
                    // ));
                }
            }

            client.add_request_processor(DistributorPermissionProcessor::new(client.clone()));
            // Lets Main know that the distributor channel is ready
            let _ = client.event(ExtnStatus::Ready);
        });
        client_c.initialize().await;
    });
}

fn build(extn_id: String) -> Result<Box<ExtnChannel>, RippleError> {
    if let Ok(id) = ExtnId::try_from(extn_id) {
        let current_id = ExtnId::new_channel(ExtnClassId::Distributor, "general".into());

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

fn get_extended_capabilities() -> Option<String> {
    None
}

fn init_extn_channel() -> ExtnChannelBuilder {
    ExtnChannelBuilder {
        get_extended_capabilities,
        build,
        service: "distributor_general".into(),
    }
}

export_extn_channel!(ExtnChannelBuilder, init_extn_channel);
