use serde::{Deserialize, Serialize};
use tokio::sync::oneshot;
use uuid::Uuid;

use crate::{
    extn::{
        extn_client_message::{ExtnPayload, ExtnPayloadProvider, ExtnResponse},
    },
    utils::{channel_utils::oneshot_send_and_log, error::RippleError}, framework::ripple_contract::{RippleContract, MainContract},
};

use super::{firebolt::{
    fb_discovery::{LaunchRequest, NavigationIntent},
    fb_lifecycle::LifecycleState,
    fb_parameters::SecondScreenEvent,
}, apps::AppError, device::device_wifi::{AccessPointList, AccessPoint}};


#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WifiResponse {
    None(()),
    String(String),
    Boolean(bool),
    Number(u32),
    Error(RippleError),
    WifiScanListResponse(AccessPointList),
    WifiConnectSuccessResponse(AccessPoint),
}

impl ExtnPayloadProvider for WifiResponse {
    fn get_extn_payload(&self) -> ExtnPayload {
        ExtnPayload::Response(ExtnResponse::Value(
            serde_json::to_value(self.clone()).unwrap(),
        ))
    }

    fn get_from_payload(payload: ExtnPayload) -> Option<Self> {
        match payload {
            ExtnPayload::Response(response) => match response {
                ExtnResponse::Value(value) => {
                    if let Ok(v) = serde_json::from_value(value) {
                        return Some(v);
                    }
                }
                _ => {}
            },
            _ => {}
        }
        None
    }

    fn contract() -> RippleContract {
        RippleContract::Main(MainContract::Rpc)
    }
}