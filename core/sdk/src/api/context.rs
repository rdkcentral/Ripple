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

use crate::{
    extn::{
        extn_client_message::{
            ExtnEvent, ExtnMessage, ExtnPayload, ExtnPayloadProvider, ExtnRequest,
        },
        extn_id::ExtnId,
    },
    framework::ripple_contract::RippleContract,
};
use serde::{Deserialize, Serialize};

use super::device::device_request::{
    AccountToken, InternetConnectionStatus, SystemPowerState, TimeZone,
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ActivationStatus {
    NotActivated,
    AccountToken(AccountToken),
    Activated,
}

impl From<bool> for ActivationStatus {
    fn from(value: bool) -> Self {
        if value {
            ActivationStatus::Activated
        } else {
            ActivationStatus::NotActivated
        }
    }
}

impl From<AccountToken> for ActivationStatus {
    fn from(value: AccountToken) -> Self {
        ActivationStatus::AccountToken(value)
    }
}

// Instead of we chosing the default value, we make them as optional
// This enables us to differentiate from the default value to actual value
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct RippleContext {
    pub activation_status: Option<ActivationStatus>,
    pub internet_connectivity: Option<InternetConnectionStatus>,
    pub system_power_state: Option<SystemPowerState>,
    pub time_zone: Option<TimeZone>,
    pub update_type: Option<RippleContextUpdateType>,
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub enum RippleContextUpdateType {
    ActivationStatusChanged,
    TokenChanged,
    InternetConnectionChanged,
    PowerStateChanged,
    TimeZoneChanged,
}

impl RippleContext {
    pub fn is_ripple_context(msg: &ExtnPayload) -> Option<Self> {
        RippleContext::get_from_payload(msg.clone())
    }

    pub fn update(&mut self, request: RippleContextUpdateRequest) -> bool {
        match request {
            RippleContextUpdateRequest::Activation(a) => {
                let activation_status: ActivationStatus = a.into();
                if (self.activation_status.is_some()
                    && self.activation_status.as_ref().unwrap() != &activation_status)
                    || self.activation_status.is_none()
                {
                    self.activation_status = Some(a.into());
                    self.update_type = Some(RippleContextUpdateType::ActivationStatusChanged);
                    true
                } else {
                    false
                }
            }
            RippleContextUpdateRequest::InternetStatus(s) => {
                if (self.internet_connectivity.is_some()
                    && self.internet_connectivity.as_ref().unwrap() != &s)
                    || self.internet_connectivity.is_none()
                {
                    self.internet_connectivity = Some(s);
                    self.update_type = Some(RippleContextUpdateType::InternetConnectionChanged);
                    true
                } else {
                    false
                }
            }
            RippleContextUpdateRequest::Token(t) => {
                let account_token: ActivationStatus = t.into();
                if (self.activation_status.is_some()
                    && self.activation_status.as_ref().unwrap() != &account_token)
                    || self.activation_status.is_none()
                {
                    self.activation_status = Some(account_token);
                    self.update_type = Some(RippleContextUpdateType::TokenChanged);
                    true
                } else {
                    false
                }
            }
            RippleContextUpdateRequest::PowerState(p) => {
                if (self.system_power_state.is_some()
                    && self.system_power_state.as_ref().unwrap() != &p)
                    || self.system_power_state.is_none()
                {
                    self.system_power_state = Some(p);
                    self.update_type = Some(RippleContextUpdateType::PowerStateChanged);
                    true
                } else {
                    false
                }
            }
            RippleContextUpdateRequest::TimeZone(tz) => {
                if (self.time_zone.is_some() && self.time_zone.as_ref().unwrap() != &tz)
                    || self.time_zone.is_none()
                {
                    self.time_zone = Some(tz);
                    self.update_type = Some(RippleContextUpdateType::TimeZoneChanged);
                    true
                } else {
                    false
                }
            }
            RippleContextUpdateRequest::RefreshContext(_context) => {
                false
                // This is not an update request so need not to honour it
            }
        }
    }

    pub fn deep_copy(&mut self, context: RippleContext) {
        self.activation_status = context.activation_status;
        self.internet_connectivity = context.internet_connectivity;
        self.time_zone = context.time_zone;
    }

    pub fn get_event_message(&self) -> ExtnMessage {
        ExtnMessage {
            id: "context_update".to_owned(),
            requestor: ExtnId::get_main_target("ripple_context".to_owned()),
            target: RippleContract::RippleContext,
            target_id: None,
            payload: self.get_extn_payload(),
            callback: None,
            ts: None,
        }
    }

    pub fn what_changed(&self, context: &RippleContext) -> RippleContextUpdateType {
        if self.internet_connectivity == context.internet_connectivity {
            RippleContextUpdateType::InternetConnectionChanged
        } else if self.time_zone == context.time_zone {
            RippleContextUpdateType::TimeZoneChanged
        } else {
            RippleContextUpdateType::ActivationStatusChanged
        }
    }
}

impl ExtnPayloadProvider for RippleContext {
    fn get_extn_payload(&self) -> ExtnPayload {
        ExtnPayload::Event(ExtnEvent::Context(self.clone()))
    }

    fn get_from_payload(payload: ExtnPayload) -> Option<RippleContext> {
        if let ExtnPayload::Event(ExtnEvent::Context(r)) = payload {
            return Some(r);
        }

        None
    }

    fn contract() -> RippleContract {
        RippleContract::RippleContext
    }
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub enum RippleContextUpdateRequest {
    Activation(bool),
    Token(AccountToken),
    InternetStatus(InternetConnectionStatus),
    PowerState(SystemPowerState),
    TimeZone(TimeZone),
    RefreshContext(Option<RippleContextUpdateType>),
}

impl RippleContextUpdateRequest {
    pub fn is_ripple_context_update(msg: &ExtnPayload) -> Option<RippleContextUpdateRequest> {
        RippleContextUpdateRequest::get_from_payload(msg.clone())
    }
}

impl ExtnPayloadProvider for RippleContextUpdateRequest {
    fn get_extn_payload(&self) -> ExtnPayload {
        ExtnPayload::Request(ExtnRequest::Context(self.clone()))
    }

    fn get_from_payload(payload: ExtnPayload) -> Option<RippleContextUpdateRequest> {
        if let ExtnPayload::Request(ExtnRequest::Context(r)) = payload {
            return Some(r);
        }

        None
    }

    fn contract() -> RippleContract {
        RippleContract::RippleContext
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::device::device_request::PowerState;
    use crate::utils::test_utils::test_extn_payload_provider;

    #[test]
    fn test_extn_request_ripple_context_update() {
        let activation_request = RippleContextUpdateRequest::Activation(true);
        let contract_type: RippleContract = RippleContract::RippleContext;
        test_extn_payload_provider(activation_request, contract_type);
    }

    #[test]
    fn test_extn_payload_provider_for_ripple_context() {
        let ripple_context = RippleContext {
            activation_status: ActivationStatus::NotActivated,
            internet_connectivity: InternetConnectionStatus::FullyConnected,
            system_power_state: SystemPowerState {
                power_state: PowerState::On,
                current_power_state: PowerState::On,
            },
            time_zone: TimeZone {
                time_zone: String::from("America/Los_Angeles"),
                offset: -28800,
            },
            update_type: None,
        };

        let contract_type: RippleContract = RippleContract::RippleContext;
        test_extn_payload_provider(ripple_context, contract_type);
    }
}
