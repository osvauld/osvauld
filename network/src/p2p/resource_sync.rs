//! Resource Sync - Resource Publishing to Node
//!
//! Orchestrates sending resources from owner to node as part of folder publishing.
//! Resources are re-encrypted with the recipient's UCAN and sent with all share_records.

use crate::p2p::{errors::P2PResult, peer_connection::PeerConnection};
use crypto_utils::CryptoUtils;
use osvauld_core::models::{
    p2p::{Message, ResourceDataSync, ResourceMessage, ResourceNotFoundRequestMsg, ResourceSyncRequestMsg, ResourceUpdateMsg}, ShareRecord, User,
};
use persistance::database::RepositoryContext;
use services;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;
use tracing::{debug, error, info, instrument};

/// Send a single resource to peer with given parameters
///
/// Low-level function that both persistent and ephemeral resource sending can use.
/// Prepares the resource (validates, delegates, filters, encrypts) and sends it with share records.
///
/// # Arguments
/// * `resource_id` - ID of the resource to send
/// * `current_user` - Current user (sender)
/// * `peer_user` - Peer user (recipient)
/// * `peer_relationship` - Peer's relationship/role (node, viewer, etc.)
/// * `folder_permit` - Folder UCAN to validate against
/// * `share_records` - Share records to send (from DB or generated on-the-fly)
/// * `owner_folder_permit` - Owner's folder UCAN (proves add_resources permission)
/// * `peer_conn` - Peer connection to send through
/// * `repo_ctx` - Database repository context
/// * `crypto_utils` - Crypto utilities for encryption
///
/// # Returns
/// * `Ok(bytes_sent)` - Resource sent successfully, returns size in bytes
/// * `Err` - If preparation or sending fails
pub async fn send_resource_with_permit(
    resource_id: &str,
    current_user: &User,
    peer_user: &User,
    peer_relationship: &str,
    folder_permit: &str,
    share_records: Vec<ShareRecord>,
    owner_folder_permit: String,
    peer_conn: Arc<PeerConnection>,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: &Arc<RwLock<CryptoUtils>>,
) -> P2PResult<u64> {
    info!("   Sending resource: {}", resource_id);

    // Prepare resource for peer (UCAN-first: validates folder access, delegates, filters, encrypts)
    let peer_encrypted_resource = services::prepare_resource_transfer(
        resource_id,
        current_user,
        folder_permit,
        peer_relationship,
        peer_user,
        repo_ctx.clone(),
        crypto_utils,
        &peer_conn.ucan_service,
    )
    .await
    .map_err(|e| {
        error!(
            "Failed to prepare resource {} for peer: {}",
            resource_id, e
        );
        crate::p2p::errors::P2PError::InvalidState(format!(
            "Failed to prepare resource: {}",
            e
        ))
    })?;

    // Track resource size for metrics
    let resource_bytes = peer_encrypted_resource.encrypted_data.len() as u64;

    info!(
        "     Including {} share records",
        share_records.len()
    );

    // Create ResourceDataSync message with share records
    let resource_data = ResourceDataSync {
        resource: peer_encrypted_resource,
        share_records,
        owner_folder_permit,
    };

    // Send the resource
    peer_conn
        .send_message(Message::Resource(ResourceMessage::ResourceDataSync(
            resource_data,
        )))
        .await?;

    info!("     ✓ Sent resource {}", resource_id);

    Ok(resource_bytes)
}

