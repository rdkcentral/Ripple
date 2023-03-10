pub mod apps;
pub mod config;
pub mod device;
pub mod lifecycle;
pub mod manifest;
pub mod status_update;
pub mod gateway {
    pub mod rpc_error;
    pub mod rpc_gateway_api;
}

pub mod distributor {
    pub mod distributor_permissions;
    pub mod distributor_request;
    pub mod distributor_session;
}

pub mod firebolt {
    pub mod fb_capabilities;
    pub mod fb_discovery;
    pub mod fb_general;
    pub mod fb_lifecycle;
    pub mod fb_lifecycle_management;
    pub mod fb_openrpc;
    pub mod fb_parameters;
    pub mod fb_secondscreen;
}
