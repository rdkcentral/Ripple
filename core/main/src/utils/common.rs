use crate::processor::storage::storage_manager::StorageManager;
use crate::state::platform_state::PlatformState;
use jsonrpsee::core::RpcResult;
use ripple_sdk::{api::storage_property::StoragePropertyData, uuid::Uuid};

const UID_SCOPE: &str = "device";

pub async fn get_uid(state: PlatformState, app_id: String, key: &'static str) -> RpcResult<String> {
    let uid: String;
    let mut data = StoragePropertyData {
        scope: Some(UID_SCOPE.to_string()),
        key,
        namespace: app_id.clone(),
        value: String::new(),
    };

    if let Ok(id) = StorageManager::get_string_for_scope(state.clone(), &data).await {
        uid = id;
    } else {
        // Using app_id as namespace will result in different uid for each app
        data.value = Uuid::new_v4().to_string();
        StorageManager::set_string_for_scope(state.clone(), &data, None).await?;
        uid = data.value;
    }
    Ok(uid)
}
