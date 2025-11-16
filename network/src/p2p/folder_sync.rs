//! Folder Sync - Folder Publishing to Node
//!
//! Orchestrates sending folders and their resources from owner to node.
//! This is a lightweight module focused on the simple push flow.

use crate::p2p::{errors::P2PResult, peer_connection::PeerConnection, resource_sync};
use crypto_utils::CryptoUtils;
use osvauld_core::models::{
    p2p::{FolderDataSync, FolderMessage, FolderResourcesRequest, Message}, User,
};
use persistance::database::RepositoryContext;
use services::{get_folder_by_id, get_folder_share_record};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info};

/// Send folder and all its resources to a node
///
/// This is the main entry point for folder publishing. It:
/// 1. Sends the folder data with the folder_share_record (contains node's folder UCAN)
/// 2. Sends all resources with their share_records (contains node's resource UCANs)
///
/// The folder and resource UCANs are automatically replaced with the recipient's
/// UCANs from their respective share_records.
///
/// # Arguments
/// * `folder_id` - ID of the folder to send
/// * `recipient_user_id` - User ID of the recipient (node)
/// * `current_user` - Current user (owner) sending the folder
/// * `peer_conn` - Peer connection to send data through
/// * `repo_ctx` - Database repository context
/// * `crypto_utils` - Crypto utilities for encryption
///
/// # Returns
/// * `Ok(())` - Folder and all resources sent successfully
/// * `Err` - If folder not found, share records missing, or send fails
pub async fn send_folder_with_resources(
    folder_id: &str,
    recipient_user_id: &str,
    current_user: &User,
    peer_conn: Arc<PeerConnection>,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: Arc<RwLock<CryptoUtils>>,
) -> P2PResult<()> {
    info!(
        "📤 Sending folder {} to node {}",
        folder_id, recipient_user_id
    );

    // 1. Get owner's folder to extract UCAN
    let owner_folder = get_folder_by_id(folder_id, repo_ctx.clone())
        .await
        .map_err(|e| {
            error!("Failed to get folder {}: {}", folder_id, e);
            crate::p2p::errors::P2PError::InvalidState(format!("Failed to get folder: {}", e))
        })?;
    let folder_ucan = owner_folder.ucan.clone();

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
        folder_ucan,
        peer_conn,
        repo_ctx,
        crypto_utils,
    )
    .await?;

    info!("✅ Successfully sent folder {} to node", folder_id);
    Ok(())
}

/// Send folder data with folder_share_record
///
/// Sends the folder metadata along with the folder_share_record that contains
/// the recipient's folder UCAN token. The folder's UCAN is replaced with the
/// recipient's UCAN before sending.
///
/// # Arguments
/// * `folder_id` - ID of the folder to send
/// * `recipient_user_id` - User ID of the recipient
/// * `peer_conn` - Peer connection to send through
/// * `repo_ctx` - Database repository context
///
/// # Returns
/// * `Ok(())` - Folder data sent successfully
/// * `Err` - If folder or share record not found, or send fails
async fn send_folder_data(
    folder_id: &str,
    recipient_user_id: &str,
    peer_conn: Arc<PeerConnection>,
    repo_ctx: Arc<RepositoryContext>,
) -> P2PResult<()> {
    info!("📁 Sending folder data for: {}", folder_id);

    // Get folder by ID
    let mut folder = get_folder_by_id(folder_id, repo_ctx.clone())
        .await
        .map_err(|e| {
            error!("Failed to get folder {}: {}", folder_id, e);
            crate::p2p::errors::P2PError::InvalidState(format!("Failed to get folder: {}", e))
        })?;

    // Get share record (contains recipient's folder UCAN)
    let folder_share_record = get_folder_share_record(folder_id, recipient_user_id, repo_ctx)
        .await
        .map_err(|e| {
            error!("Failed to get folder share record: {}", e);
            crate::p2p::errors::P2PError::InvalidState(format!(
                "Failed to get folder share record: {}",
                e
            ))
        })?;

    // Replace folder's UCAN with the recipient's folder UCAN token
    info!("   Replacing folder UCAN with recipient's token");
    folder.ucan = folder_share_record.ucan_token.clone();

    // Send message
    let data = FolderDataSync {
        folder: folder.clone(),
        folder_share_record,
    };

    peer_conn
        .send_message(Message::Folder(FolderMessage::FolderDataSync(data)))
        .await?;

    info!("✓ Sent folder data: {}", folder.name);
    Ok(())
}