/// Send all resources in a folder to the recipient
///
/// This function is called by folder_sync after the folder data is sent.
/// It loops through all resources in the folder and sends each one with:
/// - Resource data re-encrypted with recipient's UCAN
/// - ALL share_records for that resource (enables node to forward to viewers)
/// - Owner's folder UCAN (proves add_resources permission)
///
/// # Arguments
/// * `folder_id` - ID of the folder containing the resources
/// * `recipient_user_id` - User ID of the recipient (node)
/// * `current_user` - Current user (owner) sending the resources
/// * `folder_ucan` - Owner's folder UCAN token
/// * `peer_conn` - Peer connection to send through
/// * `repo_ctx` - Database repository context
/// * `crypto_utils` - Crypto utilities for encryption
///
/// # Returns
/// * `Ok(())` - All resources sent successfully (partial failures are logged but not returned)
/// * `Err` - If critical errors occur (database access, etc.)
#[instrument(skip(current_user, folder_ucan, peer_conn, repo_ctx, crypto_utils), fields(
    folder_id = %folder_id,
    recipient_user_id = %recipient_user_id,
    resources_sent,
    total_bytes,
    avg_speed_mbps,
    total_duration_ms
))]
pub async fn send_all_resources_for_folder(
    folder_id: &str,
    recipient_user_id: &str,
    current_user: &User,
    folder_ucan: String,
    peer_conn: Arc<PeerConnection>,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: Arc<RwLock<CryptoUtils>>,
) -> P2PResult<()> {
    let start = Instant::now();
    let mut total_bytes = 0u64;
    let mut resources_sent = 0usize;

    info!("📦 Sending all resources for folder to user");

    // 1. Get all resources in folder
    let resources = repo_ctx
        .resource_repo
        .get_resource_ids_by_folder_id(folder_id)
        .await
        .map_err(|e| {
            error!("Failed to get resources for folder {}: {}", folder_id, e);
            crate::p2p::errors::P2PError::InvalidState(format!("Failed to get resources: {}", e))
        })?;

    if resources.is_empty() {
        info!("No resources to send for folder {}", folder_id);
        return Ok(());
    }

    info!("Found {} resources to send", resources.len());

    // 2. Get recipient's share records to know which resources to send
    let recipient_share_records =
        services::get_resource_share_records_for_folder(folder_id, recipient_user_id, repo_ctx.clone())
            .await
            .map_err(|e| {
                error!(
                    "Failed to get recipient share records for folder {}: {}",
                    folder_id, e
                );
                crate::p2p::errors::P2PError::InvalidState(format!(
                    "Failed to get share records: {}",
                    e
                ))
            })?;

    info!(
        "Found {} resources shared with recipient",
        recipient_share_records.len()
    );

    // 3. Get recipient user
    let recipient = repo_ctx
        .user_repo
        .get_user_by_id(recipient_user_id)
        .await
        .map_err(|e| {
            error!("Failed to get recipient user {}: {}", recipient_user_id, e);
            crate::p2p::errors::P2PError::InvalidState(format!(
                "Failed to get recipient user: {}",
                e
            ))
        })?;

    // 4. Get recipient's folder_share_record (contains their folder UCAN and role)
    let recipient_folder_share = services::get_folder_share_record(folder_id, recipient_user_id, repo_ctx.clone())
        .await
        .map_err(|e| {
            error!("Failed to get recipient folder share record: {}", e);
            crate::p2p::errors::P2PError::InvalidState(format!(
                "Failed to get folder share record: {}",
                e
            ))
        })?;

    let recipient_folder_ucan = &recipient_folder_share.ucan_token;

    // Extract role from folder UCAN (stored in facts)
    let parsed_ucan = gurkha::parser::Permit::from_token(recipient_folder_ucan)
        .map_err(|e| {
            crate::p2p::errors::P2PError::InvalidState(format!("Failed to parse folder UCAN: {}", e))
        })?;

    let peer_relationship = parsed_ucan.relationship()
        .unwrap_or("node"); // Default to "node" for Owner → Node sync

    info!("  Recipient relationship: {}", peer_relationship);

    // 5. Loop through recipient's share records and send each resource
    for recipient_share in recipient_share_records {
        info!("   Sending resource: {}", recipient_share.resource_id);

        // Get ALL share records for this resource (for forwarding viewer updates)
        let all_share_records =
            services::get_all_share_records_for_resource(&recipient_share.resource_id, repo_ctx.clone())
                .await
                .map_err(|e| {
                    error!(
                        "Failed to get all share records for resource {}: {}",
                        recipient_share.resource_id, e
                    );
                    crate::p2p::errors::P2PError::InvalidState(format!(
                        "Failed to get all share records: {}",
                        e
                    ))
                })?;

        // Use the shared send_resource_with_permit function
        match send_resource_with_permit(
            &recipient_share.resource_id,
            current_user,
            &recipient,
            &peer_relationship,
            recipient_folder_ucan,
            all_share_records,
            folder_ucan.clone(),
            peer_conn.clone(),
            repo_ctx.clone(),
            &crypto_utils,
        )
        .await
        {
            Ok(resource_bytes) => {
                total_bytes += resource_bytes;
                resources_sent += 1;
                debug!(
                    "✓ Resource sent: {} bytes",
                    resource_bytes
                );
            }
            Err(e) => {
                error!(
                    "Failed to send resource {}: {}, continuing",
                    recipient_share.resource_id, e
                );
                // Continue with other resources even if one fails
            }
        }
    }

    // Calculate and record metrics
    let duration = start.elapsed();
    let avg_speed_mbps = if duration.as_secs_f64() > 0.0 {
        (total_bytes as f64 / duration.as_secs_f64()) / 1_000_000.0
    } else {
        0.0
    };

    // Record to span
    tracing::Span::current().record("resources_sent", resources_sent);
    tracing::Span::current().record("total_bytes", total_bytes);
    tracing::Span::current().record("avg_speed_mbps", format!("{:.2}", avg_speed_mbps).as_str());
    tracing::Span::current().record("total_duration_ms", duration.as_millis());

    info!(
        "✅ Folder sync complete: {} resources, {:.2} MB, {:.2} MB/s in {}ms",
        resources_sent,
        total_bytes as f64 / 1_000_000.0,
        avg_speed_mbps,
        duration.as_millis()
    );
    Ok(())
}

