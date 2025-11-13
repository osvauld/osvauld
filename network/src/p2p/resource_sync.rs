//! Resource sync - simple push after share_folder()
//!
//! This module handles resource sync operations.

use crate::p2p::{errors::P2PResult, peer_connection::PeerConnection};
use osvauld_core::models::AssetTransferMsg;
use crypto_utils::CryptoUtils;
use osvauld_core::models::{
    Message, ResourceDataSync, ResourceMessage, ResourceNotFoundRequestMsg, ResourceSyncRequestMsg,
    ResourceTransferMsg, ResourceUpdateMsg, User,
};
use persistance::database::RepositoryContext;
use services::{
    get_all_share_records_for_resource, get_resource_share_records_for_folder,
    prepare_resource_for_peer,
};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info};

/// Process all Resource messages - central routing function
///
/// This function receives all Resource message variants and delegates
/// to the appropriate handler based on message type.
///
/// # Arguments
/// * `peer_conn` - The peer connection
/// * `message` - The ResourceMessage variant to process
/// * `repo_ctx` - Repository context
/// * `crypto_utils` - Crypto utilities
///
/// # Returns
/// * `Ok(())` - Message processed successfully
/// * `Err` - If processing fails
pub async fn process_message(
    peer_conn: Arc<PeerConnection>,
    message: ResourceMessage,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: Arc<RwLock<CryptoUtils>>,
) -> P2PResult<()> {
    match message {
        ResourceMessage::MergeUpdate(payload) => match payload {
            ResourceUpdateMsg::StateVectorRequest {
                resource_id,
                state_vectors,
                asset_ids,
                ucan_token,
            } => {
                handle_state_vector_request(
                    resource_id,
                    state_vectors,
                    asset_ids,
                    ucan_token,
                    peer_conn,
                    repo_ctx,
                    crypto_utils,
                )
                .await
            }
            ResourceUpdateMsg::UpdatesResponse {
                resource_id,
                updates,
                state_vectors,
                missing_asset_ids,
                ucan_token,
            } => {
                handle_updates_response(
                    resource_id,
                    updates,
                    state_vectors,
                    missing_asset_ids,
                    ucan_token,
                    peer_conn,
                    repo_ctx,
                    crypto_utils,
                )
                .await
            }
        },
        ResourceMessage::ResourceSyncRequest(payload) => {
            handle_resource_sync_request(payload, peer_conn, repo_ctx, crypto_utils).await
        }
        ResourceMessage::ResourceNotFoundRequest(payload) => {
            handle_resource_not_found_request(payload, peer_conn, repo_ctx, crypto_utils).await
        }
        ResourceMessage::ResourceTransfer(payload) => {
            handle_resource_transfer(payload, peer_conn, repo_ctx, crypto_utils).await
        }
        ResourceMessage::ResourceTransferAck => handle_resource_transfer_ack(peer_conn).await,
        ResourceMessage::ResourceDataSync(payload) => {
            handle_resource_data_sync(payload, peer_conn, repo_ctx, crypto_utils).await
        }
        ResourceMessage::AssetTransfer(payload) => {
            handle_asset_transfer(payload, peer_conn, repo_ctx, crypto_utils).await
        }
    }
}

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

    info!(
        "Found {} resources shared with recipient",
        recipient_share_records.len()
    );

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
        let all_share_records =
            get_all_share_records_for_resource(&recipient_share.resource_id, repo_ctx.clone())
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
        .send_message(Message::Resource(ResourceMessage::ResourceDataSync(
            resource_data,
        )))
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

// =============================================================================
// Resource Request Protocol Handlers
// =============================================================================

