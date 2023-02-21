use ripple_sdk::{
    api::manifest::device_manifest::DeviceManifest, async_trait::async_trait,
    framework::bootstrap::Bootstep, log::info, serde_json, utils::error::RippleError,
};

use crate::{ state::platform_state::PlatformState};

pub struct LoadDeviceManifestStep;

#[async_trait]
impl Bootstep<PlatformState> for LoadDeviceManifestStep {
    fn get_name(&self) -> String {
        "LoadDeviceManifestStep".into()
    }

    async fn setup(&self, s: PlatformState) -> Result<(), RippleError> {
        info!("Loading Device manifest step");

        let v = try_manifest_files();
        let manifest = if v.is_some() {
            v.unwrap()
        } else {
            load_default_manifest()
        };

        {
            let mut device_manifest = s.device_manifest.write().unwrap();
            let _ = device_manifest.insert(manifest);
        }

        Ok(())
    }
}

fn try_manifest_files() -> Option<DeviceManifest> {
    let dm_arr: Vec<fn() -> Result<(String, DeviceManifest), RippleError>>;
    if cfg!(test) {
        dm_arr = vec![load_from_env];
    } else {
        dm_arr = vec![load_from_env, load_from_home, load_from_opt, load_from_etc];
    }

    for dm_provider in dm_arr {
        if let Ok((p, m)) = dm_provider() {
            info!("loaded_manifest_file_content={}", p);
            return Some(m);
        }
    }
    None
}

fn load_default_manifest() -> DeviceManifest {
    info!("loading default manifest");
    let v = std::include_str!("./default-device-manifest.json");
    info!("loaded_default_manifest_file_content={}", v);
    let r: serde_json::Result<DeviceManifest> = serde_json::from_str(&v);
    r.unwrap()
}

fn load_from_env() -> Result<(String, DeviceManifest), RippleError> {
    let device_manifest_path = std::env::var("DEVICE_MANIFEST");
    match device_manifest_path {
        Ok(path) => DeviceManifest::load(path),
        Err(_) => Err(RippleError::MissingInput),
    }
}

fn load_from_home() -> Result<(String, DeviceManifest), RippleError> {
    match std::env::var("HOME") {
        Ok(home) => DeviceManifest::load(format!("{}/.ripple/firebolt-device-manifest.json", home)),
        Err(_) => Err(RippleError::MissingInput),
    }
}

fn load_from_opt() -> Result<(String, DeviceManifest), RippleError> {
    DeviceManifest::load("/opt/firebolt-device-manifest.json".into())
}

fn load_from_etc() -> Result<(String, DeviceManifest), RippleError> {
    DeviceManifest::load("/etc/firebolt-device-manifest.json".into())
}