/// Send single resource data to node
///
/// Low-level function to send a ResourceDataSync message.
///
/// # Arguments
/// * `peer_conn` - Peer connection to send through
/// * `resource_data` - ResourceDataSync message to send
///
/// # Returns
/// * `Ok(())` - Message sent successfully
/// * `Err` - If send fails
#[instrument(skip(peer_conn, resource_data), fields(resource_id = %resource_data.resource.id), level = "info")]
pub async fn send_resource_data(
    peer_conn: Arc<PeerConnection>,
    resource_data: ResourceDataSync,
) -> P2PResult<()> {
    peer_conn
        .send_message(Message::Resource(ResourceMessage::ResourceDataSync(
            resource_data,
        )))
        .await
}

/// Process resource messages (central dispatcher)
///
/// Single entry point for all resource-related messages.
/// Matches on ResourceMessage variants and delegates to appropriate handlers.
///
/// # Arguments
/// * `resource_msg` - ResourceMessage variant to process
/// * `peer_conn` - Peer connection
/// * `repo_ctx` - Database repository context
/// * `crypto_utils` - Crypto utilities
///
/// # Returns
/// * `Ok(())` - Message processed successfully
/// * `Err` - If processing fails
#[instrument(skip(resource_msg, peer_conn, repo_ctx, crypto_utils), fields(message_type = ?std::mem::discriminant(resource_msg)), level = "info")]
pub async fn process_resource_message(
    resource_msg: &ResourceMessage,
    peer_conn: Arc<PeerConnection>,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: Arc<RwLock<CryptoUtils>>,
) -> P2PResult<()> {
    match resource_msg {
        ResourceMessage::ResourceDataSync(payload) => {
            handle_resource_data_sync(payload, peer_conn, repo_ctx, crypto_utils).await
        }
        ResourceMessage::MergeUpdate(payload) => {
            handle_merge_update(payload, peer_conn, repo_ctx, crypto_utils).await
        }
        ResourceMessage::ResourceSyncRequest(payload) => {
            handle_resource_sync_request(payload, peer_conn, repo_ctx, crypto_utils).await
        }
        ResourceMessage::ResourceNotFoundRequest(payload) => {
            handle_resource_not_found_request(payload, peer_conn, repo_ctx, crypto_utils).await
        }
        ResourceMessage::ResourceTransfer(_) => {
            info!("ResourceTransfer not yet implemented");
            Ok(())
        }
        ResourceMessage::ResourceTransferAck => {
            info!("ResourceTransferAck not yet implemented");
            Ok(())
        }
        ResourceMessage::AssetTransfer(_) => {
            info!("AssetTransfer not yet implemented");
            Ok(())
        }
    }
}

