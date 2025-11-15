// Resource handlers - to be reimplemented with Loro architecture
// See HANDOFF.md and LISTENERS_ARCHITECTURE.md for migration context

use crate::config::HandlerConfig;
use crate::types::{AddResourceInput, BaseCryptoResponse, GetResource, ResourceMetadata, ResourceResponse, SyncResourceInput, UpdateResourceInput};
use crate::user_state::UserState;
use crypto_utils::CryptoUtils;
use tracing::{error, info, instrument};
use network::P2PService;
use persistance::database::RepositoryContext;
use services::{create_resource, get_all_resources_metadata, get_resource_by_id_direct, update_resource};
use std::sync::Arc;
use tauri::State;
use tokio::sync::RwLock;

#[tauri::command]
#[instrument(skip(input, user_state, repo_ctx, ucan_service, config), fields(folder_id = %input.folder_id, resource_type = %input.resource_type))]
pub async fn handle_add_resource(
    input: AddResourceInput,
    user_state: State<'_, UserState>,
    repo_ctx: State<'_, Arc<RepositoryContext>>,
    ucan_service: State<'_, Arc<RwLock<gurkha::UcanService>>>,
    config: State<'_, HandlerConfig>,
) -> Result<BaseCryptoResponse, String> {

    // Get current user and device
    let user = user_state.get_user().await?;
    let device = user_state.get_device().await?;

    // Parse metadata to extract title
    let metadata: serde_json::Value = serde_json::from_str(&input.metadata_json)
        .map_err(|e| format!("Failed to parse metadata: {}", e))?;
    let title = metadata
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("Untitled")
        .to_string();

    // Call service to create resource
    let resource = create_resource(
        input.resource_payload,
        input.ucan_template_json,
        input.metadata_json,
        input.folder_id.clone(),
        &user,
        &device.id,
        &config.domain,
        repo_ctx.inner().clone(),
        &ucan_service,
    )
    .await
    .map_err(|e| e.to_string())?;

    // Build response with metadata
    let response = ResourceMetadata {
        id: resource.id.clone(),
        title,
        resource_type: input.resource_type,
        folder_id: input.folder_id,
        last_modified: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64,
        favourite: false,
        preview: None,
    };

    info!(resource_id = %response.id, title = %response.title, "Resource created successfully");
    Ok(BaseCryptoResponse::ResourceCreated(response))
}

#[tauri::command]
#[instrument(skip(input, repo_ctx, crypto_utils), fields(resource_id = %input.resource_id))]
pub async fn handle_get_resource(
    input: GetResource,
    repo_ctx: State<'_, Arc<RepositoryContext>>,
    crypto_utils: State<'_, Arc<RwLock<CryptoUtils>>>,
) -> Result<BaseCryptoResponse, String> {

    // Fetch and decrypt resource
    let resource = get_resource_by_id_direct(
        &input.resource_id,
        repo_ctx.inner().clone(),
        &crypto_utils,
    )
    .await
    .map_err(|e| e.to_string())?;

    // Convert Resource to JSON format expected by frontend
    let data_json = resource
        .to_json()
        .map_err(|e| format!("Failed to serialize resource: {}", e))?;
    let data: serde_json::Value = serde_json::from_str(&data_json)
        .map_err(|e| format!("Failed to parse resource JSON: {}", e))?;

    // Build response
    let response = ResourceResponse {
        id: resource.id,
        data,
        favourite: false, // TODO: Add favourite tracking
        last_accessed: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64,
        folder_id: resource.folder_id,
    };

    info!(resource_id = %response.id, "Resource fetched successfully");
    Ok(BaseCryptoResponse::SelectedResourceResponse(response))
}

