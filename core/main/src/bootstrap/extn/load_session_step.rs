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
use ripple_sdk::{framework::bootstrap::Bootstep, api::session::SessionRequest};
use ripple_sdk::{
    async_trait::async_trait,
    framework::RippleResponse,
};

use crate::state::bootstrap_state::BootstrapState;

pub struct LoadDistributorValuesStep;

#[async_trait]
impl Bootstep<BootstrapState> for LoadDistributorValuesStep {
    fn get_name(&self) -> String {
        "LoadDistributorSessionStep".into()
    }

    async fn setup(&self, s: BootstrapState) -> RippleResponse {
        let response = s
            .platform_state
            .get_client()
            .send_extn_request(SessionRequest::Session)
            .await
            .expect("session");
        if let Some(session) = response.payload.extract() {
            s.platform_state
                .session_state
                .insert_distributor_session(session)
        }
        Ok(())
    }
}
