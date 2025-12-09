//! Space handlers - Uses Butler facade
//!
//! Handlers:
//! - handle_create_space: Create a new space
//! - handle_list_spaces: List all accessible spaces
//! - handle_delete_space: Soft delete a space
//! - handle_share_space: Share space with another user

use crate::types::{BaseCryptoResponse, CreateSpaceInput, DeleteSpaceInput, ShareSpaceInput, SpaceResponse};
use butler::Butler;
use std::sync::Arc;
use tauri::State;
use tracing::{info, instrument};

/// Create a new space
///
/// **Context**: User creates a new space to organize pages
/// **We call**: Butler.create_space with user's DID as owner
/// **We return**: Created space for UI display
#[tauri::command]
#[instrument(skip(input, butler), fields(space_name = %input.name))]
pub async fn handle_create_space(
    input: CreateSpaceInput,
    butler: State<'_, Arc<Butler>>,
) -> Result<BaseCryptoResponse, String> {
    let user = butler.user_info().await.map_err(|e| e.to_string())?;

    let space = butler
        .create_space(input.name.clone(), user.did.clone(), &input.space_template_json)
        .await
        .map_err(|e| format!("Failed to create space: {}", e))?;

    info!("Space created: {} (id: {})", space.name, space.id);

    Ok(BaseCryptoResponse::SpaceCreated(space))
}

/// List all accessible spaces
///
/// **Context**: User views their space list
/// **We call**: Butler.list_spaces
/// **We return**: List of spaces for UI display
#[tauri::command]
#[instrument(skip(butler))]
pub async fn handle_list_spaces(
    butler: State<'_, Arc<Butler>>,
) -> Result<BaseCryptoResponse, String> {
    let spaces = butler.list_spaces().map_err(|e| e.to_string())?;

    let space_responses: Vec<SpaceResponse> = spaces
        .into_iter()
        .map(|space| SpaceResponse {
            id: space.id,
            name: space.name,
            description: space.description.unwrap_or_default(),
            is_default: space.is_default,
        })
        .collect();

    Ok(BaseCryptoResponse::Spaces(space_responses))
}

/// Delete a space (soft delete)
///
/// **Context**: User deletes a space
/// **We call**: Butler.delete_space
#[tauri::command]
#[instrument(skip(input, butler), fields(space_id = %input.space_id))]
pub async fn handle_delete_space(
    input: DeleteSpaceInput,
    butler: State<'_, Arc<Butler>>,
) -> Result<BaseCryptoResponse, String> {
    butler
        .delete_space(&input.space_id)
        .map_err(|e| format!("Failed to delete space: {}", e))?;

    info!("Space deleted: {}", input.space_id);
    Ok(BaseCryptoResponse::Success)
}

/// Share a space with another user
///
/// **Context**: User shares their space with someone else
/// **We call**: Butler.share_space to record the sharing
/// **Note**: Actual permit delegation happens during sync
#[tauri::command]
#[instrument(skip(input, butler), fields(space_id = %input.space_id, user_id = %input.user_id))]
pub async fn handle_share_space(
    input: ShareSpaceInput,
    butler: State<'_, Arc<Butler>>,
) -> Result<BaseCryptoResponse, String> {
    butler
        .share_space(&input.space_id, input.user_id.clone())
        .map_err(|e| format!("Failed to share space: {}", e))?;

    info!("Space shared: {} with {}", input.space_id, input.user_id);
    Ok(BaseCryptoResponse::Success)
}
