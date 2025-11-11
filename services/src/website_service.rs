use crate::errors::ServiceResult;
use crate::merge_service;
use crate::ucan_service;
use chrono::Utc;
use crypto_utils::{encrypt_data_for_user, CryptoUtils};
use osvauld_core::models::{
    resource::{EncryptedResource, Resource}, Folder, FolderShareRecord, PermissionLevel, ShareOperation,
    ShareRecord, User,
};
use osvauld_core::repositories::RepositoryError;
use persistance::database::RepositoryContext;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info};
use uuid::Uuid;

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

/// Get folder prepared for sending to viewer (issue UCAN and create share record)
///
/// This function prepares a folder to be sent to a viewer by:
/// 1. Getting the folder from database
/// 2. Issuing a delegated UCAN token for the viewer
/// 3. Creating a FolderShareRecord in memory (not saved - viewer will save it)
///
/// # Arguments
/// * `folder_id` - The folder to send to viewer
/// * `node_user_id` - Node's user ID (used as recipient in share record)
/// * `viewer_role` - Role extracted from viewer's auth token (e.g., "viewer")
/// * `viewer_ucan_pub_key` - Viewer's UCAN public key (for token audience)
/// * `domain` - Domain for UCAN generation
/// * `repo_ctx` - Database repository context
/// * `crypto_utils` - Cryptography utilities
///
/// # Returns
/// * `Ok((Folder, FolderShareRecord))` - Folder with viewer UCAN and share record to send
/// * `Err` - If folder not found or UCAN generation fails
pub async fn get_folder_to_send(
    folder_id: &str,
    node_user_id: &str,
    viewer_role: &str,
    viewer_ucan_pub_key: &str,
    domain: &str,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: &Arc<RwLock<CryptoUtils>>,
) -> ServiceResult<(Folder, FolderShareRecord)> {
    info!("Preparing folder {} for viewer (role: {})", folder_id, viewer_role);

    // 1. Get folder from database
    let folder = repo_ctx.folder_repo.find_by_id(folder_id).await?;
    info!("Retrieved folder from database");

    // 2. Extract folder capabilities for viewer role
    let folder_capabilities = ucan_service::extract_folder_capabilities(
        &folder.ucan,
        viewer_role,
    )
    .await?;
    info!("Extracted {} folder capabilities for viewer role", folder_capabilities.len());

    // 3. Issue delegated folder UCAN for viewer
    let (folder_ucan_token, folder_ucan_cid) = ucan_service::issue_delegated_folder_token(
        folder_id,
        domain,
        folder_capabilities,
        viewer_ucan_pub_key,
        viewer_role,
        crypto_utils,
        &repo_ctx,
    )
    .await?;
    info!("Issued delegated folder UCAN for viewer (CID: {})", folder_ucan_cid);

    // 4. Create FolderShareRecord in memory (not saved - viewer will save it)
    // TODO: Refactor - self-referencing share record is confusing
    let folder_share_record = FolderShareRecord::prepare_folder_share_record(
        folder_id.to_string(),
        node_user_id.to_string(),  // shared_by_user_id (node)
        node_user_id.to_string(),  // recipient_user_id (node itself - self-referencing)
        PermissionLevel::Read,
        folder_ucan_token.clone(),
        folder_ucan_cid,
    );
    info!("Created FolderShareRecord in memory (self-referencing with node user)");

    // 5. Clone folder and replace UCAN with viewer's token
    let mut viewer_folder = folder;
    viewer_folder.ucan = folder_ucan_token;
    info!("Replaced folder UCAN with viewer token");

    Ok((viewer_folder, folder_share_record))
}

