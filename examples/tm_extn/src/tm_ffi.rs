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
    api::{firebolt::fb_telemetry::OperationalMetricRequest, status_update::ExtnStatus},
    async_channel::Receiver as CReceiver,
    export_extn_channel, export_extn_metadata,
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
    log::{debug, error, info},
    semver::Version,
    tokio::{self, sync::mpsc::channel},
    utils::{error::RippleError, extn_utils::ExtnUtils, logger::init_logger},
};
use tokio_tungstenite::tungstenite::{connect, Message};
use url::Url;

use crate::telemetry_processor::TelemetryProcessor;

fn init_library() -> CExtnMetadata {
    let _ = init_logger("tm".into());

    let dist_meta = ExtnSymbolMetadata::get(
        ExtnId::new_channel(ExtnClassId::Distributor, "tm".into()),
        ContractFulfiller::new(vec![RippleContract::OperationalMetricListener]),
        Version::new(1, 1, 0),
    );

    debug!("Returning tm builder");
    let extn_metadata = ExtnMetadata {
        name: "tm".into(),
        symbols: vec![dist_meta],
    };
    extn_metadata.into()
}

export_extn_metadata!(CExtnMetadata, init_library);

fn start_launcher(sender: ExtnSender, receiver: CReceiver<CExtnMessage>) {
    let _ = init_logger("tm".into());
    info!("Starting tm channel");
    let mut client = ExtnClient::new_extn(receiver);

    if !client.check_contract_permitted(RippleContract::OperationalMetricListener) {
        let _ = client.event(ExtnStatus::Error);
        return;
    }
    if let Some(ws_url) = client.get_config("ws_url") {
        info!("ws_url={}", ws_url);
        let runtime = ExtnUtils::get_runtime("e-tm".to_owned(), client.get_stack_size());

        runtime.block_on(async move {
            let client_c = client.clone();
            tokio::spawn(async move {
                if (client.request(OperationalMetricRequest::Subscribe).await).is_ok() {
                    let (tx, mut tr) = channel(3);
                    client.add_event_processor(TelemetryProcessor::new(tx));
                    if let Ok((mut socket, _r)) = connect(Url::parse(&ws_url).unwrap()) {
                        // Lets Main know that the distributor channel is ready
                        let _ = client.event(ExtnStatus::Ready);
                        while let Some(v) = tr.recv().await {
                            if let Err(e) = socket.send(Message::Text(v)) {
                                error!("Error sending to socket {:?}", e);
                            }
                        }
                    }
                } else {
                    let _ = client.event(ExtnStatus::Error);
                }
            });
            client_c.initialize().await;
        });
    } else {
        error!("no ws_url");
        let _ = client.event(ExtnStatus::Error);
    }
}

fn build(extn_id: String) -> Result<Box<ExtnChannel>, RippleError> {
    if let Ok(id) = ExtnId::try_from(extn_id) {
        let current_id = ExtnId::new_channel(ExtnClassId::Distributor, "tm".into());

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
        service: "tm".into(),
    }
}

export_extn_channel!(ExtnChannelBuilder, init_extn_channel);
