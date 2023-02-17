use std::collections::HashMap;

use serde::Deserialize;

use super::device_manifest::DeviceManifest;

#[derive(Debug, Clone, Deserialize)]
pub struct AppAuthorizationRules {
    pub app_ignore_rules: HashMap<String, Vec<String>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ExclusoryImpl {
    pub resolve_only: Option<Vec<String>>,
    pub app_authorization_rules: AppAuthorizationRules,
    /*
    method names to ignore regardless of appid
    */
    pub method_ignore_rules: Vec<String>,
}

impl ExclusoryImpl {
    pub fn get(dm: DeviceManifest) -> ExclusoryImpl {
        if let Some(e) = dm.configuration.exclusory {
            return e;
        }
        ExclusoryImpl {
            resolve_only: None,
            method_ignore_rules: vec!["*".into()],
            app_authorization_rules: AppAuthorizationRules {
                app_ignore_rules: HashMap::new(),
            },
        }
    }
}

impl Exclusory for ExclusoryImpl {
    fn can_resolve(&self, method: String) -> bool {
        if let Some(r) = &self.resolve_only {
            if r.len() > 0 {
                return r.contains(&method);
            }
        }
        true
    }

    fn is_excluded(&self, app_id: String, method: String) -> bool {
        /*dangerous, but possible... * method matcher,let em all in  */
        if self.clone().is_all_excluded() || self.clone().is_method_excluded(method.clone()) {
            return true;
        };

        let default: Vec<String> = vec![];
        let methods = self
            .app_authorization_rules
            .app_ignore_rules
            .get(&app_id)
            .unwrap_or(&default);
        if methods.contains(&String::from("*")) {
            return true;
        };
        return methods.contains(&method);
    }

    fn is_all_excluded(&self) -> bool {
        if self.method_ignore_rules.contains(&String::from("*")) {
            return true;
        } else {
            false
        }
    }

    fn is_method_excluded(&self, method: String) -> bool {
        if self.method_ignore_rules.contains(&method) {
            return true;
        } else {
            false
        }
    }
}
pub trait Exclusory {
    fn is_excluded(&self, app_id: String, method: String) -> bool;
    fn is_all_excluded(&self) -> bool;
    fn is_method_excluded(&self, method: String) -> bool;
    fn can_resolve(&self, method: String) -> bool;
}
