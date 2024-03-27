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
use ripple_sdk::api::config::FEATURE_CLOUD_PERMISSIONS;
use ripple_sdk::api::context::{FeatureUpdate, RippleContextUpdateRequest};
use ripple_sdk::api::device::device_events::{
    DeviceEvent, DeviceEventCallback, DeviceEventRequest,
};
use ripple_sdk::api::device::device_info_request::{DeviceInfoRequest, DeviceResponse};
use ripple_sdk::api::device::device_request::{
    OnInternetConnectedRequest, SystemPowerState, TimeZone,
};
use ripple_sdk::api::session::{AccountSessionRequest, AccountSessionResponse};
use ripple_sdk::extn::extn_client_message::ExtnResponse;
use ripple_sdk::log::{debug, error, warn};
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
        debug!("Subscribing for change in Internet connection Status");
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

        // Setup the TimeZoneChanged status listener
        if ps
            .get_client()
            .send_extn_request(DeviceEventRequest {
                event: DeviceEvent::TimeZoneChanged,
                subscribe: true,
                callback_type: DeviceEventCallback::ExtnEvent,
            })
            .await
            .is_err()
        {
            warn!("No processor to set TimeZoneChanged status listener")
        }

        let ps_c = ps.clone();

        // Asynchronously get context and update the state
        tokio::spawn(async move {
            // Set default cloud permissions value
            ps_c.get_client().get_extn_client().context_update(
                RippleContextUpdateRequest::UpdateFeatures(vec![FeatureUpdate::new(
                    FEATURE_CLOUD_PERMISSIONS.into(),
                    ps_c.get_device_manifest().get_features().cloud_permissions,
                )]),
            );

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

            debug!("Getting the current status of internet connection");
            // Get Internet Connection state
            if let Ok(resp) = ps_c
                .get_client()
                .send_extn_request(DeviceInfoRequest::OnInternetConnected(
                    OnInternetConnectedRequest { timeout: 1000 },
                ))
                .await
            {
                ripple_sdk::log::debug!("[RIPPLE CONTEXT] Sending OnInternetConnected request");
                if let Some(DeviceResponse::InternetConnectionStatus(s)) =
                    resp.payload.extract::<DeviceResponse>()
                {
                    ripple_sdk::log::debug!(
                        "[RIPPLE CONTEXT] OnInternetConnected response: {:?}",
                        s
                    );
                    ps_c.get_client()
                        .get_extn_client()
                        .context_update(RippleContextUpdateRequest::InternetStatus(s));
                } else {
                    ripple_sdk::log::debug!(
                        "[RIPPLE CONTEXT] OnInternetConnected response was NONE"
                    );
                }
            }

            // Get Initial TimeZone
            if let Ok(resp) = ps_c
                .get_client()
                .send_extn_request(DeviceInfoRequest::GetTimezoneWithOffset)
                .await
            {
                if let Some(ExtnResponse::TimezoneWithOffset(tz, offset)) =
                    resp.payload.extract::<ExtnResponse>()
                {
                    ps_c.get_client().get_extn_client().context_update(
                        RippleContextUpdateRequest::TimeZone(TimeZone {
                            time_zone: tz,
                            offset,
                        }),
                    );
                }
            }

            // Get Account session
            if let Ok(resp) = ps_c
                .get_client()
                .send_extn_request(AccountSessionRequest::GetAccessToken)
                .await
            {
                if let Some(ExtnResponse::AccountSession(
                    AccountSessionResponse::AccountSessionToken(account_token),
                )) = resp.payload.extract::<ExtnResponse>()
                {
                    ps_c.get_client()
                        .get_extn_client()
                        .context_update(RippleContextUpdateRequest::Token(account_token));
                }
            }
        });
    }

    // Update the Context with session information during startup
    pub fn update_context_for_session(state_c: PlatformState) {
        // update ripple context for token asynchronously
        tokio::spawn(async move {
            if let Ok(response) = state_c
                .get_client()
                .send_extn_request(AccountSessionRequest::GetAccessToken)
                .await
            {
                if let Some(ExtnResponse::AccountSession(
                    AccountSessionResponse::AccountSessionToken(token),
                )) = response.payload.extract::<ExtnResponse>()
                {
                    state_c.get_client().get_extn_client().context_update(
                        ripple_sdk::api::context::RippleContextUpdateRequest::Token(token),
                    )
                } else {
                    error!("couldnt update the session response")
                }
            }
        });
    }
}
