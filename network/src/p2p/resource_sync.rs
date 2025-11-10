//! Resource sync - simple push after share_folder()
//!
//! This module handles resource sync operations.

use crate::p2p::{errors::P2PResult, peer_connection::PeerConnection};
use crypto_utils::CryptoUtils;
use osvauld_core::models::{Message, ResourceDataSync, ResourceNotFoundRequestMsg, ResourceSyncRequestMsg, ResourceTransferMsg, ResourceUpdateMsg, User};
use persistance::database::RepositoryContext;
use services::{get_all_share_records_for_resource, get_resource_share_records_for_folder, prepare_resource_for_peer};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info};

/// Send all resources for a folder to a peer
///
/// Bulk operation: Gets all resources + share records in one query,
/// then sends each resource to the peer.
pub async fn send_all_resources_for_folder(
    folder_id: &str,
    recipient_user_id: &str,
    current_user: &User,
    owner_folder_ucan: String,
    peer_conn: Arc<PeerConnection>,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: Arc<RwLock<CryptoUtils>>,
) -> P2PResult<()> {
    info!(
        "Sending all resources for folder {} to user {}",
        folder_id, recipient_user_id
    );

    // 1. Get all resources in folder
    let resources = repo_ctx
        .resource_repo
        .find_all_by_folder(folder_id, &current_user.id)
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
        get_resource_share_records_for_folder(folder_id, recipient_user_id, repo_ctx.clone())
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

    info!("Found {} resources shared with recipient", recipient_share_records.len());

    // 3. Get recipient user for public key
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

    // 4. Loop through recipient's share records and send each resource
    for recipient_share in recipient_share_records {
        // Get ALL share records for this resource (for forwarding viewer updates)
        let all_share_records = get_all_share_records_for_resource(&recipient_share.resource_id, repo_ctx.clone())
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

        // Prepare resource for peer (decrypt, filter, re-encrypt using recipient's UCAN)
        let peer_encrypted_resource = match prepare_resource_for_peer(
            &recipient_share.resource_id,
            recipient_user_id,
            &recipient_share.ucan_token,
            &recipient.public_key,
            repo_ctx.clone(),
            &crypto_utils,
        )
        .await
        {
            Ok(res) => res,
            Err(e) => {
                error!(
                    "Failed to prepare resource {} for peer: {}, skipping",
                    recipient_share.resource_id, e
                );
                continue; // Partial success - continue with other resources
            }
        };

        // Create ResourceDataSync message with ALL share records
        let resource_data = ResourceDataSync {
            resource: peer_encrypted_resource,
            share_records: all_share_records,
            owner_folder_ucan: owner_folder_ucan.clone(),
        };

        // Send to peer (fire-and-forget pattern, log errors)
        if let Err(e) = send_resource_data(peer_conn.clone(), resource_data).await {
            error!(
                "Failed to send resource {} to peer: {}, continuing",
                recipient_share.resource_id, e
            );
            // Continue with other resources even if one fails
        } else {
            info!("Successfully sent resource {}", recipient_share.resource_id);
        }
    }

    info!(
        "Finished sending resources for folder {} to user {}",
        folder_id, recipient_user_id
    );
    Ok(())
}

/// Send single resource data to node
pub async fn send_resource_data(
    peer_conn: Arc<PeerConnection>,
    resource_data: ResourceDataSync,
) -> P2PResult<()> {
    info!("Sending resource {} to node", resource_data.resource.id);

    peer_conn
        .send_message(Message::ResourceDataSync(resource_data))
        .await
}

/// Handle resource data sync from owner (node side)
///
/// Orchestrates resource acceptance by delegating to resource_service
pub async fn handle_resource_data_sync(
    payload: ResourceDataSync,
    peer_conn: Arc<PeerConnection>,
    repo_ctx: Arc<RepositoryContext>,
    _crypto_utils: Arc<RwLock<CryptoUtils>>,
) -> P2PResult<()> {
    info!("Received resource {} from peer", payload.resource.id);

    let domain = &peer_conn.domain;

    // Delegate to resource_service for validation and saving
    services::accept_resource_from_peer(
        &payload.resource,
        &payload.share_records,
        &payload.owner_folder_ucan,
        domain,
        repo_ctx,
    )
    .await
    .map_err(|e| {
        error!("Failed to accept resource from peer: {}", e);
        crate::p2p::errors::P2PError::InvalidState(format!("Failed to accept resource: {}", e))
    })?;

    info!("✓ Accepted and saved resource {}", payload.resource.id);
    Ok(())
}

