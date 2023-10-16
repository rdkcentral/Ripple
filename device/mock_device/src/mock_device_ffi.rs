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

use std::sync::Arc;

use jsonrpsee::core::server::rpc_module::Methods;
use ripple_sdk::{
    api::{mock_server::MockServerAdjective, status_update::ExtnStatus},
    crossbeam::channel::Receiver as CReceiver,
    export_channel_builder, export_extn_metadata, export_jsonrpc_extn_builder,
    extn::{
        client::{extn_client::ExtnClient, extn_sender::ExtnSender},
        extn_id::{ExtnClassId, ExtnId},
        ffi::{
            ffi_channel::{ExtnChannel, ExtnChannelBuilder},
            ffi_jsonrpsee::JsonRpseeExtnBuilder,
            ffi_library::{CExtnMetadata, ExtnMetadata, ExtnSymbolMetadata},
            ffi_message::CExtnMessage,
        },
    },
    framework::ripple_contract::{ContractFulfiller, RippleContract},
    log::{debug, error, info},
    semver::Version,
    tokio::{self, runtime::Runtime, sync::RwLock},
    utils::{error::RippleError, logger::init_logger},
};

use crate::{
    mock_device_controller::{MockDeviceController, MockDeviceControllerServer},
    mock_device_processor::MockDeviceProcessor,
    utils::{boot_ws_server, load_mock_data},
};

const EXTN_NAME: &str = "mock_device";

fn init_library() -> CExtnMetadata {
    let _ = init_logger(EXTN_NAME.into());

    let mock_device_channel = ExtnSymbolMetadata::get(
        ExtnId::new_channel(ExtnClassId::Device, EXTN_NAME.into()),
        ContractFulfiller::new(vec![RippleContract::MockServer(
            MockServerAdjective::WebSocket,
        )]),
        Version::new(1, 0, 0),
    );
    let mock_device_extn = ExtnSymbolMetadata::get(
        ExtnId::new_extn(ExtnClassId::Jsonrpsee, EXTN_NAME.into()),
        ContractFulfiller::new(vec![RippleContract::JsonRpsee]),
        Version::new(1, 0, 0),
    );

    debug!("Returning mock_device metadata builder");
    let extn_metadata = ExtnMetadata {
        name: EXTN_NAME.into(),
        symbols: vec![mock_device_channel, mock_device_extn],
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
            let mock_data = load_mock_data(client.clone())
                .await
                .map_err(|e| {
                    error!("{:?}", e);
                    e
                })
                .unwrap_or_default();
            debug!("mock_data={:?}", mock_data);

            if let Ok(server) =
                boot_ws_server(client.clone(), Arc::new(RwLock::new(mock_data))).await
            {
                client.add_request_processor(MockDeviceProcessor::new(client.clone(), server));
            } else {
                // TODO: check panic message
                panic!("Mock Device can only be used with platform using a WebSocket gateway")
            }

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

fn get_rpc_extns(sender: ExtnSender, receiver: CReceiver<CExtnMessage>) -> Methods {
    let mut methods = Methods::new();
    let client = ExtnClient::new(receiver, sender);
    let _ = methods.merge(MockDeviceController::new(client).into_rpc());

    methods
}

fn get_extended_capabilities() -> Option<String> {
    None
}

fn init_jsonrpsee_builder() -> JsonRpseeExtnBuilder {
    JsonRpseeExtnBuilder {
        get_extended_capabilities,
        build: get_rpc_extns,
        service: EXTN_NAME.into(),
    }
}

export_jsonrpc_extn_builder!(JsonRpseeExtnBuilder, init_jsonrpsee_builder);

#[cfg(test)]
mod tests {

    use ripple_sdk::crossbeam::channel::unbounded;
    use serde_json::json;

    use super::*;

    #[test]
    fn test_init_library() {
        assert_eq!(
            init_library(),
            CExtnMetadata {
                name: "mock_device".to_owned(),
                metadata: json!([
                    {"fulfills": json!([json!({"mock_server": "web_socket"}).to_string()]).to_string(), "id": "ripple:channel:device:mock_device", "required_version": "1.0.0"},
                    {"fulfills": json!([json!("json_rpsee").to_string()]).to_string(), "id": "ripple:extn:jsonrpsee:mock_device", "required_version": "1.0.0"}
                    ])
                    .to_string()
            }
        )
    }

    #[test]
    fn test_init_jsonrpsee_builder() {
        let builder = init_jsonrpsee_builder();

        let (tx, receiver) = unbounded();
        let methods = (builder.build)(
            ExtnSender::new(
                tx,
                ExtnId::new_channel(ExtnClassId::Device, "mock_device".to_owned()),
                vec![],
                vec![],
                None,
            ),
            receiver,
        );

        assert_eq!(builder.service, "mock_device".to_owned());
        assert!((builder.get_extended_capabilities)().is_none());
        assert!(methods.method("mockdevice.addRequestResponse").is_some());
        assert!(methods.method("mockdevice.removeRequest").is_some());
        assert!(methods.method("mockdevice.emitEvent").is_some());
    }

    #[test]
    fn test_init_extn_builder() {
        todo!("test that the mock device web socket server is started when the channel extension is launched")
        // FIXME: Currently unable to mock the extn client responses so that the Config::PlatformParameter response works.
        // let ripple_client = RippleClient::new        client.add_request_processor(ConfigRequestProcessor::new(state.platform_state.clone()));

        // let builder = init_extn_builder();
        // let (tx, receiver) = unbounded();
        // let sender = ExtnSender::new(
        //     tx,
        //     ExtnId::new_channel(ExtnClassId::Device, "mock_device".to_owned()),
        //     vec!["config".to_owned()],
        //     vec![],
        //     Some(HashMap::from([(
        //         "mock_data_file".to_owned(),
        //         "examples/device-mock-data/mock-device.json".to_owned(),
        //     )])),
        // );
        // let channel = (builder.build)("ripple:channel:device:mock_device".to_owned());

        // assert_eq!(builder.service, "mock_device".to_owned());
        // assert!(channel.is_ok());

        // (channel.unwrap().start)(sender, receiver.clone());

        // let ready = receiver.recv();
        // assert!(ready.is_ok());
        // let message: ExtnMessage = ready.unwrap().try_into().unwrap();
        // let status: Option<ExtnStatus> = message.payload.extract();
        // assert_eq!(status, Some(ExtnStatus::Ready));
    }
}
