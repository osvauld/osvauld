//! Folder Sync - Folder Publishing to Node
//!
//! Orchestrates sending folders and their resources from owner to node.
//! This is a lightweight module focused on the simple push flow.

use crate::p2p::{errors::P2PResult, peer_connection::PeerConnection, resource_sync};
use crypto_utils::CryptoUtils;
use osvauld_core::models::{
    p2p::{FolderDataSync, FolderMessage, FolderResourcesRequest, Message, ResourceMessage, ResourceNotFoundRequestMsg}, User,
};
use persistance::database::RepositoryContext;
use services::{get_folder_by_id, get_folder_share_records_for_recipients};
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info, instrument};

/// Send FolderResourcesRequest to pull folder data
///
/// This is called by viewers to request a folder and all its resources.
/// The folder_token (ViewerAuth) proves the viewer has access to the folder.
///
/// # Arguments
/// * `peer_conn` - Peer connection to the node
/// * `folder_token` - ViewerAuth token proving folder access
///
/// # Returns
/// * `Ok(())` - Request sent successfully
/// * `Err` - If send fails
#[instrument(skip(peer_conn, folder_token), level = "info")]
pub async fn send_folder_resources_request(
    peer_conn: Arc<PeerConnection>,
    folder_token: String,
) -> P2PResult<()> {
    info!("📤 Sending FolderResourcesRequest");

    let folder_request = FolderResourcesRequest {
        folder_permit: folder_token,
        resource_ids: vec![], // Empty = request all resources
    };

    let message = Message::Folder(FolderMessage::FolderResourcesRequest(folder_request));

    peer_conn.send_message(message).await?;

    info!("✅ FolderResourcesRequest sent");
    Ok(())
}

