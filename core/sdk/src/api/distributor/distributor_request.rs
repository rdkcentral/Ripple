use serde::{Deserialize, Serialize};

use super::{
    distributor_permissions::PermissionRequest, distributor_session::DistributorSessionRequest,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DistributorRequest {
    Session(DistributorSessionRequest),
    Permission(PermissionRequest),
}