/// Process folder messages (central dispatcher)
///
/// Single entry point for all folder-related messages.
/// Matches on FolderMessage variants and delegates to appropriate handlers.
///
/// # Arguments
/// * `folder_msg` - FolderMessage variant to process
/// * `peer_conn` - Peer connection
/// * `repo_ctx` - Database repository context
/// * `crypto_utils` - Crypto utilities
///
/// # Returns
/// * `Ok(())` - Message processed successfully
/// * `Err` - If processing fails
pub async fn process_folder_message(
    folder_msg: &FolderMessage,
    peer_conn: Arc<PeerConnection>,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: Arc<RwLock<CryptoUtils>>,
) -> P2PResult<()> {
    match folder_msg {
        FolderMessage::FolderDataSync(payload) => {
            handle_folder_data_sync(payload, peer_conn, repo_ctx, crypto_utils).await
        }
        FolderMessage::FolderResourcesRequest(payload) => {
            handle_folder_resources_request(payload, peer_conn, repo_ctx, crypto_utils).await
        }
        FolderMessage::FolderSyncRequest(_) => {
            info!("FolderSyncRequest not yet implemented (CRDT sync)");
            Ok(())
        }
        FolderMessage::FolderSyncResponse(_) => {
            info!("FolderSyncResponse not yet implemented (CRDT sync)");
            Ok(())
        }
        FolderMessage::FolderTokenRequest(payload) => {
            handle_folder_token_request(payload, peer_conn, repo_ctx, crypto_utils).await
        }
        FolderMessage::FolderTokenResponse(payload) => {
            handle_folder_token_response(payload, peer_conn).await
        }
    }
}

