use std::sync::Arc;

use crate::state::platform_state::PlatformState;
use ripple_sdk::api::firebolt::fb_telemetry::OperationalMetricRequest;
static mut PLATFORM_STATE: Option<Arc<PlatformState>> = None;
pub struct ObservabilityClient {}
// impl ObservabilityClient {
//     pub fn init(ps: PlatformState) {
//         unsafe {
//             PLATFORM_STATE = Some(Arc::new(ps));
//         }
//     }
//     pub fn report(payload: OperationalMetricRequest) {
//         match unsafe { PLATFORM_STATE } {
//             Some(ref ps) => {
//                 let request = ripple_sdk::api::observability::ObservabilityRequest {
//                     payload: payload.,
//                 };
//                 let _ = ps.get_client().send_extn_request_transient(request);
//             },
//             None => {
//                 println!("ObservabilityClient not initialized");
//             }
//         }
//         println!("payload: {:?}",payload);
//     }
// }