/// Handle resource data sync from owner (node side)
///
/// Receives a resource and its share_records from the owner and saves them.
/// Validates the owner's folder UCAN has add_resources capability.
///
/// # Arguments
/// * `payload` - Reference to ResourceDataSync containing resource, share_records, and folder_ucan
/// * `peer_conn` - Peer connection (for emitting events)
/// * `repo_ctx` - Database repository context
/// * `_crypto_utils` - Crypto utilities (unused for now)
///
/// # Returns
/// * `Ok(())` - Resource accepted and saved successfully
/// * `Err` - If validation fails or database save fails
#[instrument(skip(payload, peer_conn, repo_ctx, _crypto_utils), fields(resource_id = %payload.resource.id), level = "info")]
pub async fn handle_resource_data_sync(
    payload: &ResourceDataSync,
    peer_conn: Arc<PeerConnection>,
    repo_ctx: Arc<RepositoryContext>,
    _crypto_utils: Arc<RwLock<CryptoUtils>>,
) -> P2PResult<()> {
    info!("📥 Received resource {} from peer", payload.resource.id);

    // Delegate to resource_service for validation and saving
    services::accept_resource_from_peer(
        &payload.resource,
        &payload.share_records,
        &payload.owner_folder_permit,
        repo_ctx,
        &peer_conn.ucan_service,
    )
    .await
    .map_err(|e| {
        error!("Failed to accept resource from peer: {}", e);
        crate::p2p::errors::P2PError::InvalidState(format!("Failed to accept resource: {}", e))
    })?;

    info!("✓ Accepted and saved resource {}", payload.resource.id);

    // Extract metadata and emit ResourceSynced event
    let metadata = &payload.resource.metadata;
    let title = metadata
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("Untitled")
        .to_string();
    let resource_type = metadata
        .get("type")
        .and_then(|v| v.as_str())
        .unwrap_or("website")
        .to_string();
    let last_modified = metadata
        .get("last_modified")
        .and_then(|v| v.as_i64())
        .unwrap_or_else(|| payload.resource.updated_at);

    let metadata_json = serde_json::json!({
        "id": payload.resource.id,
        "title": title,
        "resourceType": resource_type,
        "folderId": payload.resource.folder_id,
        "lastModified": last_modified,
        "favourite": false,
        "preview": null,
    });

    peer_conn.event_emitter.emit(crate::p2p::emitter::P2PEvent::ResourceSynced {
        metadata_json: metadata_json.to_string(),
    });

    Ok(())
}

// ============================================================================
// Single Resource Sync - Permit-Driven CRDT Merge
// ============================================================================

/// Sync a single resource with peer (viewer initiates)
///
/// Viewer calls this to initiate CRDT sync with node.
/// Sends ResourceSyncRequestMsg, receives UpdatesResponse, applies updates,
/// then immediately sends collaborative updates back (if any).
///
/// # Flow
/// 1. Viewer→Node: ResourceSyncRequestMsg (state vectors + submissions)
/// 2. Node→Viewer: UpdatesResponse (diff updates + state vectors)
/// 3. Viewer→Node: UpdatesResponse (collaborative updates, if any)
/// 4. Node: Final merge
///
/// # Arguments
/// * `resource_id` - Resource to sync
/// * `peer_user_id` - Node's user ID
/// * `current_user` - Current user (viewer)
/// * `peer_conn` - Peer connection
/// * `repo_ctx` - Database context
/// * `crypto_utils` - Crypto utilities
///
/// # Returns
/// * `Ok(())` - Sync completed successfully
/// * `Err` - If sync fails at any step
#[instrument(skip(current_user, peer_conn, repo_ctx, crypto_utils), fields(resource_id = %resource_id, peer_user_id = %peer_user_id), level = "info")]
pub async fn sync_resource(
    resource_id: &str,
    peer_user_id: &str,
    current_user: &User,
    peer_conn: Arc<PeerConnection>,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: Arc<RwLock<CryptoUtils>>,
) -> P2PResult<()> {
    info!("🔄 Initiating resource sync with peer");

    // Step 1: Prepare sync request (viewer)
    let sync_request = services::prepare_single_resource_sync_request(
        resource_id,
        peer_user_id,
        current_user,
        repo_ctx.clone(),
        &crypto_utils,
    )
    .await
    .map_err(|e| {
        error!("Failed to prepare sync request: {}", e);
        crate::p2p::errors::P2PError::InvalidState(format!("Failed to prepare sync request: {}", e))
    })?;

    info!("📤 Sending ResourceSyncRequestMsg to peer");

    // Send sync request to node
    peer_conn
        .send_message(Message::Resource(ResourceMessage::ResourceSyncRequest(
            sync_request,
        )))
        .await?;

    // Note: Steps 2-4 are handled in message handlers:
    // - Step 2: handle_resource_sync_request (node) → sends UpdatesResponse
    // - Step 3: handle_merge_update (viewer) → applies, sends collaborative updates
    // - Step 4: handle_merge_update (node) → final merge

    Ok(())
}

