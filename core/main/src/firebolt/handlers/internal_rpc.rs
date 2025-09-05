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

use jsonrpsee::{core::RpcResult, proc_macros::rpc, RpcModule};
use ripple_sdk::{
    api::{
        apps::{AppEvent, AppManagerResponse, AppMethod, AppRequest, AppResponse},
        caps::CapsRequest,
        firebolt::{
            fb_discovery::{AgePolicy, PolicyIdentifierAlias},
            fb_general::ListenRequestWithEvent,
            fb_telemetry::TelemetryPayload,
        },
        gateway::rpc_gateway_api::CallContext,
    },
    async_trait::async_trait,
    log::{debug, error},
    tokio::sync::oneshot,
};

use serde::de::Error as DeError;
use serde::{Deserialize, Deserializer};
use serde_json::Value;
use std::{
    collections::{BTreeMap, HashMap},
    sync::{Arc, RwLock},
};

use crate::{
    firebolt::rpc::RippleRPCProvider,
    service::{apps::app_events::AppEvents, telemetry_builder::TelemetryBuilder},
    state::platform_state::PlatformState,
    utils::rpc_utils::rpc_await_oneshot,
};
// ---- strict parser: ONLY ["app:child", "app:teen", ...] ----
fn parse_policy_identifier_alias_strict_enum<'de, D>(
    d: D,
) -> Result<Option<PolicyIdentifierAlias>, D::Error>
where
    D: Deserializer<'de>,
{
    // GET path: field missing or null => None
    let raw: Option<Vec<String>> = Option::deserialize(d)?;
    let Some(items) = raw else { return Ok(None) };

    let mut out = Vec::with_capacity(items.len());
    for s in items {
        let s = s.trim();
        if s.is_empty() {
            return Err(D::Error::custom("alias must be a non-empty string"));
        }
        // Enforce "app:<policy>"
        let (prefix, suffix) = s.split_once(':').ok_or_else(|| {
            D::Error::custom(format!(
                "alias \"{s}\" must be of the form \"app:<policy>\""
            ))
        })?;
        if !prefix.eq_ignore_ascii_case("app") {
            return Err(D::Error::custom(format!(
                "unsupported prefix \"{prefix}\" (expected \"app\")"
            )));
        }

        let pol = match suffix.to_ascii_lowercase().as_str() {
            "child" => AgePolicy::Child,
            "teen" => AgePolicy::Teen,
            "adult" => AgePolicy::Adult,
            other => {
                return Err(D::Error::custom(format!(
                    "unknown policy \"{other}\" (expected child|teen|adult)"
                )))
            }
        };
        out.push(pol);
    }

    Ok(Some(PolicyIdentifierAlias {
        policy_identifier_alias: out,
    }))
}

// Wrapper for jsonrpsee params (keeps your current signature; tolerates injected extras)
#[derive(Debug, Deserialize, Default)]
pub struct PolicyIdentifierAliasArgs {
    #[serde(
        default,
        alias = "policyIdentifierAlias",
        deserialize_with = "parse_policy_identifier_alias_strict_enum"
    )]
    request: Option<PolicyIdentifierAlias>,

    // ignore other keys you inject into params
    #[serde(flatten)]
    _extras: BTreeMap<String, Value>,
}
#[rpc(server)]
pub trait Internal {
    #[method(name = "ripple.sendTelemetry")]
    async fn send_telemetry(&self, ctx: CallContext, payload: TelemetryPayload) -> RpcResult<()>;

    #[method(name = "ripple.setTelemetrySessionId")]
    fn set_telemetry_session_id(&self, ctx: CallContext, session_id: String) -> RpcResult<()>;

    #[method(name = "ripple.sendAppEvent")]
    async fn send_app_event(&self, ctx: CallContext, event: AppEvent) -> RpcResult<()>;

    #[method(name = "ripple.registerAppEvent")]
    async fn register_app_event(
        &self,
        ctx: CallContext,
        request: ListenRequestWithEvent,
    ) -> RpcResult<()>;

    #[method(name = "ripple.getAppCatalogId")]
    async fn get_app_catalog_id(&self, ctx: CallContext, app_id: String) -> RpcResult<String>;

    #[method(name = "ripple.checkCapsRequest")]
    async fn check_caps_request(
        &self,
        ctx: CallContext,
        caps_request: CapsRequest,
    ) -> RpcResult<HashMap<String, bool>>;

