use ripple_sdk::framework::bootstrap::Bootstep;
use ripple_sdk::{
    api::distributor::distributor_session::DistributorSessionRequest, async_trait::async_trait,
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
            .send_extn_request(DistributorSessionRequest::Session)
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
