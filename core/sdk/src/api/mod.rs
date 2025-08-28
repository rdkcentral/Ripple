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

pub mod accessory;
pub mod account_link;
pub mod apps;
pub mod caps;
pub mod config;
pub mod context;
pub mod default_storage_properties;
pub mod device;
pub mod manifest;
pub mod ripple_cache;
pub mod rules_engine;
pub mod session;
pub mod settings;
pub mod status_update;
pub mod storage_manager;
pub mod storage_manager_utils;
pub mod storage_property;
pub mod usergrant_entry;
pub mod wifi;

pub mod gateway {
    pub mod rpc_error;
    pub mod rpc_gateway_api;
}

pub mod distributor {
    pub mod distributor_permissions;
    pub mod distributor_privacy;
    pub mod distributor_usergrants;
}

pub mod firebolt {
    pub mod fb_advertising;
    pub mod fb_capabilities;
    pub mod fb_discovery;
    pub mod fb_general;
    pub mod fb_keyboard;
    pub mod fb_lifecycle;
    pub mod fb_lifecycle_management;
    pub mod fb_localization;
    pub mod fb_metrics;
    pub mod fb_openrpc;
    pub mod fb_parameters;
    pub mod fb_pin;
    pub mod fb_secondscreen;
    pub mod fb_telemetry;
    pub mod fb_user_grants;
    pub mod provider;
}

pub mod observability {
    pub mod log_signal;
    pub mod metrics_util;
    pub mod operational_metrics;
}
pub mod rules;
