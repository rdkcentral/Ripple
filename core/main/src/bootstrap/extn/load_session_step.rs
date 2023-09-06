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

use ripple_sdk::api::device::device_events::{
    DeviceEvent, DeviceEventCallback, DeviceEventRequest,
};
use ripple_sdk::api::firebolt::fb_capabilities::{CapEvent, FireboltCap};
use ripple_sdk::log::debug;
use ripple_sdk::{api::session::AccountSessionRequest, framework::bootstrap::Bootstep};
use ripple_sdk::{async_trait::async_trait, framework::RippleResponse};

use crate::state::bootstrap_state::BootstrapState;
use crate::state::cap::cap_state::CapState;
use crate::state::metrics_state::MetricsState;

pub struct LoadDistributorValuesStep;

#[async_trait]
impl Bootstep<BootstrapState> for LoadDistributorValuesStep {
    fn get_name(&self) -> String {
        "LoadDistributorSessionStep".into()
    }

    async fn setup(&self, s: BootstrapState) -> RippleResponse {
        // AccountSessionRequest::Get will not always be successful,
        // on Error, it will set the ripple context activation status as not activated.
        // On failure call activated state and send a telemetry event conveying any provisioning issue.
        // Start a thread which will process the session token update event.
        // Upon receving the event, if the ripple context is not set as active, set it to active.
        //
        //
        let resp = s
            .platform_state
            .get_client()
            .send_extn_request(DeviceEventRequest {
                event: DeviceEvent::DistributorSessionTokenChanged,
                id: "internal".to_owned(),
                subscribe: true,
                callback_type: DeviceEventCallback::ExtnEvent,
            })
            .await;
        debug!(
            "Karthick: result for registering event from device: {:?}",
            resp
        );
        let response = s
            .platform_state
            .get_client()
            .send_extn_request(AccountSessionRequest::Get)
            .await
            .expect("session");
        let mut event = CapEvent::OnUnavailable;
        if let Some(session) = response.payload.extract() {
            s.platform_state
                .session_state
                .insert_account_session(session);
            MetricsState::initialize(&s.platform_state).await;
            event = CapEvent::OnAvailable;
        }
        CapState::emit(
            &s.platform_state,
            event,
            FireboltCap::Short("token:platform".to_owned()),
            None,
        )
        .await;
        Ok(())
    }
}
