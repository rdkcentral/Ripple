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

pub mod api;
pub mod extn;
pub mod framework;
pub mod utils;

// Externalize the reusable crates to avoid version
// mismatch and standardization of these libraries
// across extensions
pub extern crate async_channel;
pub extern crate async_trait;
pub extern crate chrono;
pub extern crate futures;
pub extern crate libloading;
pub extern crate log;
pub extern crate semver;
pub extern crate serde;
pub extern crate serde_json;
pub extern crate serde_yaml;
pub extern crate tokio;
pub extern crate uuid;

pub trait Mockable {
    fn mock() -> Self;
}