// =============================================================================
// Resource Request Protocol Handlers
// =============================================================================

/// Handle resource sync request from initiator (responder side)
///
/// Flow:
/// - Receives resource_ucan + folder_ucan from initiator
/// - Extracts resource_id and checks if resource exists locally
/// - If not found: Sends ResourceNotFoundRequest with responder's folder_ucan
/// - If found: Continues with normal merge sync (StateVectorRequest)
///
/// # Arguments
/// * `payload` - ResourceSyncRequestMsg containing resource_ucan and initiator's folder_ucan
/// * `peer_conn` - Peer connection to send response
/// * `repo_ctx` - Database repository context
/// * `crypto_utils` - Crypto utilities for UCAN parsing
///
/// # Returns
/// * `Ok(())` - Response sent (either ResourceNotFoundRequest or StateVectorRequest)
/// * `Err` - If UCAN parsing or database lookup fails
pub async fn handle_resource_sync_request(
    payload: ResourceSyncRequestMsg,
    peer_conn: Arc<PeerConnection>,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: Arc<RwLock<CryptoUtils>>,
) -> P2PResult<()> {
    info!("Received resource sync request from peer");

    // Extract resource_id from UCAN token
    let resource_id = services::ucan_service::extract_resource_id(&payload.resource_ucan)
        .await
        .map_err(|e| {
            error!("Failed to extract resource_id from UCAN: {}", e);
            crate::p2p::errors::P2PError::InvalidState(format!("Invalid UCAN token: {}", e))
        })?;

    info!("Checking if resource {} exists locally", resource_id);

    // Check if resource exists locally
    let resource_exists = repo_ctx
        .resource_repo
        .find_by_id(&resource_id)
        .await
        .is_ok();

    if !resource_exists {
        info!("Resource {} not found locally, requesting from peer", resource_id);

        // Get local user
        let local_user = peer_conn.get_local_user().await?;

        // Get responder's folder_ucan using folder_service
        let domain = &peer_conn.domain;
        let responder_folder_ucan = services::get_responder_folder_ucan_for_folder(
            &payload.folder_ucan,
            &local_user.id,
            domain,
            repo_ctx.clone(),
        )
        .await
        .map_err(|e| {
            error!("Failed to get responder's folder_ucan: {}", e);
            crate::p2p::errors::P2PError::InvalidState(format!(
                "Failed to get folder access: {}",
                e
            ))
        })?;

        info!("✓ Found responder's folder_ucan, sending ResourceNotFoundRequest");

        // Send ResourceNotFoundRequest with responder's folder_ucan
        let request = ResourceNotFoundRequestMsg {
            resource_id: resource_id.clone(),
            folder_ucan: responder_folder_ucan,
        };

        peer_conn
            .send_message(Message::ResourceNotFoundRequest(request))
            .await?;

        info!("✓ Sent ResourceNotFoundRequest for resource {}", resource_id);
    } else {
        info!("Resource {} found locally, getting state vectors for sync", resource_id);

        // Get our state vectors filtered by initiator's UCAN
        let domain = &peer_conn.domain;
        let our_state_vectors = services::get_resource_state_vectors_by_ucan(
            &payload.resource_ucan,
            domain,
            repo_ctx.clone(),
            &crypto_utils,
        )
        .await
        .map_err(|e| {
            error!("Failed to get state vectors for resource {}: {}", resource_id, e);
            crate::p2p::errors::P2PError::InvalidState(format!("Failed to get state vectors: {}", e))
        })?;

        info!("✓ Got state vectors, sending StateVectorRequest to initiator");

        // TODO: Get our asset IDs from resource
        let our_asset_ids: Vec<String> = vec![]; // Placeholder

        // Send StateVectorRequest with our state and assets
        let state_vector_request = ResourceUpdateMsg::StateVectorRequest {
            resource_id: resource_id.clone(),
            state_vectors: our_state_vectors,
            asset_ids: our_asset_ids,
            ucan_token: payload.resource_ucan.clone(), // Use initiator's UCAN to identify which resource
        };

        peer_conn
            .send_message(Message::MergeUpdate(state_vector_request))
            .await?;

        info!("✓ Sent StateVectorRequest for resource {}", resource_id);
    }

    Ok(())
}

