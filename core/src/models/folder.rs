use chrono::Local;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Folder {
    pub id: String,
    pub name: String,
    pub default_folder: bool,
    pub description: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub deleted: bool,
    pub deleted_at: Option<i64>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FolderManifestData {
    pub folder_id: String,
    pub recipient_user_ids: Vec<String>,
}
impl Folder {
    pub fn new(name: String, description: Option<String>, default_folder: bool) -> Self {
        let now = Local::now().timestamp_millis();

        Self {
            id: Uuid::new_v4().to_string(),
            name,
            description,
            created_at: now,
            updated_at: now,
            default_folder,
            deleted: false,
            deleted_at: None,
        }
    }
}
