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
    RpcModule,
};
use ripple_sdk::api::{
    device::device_peristence::SetStringProperty,
    firebolt::fb_general::{ListenRequest, ListenerResponse},
    gateway::rpc_gateway_api::CallContext,
    storage_property::{StorageProperty, KEY_POSTAL_CODE},
};
use serde_json::{json, Value};

use crate::broker::broker_utils::BrokerUtils;
use crate::{
    firebolt::rpc::RippleRPCProvider, processor::storage::storage_manager::StorageManager,
    service::apps::provider_broker::ProviderBroker, state::platform_state::PlatformState,
};
use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct SetMapEntryProperty {
    pub key: String,
    pub value: Value,
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
}

enum MapEntryProperty {
    Set(SetMapEntryProperty),
    Remove(RemoveMapEntryProperty),
}

async fn update_additional_info(
    platform_state: PlatformState,
    map_entry_property: MapEntryProperty,
) -> RpcResult<()> {
    match BrokerUtils::process_internal_main_request(
        platform_state.clone(),
        "localization.additionalInfo",
        None,
    )
    .await
    {
        Ok(Value::Object(mut additional_info_map)) => {
            match map_entry_property {
                MapEntryProperty::Set(set_map_entry_property) => {
                    additional_info_map.insert(
                        set_map_entry_property.key.clone(),
                        set_map_entry_property.value.clone(),
                    );
                }
                MapEntryProperty::Remove(remove_map_entry_property) => {
                    additional_info_map.remove(&remove_map_entry_property.key);
                }
            }

            if let Ok(value) = serde_json::to_string(&additional_info_map) {
                let params = Some(json!({
                    "value": value,
                }));

                BrokerUtils::process_internal_main_request(
                    platform_state.clone(),
                    "localization.setAdditionalInfo",
                    params,
                )
                .await?;
            } else {
                return Err(jsonrpsee::core::Error::Custom(String::from(
                    "Error while serializing additional info",
                )));
            }
        }
        Err(e) => {
            return Err(e);
        }
        _ => {
            return Err(jsonrpsee::core::Error::Custom(String::from(
                "Existing additional info is not an object",
            )));
        }
    }

    Ok(())
}

#[derive(Debug)]
pub struct LocalizationImpl {
    pub platform_state: PlatformState,
}

impl LocalizationImpl {
    pub async fn postal_code(state: PlatformState, app_id: String) -> Option<String> {
        match StorageManager::get_string(state.clone(), StorageProperty::PostalCode).await {
            Ok(resp) => Some(resp),
            Err(_) => {
                match StorageManager::get_string_from_namespace(
                    state.clone(),
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
            self.platform_state.clone(),
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
        StorageManager::get_string(self.platform_state.clone(), StorageProperty::Locality).await
    }

    async fn locality_set(
        &self,
        _ctx: CallContext,
        set_request: SetStringProperty,
    ) -> RpcResult<()> {
        StorageManager::set_string(
            self.platform_state.clone(),
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
        match LocalizationImpl::postal_code(self.platform_state.clone(), ctx.app_id).await {
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
            self.platform_state.clone(),
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
        StorageManager::get_string(self.platform_state.clone(), StorageProperty::Locale).await
    }

    async fn locale_set(&self, _ctx: CallContext, set_request: SetStringProperty) -> RpcResult<()> {
        StorageManager::set_string(
            self.platform_state.clone(),
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
        StorageManager::get_string(self.platform_state.clone(), StorageProperty::LatLon).await
    }

    async fn latlon_set(&self, _ctx: CallContext, set_request: SetStringProperty) -> RpcResult<()> {
        StorageManager::set_string(
            self.platform_state.clone(),
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

    async fn add_additional_info(
        &self,
        _ctx: CallContext,
        set_map_entry_property: SetMapEntryProperty,
    ) -> RpcResult<()> {
        update_additional_info(
            self.platform_state.clone(),
            MapEntryProperty::Set(set_map_entry_property),
        )
        .await
    }

    async fn remove_additional_info(
        &self,
        _ctx: CallContext,
        remove_map_entry_property: RemoveMapEntryProperty,
    ) -> RpcResult<()> {
        update_additional_info(
            self.platform_state.clone(),
            MapEntryProperty::Remove(remove_map_entry_property),
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
