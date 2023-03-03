use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const DISCOVERY_EVENT_ON_NAVIGATE_TO: &'static str = "discovery.onNavigateTo";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct DiscoveryContext {
    pub source: String,
}

impl DiscoveryContext {
    fn new(source: &str) -> DiscoveryContext {
        return DiscoveryContext {
            source: source.to_string(),
        };
    }
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct LaunchRequest {
    #[serde(rename = "appId")]
    pub app_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub intent: Option<NavigationIntent>,
}

impl LaunchRequest {
    pub fn new(
        app_id: String,
        action: String,
        data: Option<Value>,
        source: String,
    ) -> LaunchRequest {
        LaunchRequest {
            app_id,
            intent: Some(NavigationIntent {
                action,
                data,
                context: DiscoveryContext { source },
            }),
        }
    }

    pub fn get_intent(&self) -> NavigationIntent {
        self.intent.clone().unwrap_or_default()
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct NavigationIntent {
    pub action: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
    pub context: DiscoveryContext,
}

impl Default for NavigationIntent {
    fn default() -> NavigationIntent {
        NavigationIntent {
            action: "home".to_string(),
            data: None,
            context: DiscoveryContext::new("device"),
        }
    }
}

impl PartialEq for NavigationIntent {
    fn eq(&self, other: &Self) -> bool {
        self.action.eq(&other.action) && self.context.eq(&other.context)
    }
}
