use std::collections::HashMap;
use std::sync::RwLock;
use std::{str::FromStr, sync::atomic::AtomicU32};

pub static LOG_COUNTER: AtomicU32 = AtomicU32::new(1);

lazy_static::lazy_static! {
    pub static ref MODULE_LOG_LEVELS: RwLock<HashMap<String, log::LevelFilter>> = RwLock::new(HashMap::new());
}

pub fn init_logger(name: String) -> Result<(), fern::InitError> {
    let log_string: String = std::env::var("RUST_LOG").unwrap_or_else(|_| "debug".into());
    println!("log level {}", log_string);
    let filter = log::LevelFilter::from_str(&log_string).unwrap_or(log::LevelFilter::Info);
    fern::Dispatch::new()
        .format(move |out, message, record| {
            #[cfg(not(feature = "sysd"))]
            return out.finish(format_args!(
                "{}[{}][{}][{}]-{}",
                chrono::Local::now().format("%Y-%m-%d-%H:%M:%S.%3f"),
                record.level(),
                record.target(),
                name,
                message
            ));
            #[cfg(feature = "sysd")]
            return out.finish(format_args!(
                "[{}][{}][{}]-{}",
                record.level(),
                record.target(),
                name,
                message
            ));
        })
        .level(filter)
        //log filter applied here, making the log level to OFF for the below mentioned crates
        .level_for("h2", log::LevelFilter::Off)
        .level_for("hyper", log::LevelFilter::Off)
        .level_for("rustls", log::LevelFilter::Off)
        .level_for("tower", log::LevelFilter::Off)
        .level_for("tower_http", log::LevelFilter::Off)
        .level_for("jsonrpsee_client_transport", log::LevelFilter::Off)
        .level_for("jsonrpsee_core", log::LevelFilter::Off)
        .chain(std::io::stdout())
        .apply()?;
    Ok(())
}

pub fn init_and_configure_logger(
    version: &str,
    name: String,
    additional_modules: Option<Vec<(String, log::LevelFilter)>>,
) -> Result<(), fern::InitError> {
    let log_string: String = std::env::var("RUST_LOG").unwrap_or_else(|_| "debug".into());
    println!("log level {}", log_string);
    let _version_string = version.to_string();
    let filter = log::LevelFilter::from_str(&log_string).unwrap_or(log::LevelFilter::Info);
    let mut dispatch = fern::Dispatch::new()
        .format(move |out, message, record| {
            let _v = LOG_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            #[cfg(not(feature = "sysd"))]
            if _v % 100 == 0 {
                LOG_COUNTER.store(1, std::sync::atomic::Ordering::Relaxed);
                return out.finish(format_args!(
                    "{}[{}][{}][{}][{}]-{}",
                    chrono::Local::now().format("%Y-%m-%d-%H:%M:%S.%3f"),
                    record.level(),
                    record.target(),
                    name,
                    _version_string,
                    message
                ));
            } else {
                return out.finish(format_args!(
                    "{}[{}][{}][{}]-{}",
                    chrono::Local::now().format("%Y-%m-%d-%H:%M:%S.%3f"),
                    record.level(),
                    record.target(),
                    name,
                    message
                ));
            }
            #[cfg(feature = "sysd")]
            if _v % 100 == 0 {
                LOG_COUNTER.store(1, std::sync::atomic::Ordering::Relaxed);
                return out.finish(format_args!(
                    "[{}][{}][{}][{}]-{}",
                    record.level(),
                    record.target(),
                    name,
                    _version_string,
                    message
                ));
            } else {
                return out.finish(format_args!(
                    "[{}][{}][{}]-{}",
                    record.level(),
                    record.target(),
                    name,
                    message
                ));
            }
        })
        .level(filter)
        //log filter applied here, making the log level to OFF for the below mentioned crates
        .level_for("h2", log::LevelFilter::Off)
        .level_for("hyper", log::LevelFilter::Off)
        .level_for("rustls", log::LevelFilter::Off)
        .level_for("tower", log::LevelFilter::Off)
        .level_for("tower_http", log::LevelFilter::Off)
        .level_for("jsonrpsee_client_transport", log::LevelFilter::Off)
        .level_for("jsonrpsee_core", log::LevelFilter::Off)
        .level_for("tokio_tungstenite", log::LevelFilter::Off)
        .level_for("tungstenite", log::LevelFilter::Off)
        .level_for("soketto", log::LevelFilter::Off)
        .level_for("tracing", log::LevelFilter::Off)
        .chain(std::io::stdout());

    if let Some(modules) = additional_modules {
        let mut log_levels = MODULE_LOG_LEVELS.write().unwrap();
        for (module_name, log_level) in modules {
            println!("Setting log level for {} to {:?}", module_name, log_level);
            dispatch = dispatch.level_for(module_name.clone(), log_level);
            log_levels.insert(module_name, log_level);
        }
    }
    dispatch.apply()?;

    Ok(())
}
