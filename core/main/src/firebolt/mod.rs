//pub mod rpc_gateway;
//pub mod firebolt_gateway;
pub mod handlers {
    pub mod device_rpc;
    pub mod lcm_rpc;
    pub mod lifecycle_rpc;
    pub mod capabilities_rpc;
}
pub mod firebolt_gatekeeper;
pub mod firebolt_gateway;
pub mod firebolt_ws;
pub mod rpc;
pub mod rpc_router;
