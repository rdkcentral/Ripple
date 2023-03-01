pub mod config;
pub mod device;
pub mod manifest;
pub mod apps;
pub mod gateway {
    pub mod rpc_error;
    pub mod rpc_gateway_api;
}

pub mod firebolt {
    pub mod fb_discovery;
    pub mod fb_lifecycle;
    pub mod fb_parameters;
    pub mod fb_lifecycle_management;
}
