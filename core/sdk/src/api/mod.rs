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
pub mod apps;
pub mod config;
pub mod device;
pub mod lifecycle;
pub mod manifest;
pub mod status_update;
pub mod gateway {
    pub mod rpc_error;
    pub mod rpc_gateway_api;
}

pub mod distributor {
    pub mod distributor_permissions;
    pub mod distributor_request;
    pub mod distributor_session;
}

pub mod firebolt {
    pub mod fb_capabilities;
    pub mod fb_discovery;
    pub mod fb_general;
    pub mod fb_lifecycle;
    pub mod fb_lifecycle_management;
    pub mod fb_openrpc;
    pub mod fb_parameters;
    pub mod fb_secondscreen;
}
