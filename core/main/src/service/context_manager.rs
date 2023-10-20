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
use ripple_sdk::api::context::RippleContextUpdateRequest;
use ripple_sdk::api::device::device_events::{
    DeviceEvent, DeviceEventCallback, DeviceEventRequest,
};
use ripple_sdk::api::device::device_info_request::{DeviceInfoRequest, DeviceResponse};
use ripple_sdk::api::device::device_request::{OnInternetConnectedRequest, SystemPowerState};
use ripple_sdk::api::session::AccountSessionRequest;
use ripple_sdk::log::warn;
use ripple_sdk::tokio;

pub struct ContextManager;

impl ContextManager {
    pub async fn setup(ps: &PlatformState) {
        // Setup listeners here

        // Setup the Session listener is session is enabled on the manifest
        if ps.supports_session()
            && ps
                .get_client()
                .send_extn_request(AccountSessionRequest::Subscribe)
                .await
                .is_err()
        {
            warn!("No processor to set Session Token changed listener")
        }

        // Setup the Internet Status listener
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

        // Setup the Power status listener
        if ps
            .get_client()
            .send_extn_request(DeviceEventRequest {
                event: DeviceEvent::SystemPowerStateChanged,
                subscribe: true,
                callback_type: DeviceEventCallback::ExtnEvent,
            })
            .await
            .is_err()
        {
            warn!("No processor to set System power status listener")
        }

        let ps_c = ps.clone();

        // Asynchronously get context and update the state
        tokio::spawn(async move {
            // Get Initial power state
            if let Ok(resp) = ps_c
                .get_client()
                .send_extn_request(DeviceInfoRequest::PowerState)
                .await
            {
                if let Some(DeviceResponse::PowerState(p)) =
                    resp.payload.extract::<DeviceResponse>()
                {
                    ps_c.get_client().get_extn_client().context_update(
                        RippleContextUpdateRequest::PowerState(SystemPowerState {
                            current_power_state: p.clone(),
                            power_state: p,
                        }),
                    );
                }
            }

            // Get Internet Connection state
            if let Ok(resp) = ps_c
                .get_client()
                .send_extn_request(DeviceInfoRequest::OnInternetConnected(
                    OnInternetConnectedRequest { timeout: 1000 },
                ))
                .await
            {
                if let Some(DeviceResponse::InternetConnectionStatus(s)) =
                    resp.payload.extract::<DeviceResponse>()
                {
                    ps_c.get_client()
                        .get_extn_client()
                        .context_update(RippleContextUpdateRequest::InternetStatus(s));
                }
            }
        });
    }
}
