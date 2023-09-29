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

use crate::state::platform_state::PlatformState;
use ripple_sdk::api::device::device_events::{
    DeviceEvent, DeviceEventCallback, DeviceEventRequest,
};
use ripple_sdk::api::session::AccountSessionRequest;
use ripple_sdk::log::warn;

pub struct ContextManager;

impl ContextManager {
    pub async fn setup(ps: &PlatformState) {
        if ps.supports_session()
            && ps
                .get_client()
                .send_extn_request(AccountSessionRequest::Subscribe)
                .await
                .is_err()
        {
            warn!("No processor to set Session Token changed listener")
        }

        if ps
            .get_client()
            .send_extn_request(DeviceEventRequest {
                event: DeviceEvent::InternetConnectionStatusChanged,
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
