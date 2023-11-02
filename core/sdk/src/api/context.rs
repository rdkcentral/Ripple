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

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RippleContext {
    pub activation_status: ActivationStatus,
    pub internet_connectivity: InternetConnectionStatus,
    pub system_power_state: SystemPowerState,
    pub time_zone: TimeZone,
    pub update_type: Option<RippleContextUpdateType>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
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

    pub fn update(&mut self, request: RippleContextUpdateRequest) {
        match request {
            RippleContextUpdateRequest::Activation(a) => {
                self.activation_status = a.into();
                self.update_type = Some(RippleContextUpdateType::ActivationStatusChanged);
            }
            RippleContextUpdateRequest::InternetStatus(s) => {
                self.internet_connectivity = s;
                self.update_type = Some(RippleContextUpdateType::InternetConnectionChanged);
            }
            RippleContextUpdateRequest::Token(t) => {
                self.activation_status = t.into();
                self.update_type = Some(RippleContextUpdateType::TokenChanged);
            }
            RippleContextUpdateRequest::PowerState(p) => {
                self.system_power_state = p;
                self.update_type = Some(RippleContextUpdateType::PowerStateChanged)
            }
            RippleContextUpdateRequest::TimeZone(tz) => {
                self.time_zone = tz;
                self.update_type = Some(RippleContextUpdateType::TimeZoneChanged)
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

impl Default for RippleContext {
    fn default() -> Self {
        RippleContext {
            activation_status: ActivationStatus::NotActivated,
            internet_connectivity: InternetConnectionStatus::NoInternet,
            update_type: None,
            system_power_state: SystemPowerState::default(),
            time_zone: TimeZone::default(),
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RippleContextUpdateRequest {
    Activation(bool),
    Token(AccountToken),
    InternetStatus(InternetConnectionStatus),
    PowerState(SystemPowerState),
    TimeZone(TimeZone),
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
