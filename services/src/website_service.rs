use crate::errors::ServiceResult;
use crate::ucan_service;
use osvauld_core::repositories::RepositoryError;
use persistance::database::RepositoryContext;
use std::sync::Arc;
use tracing::{error, info};

/// Check user first_sync status and folder existence for viewer connection
///
/// This function validates that:
/// 1. The viewer's UCAN token contains a valid folder_id
/// 2. The folder exists in the node's database
/// 3. The node user has completed first_sync
///
/// # Arguments
/// * `viewer_ucan_token` - UCAN token from viewer's connection string
/// * `node_user_id` - User ID of the node receiving the connection
/// * `domain` - Domain for UCAN validation (e.g., "sthalam")
/// * `repo_ctx` - Database repository context
///
/// # Returns
/// * `Ok((first_sync_done, folder_exists))` - Tuple of boolean status flags
/// * `Err` - If UCAN parsing fails or database query fails
pub async fn check_user_and_folder_status(
    viewer_ucan_token: &str,
    node_user_id: &str,
    domain: &str,
    repo_ctx: Arc<RepositoryContext>,
) -> ServiceResult<(bool, bool)> {
    info!("Checking user and folder status for viewer connection");

    // 1. Extract folder_id from UCAN token (viewer tokens use request_resources)
    let folder_id = ucan_service::extract_folder_id_from_viewer_token(
        viewer_ucan_token,
        domain,
    )
    .await
    .map_err(|e| {
        error!("Failed to extract folder_id from viewer UCAN: {}", e);
        e
    })?;

    info!("Extracted folder_id from UCAN (not returned to caller)");

    // 2. Check if folder exists
    let folder_exists = match repo_ctx.folder_repo.find_by_id(&folder_id).await {
        Ok(_folder) => {
            info!("Folder {} exists in database", folder_id);
            true
        }
        Err(RepositoryError::NotFound) => {
            info!("Folder {} not found in database", folder_id);
            false
        }
        Err(e) => {
            error!("Database error checking folder existence: {}", e);
            return Err(e.into());
        }
    };

    // 3. Check first_sync status
    let first_sync_done = match repo_ctx.user_repo.get_user_by_id(node_user_id).await {
        Ok(user) => {
            info!("Node user first_sync status: {}", user.first_sync);
            user.first_sync
        }
        Err(e) => {
            error!("Failed to get node user: {}", e);
            return Err(e.into());
        }
    };

    info!(
        "Status check complete: first_sync={}, folder_exists={}",
        first_sync_done, folder_exists
    );

    Ok((first_sync_done, folder_exists))
}
