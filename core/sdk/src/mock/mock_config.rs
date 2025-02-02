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

use crate::serde_yaml::with;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct MockConfig {
    pub activate_all_plugins: bool,
    pub stats_file: String,
}
impl MockConfig {
    pub fn new(activate_all_plugins: bool, stats_file: String) -> Self {
        Self {
            activate_all_plugins,
            stats_file,
        }
    }
    pub fn with_activate_all_plugins(&mut self, activate_all_plugins: bool) -> &mut Self {
        self.activate_all_plugins = activate_all_plugins;
        self
    }
    pub fn with_stats_file(&mut self, stats_file: String) -> &mut Self {
        self.stats_file = stats_file;
        self
    }
}

impl Default for MockConfig {
    fn default() -> Self {
        Self {
            activate_all_plugins: true,
            stats_file: "stats.json".to_string(),
        }
    }
}
