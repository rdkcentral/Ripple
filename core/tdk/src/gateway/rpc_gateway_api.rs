use ripple_sdk::api::gateway::rpc_gateway_api::{ApiProtocol, CallContext};

use crate::utils::test_utils::Mockable;

impl Mockable for CallContext {
    fn mock() -> Self {
        Self::new(
            "sess_id".to_owned(),
            "1".to_owned(),
            "app_id".to_owned(),
            1,
            ApiProtocol::Extn,
            "method".to_owned(),
            None,
            false,
        )
    }
}