/// Handle resource not found request from responder (initiator side)
///
/// Flow:
/// - Receives ResourceNotFoundRequest with resource_id and responder's folder_ucan
/// - Delegates to resource_service to validate access and prepare resource
/// - Sends ResourceTransfer with resource + share records
///
/// # Arguments
/// * `payload` - ResourceNotFoundRequestMsg with resource_id and responder's folder_ucan
/// * `peer_conn` - Peer connection to send resource
/// * `repo_ctx` - Database repository context
/// * `crypto_utils` - Crypto utilities for encryption
///
/// # Returns
/// * `Ok(())` - Resource sent successfully
/// * `Err` - If validation fails or resource preparation fails
pub async fn handle_resource_not_found_request(
    payload: ResourceNotFoundRequestMsg,
    peer_conn: Arc<PeerConnection>,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: Arc<RwLock<CryptoUtils>>,
) -> P2PResult<()> {
    info!("Received resource not found request for resource {}", payload.resource_id);

    // Get peer user for re-encryption
    let peer_user = peer_conn.get_peer_user().await;

    let domain = &peer_conn.domain;

    // Delegate to resource_service to validate and prepare resource
    let (peer_encrypted_resource, all_share_records) = services::prepare_resource_transfer(
        &payload.resource_id,
        &payload.folder_ucan,
        &peer_user.id,
        &peer_user.public_key,
        domain,
        repo_ctx.clone(),
        &crypto_utils,
    )
    .await
    .map_err(|e| {
        error!(
            "Failed to prepare resource transfer for {}: {}",
            payload.resource_id, e
        );
        crate::p2p::errors::P2PError::InvalidState(format!("Failed to prepare transfer: {}", e))
    })?;

    info!("✓ Resource prepared, sending to peer");

    // Send ResourceTransfer message
    let transfer_msg = ResourceTransferMsg {
        resource: peer_encrypted_resource,
        share_records: all_share_records,
    };

    peer_conn
        .send_message(Message::ResourceTransfer(transfer_msg))
        .await?;

    info!("✓ Resource {} sent to peer", payload.resource_id);
    Ok(())
}

/// Handle resource transfer from initiator (responder side)
///
/// Flow:
/// - Receives ResourceTransfer with complete resource + share records
/// - Delegates to resource_service to save resource
/// - Sends ResourceTransferAck to confirm receipt
///
/// # Arguments
/// * `payload` - ResourceTransferMsg with resource and share records
/// * `peer_conn` - Peer connection to send ack
/// * `repo_ctx` - Database repository context
/// * `_crypto_utils` - Crypto utilities (unused, for consistency)
///
/// # Returns
/// * `Ok(())` - Resource saved and ack sent
/// * `Err` - If save fails
pub async fn handle_resource_transfer(
    payload: ResourceTransferMsg,
    peer_conn: Arc<PeerConnection>,
    repo_ctx: Arc<RepositoryContext>,
    _crypto_utils: Arc<RwLock<CryptoUtils>>,
) -> P2PResult<()> {
    info!("Received resource transfer for resource {}", payload.resource.id);

    // Delegate to resource_service to save resource transfer
    services::save_resource_transfer(&payload.resource, &payload.share_records, repo_ctx)
        .await
        .map_err(|e| {
            error!(
                "Failed to save resource transfer for {}: {}",
                payload.resource.id, e
            );
            crate::p2p::errors::P2PError::InvalidState(format!("Failed to save resource: {}", e))
        })?;

    // Send acknowledgment
    peer_conn
        .send_message(Message::ResourceTransferAck)
        .await?;

    info!("✓ Resource transfer complete for {}", payload.resource.id);
    Ok(())
}

/// Handle resource transfer acknowledgment from responder (initiator side)
///
/// Flow:
/// - Receives ResourceTransferAck confirming responder saved the resource
/// - Logs completion (no further action needed)
///
/// # Returns
/// * `Ok(())` - Always succeeds
pub async fn handle_resource_transfer_ack(
    peer_conn: Arc<PeerConnection>,
) -> P2PResult<()> {
    info!("Received resource transfer acknowledgment from peer {}", peer_conn.get_id());
    // Transfer complete - no further action needed
    Ok(())
}

// =============================================================================
// CRDT Merge Sync Protocol Handlers
// =============================================================================

