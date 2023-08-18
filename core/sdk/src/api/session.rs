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

use crate::{
    extn::extn_client_message::{ExtnPayload, ExtnPayloadProvider, ExtnRequest, ExtnResponse},
    framework::ripple_contract::{ContractAdjective, RippleContract},
};

pub fn deserialize_expiry<'de, D>(deserializer: D) -> Result<Expiry, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = u32::deserialize(deserializer)?;
    if value < 1 {
        Err(serde::de::Error::custom(
            "Invalid value for expiresIn. Minimum value should be 1",
        ))
    } else {
        Ok(value)
    }
}

type Expiry = u32;
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AccountSessionTokenRequest {
    pub token: String,
    #[serde(default, deserialize_with = "deserialize_expiry")]
    pub expires_in: Expiry,
}

#[derive(Serialize, Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProvisionRequest {
    pub account_id: String,
    pub device_id: String,
    pub distributor_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum AccountSessionRequest {
    Get,
    Provision(ProvisionRequest),
    SetAccessToken(AccountSessionTokenRequest),
}

impl ExtnPayloadProvider for AccountSessionRequest {
    fn get_from_payload(payload: ExtnPayload) -> Option<Self> {
        if let ExtnPayload::Request(ExtnRequest::AccountSession(v)) = payload {
            return Some(v);
        }

        None
    }

    fn get_extn_payload(&self) -> ExtnPayload {
        ExtnPayload::Request(ExtnRequest::AccountSession(self.clone()))
    }

    fn contract() -> RippleContract {
        RippleContract::Session(SessionAdjective::Account)
    }
}

#[repr(C)]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AccountSession {
    pub id: String,
    pub token: String,
    pub account_id: String,
    pub device_id: String,
}

impl ExtnPayloadProvider for AccountSession {
    fn get_from_payload(payload: ExtnPayload) -> Option<Self> {
        if let ExtnPayload::Response(ExtnResponse::AccountSession(v)) = payload {
            return Some(v);
        }

        None
    }

    fn get_extn_payload(&self) -> ExtnPayload {
        ExtnPayload::Response(ExtnResponse::AccountSession(self.clone()))
    }

    fn contract() -> RippleContract {
        RippleContract::Session(SessionAdjective::Account)
    }
}

pub struct OptionalAccountSession {
    pub id: Option<String>,
    pub token: Option<String>,
    pub account_id: Option<String>,
    pub device_id: Option<String>,
}

impl AccountSession {
    pub fn get_only_id(&self) -> OptionalAccountSession {
        OptionalAccountSession {
            id: Some(self.id.clone()),
            token: None,
            account_id: None,
            device_id: None,
        }
    }
}

/**
 * https://developer.comcast.com/firebolt/core/sdk/latest/api/authentication
 */
/*TokenType and Token are Firebolt spec types */
#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub enum TokenType {
    #[serde(rename = "platform")]
    Platform,
    #[serde(rename = "device")]
    Device,
    #[serde(rename = "distributor")]
    Distributor,
    #[serde(rename = "root")]
    Root,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenContext {
    pub distributor_id: String,
    pub app_id: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SessionTokenRequest {
    pub token_type: TokenType,
    pub options: Vec<String>,
    pub context: Option<TokenContext>,
}

impl ExtnPayloadProvider for SessionTokenRequest {
    fn get_extn_payload(&self) -> ExtnPayload {
        ExtnPayload::Request(ExtnRequest::SessionToken(self.clone()))
    }

    fn get_from_payload(payload: ExtnPayload) -> Option<SessionTokenRequest> {
        if let ExtnPayload::Request(ExtnRequest::SessionToken(r)) = payload {
            return Some(r);
        }

        None
    }

    fn get_contract(&self) -> RippleContract {
        match self.token_type {
            TokenType::Root => RippleContract::Session(SessionAdjective::Root),
            TokenType::Device => RippleContract::Session(SessionAdjective::Device),
            TokenType::Platform => RippleContract::Session(SessionAdjective::Platform),
            TokenType::Distributor => RippleContract::Session(SessionAdjective::Distributor),
        }
    }

    fn contract() -> RippleContract {
        RippleContract::Session(SessionAdjective::Device)
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub enum SessionAdjective {
    Account,
    Platform,
    Device,
    Distributor,
    Root,
}

impl ContractAdjective for SessionAdjective {
    fn get_contract(&self) -> RippleContract {
        RippleContract::Session(self.clone())
    }
}