/// Handle incoming resource sync request (node receives from viewer)
///
/// Node receives ResourceSyncRequestMsg from viewer, processes it, and sends back UpdatesResponse.
///
/// # Arguments
/// * `payload` - ResourceSyncRequestMsg from viewer
/// * `peer_conn` - Peer connection
/// * `repo_ctx` - Database context
/// * `crypto_utils` - Crypto utilities
///
/// # Returns
/// * `Ok(())` - Sync request processed and response sent
/// * `Err` - If processing fails
#[instrument(skip(payload, peer_conn, repo_ctx, crypto_utils), fields(resource_id = %payload.resource_id), level = "info")]
pub async fn handle_resource_sync_request(
    payload: &ResourceSyncRequestMsg,
    peer_conn: Arc<PeerConnection>,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: Arc<RwLock<CryptoUtils>>,
) -> P2PResult<()> {
    info!("📥 Received ResourceSyncRequest from peer");

    // Get current user (local user, not peer user)
    let current_user = peer_conn.get_local_user().await?;

    // Process sync request and generate response
    let updates_response = services::process_single_resource_sync_request(
        &payload.resource_id,
        &payload.sender_permit,
        &payload.state_vectors,
        &payload.full_docs,
        &current_user,
        repo_ctx,
        &crypto_utils,
    )
    .await
    .map_err(|e| {
        error!("Failed to process sync request: {}", e);
        crate::p2p::errors::P2PError::InvalidState(format!("Failed to process sync request: {}", e))
    })?;

    info!("📤 Sending UpdatesResponse to peer");

    // Send updates response back to viewer
    peer_conn
        .send_message(Message::Resource(ResourceMessage::MergeUpdate(
            updates_response,
        )))
        .await?;

    Ok(())
}

