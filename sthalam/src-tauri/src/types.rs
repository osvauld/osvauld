// Re-export all shared types from tauri_handlers for convenience
pub use tauri_handlers::types::*;

use serde::{Deserialize, Serialize};

// Sthalam-specific CryptoResponse (extends BaseCryptoResponse)
// For now, we just use BaseCryptoResponse directly by re-exporting it
pub type CryptoResponse = BaseCryptoResponse;

// Project-specific types that aren't in the shared package
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ResourceResponse2 {
    pub id: String,
    pub data: serde_json::Value,
    pub favourite: bool,
    pub last_accessed: i64,
    pub folder_id: String,
    pub resource_type: String,
}
