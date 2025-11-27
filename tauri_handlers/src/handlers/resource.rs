//! Resource handlers - Uses Butler's SpaceService

use crate::types::{
    AddResourceInput, BaseCryptoResponse, GetResource, ResourceMetadata, ResourceResponse,
    SyncResourceInput, UpdateResourceInput,
};
use crate::user_state::UserState;
use butler::{SpaceService, PageType};
use tracing::{info, instrument};
use std::sync::Arc;
use tauri::State;

/// Extract layer names from Permit template JSON
fn extract_layer_names_from_template(permit_template_json: &str) -> Vec<String> {
    let Ok(template) = serde_json::from_str::<serde_json::Value>(permit_template_json) else {
        return Vec::new();
    };

    let Some(documents) = template.get("documents").and_then(|d| d.as_object()) else {
        return Vec::new();
    };

    documents
        .iter()
        .filter_map(|(name, config)| {
            let layer_type = config.get("type").and_then(|t| t.as_str()).unwrap_or("crdt");
            if layer_type == "crdt" {
                Some(name.clone())
            } else {
                None
            }
        })
        .collect()
}

#[tauri::command]
#[instrument(skip(input, user_state, space_service), fields(space_id = %input.folder_id, resource_type = %input.resource_type))]
pub async fn handle_add_resource(
    input: AddResourceInput,
    user_state: State<'_, UserState>,
    space_service: State<'_, Arc<SpaceService>>,
) -> Result<BaseCryptoResponse, String> {
    // Parse metadata to extract title
    let metadata: serde_json::Value = serde_json::from_str(&input.metadata_json)
        .map_err(|e| format!("Failed to parse metadata: {}", e))?;
    let title = metadata
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("Untitled")
        .to_string();

    // Get identity for encryption key
    let identity = user_state
        .get_identity()
        .await
        .map_err(|e| format!("Failed to get identity: {}", e))?;
    let owner_did = identity.did().to_string();
    let owner_pub_key = identity.public_encryption_key();

    // Extract layer names from UCAN template
    let layer_names = extract_layer_names_from_template(&input.permit_template_json);

    // Determine page type
    let page_type = match input.resource_type.as_str() {
        "content" => PageType::Content,
        "comments" => PageType::Comments,
        "submissions" => PageType::Submissions,
        _ => PageType::Content,
    };

    // Create page in Butler
    let page = space_service
        .create_page(
            input.folder_id.clone(),
            title.clone(),
            owner_did,
            &owner_pub_key,
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
#[instrument(skip(input, user_state, space_service), fields(page_id = %input.resource_id))]
pub async fn handle_get_resource(
    input: GetResource,
    user_state: State<'_, UserState>,
    space_service: State<'_, Arc<SpaceService>>,
) -> Result<BaseCryptoResponse, String> {
    let identity = user_state
        .get_identity()
        .await
        .map_err(|e| format!("Failed to get identity: {}", e))?;
    let secret_key = identity.secret_encryption_key();
    let user_did = identity.did().to_string();

    let decrypted_page = space_service
        .get_decrypted_page(&input.resource_id, &user_did, &secret_key)
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
#[instrument(skip(user_state, space_service))]
pub async fn handle_get_all_resources_metadata(
    user_state: State<'_, UserState>,
    space_service: State<'_, Arc<SpaceService>>,
) -> Result<BaseCryptoResponse, String> {
    let identity = user_state
        .get_identity()
        .await
        .map_err(|e| format!("Failed to get identity: {}", e))?;
    let user_did = identity.did().to_string();

    let pages = space_service
        .get_accessible_pages(&user_did)
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
#[instrument(skip(input, user_state, space_service), fields(page_id = %input.id))]
pub async fn handle_update_resource(
    input: UpdateResourceInput,
    user_state: State<'_, UserState>,
    space_service: State<'_, Arc<SpaceService>>,
) -> Result<BaseCryptoResponse, String> {
    let identity = user_state
        .get_identity()
        .await
        .map_err(|e| format!("Failed to get identity: {}", e))?;
    let secret_key = identity.secret_encryption_key();

    let layer_updates: std::collections::HashMap<String, serde_json::Value> =
        serde_json::from_str(&input.data)
            .map_err(|e| format!("Failed to parse update data: {}", e))?;

    let page = space_service
        .update_page_layers(&input.id, &secret_key, &layer_updates)
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