/// Handle merge update (UpdatesResponse)
///
/// Handles UpdatesResponse messages in the sync protocol.
/// Can be called by either viewer or node:
/// - Viewer: Receives updates from node, applies them, sends collaborative updates back
/// - Node: Receives collaborative updates from viewer, applies them (final merge)
///
/// # Arguments
/// * `payload` - ResourceUpdateMsg::UpdatesResponse
/// * `peer_conn` - Peer connection
/// * `repo_ctx` - Database context
/// * `crypto_utils` - Crypto utilities
///
/// # Returns
/// * `Ok(())` - Updates applied successfully
/// * `Err` - If processing fails
#[instrument(skip(payload, peer_conn, repo_ctx, crypto_utils), level = "info")]
pub async fn handle_merge_update(
    payload: &ResourceUpdateMsg,
    peer_conn: Arc<PeerConnection>,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: Arc<RwLock<CryptoUtils>>,
) -> P2PResult<()> {
    let (resource_id, sender_permit, updates, state_vectors) = match payload {
        ResourceUpdateMsg::UpdatesResponse {
            resource_id,
            sender_permit,
            updates,
            state_vectors,
            ..
        } => (resource_id, sender_permit, updates, state_vectors),
        _ => {
            error!("Invalid ResourceUpdateMsg variant (expected UpdatesResponse)");
            return Err(crate::p2p::errors::P2PError::InvalidState(
                "Invalid ResourceUpdateMsg variant".to_string(),
            ));
        }
    };

    info!("📥 Received UpdatesResponse for resource {}", resource_id);

    // Get current user (local user, not peer user)
    let current_user = peer_conn.get_local_user().await?;

    // Try to apply updates and generate collaborative response
    // Only send collaborative updates if we're the initiator (viewer)
    // If we're NOT the initiator (node), just apply and stop (breaks ping-pong loop)
    match services::apply_updates_and_generate_collaborative_response(
        resource_id,
        sender_permit,
        updates,
        state_vectors,
        &current_user,
        repo_ctx,
        &crypto_utils,
    )
    .await
    {
        Ok(Some(collaborative_updates)) => {
            // Only send if we're the initiator (viewer)
            // Node should NOT respond to avoid infinite ping-pong
            if peer_conn.is_initiator {
                info!("📤 Sending collaborative updates back to peer (initiator)");
                peer_conn
                    .send_message(Message::Resource(ResourceMessage::MergeUpdate(
                        collaborative_updates,
                    )))
                    .await?;
            } else {
                info!("✅ Applied final updates (node - no response to prevent loop)");
            }
        }
        Ok(None) => {
            info!("✅ No collaborative updates to send (sync complete)");
        }
        Err(e) => {
            error!("Failed to apply updates: {}", e);
            return Err(crate::p2p::errors::P2PError::InvalidState(format!(
                "Failed to apply updates: {}",
                e
            )));
        }
    }

    Ok(())
}