#[tauri::command]
#[instrument(skip(user_state, repo_ctx))]
pub async fn handle_get_all_resources_metadata(
    user_state: State<'_, UserState>,
    repo_ctx: State<'_, Arc<RepositoryContext>>,
) -> Result<BaseCryptoResponse, String> {

    // Get current user
    let user = user_state.get_user().await?;

    // Get all resources metadata (no decryption)
    let resources_with_keys = get_all_resources_metadata(&user.id, repo_ctx.inner().clone())
        .await
        .map_err(|e| e.to_string())?;

    // Convert to ResourceMetadata
    let metadata_list: Vec<ResourceMetadata> = resources_with_keys
        .into_iter()
        .map(|(encrypted_resource, _encrypted_key)| {
            // Parse metadata JSON to extract fields
            let metadata_value = &encrypted_resource.metadata;

            let title = metadata_value
                .get("title")
                .and_then(|v| v.as_str())
                .unwrap_or("Untitled")
                .to_string();

            let resource_type = metadata_value
                .get("type")
                .and_then(|v| v.as_str())
                .unwrap_or("website")
                .to_string();

            let last_modified = metadata_value
                .get("last_modified")
                .and_then(|v| v.as_i64())
                .unwrap_or_else(|| encrypted_resource.updated_at);

            ResourceMetadata {
                id: encrypted_resource.id,
                title,
                resource_type,
                folder_id: encrypted_resource.folder_id,
                last_modified,
                favourite: false, // TODO: Add favourite tracking
                preview: None,    // TODO: Generate previews
            }
        })
        .collect();

    info!(count = metadata_list.len(), "Returning resources metadata");
    Ok(BaseCryptoResponse::ResourcesMetadata(metadata_list))
}

#[tauri::command]
#[instrument(skip(input, user_state, repo_ctx), fields(resource_id = %input.id))]
pub async fn handle_update_resource(
    input: UpdateResourceInput,
    user_state: State<'_, UserState>,
    repo_ctx: State<'_, Arc<RepositoryContext>>,
) -> Result<BaseCryptoResponse, String> {

    // Get current user
    let user = user_state.get_user().await?;

    // Call service to update resource
    update_resource(
        &input.id,
        input.data.clone(),
        &user,
        repo_ctx.inner().clone(),
    )
    .await
    .map_err(|e| e.to_string())?;

    // Fetch updated resource to get new metadata
    let encrypted_resource = repo_ctx
        .resource_repo
        .find_by_id(&input.id)
        .await
        .map_err(|e| format!("Failed to fetch updated resource: {}", e))?;

    // Parse metadata to extract fields
    let metadata_value = &encrypted_resource.metadata;

    let title = metadata_value
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("Untitled")
        .to_string();

    let resource_type = metadata_value
        .get("type")
        .and_then(|v| v.as_str())
        .unwrap_or("website")
        .to_string();

    let last_modified = metadata_value
        .get("last_modified")
        .and_then(|v| v.as_i64())
        .unwrap_or_else(|| encrypted_resource.updated_at);

    // Build response
    let response = ResourceMetadata {
        id: encrypted_resource.id,
        title,
        resource_type,
        folder_id: encrypted_resource.folder_id,
        last_modified,
        favourite: false, // TODO: Add favourite tracking
        preview: None,    // TODO: Generate previews
    };

    info!(resource_id = %response.id, title = %response.title, "Resource updated successfully");
    Ok(BaseCryptoResponse::ResourceUpdated(response))
}

/// Sync a resource with connected peers
///
/// Initiates the resource sync protocol by calling the network layer.
/// The sync_resource function will:
/// 1. Get all users with access to the resource
/// 2. For each user, get their devices and establish connections
/// 3. Send ResourceSyncRequest to start the sync protocol
///
/// This is fire-and-forget - spawned as async task.
#[tauri::command]
#[instrument(skip(input, repo_ctx, crypto_utils, p2p_service), fields(resource_id = %input.resource_id))]
pub async fn handle_sync_resource(
    input: SyncResourceInput,
    repo_ctx: State<'_, Arc<RepositoryContext>>,
    crypto_utils: State<'_, Arc<RwLock<CryptoUtils>>>,
    p2p_service: State<'_, Arc<P2PService>>,
) -> Result<BaseCryptoResponse, String> {

    // Spawn async task for sync
    let resource_id = input.resource_id.clone();
    let repo = repo_ctx.inner().clone();
    let crypto = crypto_utils.inner().clone();
    let p2p = p2p_service.inner().clone();

    tokio::spawn(async move {
        if let Err(e) = network::p2p::sync_handler::sync_resource(
            resource_id.clone(),
            repo,
            crypto,
            p2p,
        )
        .await
        {
            error!("Failed to sync resource {}: {}", resource_id, e);
        }
    });

    info!("Resource sync task spawned");
    Ok(BaseCryptoResponse::Success)
}

// TODO: Implement remaining resource handlers as needed:
// - handle_delete_resource
// - handle_get_resources_for_folder
// - handle_share_resource
// - handle_publish_resource
// - handle_search_resources
// etc.
