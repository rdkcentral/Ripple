use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::{
    api::session::AccountSession,
    extn::extn_client_message::{ExtnPayload, ExtnPayloadProvider, ExtnRequest, ExtnResponse},
    framework::ripple_contract::RippleContract,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AdvertisingRequest {
    GetAdInitObject(AdInitObjectRequestParams),
    GetAdIdObject(AdIdRequestParams),
    ResetAdIdentifier(AccountSession),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AdInitObjectRequestParams {
    pub privacy_data: HashMap<String, String>,
    pub environment: String,
    pub durable_app_id: String,
    pub app_version: String,
    pub distributor_app_id: String,
    pub device_ad_attributes: HashMap<String, String>,
    pub coppa: bool,
    pub authentication_entity: String,
    pub dist_session: AccountSession,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AdInitObjectResponse {
    pub ad_server_url: String,
    pub ad_server_url_template: String,
    pub ad_network_id: String,
    pub ad_profile_id: String,
    pub ad_site_section_id: String,
    pub ad_opt_out: bool,
    pub privacy_data: String,
    pub ifa_value: String,
    pub ifa: String,
    pub app_name: String,
    pub app_bundle_id: String,
    pub app_version: String,
    pub distributor_app_id: String,
    pub device_ad_attributes: String,
    pub coppa: String,
    pub authentication_entity: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AdIdRequestParams {
    pub privacy_data: HashMap<String, String>,
    pub app_id: String,
    pub dist_session: AccountSession,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AdIdResponse {
    pub ifa: String,
    pub ifa_type: String,
    pub lmt: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SessionParams {
    pub dist_session: AccountSession,
}

impl ExtnPayloadProvider for AdvertisingRequest {
    fn get_extn_payload(&self) -> ExtnPayload {
        ExtnPayload::Request(ExtnRequest::Advertising(self.clone()))
    }

    fn get_from_payload(payload: ExtnPayload) -> Option<AdvertisingRequest> {
        match payload {
            ExtnPayload::Request(request) => match request {
                ExtnRequest::Advertising(r) => return Some(r),
                _ => {}
            },
            _ => {}
        }
        None
    }

    fn contract() -> RippleContract {
        RippleContract::Advertising
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum AdvertisingResponse {
    None,
    AdInitObject(AdInitObjectResponse),
    AdIdObject(AdIdResponse),
}

impl ExtnPayloadProvider for AdvertisingResponse {
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
        RippleContract::Advertising
    }
}
