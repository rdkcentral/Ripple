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
    api::{manifest::extn_manifest::ExtnSymbol, status_update::ExtnStatus}, async_channel::Receiver as CReceiver, export_channel_builder, export_extn_metadata, extn::{
        client::{extn_client::ExtnClient, extn_sender::ExtnSender},
        extn_id::{ExtnClassId, ExtnId, ExtnProviderAdjective},
        ffi::{
            ffi_channel::{ExtnChannel, ExtnChannelBuilder},
            ffi_library::{CExtnMetadata, ExtnMetadata, ExtnSymbolMetadata},
            ffi_message::CExtnMessage,
        },
    }, framework::ripple_contract::{ContractFulfiller, RippleContract}, log::{debug, info}, processor::rpc_request_processor::RPCRequestProcessor, semver::Version, tokio::{self, runtime::Runtime}, utils::{error::RippleError, logger::init_logger}
};

use crate::{
    mock_device_controller::{MockDeviceController, MockDeviceControllerServer},
    mock_device_processor::MockDeviceProcessor,
    utils::boot_ws_server,
};

pub const EXTN_NAME: &str = "mock_device";

fn init_library() -> CExtnMetadata {
    let _ = init_logger(EXTN_NAME.into());
    let id = ExtnId::new_channel(ExtnClassId::Device, EXTN_NAME.into());
    let mock_device_channel = ExtnSymbolMetadata::get(
        id.clone(),
        ContractFulfiller::new(vec![RippleContract::ExtnProvider(ExtnProviderAdjective {
            id,
        })]),
        Version::new(1, 0, 0),
    );


    debug!("Returning mock_device metadata builder");
    let extn_metadata = ExtnMetadata {
        name: EXTN_NAME.into(),
        symbols: vec![mock_device_channel],
    };

    extn_metadata.into()
}

export_extn_metadata!(CExtnMetadata, init_library);

fn start_launcher() {
    let _ = init_logger(EXTN_NAME.into());
    info!("Starting mock device channel");
    let runtime = Runtime::new().unwrap();
    let symbol = ExtnSymbol{
        id: format!("{}",ExtnId::new_channel(ExtnClassId::Distributor, EXTN_NAME.to_string())),
        uses: Vec::new(),
        fulfills: Vec::new(), 
        config: Some(HashMap::new())
    };
    let mut client = ExtnClient::new_extn(symbol);
    runtime.block_on(async move {
        let client_c = client.clone();
        tokio::spawn(async move {
            match boot_ws_server(client.clone()).await {
                Ok(server) => {
                    client.add_request_processor(MockDeviceProcessor::new(client.clone(), server));
                    let mut methods = Methods::new();
                    let _ = methods.merge(MockDeviceController::new(client.clone()).into_rpc());
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


#[cfg(test)]
mod tests {
    use serde_json::json;


    use super::*;

    #[test]
    fn test_init_library() {
        assert_eq!(
            init_library(),
            CExtnMetadata {
                name: "mock_device".to_owned(),
                metadata: json!([
                    {"fulfills": json!([json!({"extn_provider": "ripple:channel:device:mock_device"}).to_string()]).to_string(), "id": "ripple:channel:device:mock_device", "required_version": "1.0.0"},
                    {"fulfills": json!([json!("json_rpsee").to_string()]).to_string(), "id": "ripple:extn:jsonrpsee:mock_device", "required_version": "1.0.0"}
                    ])
                    .to_string()
            }
        )
    }

}
