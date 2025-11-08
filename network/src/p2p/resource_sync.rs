//! Resource sync - simple push after share_folder()
//!
//! This module handles resource sync operations.

use crate::p2p::{errors::P2PResult, peer_connection::PeerConnection};
use crypto_utils::CryptoUtils;
use osvauld_core::models::{Message, ResourceDataSync, User};
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