/// Prepare single resource for sending to viewer
///
/// This function prepares a resource to be sent to a viewer by:
/// 1. Getting the resource from database
/// 2. Issuing a delegated UCAN token for the viewer (using existing ucan_service)
/// 3. Decrypting, filtering, and re-encrypting the resource for viewer
/// 4. Creating a ShareRecord in memory (not saved - viewer will save it)
///
/// # Arguments
/// * `resource_id` - The resource to prepare
/// * `node_user_id` - Node's user ID (for share record)
/// * `viewer_user` - Viewer's user info (NOT from DB - passed in)
/// * `domain` - Domain for UCAN generation
/// * `repo_ctx` - Database repository context
/// * `crypto_utils` - Cryptography utilities
///
/// # Returns
/// * `Ok((EncryptedResource, ShareRecord))` - Resource and share record to send
/// * `Err` - If resource not found or processing fails
pub async fn prepare_resource_for_viewer(
    resource_id: &str,
    node_user_id: &str,
    viewer_user: &User,
    domain: &str,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: &Arc<RwLock<CryptoUtils>>,
) -> ServiceResult<(EncryptedResource, ShareRecord)> {
    info!("Preparing resource {} for viewer {}", resource_id, viewer_user.username);

    // 1. Get resource from database
    let resource = repo_ctx.resource_repo.find_by_id(resource_id).await?;
    info!("Retrieved resource from database");

    // 2. Issue viewer resource UCAN (using existing ucan_service function)
    let (viewer_resource_ucan, viewer_resource_ucan_cid) =
        ucan_service::create_viewer_resource_token(
            &viewer_user.ucan_pub_key,
            resource_id,
            domain,
            crypto_utils.clone(),
            &repo_ctx,
        )
        .await?;
    info!("Issued viewer resource UCAN (CID: {})", viewer_resource_ucan_cid);

    // 3. Decrypt resource
    let crypto = crypto_utils.read().await;
    let decrypted_json = crypto
        .decrypt_resource(&resource.encrypted_data, &resource.encrypted_key)?;
    drop(crypto);

    let decrypted_resource = Resource::from_decrypted_data(
        resource.id.clone(),
        resource.folder_id.clone(),
        resource.ucan_token.clone(),
        resource.metadata.clone(),
        &decrypted_json,
    )
    .map_err(|e| crate::errors::ServiceError::validation("decrypted_resource", &format!("Failed to parse resource: {}", e)))?;
    info!("Decrypted resource");

    // 4. Filter documents based on viewer UCAN (viewer_template)
    let filtered_snapshots = merge_service::filter_documents_to_send(
        &decrypted_resource,
        &resource.ucan_token,      // Node's UCAN (all docs)
        &viewer_resource_ucan,     // Viewer's UCAN (filtered docs)
    )
    .await?;
    info!("Filtered documents for viewer");

    // 5. Convert filtered snapshots to JSON
    let filtered_json = serde_json::to_string(&filtered_snapshots)
        .map_err(|e| crate::errors::ServiceError::validation("filtered_snapshots", &format!("Serialization failed: {}", e)))?;
    info!("Serialized filtered data");

    // 6. Generate new AES key and encrypt for viewer
    let (new_encrypted_data, new_encrypted_key) =
        encrypt_data_for_user(&filtered_json, &viewer_user.public_key)?;
    info!("Re-encrypted resource for viewer with new AES key");

    // 7. Create new EncryptedResource for viewer
    let viewer_encrypted_resource = resource.re_encrypt_for_recipient(
        new_encrypted_data,
        new_encrypted_key,
        viewer_resource_ucan.clone(),
    );
    info!("Created EncryptedResource for viewer");

    // 8. Create ShareRecord in memory (not saved - viewer will save it)
    let share_record = ShareRecord {
        id: Uuid::new_v4().to_string(),
        resource_id: resource_id.to_string(),
        shared_by_user_id: node_user_id.to_string(),
        recipient_user_id: node_user_id.to_string(), // Self-referencing
        ucan_token: viewer_resource_ucan,
        ucan_cid: viewer_resource_ucan_cid,
        permission_level: PermissionLevel::Read,
        operation_type: ShareOperation::Share,
        created_at: Utc::now().timestamp(),
        updated_at: Utc::now().timestamp(),
    };
    info!("Created ShareRecord in memory (self-referencing with node user)");

    Ok((viewer_encrypted_resource, share_record))
}
