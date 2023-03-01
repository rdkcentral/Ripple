use ripple_sdk::{
    api::manifest::extn_manifest::ExtnManifest, log::info, serde_json, utils::error::RippleError,
};

pub struct LoadExtnManifestStep;

impl LoadExtnManifestStep {
    pub fn get_manifest() -> ExtnManifest {
        let r = try_manifest_files();
        if r.is_ok() {
            return r.unwrap();
        }

        load_default_manifest()
    }
}

fn try_manifest_files() -> Result<ExtnManifest, RippleError> {
    let dm_arr: Vec<fn() -> Result<(String, ExtnManifest), RippleError>>;
    if cfg!(test) {
        dm_arr = vec![load_from_env];
    } else {
        dm_arr = vec![load_from_env, load_from_home, load_from_opt, load_from_etc];
    }

    for dm_provider in dm_arr {
        if let Ok((p, m)) = dm_provider() {
            info!("loaded_extn_file_content={}", p);
            return Ok(m);
        }
    }
    Err(RippleError::BootstrapError)
}

fn load_default_manifest() -> ExtnManifest {
    info!("loading default extn manifest");
    let v = std::include_str!("./default-extn-manifest.json");
    info!("loaded_extn_manifest_file_content={}", v);
    let r: serde_json::Result<ExtnManifest> = serde_json::from_str(&v);
    r.unwrap()
}

fn load_from_env() -> Result<(String, ExtnManifest), RippleError> {
    let device_manifest_path = std::env::var("DEVICE_MANIFEST");
    match device_manifest_path {
        Ok(path) => ExtnManifest::load(path),
        Err(_) => Err(RippleError::MissingInput),
    }
}

fn load_from_home() -> Result<(String, ExtnManifest), RippleError> {
    match std::env::var("HOME") {
        Ok(home) => ExtnManifest::load(format!("{}/.ripple/firebolt-extn-manifest.json", home)),
        Err(_) => Err(RippleError::MissingInput),
    }
}

fn load_from_opt() -> Result<(String, ExtnManifest), RippleError> {
    ExtnManifest::load("/opt/firebolt-extn-manifest.json".into())
}

fn load_from_etc() -> Result<(String, ExtnManifest), RippleError> {
    ExtnManifest::load("/etc/firebolt-extn-manifest.json".into())
}