/// Handle resource not found request (bidirectional folder sync)
///
/// When a peer requests a resource they don't have (using their folder_permit),
/// we prepare and send the resource back if they have valid permissions.
///
/// # Arguments
/// * `payload` - ResourceNotFoundRequestMsg with resource_id and folder_permit
/// * `peer_conn` - Peer connection
/// * `repo_ctx` - Database repository context
/// * `crypto_utils` - Crypto utilities for encryption
///
/// # Returns
/// * `Ok(())` - Resource sent successfully
/// * `Err` - If validation fails or resource not found
#[instrument(skip(payload, peer_conn, repo_ctx, crypto_utils), fields(resource_id = %payload.resource_id), level = "info")]
pub async fn handle_resource_not_found_request(
    payload: &ResourceNotFoundRequestMsg,
    peer_conn: Arc<PeerConnection>,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: Arc<RwLock<CryptoUtils>>,
) -> P2PResult<()> {
    info!("📥 Received ResourceNotFoundRequest for resource {}", payload.resource_id);

    // Get current user (sender) and peer user (requestor)
    let current_user_guard = peer_conn.context.current_user.read().await;
    let current_user = current_user_guard.as_ref()
        .ok_or_else(|| {
            error!("No current user in peer connection");
            crate::p2p::errors::P2PError::InvalidState("No current user".to_string())
        })?
        .clone();
    drop(current_user_guard);

    let peer_user_guard = peer_conn.user.read().await;
    let peer_user = peer_user_guard.clone();
    drop(peer_user_guard);

    info!("   Peer requesting: {}", peer_user.username);
    info!("   Peer user ID: {}", peer_user.id);

    // 🔍 LOG FULL PUBLIC KEY FOR DEBUGGING
    info!("🔑 [RE-ENCRYPT] FULL peer PGP public key being used for re-encryption:");
    info!("{}", peer_user.public_key);
    info!("🔑 [RE-ENCRYPT] End of peer PGP public key");

    // Parse folder_permit to extract relationship
    let parsed_ucan = gurkha::parser::Permit::from_token(&payload.folder_permit)
        .map_err(|e| {
            error!("Failed to parse folder permit: {}", e);
            crate::p2p::errors::P2PError::InvalidState(format!("Invalid folder permit: {}", e))
        })?;

    let peer_relationship = parsed_ucan.relationship()
        .ok_or_else(|| {
            error!("No relationship in folder permit");
            crate::p2p::errors::P2PError::InvalidState("No relationship in permit".to_string())
        })?;

    info!("   Peer relationship: {}", peer_relationship);
    info!("   Current user (sender): {}", current_user.id);

    // Prepare resource using existing function (same as folder publishing)
    info!("🔄 Calling prepare_resource_transfer to re-encrypt for peer");
    let peer_encrypted_resource = match services::prepare_resource_transfer(
        &payload.resource_id,
        &current_user,
        &payload.folder_permit,
        peer_relationship,
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
                "Failed to prepare resource {} for peer: {}",
                payload.resource_id, e
            );
            return Err(crate::p2p::errors::P2PError::InvalidState(format!(
                "Failed to prepare resource: {}",
                e
            )));
        }
    };

    // Get share_records based on CEL rule (same logic as folder_sync.rs)
    let share_records = if parsed_ucan.should_persist_share() {
        // Token allows persistence: fetch ALL share_records from DB
        info!("     Token allows persistence - fetching share records from DB");
        services::get_all_share_records_for_resource(&payload.resource_id, repo_ctx.clone())
            .await
            .map_err(|e| {
                error!(
                    "Failed to get share records for resource {}: {}",
                    payload.resource_id, e
                );
                crate::p2p::errors::P2PError::InvalidState(format!(
                    "Failed to get share records: {}",
                    e
                ))
            })?
    } else {
        // Token ephemeral: create share_records on-the-fly (BOTH requestor + sender)
        info!("     Token ephemeral - generating share records on-the-fly");

        // Get original resource from DB to extract sender's UCAN
        let original_resource = repo_ctx
            .resource_repo
            .find_by_id(&payload.resource_id)
            .await
            .map_err(|e| {
                error!("Failed to get original resource {}: {}", payload.resource_id, e);
                crate::p2p::errors::P2PError::InvalidState(format!(
                    "Failed to get original resource: {}",
                    e
                ))
            })?;

        // 1. Create requestor's share record (shared_by=sender, recipient=requestor)
        let requestor_ucan_cid = gurkha::crypto::get_ucan_cid(&peer_encrypted_resource.ucan_token)
            .map_err(|e| {
                error!("Failed to compute requestor UCAN CID: {}", e);
                crate::p2p::errors::P2PError::InvalidState(format!("CID computation failed: {}", e))
            })?;

        let requestor_share_record = osvauld_core::models::ShareRecord::prepare_share_record(
            payload.resource_id.clone(),
            current_user.id.clone(),  // shared_by = sender
            peer_user.id.clone(),      // recipient = requestor
            osvauld_core::models::PermissionLevel::Read,
            peer_encrypted_resource.ucan_token.clone(),
            requestor_ucan_cid,
        );

        // 2. Create sender's share record (shared_by=sender, recipient=sender)
        // Uses the original resource UCAN (sender's own permit)
        let sender_ucan_cid = gurkha::crypto::get_ucan_cid(&original_resource.ucan_token)
            .map_err(|e| {
                error!("Failed to compute sender UCAN CID: {}", e);
                crate::p2p::errors::P2PError::InvalidState(format!("CID computation failed: {}", e))
            })?;

        let sender_share_record = osvauld_core::models::ShareRecord::prepare_share_record(
            payload.resource_id.clone(),
            current_user.id.clone(),  // shared_by = sender
            current_user.id.clone(),  // recipient = sender (self)
            osvauld_core::models::PermissionLevel::Admin,
            original_resource.ucan_token.clone(),
            sender_ucan_cid,
        );

        vec![requestor_share_record, sender_share_record]
    };

    info!("   Resource prepared with {} share records, sending ResourceDataSync", share_records.len());

    // Send ResourceDataSync message (reuse existing message type)
    let resource_data = ResourceDataSync {
        resource: peer_encrypted_resource,
        share_records,
        owner_folder_permit: payload.folder_permit.clone(),
    };

    peer_conn
        .send_message(Message::Resource(ResourceMessage::ResourceDataSync(resource_data)))
        .await?;

    info!("✅ Sent ResourceDataSync for resource {}", payload.resource_id);

    Ok(())
}
