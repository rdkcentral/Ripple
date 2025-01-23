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

use super::device::device_request::AccountToken;

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
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AccountSessionTokenRequest {
    pub token: String,
    #[serde(default, deserialize_with = "deserialize_expiry")]
    pub expires_in: Expiry,
}

#[derive(Serialize, PartialEq, Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProvisionRequest {
    pub account_id: String,
    pub device_id: String,
    pub distributor_id: Option<String>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub enum AccountSessionRequest {
    Get,
    Provision(ProvisionRequest),
    SetAccessToken(AccountSessionTokenRequest),
    GetAccessToken,
    Subscribe,
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
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub enum AccountSessionResponse {
    AccountSession(AccountSession),
    AccountSessionToken(AccountToken),
}

#[repr(C)]
#[derive(Serialize, Deserialize, PartialEq, Clone, Default)]
pub struct AccountSession {
    pub id: String,
    pub token: String,
    pub account_id: String,
    pub device_id: String,
}
impl std::fmt::Debug for AccountSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // mask token field
        f.debug_struct("AccountSession")
            .field("id", &self.id)
            .field("account_id", &self.account_id)
            .field("device_id", &self.device_id)
            .finish()
    }
}
impl std::fmt::Display for AccountSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // mask token field
        write!(
            f,
            "AccountSession {{ id: {:?}, account_id: {:?}, device_id: {:?} }}",
            self.id, self.account_id, self.device_id
        )
    }
}
impl ExtnPayloadProvider for AccountSession {
    fn get_from_payload(payload: ExtnPayload) -> Option<Self> {
        if let ExtnPayload::Response(ExtnResponse::AccountSession(
            AccountSessionResponse::AccountSession(account_session),
        )) = payload
        {
            return Some(account_session);
        }

        None
    }

    fn get_extn_payload(&self) -> ExtnPayload {
        ExtnPayload::Response(ExtnResponse::AccountSession(
            AccountSessionResponse::AccountSession(self.clone()),
        ))
    }

    fn contract() -> RippleContract {
        RippleContract::Session(SessionAdjective::Account)
    }
}

impl ExtnPayloadProvider for AccountToken {
    fn get_from_payload(payload: ExtnPayload) -> Option<Self> {
        if let ExtnPayload::Response(ExtnResponse::AccountSession(
            AccountSessionResponse::AccountSessionToken(dist_token),
        )) = payload
        {
            return Some(dist_token);
        }

        None
    }

    fn get_extn_payload(&self) -> ExtnPayload {
        ExtnPayload::Response(ExtnResponse::AccountSession(
            AccountSessionResponse::AccountSessionToken(self.clone()),
        ))
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
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone, Copy)]
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

impl ToString for TokenType {
    fn to_string(&self) -> String {
        match self {
            TokenType::Platform => String::from("platform"),
            TokenType::Device => String::from("device"),
            TokenType::Distributor => String::from("distributor"),
            TokenType::Root => String::from("root"),
        }
    }
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct TokenContext {
    pub distributor_id: String,
    pub app_id: String,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
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

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
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

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum EventAdjective {
    Input,
    VoiceGuidance,
    Internet,
    Audio,
    SystemPowerState,
    TimeZone,
}

impl ContractAdjective for EventAdjective {
    fn get_contract(&self) -> RippleContract {
        RippleContract::DeviceEvents(self.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::config::Config;
    use crate::utils::test_utils::test_extn_payload_provider;

    #[test]
    fn test_extn_request_account_session() {
        let account_session_request = AccountSessionRequest::Get;
        let contract_type: RippleContract = RippleContract::Session(SessionAdjective::Account);
        test_extn_payload_provider(account_session_request, contract_type);
    }

    #[test]
    fn test_extn_request_account_session_none() {
        let other_payload = ExtnPayload::Request(ExtnRequest::Config(Config::DefaultApp));
        let result = AccountSessionRequest::get_from_payload(other_payload);
        assert_eq!(result, None);
    }

    #[test]
    fn test_extn_request_session_token() {
        let token_context = TokenContext {
            distributor_id: String::from("test_distributor"),
            app_id: String::from("test_app"),
        };
        let session_token_request = SessionTokenRequest {
            token_type: TokenType::Device,
            options: vec![String::from("option1"), String::from("option2")],
            context: Some(token_context),
        };

        let contract_type: RippleContract = RippleContract::Session(SessionAdjective::Device);
        test_extn_payload_provider(session_token_request, contract_type);
    }

    #[test]
    fn test_extn_request_session_token_none() {
        let other_payload =
            ExtnPayload::Request(ExtnRequest::AccountSession(AccountSessionRequest::Get));
        let result = SessionTokenRequest::get_from_payload(other_payload);
        assert_eq!(result, None);
    }

    #[test]
    fn test_extn_payload_provider_for_account_session() {
        let account_session = AccountSession {
            id: String::from("your_id"),
            token: String::from("your_token"),
            account_id: String::from("your_account_id"),
            device_id: String::from("your_device_id"),
        };

        let contract_type: RippleContract = RippleContract::Session(SessionAdjective::Account);
        test_extn_payload_provider(account_session, contract_type);
    }

    #[test]
    fn test_extn_payload_provider_for_account_token() {
        let account_token = AccountToken {
            token: String::from("your_token"),
            expires: 123456789,
        };

        let contract_type: RippleContract = RippleContract::Session(SessionAdjective::Account);
        test_extn_payload_provider(account_token, contract_type);
    }
}
