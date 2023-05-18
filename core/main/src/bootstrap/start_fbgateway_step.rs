// If not stated otherwise in this file or this component's license file the
// following copyright and licenses apply:
//
// Copyright 2023 RDK Management
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

use crate::{
    firebolt::{
        firebolt_gateway::FireboltGateway,
        handlers::{
            accessory_rpc::AccessoryRippleProvider, acknowledge_rpc::AckRPCProvider,
            advertising_rpc::AdvertisingRPCProvider, authentication_rpc::AuthRPCProvider,
            capabilities_rpc::CapRPCProvider, closed_captions_rpc::ClosedcaptionsRPCProvider,
            device_rpc::DeviceRPCProvider, discovery_rpc::DiscoveryRpcProvider,
            keyboard_rpc::KeyboardRPCProvider, lcm_rpc::LifecycleManagementProvider,
            lifecycle_rpc::LifecycleRippleProvider, localization_rpc::LocalizationRPCProvider,
            metrics_rpc::MetricsRPCProvider, parameters_rpc::ParametersRPCProvider,
            pin_rpc::PinRPCProvider, privacy_rpc::PrivacyProvider, profile_rpc::ProfileRPCProvider,
            second_screen_rpc::SecondScreenRPCProvider,
            secure_storage_rpc::SecureStorageRPCProvider, user_grants_rpc::UserGrantsRPCProvider,
            voice_guidance_rpc::VoiceguidanceRPCProvider, wifi_rpc::WifiRPCProvider,
        },
        rpc::RippleRPCProvider,
    },
    processor::rpc_gateway_processor::RpcGatewayProcessor,
    state::{bootstrap_state::BootstrapState, platform_state::PlatformState},
};
use jsonrpsee::core::{async_trait, server::rpc_module::Methods};
use ripple_sdk::{framework::bootstrap::Bootstep, utils::error::RippleError};

pub struct FireboltGatewayStep;

impl FireboltGatewayStep {
    async fn init_handlers(&self, state: PlatformState, extn_methods: Methods) -> Methods {
        let mut methods = Methods::new();
        let _ = methods.merge(DeviceRPCProvider::provide_with_alias(state.clone()));
        let _ = methods.merge(WifiRPCProvider::provide_with_alias(state.clone()));
        let _ = methods.merge(LifecycleRippleProvider::provide_with_alias(state.clone()));
        let _ = methods.merge(CapRPCProvider::provide_with_alias(state.clone()));
        let _ = methods.merge(KeyboardRPCProvider::provide_with_alias(state.clone()));
        let _ = methods.merge(AckRPCProvider::provide_with_alias(state.clone()));
        let _ = methods.merge(PinRPCProvider::provide_with_alias(state.clone()));
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
        let _ = methods.merge(DiscoveryRpcProvider::provide_with_alias(state.clone()));
        let _ = methods.merge(AuthRPCProvider::provide_with_alias(state.clone()));
        // LCM Api(s) not required for internal launcher
        if !state.has_internal_launcher() {
            let _ = methods.merge(LifecycleManagementProvider::provide_with_alias(
                state.clone(),
            ));
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
        // Main can now recieve RPC requests
        state
            .platform_state
            .get_client()
            .add_request_processor(RpcGatewayProcessor::new(state.platform_state.get_client()));
        gateway.start().await;
        Ok(())
    }
}