    #[method(name = "account.policyIdentifierAlias")]
    async fn policy_identifier_alias(
        &self,
        ctx: CallContext,
        args: PolicyIdentifierAliasArgs,
    ) -> RpcResult<Option<PolicyIdentifierAlias>>;
    #[method(name = "account.getPolicyIdentifierAlias")]
    async fn get_policy_identifier_alias(&self, ctx: CallContext) -> RpcResult<Vec<AgePolicy>>;
}

#[derive(Debug, Clone, Default)]
pub struct PolicyState {
    pub policy_identifiers_alias: Arc<RwLock<Vec<AgePolicy>>>,
}

#[derive(Debug)]
pub struct InternalImpl {
    pub state: PlatformState,
}

#[async_trait]
impl InternalServer for InternalImpl {
    async fn send_telemetry(&self, _ctx: CallContext, payload: TelemetryPayload) -> RpcResult<()> {
        let _ = TelemetryBuilder::send_telemetry(&self.state, payload);
        Ok(())
    }

    fn set_telemetry_session_id(&self, _ctx: CallContext, session_id: String) -> RpcResult<()> {
        self.state.metrics.update_session_id(Some(session_id));
        Ok(())
    }

    async fn send_app_event(&self, _ctx: CallContext, event: AppEvent) -> RpcResult<()> {
        debug!("Sending App event {:?}", &event);
        AppEvents::emit_with_context(&self.state, &event.event_name, &event.result, event.context)
            .await;
        Ok(())
    }

    async fn register_app_event(
        &self,
        _ctx: CallContext,
        request: ListenRequestWithEvent,
    ) -> RpcResult<()> {
        debug!("registering App event {:?}", &request);
        let event = request.event.clone();
        AppEvents::add_listener(&self.state, event, request.context.clone(), request.request);
        Ok(())
    }

    async fn get_app_catalog_id(&self, _: CallContext, app_id: String) -> RpcResult<String> {
        let (app_resp_tx, app_resp_rx) = oneshot::channel::<AppResponse>();

        let app_request =
            AppRequest::new(AppMethod::GetAppContentCatalog(app_id.clone()), app_resp_tx);
        if let Err(e) = self.state.get_client().send_app_request(app_request) {
            error!("Send error for AppMethod::GetAppContentCatalog {:?}", e);
        }
        let resp = rpc_await_oneshot(app_resp_rx).await;

        if let Ok(Ok(AppManagerResponse::AppContentCatalog(content_catalog))) = resp {
            return Ok(content_catalog.map_or(app_id.to_owned(), |x| x));
        }

        Ok(app_id)
    }

    async fn check_caps_request(
        &self,
        _ctx: CallContext,
        caps_request: CapsRequest,
    ) -> RpcResult<HashMap<String, bool>> {
        match caps_request {
            CapsRequest::Supported(request) => {
                let result = self.state.cap_state.generic.check_for_processor(request);
                Ok(result)
            }
            CapsRequest::Permitted(app_id, request) => {
                let result = self
                    .state
                    .cap_state
                    .permitted_state
                    .check_multiple(&app_id, request);
                Ok(result)
            }
        }
    }

    async fn policy_identifier_alias(
        &self,
        _ctx: CallContext,
        args: PolicyIdentifierAliasArgs,
    ) -> RpcResult<Option<PolicyIdentifierAlias>> {
        debug!("Setting policy identifier alias: {:?}", args);
        match args.request {
            Some(policy_args) => {
                self.state.add_policy_identifier_alias(policy_args);
                Ok(None)
            }
            /*get */
            None => Ok(Some(PolicyIdentifierAlias {
                policy_identifier_alias: self.state.get_policy_identifier_alias(),
            })),
        }
    }

    async fn get_policy_identifier_alias(&self, _ctx: CallContext) -> RpcResult<Vec<AgePolicy>> {
        Ok(self.state.get_policy_identifier_alias())
    }
}

pub struct InternalProvider;
impl RippleRPCProvider<InternalImpl> for InternalProvider {
    fn provide(state: PlatformState) -> RpcModule<InternalImpl> {
        (InternalImpl { state }).into_rpc()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};

    // Helper function to test the custom deserializer directly
    fn test_deserialize_policy_args(
        json_value: Value,
    ) -> Result<PolicyIdentifierAliasArgs, serde_json::Error> {
        serde_json::from_value(json_value)
    }

    #[test]
    fn test_parse_policy_identifier_alias_valid_child() {
        let json_data = json!({
            "policyIdentifierAlias": ["app:child"]
        });

        let result = test_deserialize_policy_args(json_data);
        assert!(result.is_ok());

        let args = result.unwrap();
        assert!(args.request.is_some());

        let policy_alias = args.request.unwrap();
        assert_eq!(policy_alias.policy_identifier_alias.len(), 1);
        assert_eq!(policy_alias.policy_identifier_alias[0], AgePolicy::Child);
    }

