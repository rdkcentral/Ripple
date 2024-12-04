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

use jsonrpsee::{
    core::{async_trait, RpcResult},
    proc_macros::rpc,
    tracing::error,
    RpcModule,
};
use ripple_sdk::{
    api::{
        device::{
            device_events::{
                DeviceEvent, DeviceEventCallback, DeviceEventRequest, TIME_ZONE_CHANGED,
            },
            device_info_request::DeviceInfoRequest,
            device_peristence::SetStringProperty,
            device_request::TimezoneProperty,
        },
        firebolt::{
            fb_general::{ListenRequest, ListenerResponse},
            fb_localization::SetPreferredAudioLanguage,
        },
        gateway::rpc_gateway_api::CallContext,
        storage_property::{StorageProperty, KEY_POSTAL_CODE},
    },
    extn::extn_client_message::ExtnResponse,
};

use crate::utils::rpc_utils::{rpc_add_event_listener, rpc_err};
use crate::{
    firebolt::rpc::RippleRPCProvider, processor::storage::storage_manager::StorageManager,
    service::apps::provider_broker::ProviderBroker, state::platform_state::PlatformState,
};
use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct SetMapEntryProperty {
    pub key: String,
    pub value: String,
}
#[derive(Deserialize, Debug)]
pub struct RemoveMapEntryProperty {
    pub key: String,
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
    #[method(name = "localization.addAdditionalInfo")]
    async fn add_additional_info(
        &self,
        ctx: CallContext,
        set_map_entry_property: SetMapEntryProperty,
    ) -> RpcResult<()>;
    #[method(name = "localization.removeAdditionalInfo")]
    async fn remove_additional_info(
        &self,
        ctx: CallContext,
        remove_map_entry_property: RemoveMapEntryProperty,
    ) -> RpcResult<()>;
    #[method(name = "localization.setTimeZone")]
    async fn timezone_set(&self, ctx: CallContext, set_request: TimezoneProperty) -> RpcResult<()>;
    #[method(name = "localization.timeZone")]
    async fn timezone(&self, ctx: CallContext) -> RpcResult<String>;
    #[method(name = "localization.onTimeZoneChanged")]
    async fn on_timezone_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;

    #[method(name = "localization.preferredAudioLanguages")]
    async fn preferred_audio_languages(&self, _ctx: CallContext) -> RpcResult<Vec<String>>;
    #[method(name = "localization.setPreferredAudioLanguages")]
    async fn preferred_audio_languages_set(
        &self,
        ctx: CallContext,
        set_request: SetPreferredAudioLanguage,
    ) -> RpcResult<()>;
    #[method(name = "localization.onPreferredAudioLanguagesChanged")]
    async fn on_preferred_audio_languages(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;
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
                match StorageManager::get_string_from_namespace(
                    state,
                    app_id,
                    KEY_POSTAL_CODE,
                    None,
                )
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
            String::from(event_name),
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

    // #[instrument(skip(self))]
    async fn add_additional_info(
        &self,
        _ctx: CallContext,
        set_map_entry_property: SetMapEntryProperty,
    ) -> RpcResult<()> {
        /*
        Per FIRE-189, AdditionalInfo is now individually updatable, so read the entire map out, and update
        value in place, and then write entire map out

         */
        StorageManager::set_value_in_map(
            &self.platform_state,
            StorageProperty::AdditionalInfo,
            set_map_entry_property.key,
            set_map_entry_property.value,
        )
        .await
    }
    // #[instrument(skip(self))]
    async fn remove_additional_info(
        &self,
        _ctx: CallContext,
        remove_map_entry_property: RemoveMapEntryProperty,
    ) -> RpcResult<()> {
        StorageManager::remove_value_in_map(
            &self.platform_state,
            StorageProperty::AdditionalInfo,
            remove_map_entry_property.key,
        )
        .await
    }

    async fn timezone_set(
        &self,
        _ctx: CallContext,
        set_request: TimezoneProperty,
    ) -> RpcResult<()> {
        let resp = match self
            .platform_state
            .get_client()
            .send_extn_request(DeviceInfoRequest::GetAvailableTimezones)
            .await
        {
            Ok(r) => r,
            Err(e) => {
                error!("timezone_set: error response TBD: {:?}", e);
                return Err(jsonrpsee::core::Error::Custom(String::from(
                    "timezone_set: error response TBD",
                )));
            }
        };
        if let Some(ExtnResponse::AvailableTimezones(timezones)) = resp.payload.extract() {
            if !timezones.contains(&set_request.value) {
                error!(
                    "timezone_set: Unsupported timezone: tz={}",
                    set_request.value
                );
                return Err(jsonrpsee::core::Error::Custom(format!(
                    "timezone_set: Unsupported timezone: tz={0}",
                    set_request.value
                )));
            }
        } else {
            return Err(jsonrpsee::core::Error::Custom(String::from(
                "timezone_set: error response TBD",
            )));
        }

        if self
            .platform_state
            .get_client()
            .send_extn_request(DeviceInfoRequest::SetTimezone(set_request.value.clone()))
            .await
            .is_ok()
        {
            return Ok(());
        }

        Err(rpc_err("timezone: error response TBD"))
    }

    async fn timezone(&self, _ctx: CallContext) -> RpcResult<String> {
        if let Ok(response) = self
            .platform_state
            .get_client()
            .send_extn_request(DeviceInfoRequest::GetTimezone)
            .await
        {
            if let Some(ExtnResponse::String(v)) = response.payload.extract() {
                return Ok(v);
            }
        }
        Err(rpc_err("timezone: error response TBD"))
    }

    async fn on_timezone_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        if self
            .platform_state
            .get_client()
            .send_extn_request(DeviceEventRequest {
                event: DeviceEvent::TimeZoneChanged,
                subscribe: true,
                callback_type: DeviceEventCallback::FireboltAppEvent(ctx.app_id.to_owned()),
            })
            .await
            .is_err()
        {
            error!("on_timezone_changed: Error while registration");
        }

        rpc_add_event_listener(&self.platform_state, ctx, request, TIME_ZONE_CHANGED).await
    }

    async fn preferred_audio_languages(&self, _ctx: CallContext) -> RpcResult<Vec<String>> {
        Ok(StorageManager::get_vec_string(
            &self.platform_state,
            StorageProperty::PreferredAudioLanguages,
        )
        .await
        .unwrap_or(Vec::new()))
    }

    async fn preferred_audio_languages_set(
        &self,
        _ctx: CallContext,
        set_request: SetPreferredAudioLanguage,
    ) -> RpcResult<()> {
        StorageManager::set_vec_string(
            &self.platform_state,
            StorageProperty::PreferredAudioLanguages,
            set_request.get_string(),
            None,
        )
        .await
    }

    async fn on_preferred_audio_languages(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        self.on_request_app_event(
            ctx,
            request,
            "LocalizationPreferredAudioLanguagesChanged",
            "Localization.onPreferredAudioLanguagesChanged",
        )
        .await
    }
}

pub struct LocalizationRPCProvider;

impl RippleRPCProvider<LocalizationImpl> for LocalizationRPCProvider {
    fn provide(platform_state: PlatformState) -> RpcModule<LocalizationImpl> {
        (LocalizationImpl { platform_state }).into_rpc()
    }
}
