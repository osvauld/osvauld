//! Butler-based commands shared between kunki and sthalam
//!
//! These commands use Butler to query spaces, pages, and layers.

use crate::{Response, error_codes};
use butler::Butler;
use std::sync::Arc;

/// List all spaces
///
/// **Context**: Returns all spaces the user has access to
pub async fn list_spaces(butler: &Arc<Butler>, id: u64) -> Response {
    match butler.spaces().list() {
        Ok(spaces) => {
            let space_list: Vec<serde_json::Value> = spaces.iter().map(|s| {
                serde_json::json!({
                    "id": s.id,
                    "name": s.name,
                })
            }).collect();
            Response::ok(id, serde_json::json!({
                "spaces": space_list,
                "count": space_list.len()
            }))
        }
        Err(e) => Response::err(id, error_codes::OPERATION_FAILED, format!("Failed to list spaces: {:?}", e)),
    }
}

/// List pages in a space
///
/// **Context**: Returns all pages in the specified space
pub async fn list_pages(butler: &Arc<Butler>, space_id: &str, id: u64) -> Response {
    match butler.pages().list(space_id) {
        Ok(pages) => {
            let page_list: Vec<serde_json::Value> = pages.iter().map(|p| {
                serde_json::json!({
                    "id": p.id,
                    "name": p.name,
                })
            }).collect();
            Response::ok(id, serde_json::json!({
                "space_id": space_id,
                "pages": page_list,
                "count": page_list.len()
            }))
        }
        Err(e) => Response::err(id, error_codes::OPERATION_FAILED, format!("Failed to list pages: {:?}", e)),
    }
}

/// List layers in a page
///
/// **Context**: Returns all data layer names in the specified page
pub async fn list_layers(butler: &Arc<Butler>, page_id: &str, id: u64) -> Response {
    match butler.apps().list_data_layers(page_id) {
        Ok(layers) => {
            Response::ok(id, serde_json::json!({
                "layers": layers,
                "count": layers.len()
            }))
        }
        Err(e) => Response::err(id, error_codes::OPERATION_FAILED, format!("Failed to list layers: {:?}", e)),
    }
}

/// Helper to extract string param from JSON
pub fn get_string_param(params: &Option<serde_json::Value>, key: &str) -> Option<String> {
    params
        .as_ref()
        .and_then(|p| p.get(key))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}
