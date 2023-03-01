use std::collections::HashMap;
use log::{warn,error};
use serde::Deserialize;
use super::{device_manifest::{AppLibraryEntry, AppManifestLoad}, apps::AppManifest};

#[derive(Clone, Default)]
pub struct AppLibraryState {
    default_apps: Vec<AppLibraryEntry>,
    providers: HashMap<String, String>,
}

impl std::fmt::Debug for AppLibraryState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppLibraryState").finish()
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct DefaultLibrary {
    pub default_library: Vec<AppLibraryEntry>,
}

pub struct AppLibrary {}

impl AppLibraryState {
    pub fn new(default_apps: Vec<AppLibraryEntry>) -> AppLibraryState {
        let providers = AppLibrary::generate_provider_map(&default_apps);
        AppLibraryState {
            default_apps,
            providers,
        }
    }

    pub fn get_all_apps(&self) -> Vec<AppLibraryEntry> {
        self.default_apps.clone()
    }
}

impl AppLibrary {

    pub fn get_provider(state: &AppLibraryState, capability: String) -> Option<String> {
        let provider = state.providers.get(&capability);
        match provider {
            Some(p) => Some(p.clone()),
            None => None,
        }
    }

    pub fn get_manifest(state: &AppLibraryState, app_id: &str) -> Option<AppManifest> {
        let mut itr = state.default_apps.iter();
        let i = itr.position(|x| x.app_id == *app_id);
        if let None = i {
            return None;
        }
        let library_entry = state.default_apps.get(i.unwrap()).unwrap();
        match &library_entry.manifest {
            AppManifestLoad::Remote(_) => {
                error!("Remote manifests not supported yet");
                None
            }
            AppManifestLoad::Local(_) => {
                error!("Local manifests not supported yet");
                None
            }
            AppManifestLoad::Embedded(manifest) => Some(manifest.clone()),
        }
    }

    fn generate_provider_map(apps: &Vec<AppLibraryEntry>) -> HashMap<String, String> {
        let mut map = HashMap::new();

        for app in apps.iter() {
            let manifest = &app.manifest;
            if let AppManifestLoad::Embedded(manifest) = manifest {
                for capability in manifest.capabilities.provided.required.iter() {
                    map.insert(capability.clone(), app.app_id.clone());
                }
                for capability in manifest.capabilities.provided.optional.iter() {
                    map.insert(capability.clone(), app.app_id.clone());
                }
            } else {
                warn!("generate_provider_map: Not supported: {:?}", app.manifest);
            }
        }

        map
    }
}

