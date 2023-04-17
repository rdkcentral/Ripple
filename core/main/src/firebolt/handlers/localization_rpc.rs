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

use jsonrpsee::{
    core::{async_trait, RpcResult},
    proc_macros::rpc,
    RpcModule,
};
use ripple_sdk::api::{
    device::device_accessibility_data::SetStringProperty,
    firebolt::fb_general::{ListenRequest, ListenerResponse},
    gateway::rpc_gateway_api::CallContext,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::utils::serde_utils::*;
use crate::{
    firebolt::rpc::RippleRPCProvider,
    processor::storage::{
        storage_manager::StorageManager,
        storage_property::{StorageProperty, KEY_POSTAL_CODE},
    },
    service::apps::provider_broker::ProviderBroker,
    state::platform_state::PlatformState,
};

//const EVENT_TIMEZONE_CHANGED: &'static str = "localization.onTimeZoneChanged";

// #[derive(Deserialize, Debug)]
// pub struct SetStringProperty {
//     pub value: String,
// }

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct LanguageProperty {
    #[serde(with = "language_code_serde")]
    pub value: String,
}

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct TimezoneProperty {
    #[serde(with = "timezone_serde")]
    pub value: String,
}

#[rpc(server)]
pub trait Localization {
    #[method(name = "localization.locality")]
    async fn locality(&self, ctx: CallContext) -> RpcResult<String>;
    #[method(name = "localization.setLocality")]
    async fn locality_set(&self, ctx: CallContext, set_request: SetStringProperty)
        -> RpcResult<()>;
    #[method(name = "localization.onLocalityChanged")]
    async fn on_locality_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;
    #[method(name = "localization.countryCode")]
    async fn country_code(&self, ctx: CallContext) -> RpcResult<String>;
    #[method(name = "localization.setCountryCode")]
    async fn country_code_set(
        &self,
        ctx: CallContext,
        set_request: SetStringProperty,
    ) -> RpcResult<()>;
    #[method(name = "localization.onCountryCodeChanged")]
    async fn on_country_code_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;
    #[method(name = "localization.language")]
    async fn language(&self, ctx: CallContext) -> RpcResult<String>;
    #[method(name = "localization.setLanguage")]
    async fn language_set(&self, ctx: CallContext, set_request: LanguageProperty) -> RpcResult<()>;
    #[method(name = "localization.onLanguageChanged")]
    async fn on_language_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;
    #[method(name = "localization.postalCode")]
    async fn postal_code(&self, _ctx: CallContext) -> RpcResult<String>;
    #[method(name = "localization.setPostalCode")]
    async fn postal_code_set(
        &self,
        ctx: CallContext,
        set_request: SetStringProperty,
    ) -> RpcResult<()>;
    #[method(name = "localization.onPostalCodeChanged")]
    async fn on_postal_code_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;
    #[method(name = "localization.locale")]
    async fn locale(&self, _ctx: CallContext) -> RpcResult<String>;
    #[method(name = "localization.setLocale")]
    async fn locale_set(&self, ctx: CallContext, set_request: SetStringProperty) -> RpcResult<()>;
    #[method(name = "localization.onLocaleChanged")]
    async fn on_locale_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;
    #[method(name = "localization.latlon")]
    async fn latlon(&self, _ctx: CallContext) -> RpcResult<String>;
    #[method(name = "localization.setLatlon")]
    async fn latlon_set(&self, ctx: CallContext, set_request: SetStringProperty) -> RpcResult<()>;
    #[method(name = "localization.onLatlonChanged")]
    async fn on_latlon_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;
    #[method(name = "localization.additionalInfo")]
    async fn additional_info(&self, _ctx: CallContext) -> RpcResult<HashMap<String, String>>;
    #[method(name = "localization.setAdditionalInfo")]
    async fn additional_info_set(
        &self,
        ctx: CallContext,
        set_request: SetStringProperty,
    ) -> RpcResult<()>;
    #[method(name = "localization.onAdditionalInfoChanged")]
    async fn on_additional_info_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;
    //TODO: timezone_set
    // #[method(name = "localization.setTimeZone")]
    // async fn timezone_set(&self, ctx: CallContext, set_request: TimezoneProperty) -> RpcResult<()>;
    // TODO: timezone
    // #[method(name = "localization.timeZone")]
    // async fn timezone(&self, ctx: CallContext) -> RpcResult<String>;
    // TODO: on_timezone_changed
    // #[method(name = "localization.onTimeZoneChanged")]
    // async fn on_timezone_changed(
    //     &self,
    //     ctx: CallContext,
    //     request: ListenRequest,
    // ) -> RpcResult<ListenerResponse>;
}

#[derive(Debug)]
pub struct LocalizationImpl {
    pub platform_state: PlatformState,
}

impl LocalizationImpl {
    pub async fn postal_code(state: &PlatformState, app_id: String) -> Option<String> {
        match StorageManager::get_string(state, StorageProperty::PostalCode).await {
            Ok(resp) => Some(resp),
            Err(_) => {
                match StorageManager::get_string_from_namespace(state, app_id, KEY_POSTAL_CODE)
                    .await
                {
                    Ok(resp) => Some(resp.as_value()),
                    Err(_) => None,
                }
            }
        }
    }

    pub async fn on_request_app_event(
        &self,
        ctx: CallContext,
        request: ListenRequest,
        method: &'static str,
        event_name: &'static str,
    ) -> RpcResult<ListenerResponse> {
        let listen = request.listen;
        ProviderBroker::register_or_unregister_provider(
            &self.platform_state,
            // TODO update with Firebolt Cap in later effort
            "xrn:firebolt:capability:localization:locale".into(),
            method.into(),
            event_name,
            ctx,
            request,
        )
        .await;

        Ok(ListenerResponse {
            listening: listen,
            event: event_name.into(),
        })
    }
}

#[async_trait]
impl LocalizationServer for LocalizationImpl {
    async fn locality(&self, _ctx: CallContext) -> RpcResult<String> {
        StorageManager::get_string(&self.platform_state, StorageProperty::Locality).await
    }

    async fn locality_set(
        &self,
        _ctx: CallContext,
        set_request: SetStringProperty,
    ) -> RpcResult<()> {
        StorageManager::set_string(
            &self.platform_state,
            StorageProperty::Locality,
            set_request.value,
            None,
        )
        .await
    }

    async fn on_locality_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        self.on_request_app_event(
            ctx,
            request,
            "LocalizationLocalityChanged",
            "localization.onLocalityChanged",
        )
        .await
    }

    async fn country_code(&self, _ctx: CallContext) -> RpcResult<String> {
        StorageManager::get_string(&self.platform_state, StorageProperty::CountryCode).await
    }

    async fn country_code_set(
        &self,
        _ctx: CallContext,
        set_request: SetStringProperty,
    ) -> RpcResult<()> {
        StorageManager::set_string(
            &self.platform_state,
            StorageProperty::CountryCode,
            set_request.value,
            None,
        )
        .await
    }

    async fn on_country_code_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        self.on_request_app_event(
            ctx,
            request,
            "LocalizationCountryCodeChanged",
            "localization.onCountryCodeChanged",
        )
        .await
    }

    async fn language(&self, _ctx: CallContext) -> RpcResult<String> {
        StorageManager::get_string(&self.platform_state, StorageProperty::Language).await
    }

    async fn language_set(
        &self,
        _ctx: CallContext,
        set_request: LanguageProperty,
    ) -> RpcResult<()> {
        StorageManager::set_string(
            &self.platform_state,
            StorageProperty::Language,
            set_request.value,
            None,
        )
        .await
    }

    async fn on_language_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        self.on_request_app_event(
            ctx,
            request,
            "LocalizationLanguageChanged",
            "localization.onLanguageChanged",
        )
        .await
    }

    async fn postal_code(&self, ctx: CallContext) -> RpcResult<String> {
        match LocalizationImpl::postal_code(&self.platform_state, ctx.app_id).await {
            Some(postal_code) => Ok(postal_code),
            None => Err(StorageManager::get_firebolt_error(
                &StorageProperty::PostalCode,
            )),
        }
    }

    async fn postal_code_set(
        &self,
        _ctx: CallContext,
        set_request: SetStringProperty,
    ) -> RpcResult<()> {
        StorageManager::set_string(
            &self.platform_state,
            StorageProperty::PostalCode,
            set_request.value,
            None,
        )
        .await
    }

    async fn on_postal_code_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        self.on_request_app_event(
            ctx,
            request,
            "LocalizationPostalCodeChanged",
            "localization.onPostalCodeChanged",
        )
        .await
    }

    async fn locale(&self, _ctx: CallContext) -> RpcResult<String> {
        StorageManager::get_string(&self.platform_state, StorageProperty::Locale).await
    }

    async fn locale_set(&self, _ctx: CallContext, set_request: SetStringProperty) -> RpcResult<()> {
        StorageManager::set_string(
            &self.platform_state,
            StorageProperty::Locale,
            set_request.value,
            None,
        )
        .await
    }

    async fn on_locale_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        self.on_request_app_event(
            ctx,
            request,
            "LocalizationLocaleChanged",
            "localization.onLocaleChanged",
        )
        .await
    }

    async fn latlon(&self, _ctx: CallContext) -> RpcResult<String> {
        StorageManager::get_string(&self.platform_state, StorageProperty::LatLon).await
    }

    async fn latlon_set(&self, _ctx: CallContext, set_request: SetStringProperty) -> RpcResult<()> {
        StorageManager::set_string(
            &self.platform_state,
            StorageProperty::LatLon,
            set_request.value,
            None,
        )
        .await
    }

    async fn on_latlon_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        self.on_request_app_event(
            ctx,
            request,
            "LocalizationLatlonChanged",
            "localization.onLatlonChanged",
        )
        .await
    }

    async fn additional_info(&self, _ctx: CallContext) -> RpcResult<HashMap<String, String>> {
        let json_str =
            StorageManager::get_string(&self.platform_state, StorageProperty::AdditionalInfo).await;
        match json_str {
            Ok(s) => {
                let deserialized = serde_json::from_str::<HashMap<String, String>>(&s).unwrap();
                return Ok(deserialized);
            }
            Err(_e) => {
                return Err(jsonrpsee::core::Error::Custom(String::from(
                    "additional_info: error response TBD",
                )))
            }
        }
    }

    async fn additional_info_set(
        &self,
        _ctx: CallContext,
        set_request: SetStringProperty,
    ) -> RpcResult<()> {
        StorageManager::set_string(
            &self.platform_state,
            StorageProperty::AdditionalInfo,
            set_request.value,
            None,
        )
        .await
    }

    async fn on_additional_info_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        self.on_request_app_event(
            ctx,
            request,
            "LocalizationAdditionalInfoChanged",
            "localization.onAdditionalInfoChanged",
        )
        .await
    }

    //TODO: timezone_set
    // async fn timezone_set(
    //     &self,
    //     _ctx: CallContext,
    //     set_request: TimezoneProperty,
    // ) -> RpcResult<()> {
    //     if let Err(_e) = resp {
    //         return Err(jsonrpsee::core::Error::Custom(String::from(
    //             "timezone_set: error response TBD",
    //         )));
    //     }

    //     if let DabResponsePayload::AvailableTimezones(timezones) = resp.unwrap() {
    //         if !timezones.contains(&set_request.value) {
    //             error!(
    //                 "timezone_set: Unsupported timezone: tz={}",
    //                 set_request.value
    //             );
    //             return Err(jsonrpsee::core::Error::Custom(String::from(
    //                 "timezone_set: error response TBD",
    //             )));
    //         }
    //     } else {
    //         return Err(jsonrpsee::core::Error::Custom(String::from(
    //             "timezone_set: error response TBD",
    //         )));
    //     }

    //     let resp = self
    //         .helper
    //         .send_dab(DabRequestPayload::Device(DeviceRequest::SetTimezone(
    //             set_request.value.clone(),
    //         )))
    //         .await;
    //     match resp {
    //         Ok(_) => {
    //             AppEvents::emit(
    //                 &self.platform_state,
    //                 &EVENT_TIMEZONE_CHANGED.to_string(),
    //                 &serde_json::to_value(set_request.value).unwrap(),
    //             )
    //             .await;
    //             Ok(())
    //         }
    //         Err(_e) => Err(jsonrpsee::core::Error::Custom(String::from(
    //             "timezone_set: error response TBD",
    //         ))),
    //     }
    // }

    //TODO: timezone
    // async fn timezone(&self, _ctx: CallContext) -> RpcResult<String> {
    //     let resp = self
    //         .helper
    //         .send_dab(DabRequestPayload::Device(DeviceRequest::GetTimezone))
    //         .await;

    //     if let Err(_e) = resp {
    //         return Err(jsonrpsee::core::Error::Custom(String::from(
    //             "timezone: error response TBD",
    //         )));
    //     }

    //     if let DabResponsePayload::Timezone(timezone) = resp.unwrap() {
    //         Ok(timezone)
    //     } else {
    //         Err(jsonrpsee::core::Error::Custom(String::from(
    //             "timezone: error response TBD",
    //         )))
    //     }
    // }

    //TODO: on_timezone_changed
    // async fn on_timezone_changed(
    //     &self,
    //     ctx: CallContext,
    //     request: ListenRequest,
    // ) -> RpcResult<ListenerResponse> {
    //     let listen = request.listen;
    //     AppEvents::add_listener(
    //         &&self.platform_state.app_events_state,
    //         EVENT_TIMEZONE_CHANGED.to_string(),
    //         ctx,
    //         request,
    //     );
    //     Ok(ListenerResponse {
    //         listening: listen,
    //         event: EVENT_TIMEZONE_CHANGED,
    //     })
    // }
}

pub struct LocalizationRPCProvider;

impl RippleRPCProvider<LocalizationImpl> for LocalizationRPCProvider {
    fn provide(platform_state: PlatformState) -> RpcModule<LocalizationImpl> {
        (LocalizationImpl { platform_state }).into_rpc()
    }
}
