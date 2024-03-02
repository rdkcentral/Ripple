pub mod bootstrap;
pub mod utils;
pub mod firebolt;
pub mod service;
pub mod state;
pub mod processor;

include!(concat!(env!("OUT_DIR"), "/version.rs"));