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
    async_channel::Receiver,
    export_extn_metadata, export_jsonrpc_extn_builder,
    extn::{
        client::{extn_client::ExtnClient, extn_sender::ExtnSender},
        extn_id::{ExtnClassId, ExtnId},
        ffi::{
            ffi_jsonrpsee::JsonRpseeExtnBuilder,
            ffi_library::{CExtnMetadata, ExtnMetadata, ExtnSymbolMetadata},
            ffi_message::CExtnMessage,
        },
    },
    framework::ripple_contract::{ContractFulfiller, RippleContract},
    log::debug,
    semver::Version,
    utils::logger::init_logger,
};

use crate::rpc::{
    custom_jsonrpsee_extn::{CustomImpl, CustomServer},
    legacy_jsonrpsee_extn::{LegacyImpl, LegacyServer},
};

fn init_library() -> CExtnMetadata {
    let _ = init_logger("rpc_extn".into());

    let json_rpsee_extn_meta = ExtnSymbolMetadata::get(
        ExtnId::new_extn(ExtnClassId::Jsonrpsee, "custom".into()),
        ContractFulfiller::new(vec![RippleContract::JsonRpsee]),
        Version::new(1, 1, 0),
    );

    debug!("Returning extended custom library entries");
    ExtnMetadata::new(
        "custom".into(),
        vec![json_rpsee_extn_meta],
    ).into()
}

export_extn_metadata!(CExtnMetadata, init_library);

fn get_rpc_extns(sender: ExtnSender, receiver: Receiver<CExtnMessage>) -> Methods {
    let mut methods = Methods::new();
    let client = ExtnClient::new(receiver, sender);
    let _ = methods.merge(CustomImpl::new(client.clone()).into_rpc());
    let _ = methods.merge(LegacyImpl::new(client).into_rpc());
    methods
}

fn get_extended_capabilities() -> Option<String> {
    Some(String::from(std::include_str!("./extended-open-rpc.json")))
}

fn init_jsonrpsee_builder() -> JsonRpseeExtnBuilder {
    JsonRpseeExtnBuilder {
        get_extended_capabilities,
        build: get_rpc_extns,
        service: "custom".into(),
    }
}

export_jsonrpc_extn_builder!(JsonRpseeExtnBuilder, init_jsonrpsee_builder);
