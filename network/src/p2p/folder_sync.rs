//! Folder sync protocol - simple push after share_folder()
//!
//! This module handles folder sync operations.

use crate::p2p::{errors::P2PResult, peer_connection::PeerConnection, resource_sync};
use crypto_utils::CryptoUtils;
use osvauld_core::models::{FolderDataSync, FolderMessage, Message, User};
use persistance::database::RepositoryContext;
use services::{get_folder_by_id, get_folder_share_record};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info};

/// Process all Folder messages - central routing function
///
/// This function receives all Folder message variants and delegates
/// to the appropriate handler based on message type.
///
/// # Arguments
/// * `peer_conn` - The peer connection
/// * `message` - The FolderMessage variant to process
/// * `repo_ctx` - Repository context
/// * `crypto_utils` - Crypto utilities
///
/// # Returns
/// * `Ok(())` - Message processed successfully
/// * `Err` - If processing fails
pub async fn process_message(
    peer_conn: Arc<PeerConnection>,
    message: FolderMessage,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: Arc<RwLock<CryptoUtils>>,
) -> P2PResult<()> {
    match message {
        FolderMessage::FolderDataSync(payload) => {
            handle_folder_data_sync(payload, peer_conn, repo_ctx, crypto_utils).await
        }
        FolderMessage::FolderSyncRequest(payload) => {
            handle_folder_sync_request(payload, peer_conn, repo_ctx, crypto_utils).await
        }
        FolderMessage::FolderSyncResponse(payload) => {
            handle_folder_sync_response(payload, peer_conn, repo_ctx, crypto_utils).await
        }
        FolderMessage::FolderTokenRequest(payload) => {
            handle_folder_token_request(payload, peer_conn).await
        }
        FolderMessage::FolderTokenResponse(payload) => {
            handle_folder_token_response(payload, peer_conn).await
        }
    }
}

/// Send folder data then all resources
///
/// Entry point called by sync_service.
/// Orchestrates: folder send + all resources send.
pub async fn send_folder_with_resources(
    folder_id: &str,
    recipient_user_id: &str,
    current_user: &User,
    peer_conn: Arc<PeerConnection>,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: Arc<RwLock<CryptoUtils>>,
) -> P2PResult<()> {
    // 1. Get owner's folder to extract UCAN
    let owner_folder = get_folder_by_id(folder_id, repo_ctx.clone())
        .await
        .map_err(|e| {
            error!("Failed to get folder {}: {}", folder_id, e);
            crate::p2p::errors::P2PError::InvalidState(format!("Failed to get folder: {}", e))
        })?;
    let owner_folder_ucan = owner_folder.ucan.clone();

    // 2. Send folder first
    send_folder_data(
        folder_id,
        recipient_user_id,
        peer_conn.clone(),
        repo_ctx.clone(),
    )
    .await?;

    // 3. Send all resources with owner's folder UCAN (delegate to resource_sync)
    resource_sync::send_all_resources_for_folder(
        folder_id,
        recipient_user_id,
        current_user,
        owner_folder_ucan,
        peer_conn,
        repo_ctx,
        crypto_utils,
    )
    .await
}

/// Send just folder data
async fn send_folder_data(
    folder_id: &str,
    recipient_user_id: &str,
    peer_conn: Arc<PeerConnection>,
    repo_ctx: Arc<RepositoryContext>,
) -> P2PResult<()> {
    info!("Sending folder {} to node", folder_id);

    // Get folder by ID
    let mut folder = get_folder_by_id(folder_id, repo_ctx.clone())
        .await
        .map_err(|e| {
            error!("Failed to get folder {}: {}", folder_id, e);
            crate::p2p::errors::P2PError::InvalidState(format!("Failed to get folder: {}", e))
        })?;

    // Get share record
    let folder_share_record = get_folder_share_record(folder_id, recipient_user_id, repo_ctx)
        .await
        .map_err(|e| {
            error!("Failed to get folder share record: {}", e);
            crate::p2p::errors::P2PError::InvalidState(format!(
                "Failed to get folder share record: {}",
                e
            ))
        })?;

    // Update folder's UCAN with the recipient's folder UCAN token
    folder.ucan = folder_share_record.ucan_token.clone();

    // Send message
    let data = FolderDataSync {
        folder,
        folder_share_record,
    };

    peer_conn.send_message(Message::Folder(FolderMessage::FolderDataSync(data))).await
}

