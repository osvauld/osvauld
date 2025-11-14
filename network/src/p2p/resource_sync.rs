//! Resource Sync - Resource Publishing to Node
//!
//! Orchestrates sending resources from owner to node as part of folder publishing.
//! Resources are re-encrypted with the recipient's UCAN and sent with all share_records.

use crate::p2p::{errors::P2PResult, peer_connection::PeerConnection};
use crypto_utils::CryptoUtils;
use osvauld_core::models::{
    p2p::{Message, ResourceDataSync, ResourceMessage},
    EncryptedResource, ShareRecord, User,
};
use persistance::database::RepositoryContext;
use services;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info};

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
/// * `owner_folder_ucan` - Owner's folder UCAN token
/// * `peer_conn` - Peer connection to send through
/// * `repo_ctx` - Database repository context
/// * `crypto_utils` - Crypto utilities for encryption
///
/// # Returns
/// * `Ok(())` - All resources sent successfully (partial failures are logged but not returned)
/// * `Err` - If critical errors occur (database access, etc.)
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
        "📦 Sending all resources for folder {} to user {}",
        folder_id, recipient_user_id
    );

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
    let peer_role = services::ucan_service::extract_facts(recipient_folder_ucan)
        .await?
        .and_then(|facts| {
            facts.get("role")
                .and_then(|v| v.as_str())
                .map(String::from)
        })
        .unwrap_or_else(|| "node".to_string()); // Default to "node" for Owner → Node sync

    info!("  Recipient role: {}", peer_role);

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

        info!(
            "     Including {} share records (for viewer forwarding)",
            all_share_records.len()
        );

        // Prepare resource for peer (UCAN-first: validates folder access, delegates, filters, encrypts)
        let peer_encrypted_resource = match services::prepare_resource_transfer(
            &recipient_share.resource_id,
            current_user,
            recipient_folder_ucan,
            &peer_role,
            &recipient,
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
            info!("     ✓ Successfully sent resource");
        }
    }

    info!(
        "✅ Finished sending resources for folder {} to user {}",
        folder_id, recipient_user_id
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
async fn send_resource_data(
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
        ResourceMessage::MergeUpdate(_) => {
            info!("MergeUpdate not yet implemented (CRDT merge)");
            Ok(())
        }
        ResourceMessage::ResourceSyncRequest(_) => {
            info!("ResourceSyncRequest not yet implemented (CRDT sync)");
            Ok(())
        }
        ResourceMessage::ResourceNotFoundRequest(_) => {
            info!("ResourceNotFoundRequest not yet implemented");
            Ok(())
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
/// * `payload` - Reference to ResourceDataSync containing resource, share_records, and owner_folder_ucan
/// * `peer_conn` - Peer connection (for emitting events)
/// * `repo_ctx` - Database repository context
/// * `_crypto_utils` - Crypto utilities (unused for now)
///
/// # Returns
/// * `Ok(())` - Resource accepted and saved successfully
/// * `Err` - If validation fails or database save fails
pub async fn handle_resource_data_sync(
    payload: &ResourceDataSync,
    peer_conn: Arc<PeerConnection>,
    repo_ctx: Arc<RepositoryContext>,
    _crypto_utils: Arc<RwLock<CryptoUtils>>,
) -> P2PResult<()> {
    info!("📥 Received resource {} from peer", payload.resource.id);

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