/// Handle resource sync request from initiator (responder side) - Round 1
///
/// NEW PROTOCOL: ResourceSyncRequest now contains state_vectors and full_docs.
/// This is the first round of the 2-round sync protocol.
///
/// Flow:
/// - Receives resource_ucan + folder_ucan + state_vectors + full_docs from initiator
/// - Extracts resource_id and checks if resource exists locally
/// - If not found: Sends ResourceNotFoundRequest with responder's folder_ucan
/// - If found:
///   1. Parse and apply full_docs (e.g., viewer submissions)
///   2. Generate updates for peer based on their state_vectors
///   3. Compare asset_ids to determine missing assets
///   4. Send UpdatesResponse (first round response with updates)
///
/// # Arguments
/// * `payload` - ResourceSyncRequestMsg containing resource_ucan, folder_ucan, state_vectors, full_docs
/// * `peer_conn` - Peer connection to send response
/// * `repo_ctx` - Database repository context
/// * `crypto_utils` - Crypto utilities for UCAN parsing
///
/// # Returns
/// * `Ok(())` - Response sent (either ResourceNotFoundRequest or UpdatesResponse)
/// * `Err` - If UCAN parsing, database lookup, or update generation fails
pub async fn handle_resource_sync_request(
    payload: ResourceSyncRequestMsg,
    peer_conn: Arc<PeerConnection>,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: Arc<RwLock<CryptoUtils>>,
) -> P2PResult<()> {
    info!("Received resource sync request from peer (NEW PROTOCOL: with state_vectors and full_docs)");

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
        info!(
            "Resource {} not found locally, requesting from peer",
            resource_id
        );

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
            .send_message(Message::Resource(ResourceMessage::ResourceNotFoundRequest(
                request,
            )))
            .await?;

        info!(
            "✓ Sent ResourceNotFoundRequest for resource {}",
            resource_id
        );
    } else {
        info!(
            "Resource {} found locally, processing sync request",
            resource_id
        );

        // PHASE 5.1: Apply full_docs if present (viewer submissions)
        if !payload.full_docs.is_empty() && payload.full_docs != "{}" {
            info!("Processing full_docs (viewer submissions)");

            // Parse full_docs JSON
            let full_docs: std::collections::HashMap<String, Vec<u8>> =
                serde_json::from_str(&payload.full_docs).map_err(|e| {
                    error!("Failed to parse full_docs JSON: {}", e);
                    crate::p2p::errors::P2PError::InvalidState(format!(
                        "Invalid full_docs JSON: {}",
                        e
                    ))
                })?;

            // Apply each full document (e.g., submissions_doc)
            for (doc_name, doc_bytes) in full_docs {
                if doc_name == "submissions_doc" {
                    // Get viewer identifier from UCAN audience (source of truth)
                    let viewer_identifier =
                        services::ucan_service::extract_audience(&payload.resource_ucan)
                            .await
                            .map_err(|e| {
                                error!("Failed to extract audience from UCAN: {}", e);
                                crate::p2p::errors::P2PError::InvalidState(format!(
                                    "Invalid UCAN: {}",
                                    e
                                ))
                            })?;

                    info!("Applying submission from viewer: {}", viewer_identifier);

                    // Get local user for saving
                    let local_user = peer_conn.get_local_user().await?;

                    // Load resource, apply submission, save
                    let mut resource = services::get_resource_by_id_direct(
                        &resource_id,
                        repo_ctx.clone(),
                        &crypto_utils,
                    )
                    .await
                    .map_err(|e| {
                        error!("Failed to load resource: {}", e);
                        crate::p2p::errors::P2PError::InvalidState(format!(
                            "Failed to load resource: {}",
                            e
                        ))
                    })?;

                    // Apply viewer submission with isolation
                    services::apply_submission(
                        &mut resource,
                        &viewer_identifier,
                        &doc_bytes,
                        &payload.resource_ucan,
                    )
                    .await
                    .map_err(|e| {
                        error!("Failed to apply submission: {}", e);
                        crate::p2p::errors::P2PError::InvalidState(format!(
                            "Failed to apply submission: {}",
                            e
                        ))
                    })?;

                    // Save updated resource
                    let updated_json = resource.to_json().map_err(|e| {
                        error!("Failed to serialize resource: {}", e);
                        crate::p2p::errors::P2PError::InvalidState(format!(
                            "Failed to serialize resource: {}",
                            e
                        ))
                    })?;

                    services::update_resource(&resource_id, updated_json, &local_user, repo_ctx.clone())
                        .await
                        .map_err(|e| {
                            error!("Failed to save resource: {}", e);
                            crate::p2p::errors::P2PError::InvalidState(format!(
                                "Failed to save resource: {}",
                                e
                            ))
                        })?;

                    info!("✓ Applied viewer submission from user {}", viewer_identifier);
                }
            }
        }

        // Generate updates for peer based on their state vectors
        let (our_updates, our_state_vectors) = services::generate_updates_for_peer(
            &payload.resource_ucan,
            &payload.state_vectors,
            repo_ctx.clone(),
            &crypto_utils,
        )
        .await
        .map_err(|e| {
            error!(
                "Failed to generate updates for resource {}: {}",
                resource_id, e
            );
            crate::p2p::errors::P2PError::InvalidState(format!(
                "Failed to generate updates: {}",
                e
            ))
        })?;

        info!("✓ Generated updates for peer");

        // Load our resource to get our UCAN token
        let our_resource = services::get_resource_by_id_direct(
            &resource_id,
            repo_ctx.clone(),
            &crypto_utils,
        )
        .await
        .map_err(|e| {
            error!("Failed to load resource for Round 1 response: {}", e);
            crate::p2p::errors::P2PError::InvalidState(format!("Failed to load resource: {}", e))
        })?;

        // PHASE 5.2: Compare asset IDs to determine missing assets
        let missing_asset_ids: Vec<String> = {
            // Parse peer's state_vectors to extract asset_ids
            let peer_state_vectors: std::collections::HashMap<String, serde_json::Value> =
                serde_json::from_str(&payload.state_vectors).map_err(|e| {
                    error!("Failed to parse state_vectors JSON: {}", e);
                    crate::p2p::errors::P2PError::InvalidState(format!(
                        "Invalid state_vectors JSON: {}",
                        e
                    ))
                })?;

            // Extract peer's asset_ids from static_assets entry
            let peer_asset_ids: Vec<String> = peer_state_vectors
                .get("static_assets")
                .and_then(|v| v.get("asset_ids"))
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();

            if !peer_asset_ids.is_empty() {
                info!("Peer has {} assets, comparing...", peer_asset_ids.len());

                // Load resource to get our asset_ids
                let resource = services::get_resource_by_id_direct(
                    &resource_id,
                    repo_ctx.clone(),
                    &crypto_utils,
                )
                .await
                .map_err(|e| {
                    error!("Failed to load resource for asset comparison: {}", e);
                    crate::p2p::errors::P2PError::InvalidState(format!(
                        "Failed to load resource: {}",
                        e
                    ))
                })?;

                // Extract our asset_ids
                let our_asset_ids = services::extract_asset_ids(&resource)
                    .await
                    .map_err(|e| {
                        error!("Failed to extract asset IDs: {}", e);
                        crate::p2p::errors::P2PError::InvalidState(format!(
                            "Failed to extract asset IDs: {}",
                            e
                        ))
                    })?;

                info!("We have {} assets", our_asset_ids.len());

                // Compare using set operations
                let (missing_on_peer, _missing_on_us) =
                    services::compare_asset_ids(&our_asset_ids, &peer_asset_ids);

                if !missing_on_peer.is_empty() {
                    info!("Peer is missing {} assets", missing_on_peer.len());
                }

                missing_on_peer
            } else {
                vec![]
            }
        };

        info!("✓ Prepared response, sending UpdatesResponse (Round 1)");

        // Send UpdatesResponse (first round)
        let updates_response = ResourceUpdateMsg::UpdatesResponse {
            resource_id,
            updates: our_updates,
            state_vectors: our_state_vectors,
            missing_asset_ids,
            ucan_token: our_resource.ucan_token.clone(),
        };

        peer_conn
            .send_message(Message::Resource(ResourceMessage::MergeUpdate(
                updates_response,
            )))
            .await?;

        info!("✓ Sent UpdatesResponse (Round 1) to peer");
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
    info!(
        "Received resource not found request for resource {}",
        payload.resource_id
    );

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
        .send_message(Message::Resource(ResourceMessage::ResourceTransfer(
            transfer_msg,
        )))
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
    info!(
        "Received resource transfer for resource {}",
        payload.resource.id
    );

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
        .send_message(Message::Resource(ResourceMessage::ResourceTransferAck))
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
pub async fn handle_resource_transfer_ack(peer_conn: Arc<PeerConnection>) -> P2PResult<()> {
    info!(
        "Received resource transfer acknowledgment from peer {}",
        peer_conn.get_id()
    );
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
        .send_message(Message::Resource(ResourceMessage::MergeUpdate(
            updates_response,
        )))
        .await?;

    info!("✓ Sent UpdatesResponse to peer");
    Ok(())
}

/// Handle UpdatesResponse from peer - Round 1 or Round 2
///
/// NEW PROTOCOL: Always 2 rounds for proper convergence.
///
/// Flow:
/// - Receives peer's updates and state vectors
/// - Applies peer's updates to local resource
/// - Generates our updates based on peer's new state vectors
/// - Saves updated resource
/// - ALWAYS sends Round 2: UpdatesResponse with our updates back to peer
/// - If peer needs assets, sends AssetTransfer messages
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
/// * `Ok(())` - Updates applied successfully and Round 2 sent
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

    // Apply peer's updates to our resource (this also saves the resource)
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

    // Check if we should send Round 2 response based on our role
    // Protocol:
    // - Round 1: Initiator sends ResourceSyncRequest → Responder sends UpdatesResponse
    // - Round 2: Initiator sends UpdatesResponse → Responder receives and STOPS
    // So only the INITIATOR should send Round 2 response when receiving UpdatesResponse
    if peer_conn.is_initiator {
        info!("We are initiator - sending Round 2 UpdatesResponse");

        // Generate our updates based on peer's current state vectors
        let (our_round2_updates, our_state_vectors) = services::generate_updates_for_peer(
            &peer_ucan,
            &peer_state_vectors,
            repo_ctx.clone(),
            &crypto_utils,
        )
        .await
        .map_err(|e| {
            error!("Failed to generate Round 2 updates: {}", e);
            crate::p2p::errors::P2PError::InvalidState(format!("Failed to generate Round 2 updates: {}", e))
        })?;

        info!("✓ Generated Round 2 updates for peer");

        // Check if peer's UCAN has no_update_from_node restriction
        // Extract facts from peer UCAN to filter out docs peer shouldn't receive
        let peer_facts = services::ucan_service::extract_facts(&peer_ucan)
            .await
            .map_err(|e| {
                error!("Failed to extract facts from peer UCAN: {}", e);
                crate::p2p::errors::P2PError::InvalidState(format!("Failed to extract UCAN facts: {}", e))
            })?
            .unwrap_or_default();

        let no_update_from_node: Vec<String> = peer_facts
            .get("no_update_from_node")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
            .unwrap_or_default();

        if !no_update_from_node.is_empty() {
            info!("Peer has no_update_from_node filter: {:?}", no_update_from_node);
            // Note: generate_updates_for_peer already filters based on no_update_from_node
            // This is just for logging/debugging
        }

        // Load our resource to get our UCAN token
        let our_resource = services::get_resource_by_id_direct(
            &resource_id,
            repo_ctx.clone(),
            &crypto_utils,
        )
        .await
        .map_err(|e| {
            error!("Failed to load resource for Round 2: {}", e);
            crate::p2p::errors::P2PError::InvalidState(format!("Failed to load resource: {}", e))
        })?;

        // Send Round 2 UpdatesResponse
        let round2_response = ResourceUpdateMsg::UpdatesResponse {
            resource_id: resource_id.clone(),
            updates: our_round2_updates,
            state_vectors: our_state_vectors,
            missing_asset_ids: vec![], // Asset transfer handled separately
            ucan_token: our_resource.ucan_token.clone(),
        };

        peer_conn
            .send_message(Message::Resource(ResourceMessage::MergeUpdate(
                round2_response,
            )))
            .await?;

        info!("✓ Sent Round 2 UpdatesResponse to peer");
    } else {
        info!("We are responder - NOT sending Round 2 (protocol complete)");
    }

    // PHASE 5.3: If peer needs assets, send AssetTransfer messages
    if !missing_asset_ids.is_empty() {
        info!(
            "Peer needs {} assets - sending AssetTransfer messages",
            missing_asset_ids.len()
        );

        for asset_id in missing_asset_ids {
            info!("Preparing to send asset: {}", asset_id);

            // Load asset binary data from storage
            let asset_data = services::get_asset_binary_data(
                &resource_id,
                &asset_id,
                repo_ctx.clone(),
            )
            .await
            .map_err(|e| {
                error!("Failed to load asset {} binary data: {}", asset_id, e);
                crate::p2p::errors::P2PError::InvalidState(format!(
                    "Failed to load asset: {}",
                    e
                ))
            })?;

            // Load resource to get asset metadata from static_assets doc
            let resource = services::get_resource_by_id_direct(
                &resource_id,
                repo_ctx.clone(),
                &crypto_utils,
            )
            .await
            .map_err(|e| {
                error!("Failed to load resource for asset metadata: {}", e);
                crate::p2p::errors::P2PError::InvalidState(format!(
                    "Failed to load resource: {}",
                    e
                ))
            })?;

            // TODO: Extract metadata from static_assets (when assets are implemented)
            // Should call merge_service function, not Loro operations directly
            let metadata_json = serde_json::json!({
                "mime_type": "application/octet-stream",
                "size": asset_data.len(),
                "filename": format!("{}.bin", asset_id),
            })
            .to_string();

            // Send AssetTransfer message
            let transfer_msg = osvauld_core::models::AssetTransferMsg {
                resource_id: resource_id.clone(),
                asset_id: asset_id.clone(),
                asset_data,
                metadata: metadata_json,
            };

            peer_conn
                .send_message(osvauld_core::models::Message::Resource(
                    osvauld_core::models::ResourceMessage::AssetTransfer(transfer_msg),
                ))
                .await?;

            info!("✓ Sent AssetTransfer for: {}", asset_id);
        }

        info!("✓ All asset transfers sent");
    }

    info!("✓ Sync Round 2 complete for resource {}", resource_id);
    Ok(())
}

