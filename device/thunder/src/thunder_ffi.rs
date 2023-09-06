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

use thunder_ripple_sdk::ripple_sdk::{
    api::{session::EventAdjective, storage_property::StorageAdjective},
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

use crate::bootstrap::boot_thunder_channel::boot_thunder_channel;

fn init_library() -> CExtnMetadata {
    let _ = init_logger("device_channel".into());
    let thunder_channel_meta = ExtnSymbolMetadata::get(
        ExtnId::new_channel(ExtnClassId::Device, "thunder".into()),
        ContractFulfiller::new(vec![
            RippleContract::DeviceInfo,
            RippleContract::WindowManager,
            RippleContract::Browser,
            RippleContract::DeviceEvents(EventAdjective::Input),
            RippleContract::DeviceEvents(EventAdjective::Hdr),
            RippleContract::DeviceEvents(EventAdjective::ScreenResolution),
            RippleContract::DeviceEvents(EventAdjective::VideoResolution),
            RippleContract::DeviceEvents(EventAdjective::VoiceGuidance),
            RippleContract::DeviceEvents(EventAdjective::Network),
            RippleContract::DeviceEvents(EventAdjective::Audio),
            RippleContract::DeviceEvents(EventAdjective::SystemPowerState),
            RippleContract::Storage(StorageAdjective::Local),
            RippleContract::RemoteAccessory,
            RippleContract::Wifi,
        ]),
        Version::new(1, 1, 0),
    );

    debug!("Returning thunder library entries");
    let extn_metadata = ExtnMetadata {
        name: "thunder".into(),
        symbols: vec![thunder_channel_meta],
    };
    extn_metadata.into()
}
export_extn_metadata!(CExtnMetadata, init_library);

pub fn start(sender: ExtnSender, receiver: CReceiver<CExtnMessage>) {
    let _ = init_logger("device_channel".into());
    info!("Starting device channel");
    let runtime = Runtime::new().unwrap();
    let client = ExtnClient::new(receiver, sender);
    runtime.block_on(async move {
        let client_for_receiver = client.clone();
        let client_for_thunder = client.clone();
        tokio::spawn(async move { boot_thunder_channel(client_for_thunder).await });
        client_for_receiver.initialize().await;
    });
}

fn build(extn_id: String) -> Result<Box<ExtnChannel>, RippleError> {
    if let Ok(id) = ExtnId::try_from(extn_id) {
        let current_id = ExtnId::new_channel(ExtnClassId::Device, "thunder".into());

        if id.eq(&current_id) {
            Ok(Box::new(ExtnChannel { start }))
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
        service: "thunder".into(),
    }
}

export_channel_builder!(ExtnChannelBuilder, init_extn_builder);
