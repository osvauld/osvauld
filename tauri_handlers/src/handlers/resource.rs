//! Resource handlers - Uses Butler facade

use crate::types::{
    AddResourceInput, BaseCryptoResponse, GetResource, ResourceMetadata, ResourceResponse,
    SyncResourceInput, UpdateResourceInput,
};
use butler::{Butler, PageType};
use tracing::{info, instrument};
use std::sync::Arc;
use tauri::State;

#[tauri::command]
#[instrument(skip(input, butler), fields(space_id = %input.folder_id, resource_type = %input.resource_type))]
pub async fn handle_add_resource(
    input: AddResourceInput,
    butler: State<'_, Arc<Butler>>,
) -> Result<BaseCryptoResponse, String> {
    // Parse metadata to extract title
    let metadata: serde_json::Value = serde_json::from_str(&input.metadata_json)
        .map_err(|e| format!("Failed to parse metadata: {}", e))?;
    let title = metadata
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("Untitled")
        .to_string();

    // Extract layer names from permit template
    let layer_names = Butler::extract_layer_names_from_template(&input.permit_template_json);

    // Determine page type
    let page_type = match input.resource_type.as_str() {
        "content" => PageType::Content,
        "comments" => PageType::Comments,
        "submissions" => PageType::Submissions,
        _ => PageType::Content,
    };

    // Create page via Butler (identity is stored internally)
    let page = butler
        .create_page(
            &input.folder_id,
            &title,
            page_type,
            layer_names,
        )
        .await
        .map_err(|e| format!("Failed to create page: {}", e))?;

    info!(page_id = %page.id, "Page created");

    let response = ResourceMetadata {
        id: page.id.clone(),
        title,
        resource_type: input.resource_type,
        folder_id: input.folder_id,
        last_modified: page.created_at,
        favourite: false,
        preview: None,
    };

    Ok(BaseCryptoResponse::ResourceCreated(response))
}

#[tauri::command]
#[instrument(skip(input, butler), fields(page_id = %input.resource_id))]
pub async fn handle_get_resource(
    input: GetResource,
    butler: State<'_, Arc<Butler>>,
) -> Result<BaseCryptoResponse, String> {
    let decrypted_page = butler
        .get_decrypted_page(&input.resource_id)
        .await
        .map_err(|e| format!("Failed to get page: {}", e))?;

    let data = decrypted_page.docs_to_json();

    let response = ResourceResponse {
        id: decrypted_page.id,
        data,
        favourite: false,
        last_accessed: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64,
        folder_id: decrypted_page.space_id,
    };

    Ok(BaseCryptoResponse::SelectedResourceResponse(response))
}

#[tauri::command]
#[instrument(skip(butler))]
pub async fn handle_get_all_resources_metadata(
    butler: State<'_, Arc<Butler>>,
) -> Result<BaseCryptoResponse, String> {
    let pages = butler
        .get_accessible_pages()
        .await
        .map_err(|e| format!("Failed to get pages: {}", e))?;

    let metadata_list: Vec<ResourceMetadata> = pages
        .into_iter()
        .map(|page| {
            let resource_type = match page.page_type {
                PageType::Content => "content",
                PageType::Comments => "comments",
                PageType::Submissions => "submissions",
                PageType::PrivateChat => "private_chat",
            }
            .to_string();

            ResourceMetadata {
                id: page.id,
                title: page.name,
                resource_type,
                folder_id: page.space_id,
                last_modified: page.updated_at,
                favourite: false,
                preview: None,
            }
        })
        .collect();

    Ok(BaseCryptoResponse::ResourcesMetadata(metadata_list))
}

#[tauri::command]
#[instrument(skip(input, butler), fields(page_id = %input.id))]
pub async fn handle_update_resource(
    input: UpdateResourceInput,
    butler: State<'_, Arc<Butler>>,
) -> Result<BaseCryptoResponse, String> {
    let layer_updates: std::collections::HashMap<String, serde_json::Value> =
        serde_json::from_str(&input.data)
            .map_err(|e| format!("Failed to parse update data: {}", e))?;

    let page = butler
        .update_page_layers_auto(&input.id, &layer_updates)
        .await
        .map_err(|e| format!("Failed to update page: {}", e))?;

    let resource_type = match page.page_type {
        PageType::Content => "content",
        PageType::Comments => "comments",
        PageType::Submissions => "submissions",
        PageType::PrivateChat => "private_chat",
    }
    .to_string();

    let response = ResourceMetadata {
        id: page.id,
        title: page.name,
        resource_type,
        folder_id: page.space_id,
        last_modified: page.updated_at,
        favourite: false,
        preview: None,
    };

    Ok(BaseCryptoResponse::ResourceUpdated(response))
}

#[tauri::command]
#[instrument(skip(input), fields(resource_id = %input.resource_id))]
pub async fn handle_sync_resource(
    input: SyncResourceInput,
) -> Result<BaseCryptoResponse, String> {
    // TODO: Implement with Courier
    info!("Resource sync requested: {}", input.resource_id);
    Ok(BaseCryptoResponse::Success)
}