/// Handle folder data sync from owner (node side)
///
/// Receives a folder and folder_share_record from the owner and saves it.
/// Validates the peer has add_folder capability and the UCAN structure is valid.
///
/// # Arguments
/// * `payload` - Reference to FolderDataSync containing folder and folder_share_record
/// * `peer_conn` - Peer connection (for getting peer user info)
/// * `repo_ctx` - Database repository context
/// * `_crypto_utils` - Crypto utilities (unused for now)
///
/// # Returns
/// * `Ok(())` - Folder accepted and saved successfully
/// * `Err` - If validation fails or database save fails
pub async fn handle_folder_data_sync(
    payload: &FolderDataSync,
    peer_conn: Arc<PeerConnection>,
    repo_ctx: Arc<RepositoryContext>,
    _crypto_utils: Arc<RwLock<CryptoUtils>>,
) -> P2PResult<()> {
    info!("📥 Received folder {} from peer", payload.folder.id);

    // Get the peer user to access their connection token
    let peer_user_guard = peer_conn.user.read().await;
    let peer_user_id = &peer_user_guard.id;
    let peer_connection_token = &peer_user_guard.ucan_token;

    services::accept_folder_from_peer(
        &payload.folder,
        &payload.folder_share_record,
        peer_connection_token,
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
    peer_conn
        .event_emitter
        .emit(crate::p2p::emitter::P2PEvent::FolderSynced {
            folder_id: payload.folder.id.clone(),
            folder_name: payload.folder.name.clone(),
        });

    // Request all resources for this folder (pull-based sync)
    info!("📦 Requesting resources for folder {}", payload.folder.id);
    let resources_request = FolderResourcesRequest {
        folder_token: payload.folder_share_record.ucan_token.clone(),
        resource_ids: vec![], // Empty = send all resources
    };

    peer_conn
        .send_message(Message::Folder(FolderMessage::FolderResourcesRequest(
            resources_request,
        )))
        .await?;

    info!("✓ Sent FolderResourcesRequest");

    Ok(())
}

/// Handle folder resources request from peer (owner side)
///
/// Receives a request from a peer asking for resources in a folder.
/// Validates the folder_token and sends all resources with their share_records.
///
/// # Arguments
/// * `payload` - Reference to FolderResourcesRequest containing folder_token and resource_ids
/// * `peer_conn` - Peer connection (for getting peer info and sending responses)
/// * `repo_ctx` - Database repository context
/// * `crypto_utils` - Crypto utilities for encryption
///
/// # Returns
/// * `Ok(())` - Resources sent successfully
/// * `Err` - If validation fails or resource send fails
pub async fn handle_folder_resources_request(
    payload: &FolderResourcesRequest,
    peer_conn: Arc<PeerConnection>,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: Arc<RwLock<CryptoUtils>>,
) -> P2PResult<()> {
    info!("📦 Received FolderResourcesRequest from peer");

    // 1. Validate folder_token and extract folder_id
    // Note: We validate that the folder token has add_resources capability because
    // we're about to use it to send resources back to the peer
    let domain = &peer_conn.domain;

    // Parse the UCAN token
    let parsed_ucan = gurkha::parser::GenericUcan::from_token(&payload.folder_token)
        .map_err(|e| {
            error!("Failed to parse folder token: {}", e);
            crate::p2p::errors::P2PError::InvalidState(format!("Invalid folder token: {}", e))
        })?;

    let folder_id = gurkha::extractors::extract_id_from_resource_type(
        parsed_ucan.parsed(),
        domain,
        "folder",
    )
    .map_err(|e| {
        error!(
            "Failed to validate folder token has add_resources capability: {}",
            e
        );
        crate::p2p::errors::P2PError::InvalidState(format!("Invalid folder token: {}", e))
    })?;

    info!("  Folder ID from token: {}", folder_id);

    // 2. Get current user (owner)
    let current_user_guard = peer_conn.user.read().await;
    let current_user = current_user_guard.clone();
    drop(current_user_guard);

    // 3. Get peer user
    let peer_user_guard = peer_conn.user.read().await;
    drop(peer_user_guard);

    // 4. Extract peer role from folder_token
    let peer_role = gurkha::extractors::get_role(parsed_ucan.parsed())
        .unwrap_or_else(|| "node".to_string());

    info!("  Peer role: {}", peer_role);

    // 5. Get all resources in folder (or specific ones if resource_ids is not empty)
    let resource_ids = if payload.resource_ids.is_empty() {
        // Empty resource_ids means "send all resources"
        repo_ctx
            .resource_repo
            .get_resource_ids_by_folder_id(&folder_id)
            .await
            .map_err(|e| {
                error!("Failed to get resources for folder {}: {}", folder_id, e);
                crate::p2p::errors::P2PError::InvalidState(format!(
                    "Failed to get resources: {}",
                    e
                ))
            })?
    } else {
        payload.resource_ids.clone()
    };

    if resource_ids.is_empty() {
        info!("  No resources to send for folder {}", folder_id);
        return Ok(());
    }

    info!("  Sending {} resources", resource_ids.len());

    // 6. Get owner's folder token
    let owner_folder = get_folder_by_id(&folder_id, repo_ctx.clone())
        .await
        .map_err(|e| {
            error!("Failed to get folder {}: {}", folder_id, e);
            crate::p2p::errors::P2PError::InvalidState(format!("Failed to get folder: {}", e))
        })?;
    let folder_ucan = owner_folder.ucan.clone();

    // 7. Send each resource with appropriate share_records
    for resource_id in resource_ids {
        info!("   Sending resource: {}", resource_id);

        // Get share_records based on peer_role
        let share_records = if peer_role == "node" {
            // For node: fetch ALL share_records from DB
            services::get_all_share_records_for_resource(&resource_id, repo_ctx.clone())
                .await
                .map_err(|e| {
                    error!(
                        "Failed to get share records for resource {}: {}",
                        resource_id, e
                    );
                    crate::p2p::errors::P2PError::InvalidState(format!(
                        "Failed to get share records: {}",
                        e
                    ))
                })?
        } else {
            // For other roles: create share_records on-the-fly
            // TODO: Implement on-the-fly share_record creation for viewers
            info!("     On-the-fly share_record creation not yet implemented, skipping");
            continue;
        };

        info!("     Including {} share records", share_records.len());

        // Get peer user for encryption
        let peer_user_guard = peer_conn.user.read().await;
        let peer_user = peer_user_guard.clone();
        drop(peer_user_guard);

        // Prepare resource for peer
        let peer_encrypted_resource = match services::prepare_resource_transfer(
            &resource_id,
            &current_user,
            &payload.folder_token,
            &peer_role,
            &peer_user,
            repo_ctx.clone(),
            &crypto_utils,
            &peer_conn.ucan_service,
        )
        .await
        {
            Ok(res) => res,
            Err(e) => {
                error!(
                    "Failed to prepare resource {} for peer: {}, skipping",
                    resource_id, e
                );
                continue;
            }
        };

        // Send ResourceDataSync
        let resource_data = osvauld_core::models::p2p::ResourceDataSync {
            resource: peer_encrypted_resource,
            share_records,
            folder_ucan: folder_ucan.clone(),
        };

        if let Err(e) = resource_sync::send_resource_data(peer_conn.clone(), resource_data).await {
            error!(
                "Failed to send resource {} to peer: {}, continuing",
                resource_id, e
            );
        } else {
            info!("     ✓ Successfully sent resource");
        }
    }

    info!("✅ Finished sending resources for folder {}", folder_id);
    Ok(())
}

/// Handle folder token request from peer (node side)
///
/// Receives a request for a shareable folder link. Validates the requester
/// has get_share_link capability, generates a ViewerAuth token with wildcard
/// audience, and sends back a connection string.
///
/// # Arguments
/// * `payload` - FolderTokenRequest with folder_id and folder_ucan
/// * `peer_conn` - Peer connection (for sending response)
/// * `repo_ctx` - Database repository context
/// * `_crypto_utils` - Crypto utilities (unused for now)
///
/// # Returns
/// * `Ok(())` - Connection string generated and sent
/// * `Err` - If validation fails or token generation fails
pub async fn handle_folder_token_request(
    payload: &osvauld_core::models::p2p::FolderTokenRequest,
    peer_conn: Arc<PeerConnection>,
    _repo_ctx: Arc<RepositoryContext>,
    _crypto_utils: Arc<RwLock<CryptoUtils>>,
) -> P2PResult<()> {
    info!("📨 Received FolderTokenRequest for folder {}", payload.folder_id);

    // 1. Parse and validate folder_ucan
    let parsed_ucan = gurkha::parser::GenericUcan::from_token(&payload.folder_ucan)
        .map_err(|e| {
            error!("Failed to parse folder token: {}", e);
            crate::p2p::errors::P2PError::InvalidState(format!("Invalid folder token: {}", e))
        })?;

    // 2. Check for get_share_link capability in folder token
    if !parsed_ucan.can_get_folder_share_link() {
        error!("Folder token does not have get_share_link capability");
        return Err(crate::p2p::errors::P2PError::InvalidState(
            "Permission denied: token lacks get_share_link capability".to_string()
        ));
    }

    info!("  ✓ Folder token has get_share_link capability");

    // 3. Verify folder_id from token matches request
    let token_folder_id = parsed_ucan.folder_id()
        .ok_or_else(|| {
            error!("Cannot extract folder_id from token");
            crate::p2p::errors::P2PError::InvalidState("Invalid folder token: no folder_id".to_string())
        })?;

    if token_folder_id != payload.folder_id {
        error!("Folder ID mismatch: token={}, request={}", token_folder_id, payload.folder_id);
        return Err(crate::p2p::errors::P2PError::InvalidState(
            "Folder ID mismatch".to_string()
        ));
    }

    info!("  ✓ Folder ID verified: {}", payload.folder_id);

    // 4. Extract domain from token
    let domain = parsed_ucan.domain()
        .ok_or_else(|| {
            error!("Cannot extract domain from token");
            crate::p2p::errors::P2PError::InvalidState("Invalid folder token: no domain".to_string())
        })?;

    // 5. Get current user (node) info for connection string
    let current_user_guard = peer_conn.user.read().await;
    let current_user = current_user_guard.clone();
    drop(current_user_guard);

    let current_device_guard = peer_conn.context.current_device.read().await;
    let current_device = current_device_guard.as_ref()
        .ok_or_else(|| {
            error!("No current device found");
            crate::p2p::errors::P2PError::InvalidState("No device information".to_string())
        })?
        .clone();
    drop(current_device_guard);

    // 6. Generate ViewerAuth token with wildcard audience (aud:*)
    let ucan_service_guard = peer_conn.ucan_service.read().await;
    let (viewer_auth_token, _cid) = ucan_service_guard
        .issue_folder_viewer_auth(&payload.folder_id, &domain)
        .await
        .map_err(|e| {
            error!("Failed to generate viewer auth token: {}", e);
            crate::p2p::errors::P2PError::InvalidState(format!("Token generation failed: {}", e))
        })?;
    drop(ucan_service_guard);

    info!("  ✓ Generated ViewerAuth token with aud:*");

    // 7. Get node's UCAN public key for connection string
    let ucan_service_guard = peer_conn.ucan_service.read().await;
    let ucan_pub_key = ucan_service_guard.get_public_key()
        .map_err(|e| {
            error!("Failed to get UCAN public key: {}", e);
            crate::p2p::errors::P2PError::InvalidState(format!("Failed to get public key: {}", e))
        })?;
    drop(ucan_service_guard);

    // 8. Build connection string
    let connection_data = serde_json::json!({
        "username": current_user.username,
        "user_public_key": current_user.public_key,
        "device_public_key": current_device.device_key,
        "ucan_token": viewer_auth_token,
        "ucan_pub_key": ucan_pub_key,
        "folder_id": payload.folder_id,
    });

    let connection_json = serde_json::to_string(&connection_data)
        .map_err(|e| {
            error!("Failed to serialize connection data: {}", e);
            crate::p2p::errors::P2PError::InvalidState(format!("Serialization failed: {}", e))
        })?;

    use base64::{Engine as _, engine::general_purpose};
    let connection_string = general_purpose::STANDARD.encode(connection_json.as_bytes());

    info!("  ✓ Connection string generated (length: {})", connection_string.len());

    // 9. Send FolderTokenResponse
    let response = osvauld_core::models::p2p::FolderTokenResponse {
        folder_id: payload.folder_id.clone(),
        connection_string: connection_string.clone(),
    };

    peer_conn
        .send_message(Message::Folder(FolderMessage::FolderTokenResponse(response)))
        .await?;

    info!("✅ Sent FolderTokenResponse for folder {}", payload.folder_id);

    Ok(())
}

/// Handle folder token response from node (requester side)
///
/// Receives the connection string from the node and emits an event
/// for the frontend to display it to the user.
///
/// # Arguments
/// * `payload` - FolderTokenResponse with folder_id and connection_string
/// * `peer_conn` - Peer connection (for event emission)
///
/// # Returns
/// * `Ok(())` - Event emitted successfully
pub async fn handle_folder_token_response(
    payload: &osvauld_core::models::p2p::FolderTokenResponse,
    peer_conn: Arc<PeerConnection>,
) -> P2PResult<()> {
    info!("📬 Received FolderTokenResponse for folder {}", payload.folder_id);

    // Emit event to frontend
    peer_conn.event_emitter.emit(crate::p2p::emitter::P2PEvent::FolderTokenReceived {
        folder_id: payload.folder_id.clone(),
        connection_string: payload.connection_string.clone(),
    });

    info!("✓ Emitted FolderTokenReceived event");

    Ok(())
}
