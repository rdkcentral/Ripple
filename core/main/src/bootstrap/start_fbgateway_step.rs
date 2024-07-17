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
    firebolt::{
        firebolt_gateway::FireboltGateway,
        handlers::{
            accessory_rpc::AccessoryRippleProvider, account_rpc::AccountRPCProvider,
            advertising_rpc::AdvertisingRPCProvider,
            audio_description_rpc::AudioDescriptionRPCProvider,
            authentication_rpc::AuthRPCProvider, capabilities_rpc::CapRPCProvider,
            closed_captions_rpc::ClosedcaptionsRPCProvider, device_rpc::DeviceRPCProvider,
            discovery_rpc::DiscoveryRPCProvider, keyboard_rpc::KeyboardRPCProvider,
            lcm_rpc::LifecycleManagementProvider, lifecycle_rpc::LifecycleRippleProvider,
            localization_rpc::LocalizationRPCProvider,
            metrics_management_rpc::MetricsManagementProvider, metrics_rpc::MetricsRPCProvider,
            parameters_rpc::ParametersRPCProvider, privacy_rpc::PrivacyProvider,
            profile_rpc::ProfileRPCProvider, provider_registrar::ProviderRegistrar,
            second_screen_rpc::SecondScreenRPCProvider,
            secure_storage_rpc::SecureStorageRPCProvider, user_grants_rpc::UserGrantsRPCProvider,
            voice_guidance_rpc::VoiceguidanceRPCProvider, wifi_rpc::WifiRPCProvider,
        },
        rpc::RippleRPCProvider,
    },
    service::telemetry_builder::TelemetryBuilder,
    state::{bootstrap_state::BootstrapState, platform_state::PlatformState},
};
use jsonrpsee::core::{async_trait, server::rpc_module::Methods};
use ripple_sdk::log::debug;
use ripple_sdk::{framework::bootstrap::Bootstep, utils::error::RippleError};
pub struct FireboltGatewayStep;

impl FireboltGatewayStep {
    async fn init_handlers(&self, state: PlatformState, extn_methods: Methods) -> Methods {
        let mut methods = Methods::new();

        // TODO: Ultimately this may be able to register all providers below, for now just does
        // those included by build_provider_relation_sets().
        ProviderRegistrar::register_methods(&state, &mut methods);

        let _ = methods.merge(DeviceRPCProvider::provide_with_alias(state.clone()));
        let _ = methods.merge(WifiRPCProvider::provide_with_alias(state.clone()));
        let _ = methods.merge(LifecycleRippleProvider::provide_with_alias(state.clone()));
        let _ = methods.merge(CapRPCProvider::provide_with_alias(state.clone()));
        let _ = methods.merge(KeyboardRPCProvider::provide_with_alias(state.clone()));
        let _ = methods.merge(ClosedcaptionsRPCProvider::provide_with_alias(state.clone()));
        let _ = methods.merge(VoiceguidanceRPCProvider::provide_with_alias(state.clone()));
        let _ = methods.merge(LocalizationRPCProvider::provide_with_alias(state.clone()));
        let _ = methods.merge(AccessoryRippleProvider::provide_with_alias(state.clone()));
        let _ = methods.merge(PrivacyProvider::provide_with_alias(state.clone()));
        let _ = methods.merge(ProfileRPCProvider::provide_with_alias(state.clone()));
        let _ = methods.merge(SecondScreenRPCProvider::provide_with_alias(state.clone()));
        let _ = methods.merge(UserGrantsRPCProvider::provide_with_alias(state.clone()));
        let _ = methods.merge(ParametersRPCProvider::provide_with_alias(state.clone()));
        let _ = methods.merge(SecureStorageRPCProvider::provide_with_alias(state.clone()));
        let _ = methods.merge(AdvertisingRPCProvider::provide_with_alias(state.clone()));
        let _ = methods.merge(MetricsRPCProvider::provide_with_alias(state.clone()));
        let _ = methods.merge(DiscoveryRPCProvider::provide_with_alias(state.clone()));
        let _ = methods.merge(AuthRPCProvider::provide_with_alias(state.clone()));
        let _ = methods.merge(AccountRPCProvider::provide_with_alias(state.clone()));
        let _ = methods.merge(MetricsManagementProvider::provide_with_alias(state.clone()));
        let _ = methods.merge(AudioDescriptionRPCProvider::provide_with_alias(
            state.clone(),
        ));

        // LCM Api(s) not required for internal launcher
        if !state.has_internal_launcher() {
            let _ = methods.merge(LifecycleManagementProvider::provide_with_alias(state));
        }
        let _ = methods.merge(extn_methods);
        methods
    }
}

#[async_trait]
impl Bootstep<BootstrapState> for FireboltGatewayStep {
    fn get_name(&self) -> String {
        "FireboltGatewayStep".into()
    }

    async fn setup(&self, state: BootstrapState) -> Result<(), RippleError> {
        let methods = self
            .init_handlers(
                state.platform_state.clone(),
                state.extn_state.get_extn_methods(),
            )
            .await;
        let gateway = FireboltGateway::new(state.clone(), methods);
        debug!("Handlers initialized");
        #[cfg(feature = "sysd")]
        if sd_notify::booted().is_ok()
            && sd_notify::notify(false, &[sd_notify::NotifyState::Ready]).is_err()
        {
            return Err(RippleError::BootstrapError);
        }
        TelemetryBuilder::send_ripple_telemetry(&state.platform_state);
        gateway.start().await;

        Err(RippleError::ServiceError)
    }
}
