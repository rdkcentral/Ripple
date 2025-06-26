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

//pub mod rpc_gateway;
//pub mod firebolt_gateway;
pub mod handlers {
    pub mod accessory_rpc;
    pub mod advertising_rpc;
    pub mod audio_description_rpc;
    pub mod capabilities_rpc;
    pub mod closed_captions_rpc;
    pub mod device_rpc;
    pub mod discovery_rpc;
    pub mod internal_rpc;
    pub mod keyboard_rpc;
    pub mod lcm_rpc;
    pub mod lifecycle_rpc;
    pub mod localization_rpc;
    pub mod parameters_rpc;
    pub mod privacy_rpc;
    pub mod profile_rpc;
    pub mod provider_registrar;
    pub mod second_screen_rpc;
    pub mod user_grants_rpc;
    pub mod wifi_rpc;
}
pub mod firebolt_gatekeeper;
pub mod firebolt_gateway;
pub mod firebolt_ws;
pub mod rpc;
pub mod rpc_router;
