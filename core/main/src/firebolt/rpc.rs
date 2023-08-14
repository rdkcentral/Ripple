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
use ripple_sdk::log::error;

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
        let mut r = Self::provide(state.clone());
        let rpc_aliases = state.get_rpc_aliases();
        let mut aliases = Vec::new();
        for method in r.method_names() {
            if let Some(a) = rpc_aliases.get(method) {
                aliases.push(RegisteredAlias {
                    method: String::from(method),
                    aliases: a.clone(),
                });
            }
        }
        for alias in aliases {
            // JSONRpsee requires aliases to be static string so in order to make the string static
            // we need to leak it so it exists for the lifetime of the running application
            let existing_method = Box::leak(alias.method.into_boxed_str());
            for a in alias.aliases {
                if r.register_alias(Box::leak(a.clone().into_boxed_str()), existing_method)
                    .is_err()
                {
                    error!(
                        "Error registering alias {} for method {}",
                        a, existing_method
                    );
                }
            }
        }
        r
    }
    fn provide(state: PlatformState) -> RpcModule<I>;
}
