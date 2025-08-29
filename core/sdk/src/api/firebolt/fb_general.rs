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

use serde::{Deserialize, Serialize};

use crate::api::gateway::rpc_gateway_api::CallContext;

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct ListenRequest {
    pub listen: bool,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct ListenRequestWithEvent {
    pub event: String,
    pub request: ListenRequest,
    pub context: CallContext,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ListenerResponse {
    pub listening: bool,
    pub event: String,
}

#[derive(Clone, Debug, Hash, Eq, PartialEq, Deserialize, Serialize, Default)]
pub enum AgePolicyIdentifierAlias {
    //app:child
    AppChild,
    //app:teen
    AppTeen,
    //app:adult
    AppAdult,
    //completeness
    #[default]
    AppUnknown,
}

impl From<Option<String>> for AgePolicyIdentifierAlias {
    fn from(s: Option<String>) -> Self {
        match s {
            Some(s) => match s.to_lowercase().as_str() {
                "app:child" => AgePolicyIdentifierAlias::AppChild,
                "app:teen" => AgePolicyIdentifierAlias::AppTeen,
                "app:adult" => AgePolicyIdentifierAlias::AppAdult,
                _ => AgePolicyIdentifierAlias::AppUnknown,
            },
            None => AgePolicyIdentifierAlias::AppUnknown,
        }
    }
}

impl AgePolicyIdentifierAlias {
    pub fn as_str(&self) -> &str {
        match self {
            AgePolicyIdentifierAlias::AppChild => "app:child",
            AgePolicyIdentifierAlias::AppTeen => "app:teen",
            AgePolicyIdentifierAlias::AppUnknown => "app:unknown",
            AgePolicyIdentifierAlias::AppAdult => "app:adult",
        }
    }
    pub fn from_string(s: Option<String>) -> Option<AgePolicyIdentifierAlias> {
        s.map(|s| match s.to_lowercase().as_str() {
            "app:child" => AgePolicyIdentifierAlias::AppChild,
            "app:teen" => AgePolicyIdentifierAlias::AppTeen,
            "app:adult" => AgePolicyIdentifierAlias::AppAdult,
            _ => AgePolicyIdentifierAlias::AppUnknown,
        })
    }
}

impl std::fmt::Display for AgePolicyIdentifierAlias {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}
