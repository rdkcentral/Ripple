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

use jsonrpsee::RpcModule;
use ripple_sdk::{
    api::firebolt::fb_openrpc::OpenRPCRegistrarRequest,
    async_trait::async_trait,
    extn::{
        client::{
            extn_client::ExtnClient,
            extn_processor::{
                DefaultExtnStreamer, ExtnRequestProcessor, ExtnStreamProcessor, ExtnStreamer,
            },
        },
        extn_client_message::{ExtnMessage, ExtnResponse},
    },
    log::error,
    tokio::sync::mpsc::{Receiver, Sender},
};

use crate::state::platform_state::PlatformState;

struct RegisteredAlias {
    method: String,
    aliases: Vec<String>,
}

pub trait RippleRPCProvider<I>
where
    I: Send + Sync + 'static,
{
    fn provide_with_alias(state: PlatformState) -> RpcModule<I> {
        let r: RpcModule<I> = Self::provide(state.clone());
        register_aliases(&state, r)
    }
    fn provide(state: PlatformState) -> RpcModule<I>;
}

pub fn register_aliases<I>(
    platform_state: &PlatformState,
    mut rpc_module: RpcModule<I>,
) -> RpcModule<I>
where
    I: std::marker::Send + 'static,
    I: std::marker::Sync,
{
    let rpc_aliases = platform_state.get_rpc_aliases();
    let mut registered_aliases = Vec::new();
    for method in rpc_module.method_names() {
        if let Some(a) = rpc_aliases.get(method) {
            registered_aliases.push(RegisteredAlias {
                method: String::from(method),
                aliases: a.clone(),
            });
        }
    }
    for registered_alias in registered_aliases {
        // JSONRpsee requires aliases to be static string so in order to make the string static
        // we need to leak it so it exists for the lifetime of the running application
        let existing_method = Box::leak(registered_alias.method.into_boxed_str());
        for a in registered_alias.aliases {
            if rpc_module
                .register_alias(Box::leak(a.clone().into_boxed_str()), existing_method)
                .is_err()
            {
                error!(
                    "Error registering alias {} for method {}",
                    a, existing_method
                );
            }
        }
    }
    rpc_module
}

// <pca>
pub struct OpenRPCSchemaRegistrar {
    platform_state: PlatformState,
    streamer: DefaultExtnStreamer,
}

impl OpenRPCSchemaRegistrar {
    pub fn new(platform_state: PlatformState) -> OpenRPCSchemaRegistrar {
        OpenRPCSchemaRegistrar {
            platform_state,
            streamer: DefaultExtnStreamer::new(),
        }
    }
}

impl ExtnStreamProcessor for OpenRPCSchemaRegistrar {
    type STATE = PlatformState;
    type VALUE = OpenRPCRegistrarRequest;

    fn get_state(&self) -> Self::STATE {
        self.platform_state.clone()
    }

    fn sender(&self) -> Sender<ExtnMessage> {
        self.streamer.sender()
    }

    fn receiver(&mut self) -> Receiver<ExtnMessage> {
        self.streamer.receiver()
    }
}

#[async_trait]
impl ExtnRequestProcessor for OpenRPCSchemaRegistrar {
    fn get_client(&self) -> ExtnClient {
        self.platform_state.get_client().get_extn_client()
    }

    async fn process_request(
        state: Self::STATE,
        msg: ExtnMessage,
        extracted_message: Self::VALUE,
    ) -> bool {
        let client = state.get_client().get_extn_client();
        match extracted_message {
            OpenRPCRegistrarRequest::Register(path) => {
                let result = match state.open_rpc_state.add_extension_open_rpc(path) {
                    Ok(_) => ExtnResponse::None(()),
                    Err(e) => ExtnResponse::Error(e),
                };
                Self::respond(client, msg, result).await.is_ok()
            }
        }
    }
}
// </pca>