/// Handle asset transfer from peer
///
/// Flow:
/// - Receives AssetTransferMsg with resource_id, asset_id, asset_data, and metadata
/// - Validates resource_id exists locally
/// - Saves asset data to local storage
/// - Updates static_assets document with new asset reference
///
/// # Arguments
/// * `payload` - AssetTransferMsg containing asset data and metadata
/// * `peer_conn` - Peer connection (for logging)
/// * `repo_ctx` - Database repository context
/// * `crypto_utils` - Crypto utilities
///
/// # Returns
/// * `Ok(())` - Asset saved successfully
/// * `Err` - If validation or save fails
pub async fn handle_asset_transfer(
    payload: AssetTransferMsg,
    peer_conn: Arc<PeerConnection>,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: Arc<RwLock<CryptoUtils>>,
) -> P2PResult<()> {
    info!(
        "Received asset transfer for resource {}, asset {}",
        payload.resource_id, payload.asset_id
    );

    // Validate resource exists locally
    let resource_exists = repo_ctx
        .resource_repo
        .find_by_id(&payload.resource_id)
        .await
        .is_ok();

    if !resource_exists {
        error!(
            "Cannot save asset - resource {} not found locally",
            payload.resource_id
        );
        return Err(crate::p2p::errors::P2PError::InvalidState(format!(
            "Resource {} not found",
            payload.resource_id
        )));
    }

    info!(
        "✓ Resource {} exists, saving asset (size: {} bytes)",
        payload.resource_id,
        payload.asset_data.len()
    );

    // Get local user for saving
    let local_user = peer_conn.get_local_user().await?;

    // Call service layer to save asset
    services::save_asset_binary_data(
        &payload.resource_id,
        &payload.asset_id,
        &payload.asset_data,
        &payload.metadata,
        &local_user,
        repo_ctx,
        &crypto_utils,
    )
    .await
    .map_err(|e| {
        error!("Failed to save asset: {}", e);
        crate::p2p::errors::P2PError::InvalidState(format!("Failed to save asset: {}", e))
    })?;

    info!(
        "✓ Asset transfer complete - saved {} ({} bytes)",
        payload.asset_id,
        payload.asset_data.len()
    );
    Ok(())
}