/// Handle folder data sync from owner (node side)
///
/// Orchestrates folder acceptance by delegating to folder_service
pub async fn handle_folder_data_sync(
    payload: FolderDataSync,
    peer_conn: Arc<PeerConnection>,
    repo_ctx: Arc<RepositoryContext>,
    _crypto_utils: Arc<RwLock<CryptoUtils>>,
) -> P2PResult<()> {
    info!("📥 Received folder {} from peer", payload.folder.id);

    // Get the peer user to access their connection token
    let peer_user_guard = peer_conn.user.read().await;
    let peer_user_id = &peer_user_guard.id;
    let peer_connection_token = &peer_user_guard.ucan_token;
    let domain = &peer_conn.domain;
    services::accept_folder_from_peer(
        &payload.folder,
        &payload.folder_share_record,
        peer_connection_token,
        domain,
        repo_ctx,
    )
    .await
    .map_err(|e| {
        error!("❌ Failed to accept folder from peer: {}", e);
        error!("   Folder ID: {}", payload.folder.id);
        error!("   Peer user ID: {}", peer_user_id);
        crate::p2p::errors::P2PError::InvalidState(format!("Failed to accept folder: {}", e))
    })?;

    info!("✅ Accepted and saved folder {}", payload.folder.id);

    // Emit FolderSynced event
    peer_conn.event_emitter.emit(crate::p2p::emitter::P2PEvent::FolderSynced {
        folder_id: payload.folder.id.clone(),
        folder_name: payload.folder.name.clone(),
    });

    Ok(())
}

/// Handle FolderTokenRequest - generate and send viewer connection string
///
/// This function:
/// 1. Validates requester has get_share_link capability
/// 2. Generates viewer UCAN token
/// 3. Creates connection string with node info + viewer token
/// 4. Sends FolderTokenResponse back to requester
pub async fn handle_folder_token_request(
    payload: osvauld_core::models::FolderTokenRequest,
    peer_conn: Arc<crate::p2p::peer_connection::PeerConnection>,
) -> P2PResult<()> {
    info!("📨 Handling FolderTokenRequest for folder: {}", payload.folder_id);

    // TODO: Implement viewer token generation
    let _ = (payload, peer_conn);
    error!("❌ Viewer support not yet implemented");
    Err(crate::p2p::errors::P2PError::InvalidState(
        "Viewer support not implemented".to_string()
    ))
}

/// Handle FolderTokenResponse - emit event to frontend
///
/// Called when receiving the connection string from the node.
/// Emits P2PEvent::FolderTokenReceived for the frontend to display.
pub async fn handle_folder_token_response(
    payload: osvauld_core::models::FolderTokenResponse,
    peer_conn: Arc<crate::p2p::peer_connection::PeerConnection>,
) -> P2PResult<()> {
    info!("📨 Handling FolderTokenResponse for folder: {}", payload.folder_id);

    // Emit event with connection string
    peer_conn.event_emitter.emit(crate::p2p::emitter::P2PEvent::FolderTokenReceived {
        folder_id: payload.folder_id.clone(),
        connection_string: payload.connection_string,
    });

    info!("✅ Emitted FolderTokenReceived event for folder: {}", payload.folder_id);
    Ok(())
}