    #[test]
    fn test_parse_policy_identifier_alias_valid_teen() {
        let json_data = json!({
            "policyIdentifierAlias": ["app:teen"]
        });

        let result = test_deserialize_policy_args(json_data);
        assert!(result.is_ok());

        let args = result.unwrap();
        assert!(args.request.is_some());

        let policy_alias = args.request.unwrap();
        assert_eq!(policy_alias.policy_identifier_alias.len(), 1);
        assert_eq!(policy_alias.policy_identifier_alias[0], AgePolicy::Teen);
    }

    #[test]
    fn test_parse_policy_identifier_alias_valid_adult() {
        let json_data = json!({
            "policyIdentifierAlias": ["app:adult"]
        });

        let result = test_deserialize_policy_args(json_data);
        assert!(result.is_ok());

        let args = result.unwrap();
        assert!(args.request.is_some());

        let policy_alias = args.request.unwrap();
        assert_eq!(policy_alias.policy_identifier_alias.len(), 1);
        assert_eq!(policy_alias.policy_identifier_alias[0], AgePolicy::Adult);
    }

    #[test]
    fn test_parse_policy_identifier_alias_multiple_valid() {
        let json_data = json!({
            "policyIdentifierAlias": ["app:child", "app:teen", "app:adult"]
        });

        let result = test_deserialize_policy_args(json_data);
        assert!(result.is_ok());

        let args = result.unwrap();
        assert!(args.request.is_some());

        let policy_alias = args.request.unwrap();
        assert_eq!(policy_alias.policy_identifier_alias.len(), 3);
        assert_eq!(policy_alias.policy_identifier_alias[0], AgePolicy::Child);
        assert_eq!(policy_alias.policy_identifier_alias[1], AgePolicy::Teen);
        assert_eq!(policy_alias.policy_identifier_alias[2], AgePolicy::Adult);
    }

    #[test]
    fn test_parse_policy_identifier_alias_case_insensitive_prefix() {
        let json_data = json!({
            "policyIdentifierAlias": ["APP:child", "App:teen", "app:adult"]
        });

        let result = test_deserialize_policy_args(json_data);
        assert!(result.is_ok());

        let args = result.unwrap();
        assert!(args.request.is_some());

        let policy_alias = args.request.unwrap();
        assert_eq!(policy_alias.policy_identifier_alias.len(), 3);
        assert_eq!(policy_alias.policy_identifier_alias[0], AgePolicy::Child);
        assert_eq!(policy_alias.policy_identifier_alias[1], AgePolicy::Teen);
        assert_eq!(policy_alias.policy_identifier_alias[2], AgePolicy::Adult);
    }

    #[test]
    fn test_parse_policy_identifier_alias_case_insensitive_suffix() {
        let json_data = json!({
            "policyIdentifierAlias": ["app:CHILD", "app:Teen", "app:ADULT"]
        });

        let result = test_deserialize_policy_args(json_data);
        assert!(result.is_ok());

        let args = result.unwrap();
        assert!(args.request.is_some());

        let policy_alias = args.request.unwrap();
        assert_eq!(policy_alias.policy_identifier_alias.len(), 3);
        assert_eq!(policy_alias.policy_identifier_alias[0], AgePolicy::Child);
        assert_eq!(policy_alias.policy_identifier_alias[1], AgePolicy::Teen);
        assert_eq!(policy_alias.policy_identifier_alias[2], AgePolicy::Adult);
    }

    #[test]
    fn test_parse_policy_identifier_alias_whitespace_handling() {
        let json_data = json!({
            "policyIdentifierAlias": ["  app:child  ", "\tapp:teen\t", "\napp:adult\n"]
        });

        let result = test_deserialize_policy_args(json_data);
        assert!(result.is_ok());

        let args = result.unwrap();
        assert!(args.request.is_some());

        let policy_alias = args.request.unwrap();
        assert_eq!(policy_alias.policy_identifier_alias.len(), 3);
        assert_eq!(policy_alias.policy_identifier_alias[0], AgePolicy::Child);
        assert_eq!(policy_alias.policy_identifier_alias[1], AgePolicy::Teen);
        assert_eq!(policy_alias.policy_identifier_alias[2], AgePolicy::Adult);
    }

