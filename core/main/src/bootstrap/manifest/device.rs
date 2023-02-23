use ripple_sdk::{
    api::manifest::device_manifest::DeviceManifest, log::info, serde_json,
    utils::error::RippleError,
};

pub struct LoadDeviceManifestStep;

impl LoadDeviceManifestStep {
    pub fn get_manifest() -> DeviceManifest {
        let r = try_manifest_files();
        if r.is_ok() {
            return r.unwrap();
        }

        load_default_manifest()
    }
}

fn try_manifest_files() -> Result<DeviceManifest, RippleError> {
    let dm_arr: Vec<fn() -> Result<(String, DeviceManifest), RippleError>>;
    if cfg!(test) {
        dm_arr = vec![load_from_env];
    } else {
        dm_arr = vec![load_from_env, load_from_home, load_from_opt, load_from_etc];
    }

    for dm_provider in dm_arr {
        if let Ok((p, m)) = dm_provider() {
            info!("loaded_manifest_file_content={}", p);
            return Ok(m);
        }
    }
    Err(RippleError::BootstrapError)
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