/// Handler for folder sync with state vectors
///
/// Called by sync_handler after connection is established.
/// This will handle the CRDT merge sync protocol for folder + resources.
///
/// # Arguments
/// * `folder_id` - The folder to sync
/// * `peer_conn` - Established peer connection
/// * `repo_ctx` - Repository context
/// * `crypto_utils` - Crypto utilities
///
/// # Returns
/// * `Ok(())` - Sync initiated successfully
/// * `Err` - If sync fails
pub async fn sync_folder_handler(
    folder_id: &str,
    peer_conn: Arc<PeerConnection>,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: Arc<RwLock<CryptoUtils>>,
) -> P2PResult<()> {
    info!("📁 Handling folder sync for: {}", folder_id);

    // 1. Get local user
    let local_user = peer_conn.get_local_user().await?;

    // 2. Get folder UCAN
    let folder = get_folder_by_id(folder_id, repo_ctx.clone())
        .await
        .map_err(|e| {
            error!("Failed to get folder {}: {}", folder_id, e);
            crate::p2p::errors::P2PError::InvalidState(format!("Failed to get folder: {}", e))
        })?;

    let folder_ucan = folder.ucan.clone();

    // 3. Get all resources in folder with their sync info
    let resource_list = services::get_resource_list_for_folder(
        folder_id,
        &local_user.id,
        repo_ctx.clone(),
        &crypto_utils,
    )
    .await
    .map_err(|e| {
        error!("Failed to get resource list for folder: {}", e);
        crate::p2p::errors::P2PError::InvalidState(format!("Failed to get resource list: {}", e))
    })?;

    info!("Found {} resources in folder", resource_list.len());

    // 4. Serialize resource list to JSON
    let resources_json = serde_json::to_string(&resource_list).map_err(|e| {
        error!("Failed to serialize resource list: {}", e);
        crate::p2p::errors::P2PError::InvalidState(format!(
            "Failed to serialize resource list: {}",
            e
        ))
    })?;

    // 5. Send FolderSyncRequest message
    let request = osvauld_core::models::FolderSyncRequestMsg {
        folder_ucan,
        resources: resources_json,
    };

    peer_conn
        .send_message(osvauld_core::models::Message::Folder(
            osvauld_core::models::FolderMessage::FolderSyncRequest(request),
        ))
        .await?;

    info!("✅ Sent FolderSyncRequest with {} resources", resource_list.len());
    Ok(())
}

/// Handle FolderSyncRequest - discover resources and compare states
///
/// This is Step 1 of the folder sync protocol.
/// Receives initiator's resource list with state vectors,
/// compares with local resources, and sends response indicating:
/// - Which resources are missing (need full transfer)
/// - Which resources exist (will sync individually)
///
/// # Arguments
/// * `payload` - FolderSyncRequestMsg with folder UCAN and resource list
/// * `peer_conn` - Peer connection
/// * `repo_ctx` - Repository context
/// * `crypto_utils` - Crypto utilities
///
/// # Returns
/// * `Ok(())` - Response sent successfully
/// * `Err` - If validation or comparison fails
pub async fn handle_folder_sync_request(
    payload: osvauld_core::models::FolderSyncRequestMsg,
    peer_conn: Arc<PeerConnection>,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: Arc<RwLock<CryptoUtils>>,
) -> P2PResult<()> {
    info!("📨 Handling FolderSyncRequest");

    // 1. Extract folder_id from folder_ucan
    let folder_id = services::ucan_service::extract_folder_id(&payload.folder_ucan)
        .await
        .map_err(|e| {
            error!("Failed to extract folder_id from UCAN: {}", e);
            crate::p2p::errors::P2PError::InvalidState(format!("Invalid folder UCAN: {}", e))
        })?;

    info!("Processing FolderSyncRequest for folder: {}", folder_id);

    // 2. Parse initiator's resource list
    let initiator_resources: Vec<serde_json::Value> =
        serde_json::from_str(&payload.resources).map_err(|e| {
            error!("Failed to parse resources JSON: {}", e);
            crate::p2p::errors::P2PError::InvalidState(format!("Invalid resources JSON: {}", e))
        })?;

    info!("Initiator has {} resources", initiator_resources.len());

    // 3. Get our local resources for this folder
    let local_user = peer_conn.get_local_user().await?;
    let our_resources = services::get_resource_list_for_folder(
        &folder_id,
        &local_user.id,
        repo_ctx.clone(),
        &crypto_utils,
    )
    .await
    .map_err(|e| {
        error!("Failed to get local resource list: {}", e);
        crate::p2p::errors::P2PError::InvalidState(format!("Failed to get resource list: {}", e))
    })?;

    info!("We have {} resources locally", our_resources.len());

    // 4. Compare resources to identify missing vs existing
    use std::collections::HashSet;

    let initiator_resource_ids: HashSet<String> = initiator_resources
        .iter()
        .filter_map(|r| r.get("resource_id").and_then(|v| v.as_str()).map(String::from))
        .collect();

    let our_resource_ids: HashSet<String> = our_resources
        .iter()
        .filter_map(|r| r.get("resource_id").and_then(|v| v.as_str()).map(String::from))
        .collect();

    // Resources initiator has that we don't (need full transfer)
    let missing_resource_ids: Vec<String> = initiator_resource_ids
        .difference(&our_resource_ids)
        .cloned()
        .collect();

    // Resources both have (will sync individually with state vectors)
    let existing_resource_ids: Vec<String> = initiator_resource_ids
        .intersection(&our_resource_ids)
        .cloned()
        .collect();

    info!(
        "Missing: {} resources, Existing: {} resources",
        missing_resource_ids.len(),
        existing_resource_ids.len()
    );

    // 5. Prepare existing_resources JSON with our state vectors for comparison
    // For now, just send resource IDs - full state vector comparison happens in individual ResourceSyncRequest
    let existing_resources_json = serde_json::json!(existing_resource_ids).to_string();

    // 6. Send FolderSyncResponse
    let response = osvauld_core::models::FolderSyncResponseMsg {
        folder_id,
        missing_resource_ids,
        existing_resources: existing_resources_json,
    };

    peer_conn
        .send_message(osvauld_core::models::Message::Folder(
            osvauld_core::models::FolderMessage::FolderSyncResponse(response),
        ))
        .await?;

    info!("✅ Sent FolderSyncResponse");
    Ok(())
}

