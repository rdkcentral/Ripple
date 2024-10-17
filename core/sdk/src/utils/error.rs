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

use crate::api::firebolt::fb_capabilities::DenyReason;

#[derive(Debug, PartialEq, Clone, Deserialize, Serialize)]
pub enum RippleError {
    MissingInput,
    InvalidInput,
    InvalidOutput,
    SenderMissing,
    SendFailure,
    ApiAuthenticationFailed,
    ExtnError,
    BootstrapError,
    ParseError,
    ProcessorError,
    ClientMissing,
    NoResponse,
    InvalidAccess,
    Permission(DenyReason),
    ServiceError,
    NotAvailable,
    RuleError,
    ServiceNotReady,
    BrokerError(String),
}

impl std::fmt::Display for RippleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RippleError::MissingInput => write!(f, "MissingInput"),
            RippleError::InvalidInput => write!(f, "InvalidInput"),
            RippleError::InvalidOutput => write!(f, "InvalidOutput"),
            RippleError::SenderMissing => write!(f, "SenderMissing"),
            RippleError::SendFailure => write!(f, "SendFailure"),
            RippleError::ApiAuthenticationFailed => write!(f, "ApiAuthenticationFailed"),
            RippleError::ExtnError => write!(f, "ExtnError"),
            RippleError::BootstrapError => write!(f, "BootstrapError"),
            RippleError::ParseError => write!(f, "ParseError"),
            RippleError::ProcessorError => write!(f, "ProcessorError"),
            RippleError::ClientMissing => write!(f, "ClientMissing"),
            RippleError::NoResponse => write!(f, "NoResponse"),
            RippleError::InvalidAccess => write!(f, "InvalidAccess"),
            RippleError::Permission(p) => write!(f, "Permission {}", p),
            RippleError::ServiceError => write!(f, "ServiceError"),
            RippleError::NotAvailable => write!(f, "NotAvailable"),
            RippleError::RuleError => write!(f, "RuleError"),
            RippleError::ServiceNotReady => write!(f, "ServiceNotReady"),
            RippleError::BrokerError(msg) => {
                let msg = format!("BrokerError {}", msg);
                write!(f, "{}", msg)
            }
        }
    }
}

#[cfg(feature = "rpc")]
impl From<RippleError> for jsonrpsee::core::Error {
    fn from(value: RippleError) -> Self {
        jsonrpsee::core::Error::Custom(format!("{}", value))
    }
}
#[cfg(all(test, feature = "rpc"))]
mod tests {
    use super::*;
    fn custom_error_match(expected: &str, error: jsonrpsee::core::Error) {
        if let jsonrpsee::core::Error::Custom(e) = error {
            assert_eq!(expected, e);
        } else {
            unreachable!("{}", " non error passed");
        }
    }
    #[test]
    pub fn test_type_converter() {
        custom_error_match("MissingInput", RippleError::MissingInput.into());
        custom_error_match("InvalidInput", RippleError::InvalidInput.into());
        custom_error_match("InvalidOutput", RippleError::InvalidOutput.into());
        custom_error_match("SenderMissing", RippleError::SenderMissing.into());
        custom_error_match("SendFailure", RippleError::SendFailure.into());
        custom_error_match(
            "ApiAuthenticationFailed",
            RippleError::ApiAuthenticationFailed.into(),
        );
        custom_error_match("ExtnError", RippleError::ExtnError.into());
        custom_error_match("BootstrapError", RippleError::BootstrapError.into());
        custom_error_match("ParseError", RippleError::ParseError.into());
        custom_error_match("ProcessorError", RippleError::ProcessorError.into());
        custom_error_match("ClientMissing", RippleError::ClientMissing.into());
        custom_error_match("NoResponse", RippleError::NoResponse.into());
        custom_error_match("InvalidAccess", RippleError::InvalidAccess.into());
        custom_error_match(
            "Permission AppNotInActiveState",
            RippleError::Permission(DenyReason::AppNotInActiveState).into(),
        );
        custom_error_match(
            "Permission Disabled",
            RippleError::Permission(DenyReason::Disabled).into(),
        );
        custom_error_match(
            "Permission GrantDenied",
            RippleError::Permission(DenyReason::GrantDenied).into(),
        );
        custom_error_match(
            "Permission GrantProviderMissing",
            RippleError::Permission(DenyReason::GrantProviderMissing).into(),
        );
        custom_error_match(
            "Permission NotFound",
            RippleError::Permission(DenyReason::NotFound).into(),
        );
        custom_error_match(
            "Permission Unavailable",
            RippleError::Permission(DenyReason::Unavailable).into(),
        );
        custom_error_match(
            "Permission Ungranted",
            RippleError::Permission(DenyReason::Ungranted).into(),
        );
        custom_error_match(
            "Permission Unpermitted",
            RippleError::Permission(DenyReason::Unpermitted).into(),
        );
        custom_error_match(
            "Permission Unsupported",
            RippleError::Permission(DenyReason::Unsupported).into(),
        );
    }
}
