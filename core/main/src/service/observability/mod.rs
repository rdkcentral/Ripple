use std::sync::Arc;

use crate::state::platform_state::PlatformState;
use ripple_sdk::api::firebolt::fb_telemetry::OperationalMetricRequest;
static mut PLATFORM_STATE: Option<Arc<PlatformState>> = None;
pub struct ObservabilityClient {}
impl ObservabilityClient {
    pub fn report(platform_state: &PlatformState, payload: OperationalMetricRequest) {
        println!("payload: {:?}", payload);
    }
}
