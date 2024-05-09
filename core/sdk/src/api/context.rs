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

use super::{
    device::device_request::{AccountToken, InternetConnectionStatus, SystemPowerState, TimeZone},
    firebolt::fb_metrics::MetricsContext,
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
    pub features: Vec<String>,
    pub metrics_context: Option<MetricsContext>,
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub enum RippleContextUpdateType {
    ActivationStatusChanged,
    TokenChanged,
    InternetConnectionChanged,
    PowerStateChanged,
    TimeZoneChanged,
    FeaturesChanged,
    MetricsContextChanged,
}

impl RippleContext {
    pub fn new(
        activation_status: Option<ActivationStatus>,
        internet_connectivity: Option<InternetConnectionStatus>,
        system_power_state: Option<SystemPowerState>,
        time_zone: Option<TimeZone>,
        update_type: Option<RippleContextUpdateType>,
        features: Vec<String>,
        metrics_context: Option<MetricsContext>,
    ) -> RippleContext {
        RippleContext {
            activation_status,
            internet_connectivity,
            system_power_state,
            time_zone,
            update_type,
            features,
            metrics_context,
        }
    }

    pub fn is_ripple_context(msg: &ExtnPayload) -> Option<Self> {
        RippleContext::get_from_payload(msg.clone())
    }

    pub fn update(&mut self, request: RippleContextUpdateRequest) -> bool {
        match request {
            RippleContextUpdateRequest::Activation(a) => {
                let activation_status: ActivationStatus = a.into();
                if let Some(status) = self.activation_status.as_ref() {
                    if status == &activation_status {
                        return false;
                    }
                }
                self.activation_status = Some(activation_status);
                self.update_type = Some(RippleContextUpdateType::ActivationStatusChanged);
                true
            }
            RippleContextUpdateRequest::InternetStatus(s) => {
                if let Some(internet_connectivity) = self.internet_connectivity.as_ref() {
                    if internet_connectivity == &s {
                        return false;
                    }
                }
                self.internet_connectivity = Some(s);
                self.update_type = Some(RippleContextUpdateType::InternetConnectionChanged);
                true
            }
            RippleContextUpdateRequest::Token(account_token) => {
                if let Some(activation_status) = self.activation_status.as_ref() {
                    if matches!(activation_status, ActivationStatus::AccountToken(acc_tok) if acc_tok.token == account_token.token)
                    {
                        return false;
                    }
                }
                self.activation_status = Some(account_token.into());
                self.update_type = Some(RippleContextUpdateType::TokenChanged);
                true
            }
            RippleContextUpdateRequest::PowerState(p) => {
                if let Some(power_state) = self.system_power_state.as_ref() {
                    if power_state == &p {
                        return false;
                    }
                }
                self.system_power_state = Some(p);
                self.update_type = Some(RippleContextUpdateType::PowerStateChanged);
                true
            }
            RippleContextUpdateRequest::TimeZone(tz) => {
                if let Some(time_zone) = self.time_zone.as_ref() {
                    if time_zone == &tz {
                        return false;
                    }
                }
                self.time_zone = Some(tz);
                self.update_type = Some(RippleContextUpdateType::TimeZoneChanged);
                true
            }
            RippleContextUpdateRequest::RefreshContext(_context) => {
                false
                // This is not an update request so need not to honour it
            }
            RippleContextUpdateRequest::UpdateFeatures(features) => {
                let mut changed = false;
                for feature in features {
                    match feature.enabled {
                        true => {
                            if !self.features.contains(&feature.name) {
                                self.features.push(feature.name);
                                changed = true;
                            }
                        }
                        false => {
                            if self.features.contains(&feature.name) {
                                self.features.retain(|f| !f.eq(&feature.name));
                                changed = true;
                            }
                        }
                    }
                }

                if changed {
                    self.update_type = Some(RippleContextUpdateType::FeaturesChanged);
                }
                changed
            }
            RippleContextUpdateRequest::MetricsContext(context) => {
                if let Some(metrics_context) = self.metrics_context.as_ref() {
                    if metrics_context == &context {
                        return false;
                    }
                }
                self.metrics_context = Some(context);
                self.update_type = Some(RippleContextUpdateType::MetricsContextChanged);
                true
            }
        }
    }

    pub fn deep_copy(&mut self, context: RippleContext) {
        self.activation_status = context.activation_status;
        self.internet_connectivity = context.internet_connectivity;
        self.time_zone = context.time_zone;
        self.features = context.features;
        self.metrics_context = context.metrics_context;
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
        if self.internet_connectivity != context.internet_connectivity {
            RippleContextUpdateType::InternetConnectionChanged
        } else if self.time_zone != context.time_zone {
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
pub struct FeatureUpdate {
    name: String,
    enabled: bool,
}

impl FeatureUpdate {
    pub fn new(name: String, enabled: bool) -> FeatureUpdate {
        FeatureUpdate { name, enabled }
    }
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub enum RippleContextUpdateRequest {
    Activation(bool),
    Token(AccountToken),
    InternetStatus(InternetConnectionStatus),
    PowerState(SystemPowerState),
    TimeZone(TimeZone),
    UpdateFeatures(Vec<FeatureUpdate>),
    MetricsContext(MetricsContext),
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
    fn test_default_ripple_context() {
        let context = RippleContext::default();
        assert_eq!(context.activation_status, None);
        assert_eq!(context.internet_connectivity, None);
        assert_eq!(context.update_type, None);
        assert_eq!(context.system_power_state, None);
        assert_eq!(context.time_zone, None);
        assert_eq!(context.features, Vec::<String>::default());
        assert_eq!(context.metrics_context, None);
    }

    #[test]
    fn test_ripple_context_update() {
        let mut context = RippleContext::default();
        let request = RippleContextUpdateRequest::Activation(true);
        context.update(request);
        assert_eq!(context.activation_status, Some(ActivationStatus::Activated));
        assert_eq!(
            context.update_type,
            Some(RippleContextUpdateType::ActivationStatusChanged)
        );
    }

    #[test]
    fn test_ripple_context_what_changed() {
        let context1 = RippleContext {
            internet_connectivity: Some(InternetConnectionStatus::NoInternet),
            time_zone: Some(TimeZone::default()),
            activation_status: Some(ActivationStatus::NotActivated),
            system_power_state: Some(SystemPowerState {
                power_state: PowerState::On,
                current_power_state: PowerState::On,
            }),
            update_type: None,
            features: Vec::default(),
            metrics_context: Some(MetricsContext::default()),
        };

        let context2 = RippleContext {
            internet_connectivity: Some(InternetConnectionStatus::FullyConnected),
            time_zone: Some(TimeZone::default()),
            activation_status: Some(ActivationStatus::NotActivated),
            system_power_state: Some(SystemPowerState {
                power_state: PowerState::On,
                current_power_state: PowerState::On,
            }),
            update_type: None,
            features: Vec::default(),
            metrics_context: Some(MetricsContext::default()),
        };

        assert_eq!(
            context1.what_changed(&context2),
            RippleContextUpdateType::InternetConnectionChanged
        );
    }

    #[test]
    fn test_extn_request_ripple_context_update() {
        let activation_request = RippleContextUpdateRequest::Activation(true);
        let contract_type: RippleContract = RippleContract::RippleContext;
        test_extn_payload_provider(activation_request, contract_type);
    }

    #[test]
    fn test_extn_payload_provider_for_ripple_context() {
        let ripple_context = RippleContext {
            activation_status: Some(ActivationStatus::NotActivated),
            internet_connectivity: Some(InternetConnectionStatus::FullyConnected),
            system_power_state: Some(SystemPowerState {
                power_state: PowerState::On,
                current_power_state: PowerState::On,
            }),
            time_zone: Some(TimeZone {
                time_zone: String::from("America/Los_Angeles"),
                offset: -28800,
            }),
            update_type: None,
            features: Vec::default(),
            metrics_context: Some(MetricsContext::default()),
        };

        let contract_type: RippleContract = RippleContract::RippleContext;
        test_extn_payload_provider(ripple_context, contract_type);
    }

    #[test]
    fn test_update_features_enabled_not_exists() {
        let name = String::from("foo");
        let some_other_feature = String::from("bar");
        let mut ripple_context = RippleContext::new(
            None,
            None,
            None,
            None,
            None,
            vec![some_other_feature.clone()],
            None,
        );
        let changed = ripple_context.update(RippleContextUpdateRequest::UpdateFeatures(vec![
            FeatureUpdate::new(name.clone(), true),
        ]));
        assert!(changed);
        assert!(ripple_context.features.contains(&name));
        assert!(ripple_context.features.contains(&some_other_feature));
    }

    #[test]
    fn test_update_features_enabled_exists() {
        let name = String::from("foo");
        let some_other_feature = String::from("bar");
        let mut ripple_context = RippleContext::new(
            None,
            None,
            None,
            None,
            None,
            vec![some_other_feature.clone()],
            None,
        );
        ripple_context.update(RippleContextUpdateRequest::UpdateFeatures(vec![
            FeatureUpdate::new(name.clone(), true),
        ]));
        let changed = ripple_context.update(RippleContextUpdateRequest::UpdateFeatures(vec![
            FeatureUpdate::new(name.clone(), true),
        ]));
        assert!(!changed);
        assert!(ripple_context.features.contains(&name));
        assert!(ripple_context.features.contains(&some_other_feature));
    }

    #[test]
    fn test_update_features_disabled_not_exists() {
        let name = String::from("foo");
        let some_other_feature = String::from("bar");
        let mut ripple_context = RippleContext::new(
            None,
            None,
            None,
            None,
            None,
            vec![some_other_feature.clone()],
            None,
        );
        let changed = ripple_context.update(RippleContextUpdateRequest::UpdateFeatures(vec![
            FeatureUpdate::new(name.clone(), false),
        ]));
        assert!(!changed);
        assert!(!ripple_context.features.contains(&name));
        assert!(ripple_context.features.contains(&some_other_feature));
    }

    #[test]
    fn test_update_features_disabled_exists() {
        let name = String::from("foo");
        let some_other_feature = String::from("bar");
        let mut ripple_context = RippleContext::new(
            None,
            None,
            None,
            None,
            None,
            vec![some_other_feature.clone()],
            None,
        );
        ripple_context.update(RippleContextUpdateRequest::UpdateFeatures(vec![
            FeatureUpdate::new(name.clone(), true),
        ]));
        let changed = ripple_context.update(RippleContextUpdateRequest::UpdateFeatures(vec![
            FeatureUpdate::new(name.clone(), false),
        ]));
        assert!(changed);
        assert!(!ripple_context.features.contains(&name));
        assert!(ripple_context.features.contains(&some_other_feature));
    }
}