    #[test]
    fn test_parse_policy_identifier_alias_empty_array() {
        let json_data = json!({
            "policyIdentifierAlias": []
        });

        let result = test_deserialize_policy_args(json_data);
        assert!(result.is_ok());

        let args = result.unwrap();
        assert!(args.request.is_some());

        let policy_alias = args.request.unwrap();
        assert_eq!(policy_alias.policy_identifier_alias.len(), 0);
    }

    #[test]
    fn test_parse_policy_identifier_alias_missing_field() {
        let json_data = json!({
            "otherField": "value"
        });

        let result = test_deserialize_policy_args(json_data);
        assert!(result.is_ok());

        let args = result.unwrap();
        assert!(args.request.is_none());
    }

    #[test]
    fn test_parse_policy_identifier_alias_null_field() {
        let json_data = json!({
            "policyIdentifierAlias": null
        });

        let result = test_deserialize_policy_args(json_data);
        assert!(result.is_ok());

        let args = result.unwrap();
        assert!(args.request.is_none());
    }

    // Error cases - Invalid prefix
    #[test]
    fn test_parse_policy_identifier_alias_invalid_prefix() {
        let json_data = json!({
            "policyIdentifierAlias": ["user:child"]
        });

        let result = test_deserialize_policy_args(json_data);
        assert!(result.is_err());

        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("unsupported prefix"));
        assert!(error_msg.contains("user"));
        assert!(error_msg.contains("expected \"app\""));
    }

    #[test]
    fn test_parse_policy_identifier_alias_multiple_invalid_prefixes() {
        let json_data = json!({
            "policyIdentifierAlias": ["admin:child", "system:teen"]
        });

        let result = test_deserialize_policy_args(json_data);
        assert!(result.is_err());

        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("unsupported prefix"));
        assert!(error_msg.contains("admin"));
    }

    // Error cases - Invalid format
    #[test]
    fn test_parse_policy_identifier_alias_no_colon() {
        let json_data = json!({
            "policyIdentifierAlias": ["appchild"]
        });

        let result = test_deserialize_policy_args(json_data);
        assert!(result.is_err());

        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("must be of the form"));
        assert!(error_msg.contains("app:<policy>"));
    }

    #[test]
    fn test_parse_policy_identifier_alias_multiple_colons() {
        let json_data = json!({
            "policyIdentifierAlias": ["app:child:extra"]
        });

        let result = test_deserialize_policy_args(json_data);
        assert!(result.is_err()); // split_once only splits on first colon
    }

    #[test]
    fn test_parse_policy_identifier_alias_empty_prefix() {
        let json_data = json!({
            "policyIdentifierAlias": [":child"]
        });

        let result = test_deserialize_policy_args(json_data);
        assert!(result.is_err());

        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("unsupported prefix"));
        assert!(error_msg.contains("expected \"app\""));
    }

    #[test]
    fn test_parse_policy_identifier_alias_empty_suffix() {
        let json_data = json!({
            "policyIdentifierAlias": ["app:"]
        });

        let result = test_deserialize_policy_args(json_data);
        assert!(result.is_err());

        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("unknown policy"));
        assert!(error_msg.contains("expected child|teen|adult"));
    }

    // Error cases - Unknown policy
    #[test]
    fn test_parse_policy_identifier_alias_unknown_policy() {
        let json_data = json!({
            "policyIdentifierAlias": ["app:unknown"]
        });

        let result = test_deserialize_policy_args(json_data);
        assert!(result.is_err());

        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("unknown policy"));
        assert!(error_msg.contains("unknown"));
        assert!(error_msg.contains("expected child|teen|adult"));
    }

    #[test]
    fn test_parse_policy_identifier_alias_invalid_policy_names() {
        let test_cases = vec![
            "app:baby",
            "app:toddler",
            "app:elderly",
            "app:senior",
            "app:minor",
            "app:youth",
        ];

        for invalid_policy in test_cases {
            let json_data = json!({
                "policyIdentifierAlias": [invalid_policy]
            });

            let result = test_deserialize_policy_args(json_data);
            assert!(
                result.is_err(),
                "Should fail for policy: {}",
                invalid_policy
            );

            let error_msg = result.unwrap_err().to_string();
            assert!(error_msg.contains("unknown policy"));
            assert!(error_msg.contains("expected child|teen|adult"));
        }
    }

    // Error cases - Empty strings
    #[test]
    fn test_parse_policy_identifier_alias_empty_string() {
        let json_data = json!({
            "policyIdentifierAlias": [""]
        });

        let result = test_deserialize_policy_args(json_data);
        assert!(result.is_err());

        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("alias must be a non-empty string"));
    }

    #[test]
    fn test_parse_policy_identifier_alias_whitespace_only() {
        let json_data = json!({
            "policyIdentifierAlias": ["   ", "\t", "\n"]
        });

        let result = test_deserialize_policy_args(json_data);
        assert!(result.is_err());

        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("alias must be a non-empty string"));
    }

    #[test]
    fn test_parse_policy_identifier_alias_mixed_valid_and_empty() {
        let json_data = json!({
            "policyIdentifierAlias": ["app:child", ""]
        });

        let result = test_deserialize_policy_args(json_data);
        assert!(result.is_err());

        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("alias must be a non-empty string"));
    }

    // Test with extra fields (should be ignored)
    #[test]
    fn test_parse_policy_identifier_alias_with_extra_fields() {
        let json_data = json!({
            "policyIdentifierAlias": ["app:child", "app:teen"],
            "extraField1": "should be ignored",
            "extraField2": 42,
            "extraField3": {"nested": "object"}
        });

        let result = test_deserialize_policy_args(json_data);
        assert!(result.is_ok());

        let args = result.unwrap();
        assert!(args.request.is_some());
        assert_eq!(args._extras.len(), 3);
        assert!(args._extras.contains_key("extraField1"));
        assert!(args._extras.contains_key("extraField2"));
        assert!(args._extras.contains_key("extraField3"));

        let policy_alias = args.request.unwrap();
        assert_eq!(policy_alias.policy_identifier_alias.len(), 2);
        assert_eq!(policy_alias.policy_identifier_alias[0], AgePolicy::Child);
        assert_eq!(policy_alias.policy_identifier_alias[1], AgePolicy::Teen);
    }

    // Test field name aliases
    #[test]
    fn test_parse_policy_identifier_alias_camel_case_field() {
        let json_data = json!({
            "policyIdentifierAlias": ["app:adult"]
        });

        let result = test_deserialize_policy_args(json_data);
        assert!(result.is_ok());

        let args = result.unwrap();
        assert!(args.request.is_some());

        let policy_alias = args.request.unwrap();
        assert_eq!(policy_alias.policy_identifier_alias.len(), 1);
        assert_eq!(policy_alias.policy_identifier_alias[0], AgePolicy::Adult);
    }

    // Edge case: very long policy names (should fail)
    #[test]
    fn test_parse_policy_identifier_alias_long_invalid_policy() {
        let long_policy = "very_long_invalid_policy_name_that_should_definitely_fail";
        let json_data = json!({
            "policyIdentifierAlias": [format!("app:{}", long_policy)]
        });

        let result = test_deserialize_policy_args(json_data);
        assert!(result.is_err());

        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("unknown policy"));
        assert!(error_msg.contains(long_policy));
    }

    // Test duplicate policies (should be allowed)
    #[test]
    fn test_parse_policy_identifier_alias_duplicates() {
        let json_data = json!({
            "policyIdentifierAlias": ["app:child", "app:child", "app:teen", "app:child"]
        });

        let result = test_deserialize_policy_args(json_data);
        assert!(result.is_ok());

        let args = result.unwrap();
        assert!(args.request.is_some());

        let policy_alias = args.request.unwrap();
        assert_eq!(policy_alias.policy_identifier_alias.len(), 4); // Duplicates preserved
        assert_eq!(policy_alias.policy_identifier_alias[0], AgePolicy::Child);
        assert_eq!(policy_alias.policy_identifier_alias[1], AgePolicy::Child);
        assert_eq!(policy_alias.policy_identifier_alias[2], AgePolicy::Teen);
        assert_eq!(policy_alias.policy_identifier_alias[3], AgePolicy::Child);
    }

    // Test wrong data type
    #[test]
    fn test_parse_policy_identifier_alias_wrong_type_string() {
        let json_data = json!({
            "policyIdentifierAlias": "app:child"  // Should be array, not string
        });

        let result = test_deserialize_policy_args(json_data);
        assert!(result.is_err());
        // This should fail at the serde level before reaching our custom deserializer
    }

    #[test]
    fn test_parse_policy_identifier_alias_wrong_type_object() {
        let json_data = json!({
            "policyIdentifierAlias": {"policy": "app:child"}  // Should be array, not object
        });

        let result = test_deserialize_policy_args(json_data);
        assert!(result.is_err());
        // This should fail at the serde level before reaching our custom deserializer
    }

    #[test]
    fn test_parse_policy_identifier_alias_wrong_type_number() {
        let json_data = json!({
            "policyIdentifierAlias": 42  // Should be array, not number
        });

        let result = test_deserialize_policy_args(json_data);
        assert!(result.is_err());
        // This should fail at the serde level before reaching our custom deserializer
    }
}
