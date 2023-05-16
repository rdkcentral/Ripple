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

//pub mod rpc_gateway;
//pub mod firebolt_gateway;
pub mod handlers {
    pub mod accessory_rpc;
    pub mod account_rpc;
    pub mod acknowledge_rpc;
    pub mod advertising_rpc;
    pub mod authentication_rpc;
    pub mod capabilities_rpc;
    pub mod closed_captions_rpc;
    pub mod device_rpc;
    pub mod keyboard_rpc;
    pub mod lcm_rpc;
    pub mod lifecycle_rpc;
    pub mod localization_rpc;
    pub mod parameters_rpc;
    pub mod pin_rpc;
    pub mod privacy_rpc;
    pub mod profile_rpc;
    pub mod second_screen_rpc;
    pub mod secure_storage_rpc;
    pub mod voice_guidance_rpc;
    pub mod wifi_rpc;
}
pub mod firebolt_gatekeeper;
pub mod firebolt_gateway;
pub mod firebolt_ws;
pub mod rpc;
pub mod rpc_router;