/// Handle StateVectorRequest from peer (responder side)
///
/// Flow:
/// - Receives peer's state vectors and asset IDs
/// - Generates incremental updates for peer based on their state
/// - Checks which assets peer is missing
/// - Sends UpdatesResponse with our updates, state vectors, and missing assets list
///
/// # Arguments
/// * `resource_id` - ID of resource being synced
/// * `peer_state_vectors` - Peer's current state vectors (JSON)
/// * `peer_asset_ids` - Asset IDs that peer has
/// * `peer_ucan` - Peer's UCAN token
/// * `peer_conn` - Peer connection
/// * `repo_ctx` - Database repository context
/// * `crypto_utils` - Crypto utilities
///
/// # Returns
/// * `Ok(())` - UpdatesResponse sent successfully
/// * `Err` - If update generation or sending fails
pub async fn handle_state_vector_request(
    resource_id: String,
    peer_state_vectors: String,
    peer_asset_ids: Vec<String>,
    peer_ucan: String,
    peer_conn: Arc<PeerConnection>,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: Arc<RwLock<CryptoUtils>>,
) -> P2PResult<()> {
    info!("Received StateVectorRequest for resource {}", resource_id);

    // Generate updates for peer based on their state vectors
    // This function loads the resource from DB, decrypts it, generates updates, and extracts state vectors
    let (our_updates, our_state_vectors) = services::generate_updates_for_peer(
        &peer_ucan,
        &peer_state_vectors,
        repo_ctx.clone(),
        &crypto_utils,
    )
    .await
    .map_err(|e| {
        error!("Failed to generate updates for peer: {}", e);
        crate::p2p::errors::P2PError::InvalidState(format!("Failed to generate updates: {}", e))
    })?;

    info!("✓ Generated updates and state vectors for peer");

    // TODO: Get our asset IDs and compare with peer's to find missing assets
    let missing_asset_ids: Vec<String> = vec![]; // Placeholder

    info!("✓ Extracted state vectors, sending UpdatesResponse");

    // Send UpdatesResponse
    let updates_response = ResourceUpdateMsg::UpdatesResponse {
        resource_id,
        updates: our_updates,
        state_vectors: our_state_vectors,
        missing_asset_ids,
        ucan_token: peer_ucan,
    };

    peer_conn
        .send_message(Message::MergeUpdate(updates_response))
        .await?;

    info!("✓ Sent UpdatesResponse to peer");
    Ok(())
}

/// Handle UpdatesResponse from peer (initiator side)
///
/// Flow:
/// - Receives peer's updates and state vectors
/// - Applies peer's updates to local resource
/// - Generates our updates based on peer's new state
/// - Saves updated resource
/// - If peer needs assets, sends AssetTransfer message
///
/// # Arguments
/// * `resource_id` - ID of resource being synced
/// * `peer_updates` - Peer's incremental updates (JSON)
/// * `peer_state_vectors` - Peer's current state vectors (JSON)
/// * `missing_asset_ids` - Asset IDs that peer needs from us
/// * `peer_ucan` - Peer's UCAN token
/// * `peer_conn` - Peer connection
/// * `repo_ctx` - Database repository context
/// * `crypto_utils` - Crypto utilities
///
/// # Returns
/// * `Ok(())` - Updates applied successfully
/// * `Err` - If applying updates or saving fails
pub async fn handle_updates_response(
    resource_id: String,
    peer_updates: String,
    peer_state_vectors: String,
    missing_asset_ids: Vec<String>,
    peer_ucan: String,
    peer_conn: Arc<PeerConnection>,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: Arc<RwLock<CryptoUtils>>,
) -> P2PResult<()> {
    info!("Received UpdatesResponse for resource {}", resource_id);

    // Get local user for encryption
    let local_user = peer_conn.get_local_user().await?;

    // Apply peer's updates and generate our updates back
    let our_updates = services::apply_peer_updates(
        &peer_ucan,
        &peer_updates,
        &local_user,
        repo_ctx.clone(),
        &crypto_utils,
    )
    .await
    .map_err(|e| {
        error!("Failed to apply peer updates: {}", e);
        crate::p2p::errors::P2PError::InvalidState(format!("Failed to apply updates: {}", e))
    })?;

    info!("✓ Applied peer updates and saved resource");

    // If we have updates to send back to peer, send them
    // Check if our_updates contains any actual updates (not just empty state vectors)
    if !our_updates.trim().is_empty() && our_updates != "{}" {
        info!("We have updates to send back to peer");

        // TODO: Send our updates back if needed (depends on sync strategy)
        // For now, we assume one-way sync from responder to initiator
    }

    // TODO: If peer needs assets, send AssetTransfer
    if !missing_asset_ids.is_empty() {
        info!("Peer needs {} assets - asset transfer not yet implemented", missing_asset_ids.len());
        // TODO: Implement asset transfer
    }

    info!("✓ Sync complete for resource {}", resource_id);
    Ok(())
}
