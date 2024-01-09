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

pub mod launcher_event_processor;
pub mod launcher_ffi;
pub mod launcher_lifecycle_processor;
pub mod launcher_state;

pub mod manager {
    pub mod app_launcher;
    pub mod container_manager;
    pub mod container_message;
    pub mod device_launcher_event_manager;
    pub mod stack;
    pub mod view_manager;
}