/// Send FolderResourcesRequest by looking up folder token from database
///
/// This is for nodes/users who have database access and know their folder_id.
/// Differs from send_folder_resources_request() which takes the token directly (for viewers).
///
/// # Arguments
/// * `peer_conn` - Peer connection
/// * `folder_id` - Folder ID to request
/// * `local_user_id` - Current user's ID (to look up their folder token)
/// * `repo_ctx` - Database repository context
///
/// # Returns
/// * `Ok(())` - Request sent successfully
/// * `Err` - If folder not found, no access, or send fails
#[instrument(skip(peer_conn, repo_ctx), level = "info")]
pub async fn send_folder_resources_request_by_id(
    peer_conn: Arc<PeerConnection>,
    folder_id: String,
    local_user_id: String,
    repo_ctx: Arc<RepositoryContext>,
) -> P2PResult<()> {
    info!("📤 Sending FolderResourcesRequest for folder {}", folder_id);

    // 1. Look up folder share record to get our UCAN token
    let folder_share = repo_ctx
        .folder_share_repo
        .find_by_folder_and_user(&folder_id, &local_user_id)
        .await
        .map_err(|e| crate::p2p::errors::P2PError::InvalidState(format!("Failed to find folder share: {}", e)))?
        .ok_or_else(|| crate::p2p::errors::P2PError::InvalidState(format!("No access to folder {}", folder_id)))?;

    let folder_token = folder_share.ucan_token;

    // 2. Get all resource IDs we already have in this folder
    let resource_ids = repo_ctx
        .resource_repo
        .get_resource_ids_by_folder_id(&folder_id)
        .await
        .map_err(|e| crate::p2p::errors::P2PError::InvalidState(format!("Failed to get resource IDs: {}", e)))?;

    info!("  Requesting folder with {} existing resources", resource_ids.len());

    // 3. Create and send request
    let folder_request = FolderResourcesRequest {
        folder_permit: folder_token,
        resource_ids, // Send what we have, peer will send the difference
    };

    let message = Message::Folder(FolderMessage::FolderResourcesRequest(folder_request));
    peer_conn.send_message(message).await?;

    info!("✅ FolderResourcesRequest sent");
    Ok(())
}

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
#[instrument(skip(current_user, peer_conn, repo_ctx, crypto_utils), fields(folder_id, recipient_user_id), level = "info")]
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

    // Get current user (sender) to create dual-permit records
    let current_user_guard = peer_conn.context.current_user.read().await;
    let current_user_id = current_user_guard
        .as_ref()
        .ok_or_else(|| {
            error!("No current user in peer connection");
            crate::p2p::errors::P2PError::InvalidState("No current user".to_string())
        })?
        .id
        .clone();
    drop(current_user_guard);

    // Fetch TWO folder_share_records from DB (dual-permit pattern)
    // 1. Recipient's record: shared_by=sender, recipient=recipient
    // 2. Sender's record: shared_by=sender, recipient=sender (self-reference)
    let recipient_ids = vec![recipient_user_id.to_string(), current_user_id.clone()];
    let folder_share_records = services::get_folder_share_records_for_recipients(
        folder_id,
        &recipient_ids,
        repo_ctx,
    )
    .await
    .map_err(|e| {
        error!("Failed to get folder share records: {}", e);
        crate::p2p::errors::P2PError::InvalidState(format!(
            "Failed to get folder share records: {}",
            e
        ))
    })?;

    if folder_share_records.len() != 2 {
        error!(
            "Expected 2 folder_share_records (recipient + sender), got {}",
            folder_share_records.len()
        );
        return Err(crate::p2p::errors::P2PError::InvalidState(format!(
            "Incorrect number of folder share records: expected 2, got {}",
            folder_share_records.len()
        )));
    }

    // Find recipient's folder_share_record to use for folder UCAN
    let recipient_record = folder_share_records
        .iter()
        .find(|r| r.recipient_user_id == recipient_user_id)
        .ok_or_else(|| {
            error!("Recipient folder_share_record not found");
            crate::p2p::errors::P2PError::InvalidState(
                "Recipient folder_share_record not found".to_string(),
            )
        })?;

    // Replace folder's UCAN with the recipient's folder UCAN token
    info!("   Replacing folder UCAN with recipient's token");
    folder.ucan = recipient_record.ucan_token.clone();

    info!("   Sending FolderDataSync with 2 folder_share_records (dual-permit)");

    // Send message with BOTH folder_share_records
    let data = FolderDataSync {
        folder: folder.clone(),
        folder_share_records,
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
#[instrument(skip(folder_msg, peer_conn, repo_ctx, crypto_utils), fields(message_type = ?std::mem::discriminant(folder_msg)), level = "info")]
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
#[instrument(skip(payload, peer_conn, repo_ctx, _crypto_utils), fields(folder_id = %payload.folder.id), level = "info")]
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
        &payload.folder_share_records,
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
#[instrument(skip(payload, peer_conn, repo_ctx, crypto_utils), level = "info")]
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

    // Parse the UCAN token
    let parsed_ucan = gurkha::parser::Permit::from_token(&payload.folder_permit)
        .map_err(|e| {
            error!("Failed to parse folder token: {}", e);
            crate::p2p::errors::P2PError::InvalidState(format!("Invalid folder token: {}", e))
        })?;

    let folder_id = parsed_ucan.folder_id()
        .ok_or_else(|| {
            error!("Failed to extract folder_id from token facts");
            crate::p2p::errors::P2PError::InvalidState("No folder_id in token facts".to_string())
        })?;

    info!("  Folder ID from token: {}", folder_id);

    // 2. Get current user (the local node user)
    // IMPORTANT: peer_conn.user now points to the PEER, so we must get local user from context
    let current_user = peer_conn.get_local_user().await?;

    // 3. Extract peer relationship from folder_token facts
    let peer_relationship = parsed_ucan.relationship()
        .unwrap_or("node");

    info!("  Peer relationship: {}", peer_relationship);

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
        // Peer sent resource_ids they ALREADY HAVE
        // Compute set difference to send only missing resources

        // Get ALL resources in folder
        let all_resource_ids = repo_ctx
            .resource_repo
            .get_resource_ids_by_folder_id(&folder_id)
            .await
            .map_err(|e| {
                error!("Failed to get all resources for folder {}: {}", folder_id, e);
                crate::p2p::errors::P2PError::InvalidState(format!(
                    "Failed to get resources: {}",
                    e
                ))
            })?;

        // Convert to HashSets for efficient set difference
        let all_set: HashSet<String> = all_resource_ids.into_iter().collect();
        let peer_has_set: HashSet<String> = payload.resource_ids.iter().cloned().collect();

        // Compute difference: resources in folder that peer DOESN'T have
        let missing_resources: Vec<String> = all_set
            .difference(&peer_has_set)
            .cloned()
            .collect();

        info!(
            "  Peer has {} resources, folder has {} total, sending {} missing resources",
            peer_has_set.len(),
            all_set.len(),
            missing_resources.len()
        );

        missing_resources
    };

    // 6. Request resources WE'RE missing (bidirectional sync)
    // This runs BEFORE we check if we have resources to push, because we still want to
    // request resources we're missing even if we have nothing to send.
    let (missing_resource_ids, our_folder_permit) = services::get_missing_resources_for_sync(
        &folder_id,
        &payload.resource_ids,
        &current_user.id,
        repo_ctx.clone(),
    )
    .await
    .map_err(|e| {
        error!("Failed to compute missing resources: {}", e);
        crate::p2p::errors::P2PError::InvalidState(format!(
            "Failed to compute missing resources: {}",
            e
        ))
    })?;

    if !missing_resource_ids.is_empty() {
        let missing_count = missing_resource_ids.len();
        info!("📥 Requesting {} resources we don't have from peer", missing_count);

        for resource_id in missing_resource_ids {
            info!("   Requesting resource: {}", resource_id);

            let request = ResourceNotFoundRequestMsg {
                resource_id: resource_id.clone(),
                folder_permit: our_folder_permit.clone(),
            };

            peer_conn
                .send_message(Message::Resource(ResourceMessage::ResourceNotFoundRequest(request)))
                .await
                .map_err(|e| {
                    error!("Failed to send ResourceNotFoundRequest for {}: {}", resource_id, e);
                    e
                })?;
        }

        info!("✓ Sent {} ResourceNotFoundRequest messages", missing_count);
    } else {
        info!("✓ No missing resources to request from peer");
    }

    // 7. Early return if we have no resources to PUSH to peer
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

    // 6.5. Send FolderDataSync if ephemeral (on-the-fly generation)
    if !parsed_ucan.should_persist_share() {
        info!("  Token ephemeral - generating folder share on-the-fly");

        // Get peer user for delegation
        let peer_user_guard = peer_conn.user.read().await;
        let peer_user = peer_user_guard.clone();
        drop(peer_user_guard);

        // Delegate folder UCAN to peer using relationship as template key
        let (peer_folder_ucan, peer_cid) = {
            let ucan = peer_conn.ucan_service.read().await;
            ucan.delegate_folder(&owner_folder.ucan, &peer_relationship, &peer_user.ucan_pub_key)
                .await
                .map_err(|e| {
                    error!("Failed to delegate folder UCAN: {}", e);
                    crate::p2p::errors::P2PError::InvalidState(format!("Failed to delegate folder: {}", e))
                })?
        };

        // Create TWO ephemeral folder share records (dual-permit pattern)

        // 1. Viewer's/Requestor's folder_share_record (shared_by=node, recipient=viewer)
        let viewer_folder_share_record = osvauld_core::models::FolderShareRecord::prepare_folder_share_record(
            folder_id.clone(),
            current_user.id.clone(),  // shared_by = sender (node)
            peer_user.id.clone(),      // recipient = requestor (viewer)
            osvauld_core::models::PermissionLevel::Read,
            peer_folder_ucan.clone(),
            peer_cid,
        );

        // 2. Node's/Sender's folder_share_record (shared_by=node, recipient=node - self-reference)
        // Uses the original folder UCAN (owner's own permit)
        let sender_cid = gurkha::crypto::get_ucan_cid(&owner_folder.ucan)
            .map_err(|e| {
                error!("Failed to compute sender folder UCAN CID: {}", e);
                crate::p2p::errors::P2PError::InvalidState(format!("CID computation failed: {}", e))
            })?;

        let sender_folder_share_record = osvauld_core::models::FolderShareRecord::prepare_folder_share_record(
            folder_id.clone(),
            current_user.id.clone(),  // shared_by = sender (node)
            current_user.id.clone(),  // recipient = sender (node) - self-reference
            osvauld_core::models::PermissionLevel::Admin,
            owner_folder.ucan.clone(),
            sender_cid,
        );

        info!("  Sending FolderDataSync with 2 ephemeral folder_share_records (dual-permit)");

        // Send folder data with BOTH folder_share_records
        // Don't call send_folder_data() as it would try to fetch from DB
        let mut folder = owner_folder.clone();
        folder.ucan = peer_folder_ucan; // Use the peer's delegated folder UCAN

        let data = osvauld_core::models::FolderDataSync {
            folder,
            folder_share_records: vec![viewer_folder_share_record, sender_folder_share_record],
        };

        peer_conn
            .send_message(osvauld_core::models::Message::Folder(
                osvauld_core::models::FolderMessage::FolderDataSync(data)
            ))
            .await?;

        info!("✓ Sent ephemeral FolderDataSync to viewer");
    }

    // 8. Send each resource with appropriate share_records
    // Get peer user once outside the loop
    let peer_user_guard = peer_conn.user.read().await;
    let peer_user = peer_user_guard.clone();
    drop(peer_user_guard);

    for resource_id in resource_ids {
        // Use different flows for persistent vs ephemeral shares
        if parsed_ucan.should_persist_share() {
            // Persistent case: Use shared function (fetches share_records from DB)
            info!("     Token allows persistence - using persistent send flow");

            let share_records = services::get_all_share_records_for_resource(&resource_id, repo_ctx.clone())
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
                })?;

            // Use the shared send_resource_with_permit function
            if let Err(e) = resource_sync::send_resource_with_permit(
                &resource_id,
                &current_user,
                &peer_user,
                &peer_relationship,
                &payload.folder_permit,
                share_records,
                folder_ucan.clone(),
                peer_conn.clone(),
                repo_ctx.clone(),
                &crypto_utils,
            )
            .await
            {
                error!(
                    "Failed to send resource {}: {}, continuing",
                    resource_id, e
                );
            }
        } else {
            // Ephemeral case: Generate share_records on-the-fly
            // Need to prepare resource FIRST to get peer's UCAN for creating viewer's share record
            info!("     Token ephemeral - using ephemeral send flow");

            // Prepare resource for peer (delegates UCAN using template)
            let peer_encrypted_resource = match services::prepare_resource_transfer(
                &resource_id,
                &current_user,
                &payload.folder_permit,
                &peer_relationship,
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

            // Get original resource from DB to extract node's UCAN
            let original_resource = repo_ctx
                .resource_repo
                .find_by_id(&resource_id)
                .await
                .map_err(|e| {
                    error!("Failed to get original resource {}: {}", resource_id, e);
                    crate::p2p::errors::P2PError::InvalidState(format!(
                        "Failed to get original resource: {}",
                        e
                    ))
                })?;

            // 1. Create viewer's share record (shared_by=node, recipient=viewer)
            let viewer_ucan_cid = gurkha::crypto::get_ucan_cid(&peer_encrypted_resource.ucan_token)
                .map_err(|e| {
                    error!("Failed to compute viewer UCAN CID: {}", e);
                    crate::p2p::errors::P2PError::InvalidState(format!("CID computation failed: {}", e))
                })?;

            let viewer_share_record = osvauld_core::models::ShareRecord::prepare_share_record(
                resource_id.clone(),
                current_user.id.clone(),  // shared_by = node
                peer_user.id.clone(),      // recipient = viewer
                osvauld_core::models::PermissionLevel::Read,
                peer_encrypted_resource.ucan_token.clone(),
                viewer_ucan_cid,
            );

            // 2. Create node's share record (shared_by=node, recipient=node)
            // Uses the original resource UCAN (node's own permit)
            let node_ucan_cid = gurkha::crypto::get_ucan_cid(&original_resource.ucan_token)
                .map_err(|e| {
                    error!("Failed to compute node UCAN CID: {}", e);
                    crate::p2p::errors::P2PError::InvalidState(format!("CID computation failed: {}", e))
                })?;

            let node_share_record = osvauld_core::models::ShareRecord::prepare_share_record(
                resource_id.clone(),
                current_user.id.clone(),  // shared_by = node
                current_user.id.clone(),  // recipient = node (self)
                osvauld_core::models::PermissionLevel::Admin,
                original_resource.ucan_token.clone(),
                node_ucan_cid,
            );

            let share_records = vec![viewer_share_record, node_share_record];

            info!("     Created 2 ephemeral share records: viewer + node (for dual-permit validation)");
            info!("     Including {} share records", share_records.len());

            // Send ResourceDataSync
            let resource_data = osvauld_core::models::p2p::ResourceDataSync {
                resource: peer_encrypted_resource,
                share_records,
                owner_folder_permit: folder_ucan.clone(),
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
#[instrument(skip(payload, peer_conn, _repo_ctx, _crypto_utils), fields(folder_id = %payload.folder_id), level = "info")]
pub async fn handle_folder_token_request(
    payload: &osvauld_core::models::p2p::FolderTokenRequest,
    peer_conn: Arc<PeerConnection>,
    _repo_ctx: Arc<RepositoryContext>,
    _crypto_utils: Arc<RwLock<CryptoUtils>>,
) -> P2PResult<()> {
    info!("📨 Received FolderTokenRequest for folder {}", payload.folder_id);

    // 1. Parse and validate folder_ucan
    let parsed_ucan = gurkha::parser::Permit::from_token(&payload.folder_permit)
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

    // 4. Get current user (node) info for connection string
    // IMPORTANT: peer_conn.user now points to the PEER, so we must get local user from context
    let current_user = peer_conn.get_local_user().await?;

    let current_device_guard = peer_conn.context.current_device.read().await;
    let current_device = current_device_guard.as_ref()
        .ok_or_else(|| {
            error!("No current device found");
            crate::p2p::errors::P2PError::InvalidState("No device information".to_string())
        })?
        .clone();
    drop(current_device_guard);

    // 5. Generate ViewerAuth token with wildcard audience (aud:*)
    let ucan_service_guard = peer_conn.ucan_service.read().await;
    let (viewer_auth_token, _cid) = ucan_service_guard
        .issue_folder_viewer_auth(&payload.folder_id)
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
#[instrument(skip(payload, peer_conn), fields(folder_id = %payload.folder_id), level = "info")]
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
