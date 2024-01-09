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
use crate::launcher_state::LauncherState;
use ripple_sdk::api::device::device_events::{
    DeviceEvent, DeviceEventCallback, DeviceEventRequest,
};
use std::{thread, time};

use ripple_sdk::log::warn;
pub struct ControllerEventManager;

impl ControllerEventManager {
    pub async fn setup(state: LauncherState) {
        //  wait setup thunder extn
        //  let millis = time::Duration::from_millis(5);
        thread::sleep(time::Duration::from_millis(200));

        // Setup listeners here
        println!("\n\n DEBUG: Ripple is allowing non firebolt apps \n\n");

        let state_y = state.clone();
        if state_y
            .clone()
            .send_extn_request(DeviceEventRequest {
                event: DeviceEvent::OnDestroyedEvent,
                subscribe: true,
                callback_type: DeviceEventCallback::ExtnEvent,
            })
            .await
            .is_err()
        {
            warn!("No processor to set Internet status listener")
        }

        // Setup the onLaunched event listener
        let state_x = state.clone();
        if state_x
            .clone()
            .send_extn_request(DeviceEventRequest {
                event: DeviceEvent::OnLaunchedEvent,
                subscribe: true,
                callback_type: DeviceEventCallback::ExtnEvent,
            })
            .await
            .is_err()
        {
            warn!("No processor to set Internet status listener")
        }
    }
}