/// Handle FolderSyncResponse - process missing and existing resources
///
/// This is Step 2 of the folder sync protocol.
/// Receives response indicating which resources need full transfer
/// vs incremental sync.
///
/// # Arguments
/// * `payload` - FolderSyncResponseMsg with missing and existing resource lists
/// * `peer_conn` - Peer connection
/// * `repo_ctx` - Repository context
/// * `crypto_utils` - Crypto utilities
///
/// # Returns
/// * `Ok(())` - Sync operations initiated
/// * `Err` - If processing fails
pub async fn handle_folder_sync_response(
    payload: osvauld_core::models::FolderSyncResponseMsg,
    peer_conn: Arc<PeerConnection>,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: Arc<RwLock<CryptoUtils>>,
) -> P2PResult<()> {
    info!("📨 Handling FolderSyncResponse for folder: {}", payload.folder_id);

    let local_user = peer_conn.get_local_user().await?;
    let peer_user = peer_conn.get_peer_user().await;

    info!(
        "Peer needs {} missing resources, {} existing resources",
        payload.missing_resource_ids.len(),
        payload.existing_resources.len()
    );

    // 1. Send full ResourceDataSync for missing resources
    if !payload.missing_resource_ids.is_empty() {
        info!("Sending {} missing resources to peer", payload.missing_resource_ids.len());

        for resource_id in &payload.missing_resource_ids {
            info!("Preparing to send missing resource: {}", resource_id);

            // Get share record for this user
            let share_records = repo_ctx
                .share_repo
                .find_by_resources_and_user(&[resource_id.clone()], &local_user.id, "read")
                .await
                .map_err(|e| {
                    error!("Failed to get share record for resource {}: {}", resource_id, e);
                    crate::p2p::errors::P2PError::InvalidState(format!(
                        "Failed to get share record: {}",
                        e
                    ))
                })?;

            let share_record = share_records.into_iter().next().ok_or_else(|| {
                error!("No share record found for resource {} and user {}", resource_id, local_user.id);
                crate::p2p::errors::P2PError::InvalidState(format!(
                    "No share record found for resource {}",
                    resource_id
                ))
            })?;

            // Get folder UCAN for validation
            let folder = get_folder_by_id(&payload.folder_id, repo_ctx.clone())
                .await
                .map_err(|e| {
                    error!("Failed to get folder {}: {}", payload.folder_id, e);
                    crate::p2p::errors::P2PError::InvalidState(format!("Failed to get folder: {}", e))
                })?;

            // Get all share records for this resource (for viewer forwarding)
            let all_share_records = services::get_all_share_records_for_resource(
                resource_id,
                repo_ctx.clone(),
            )
            .await
            .map_err(|e| {
                error!("Failed to get all share records for resource {}: {}", resource_id, e);
                crate::p2p::errors::P2PError::InvalidState(format!(
                    "Failed to get share records: {}",
                    e
                ))
            })?;

            // Prepare resource for peer (decrypt, filter, re-encrypt)
            let peer_encrypted_resource = services::prepare_resource_for_peer(
                resource_id,
                &peer_user.id,
                &share_record.ucan_token,
                &peer_user.public_key,
                repo_ctx.clone(),
                &crypto_utils,
            )
            .await
            .map_err(|e| {
                error!("Failed to prepare resource {} for peer: {}", resource_id, e);
                crate::p2p::errors::P2PError::InvalidState(format!(
                    "Failed to prepare resource: {}",
                    e
                ))
            })?;

            // Create ResourceDataSync message
            let resource_data = osvauld_core::models::ResourceDataSync {
                resource: peer_encrypted_resource,
                share_records: all_share_records,
                owner_folder_ucan: folder.ucan.clone(),
            };

            // Send to peer
            peer_conn
                .send_message(osvauld_core::models::Message::Resource(
                    osvauld_core::models::ResourceMessage::ResourceDataSync(resource_data),
                ))
                .await?;

            info!("✓ Sent missing resource: {}", resource_id);
        }
    }

    // 2. Send individual ResourceSyncRequest for existing resources (parallel)
    let existing_resource_ids: Vec<String> =
        serde_json::from_str(&payload.existing_resources).map_err(|e| {
            error!("Failed to parse existing_resources JSON: {}", e);
            crate::p2p::errors::P2PError::InvalidState(format!(
                "Invalid existing_resources JSON: {}",
                e
            ))
        })?;

    if !existing_resource_ids.is_empty() {
        info!(
            "Initiating parallel sync for {} existing resources",
            existing_resource_ids.len()
        );

        // Spawn parallel sync tasks for all existing resources
        let mut sync_tasks = Vec::new();

        for resource_id in existing_resource_ids {
            let resource_id = resource_id.clone();
            let user_id = local_user.id.clone();
            let repo_ctx = repo_ctx.clone();
            let crypto_utils = crypto_utils.clone();
            let peer_conn = peer_conn.clone();

            // Spawn async task for each resource sync
            let task = tokio::spawn(async move {
                info!("Starting sync for existing resource: {}", resource_id);

                // Prepare ResourceSyncRequest
                let (resource_ucan, folder_ucan, state_vectors, full_docs) =
                    services::prepare_resource_sync_request(
                        &resource_id,
                        &user_id,
                        repo_ctx,
                        &crypto_utils,
                    )
                    .await
                    .map_err(|e| {
                        error!("Failed to prepare sync request for {}: {}", resource_id, e);
                        crate::p2p::errors::P2PError::InvalidState(format!(
                            "Failed to prepare sync request: {}",
                            e
                        ))
                    })?;

                // Send ResourceSyncRequest
                let request = osvauld_core::models::ResourceSyncRequestMsg {
                    resource_ucan,
                    folder_ucan,
                    state_vectors,
                    full_docs,
                };

                peer_conn
                    .send_message(osvauld_core::models::Message::Resource(
                        osvauld_core::models::ResourceMessage::ResourceSyncRequest(request),
                    ))
                    .await?;

                info!("✓ Sent ResourceSyncRequest for: {}", resource_id);
                Ok::<(), crate::p2p::errors::P2PError>(())
            });

            sync_tasks.push(task);
        }

        // Wait for all sync tasks to complete
        for task in sync_tasks {
            if let Err(e) = task.await {
                error!("Resource sync task failed: {}", e);
                // Continue with other tasks even if one fails
            }
        }

        info!("✅ All parallel resource syncs initiated");
    }

    info!("✅ FolderSyncResponse processing complete");
    Ok(())
}
