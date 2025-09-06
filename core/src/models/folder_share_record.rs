// folder_share_record.rs (Domain Model)
use chrono::Local;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::share_record::{PermissionLevel, ShareOperation};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FolderShareRecord {
    pub id: String,
    pub folder_id: String,
    pub shared_by_user_id: String,
    pub recipient_user_id: String,
    pub permission_level: PermissionLevel,
    pub ucan_token: String,
    pub ucan_cid: String,
    pub operation_type: ShareOperation,
    pub created_at: i64,
    pub updated_at: i64,
}

impl FolderShareRecord {
    pub fn prepare_folder_share_record(
        folder_id: String,
        shared_by_user_id: String,
        recipient_user_id: String,
        permission_level: PermissionLevel,
        ucan_token: String,
        ucan_cid: String,
    ) -> FolderShareRecord {
        let now = Local::now().timestamp_millis();

        FolderShareRecord {
            id: Uuid::new_v4().to_string(),
            folder_id,
            shared_by_user_id,
            recipient_user_id,
            permission_level,
            ucan_token,
            ucan_cid,
            operation_type: ShareOperation::Share,
            created_at: now,
            updated_at: now,
        }
    }
}
