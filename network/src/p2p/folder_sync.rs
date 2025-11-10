//! Folder sync protocol - simple push after share_folder()
//!
//! This module handles folder sync operations.

use crate::p2p::{errors::P2PResult, peer_connection::PeerConnection, resource_sync};
use crypto_utils::CryptoUtils;
use osvauld_core::models::{FolderDataSync, Message, User};
use persistance::database::RepositoryContext;
use services::{get_folder_by_id, get_folder_share_record};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info};

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

    peer_conn.send_message(Message::FolderDataSync(data)).await
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

    // 1. Generate viewer token
    let viewer_token = services::ucan_service::generate_viewer_token_for_folder(
        &payload.folder_ucan,
        &payload.folder_id,
        &peer_conn.domain,
        peer_conn.repo_ctx.clone(),
        peer_conn.crypto_utils.clone(),
    )
    .await
    .map_err(|e| {
        error!("❌ Failed to generate viewer token: {}", e);
        crate::p2p::errors::P2PError::InvalidState(format!(
            "Failed to generate viewer token: {}",
            e
        ))
    })?;

    info!("✓ Generated viewer token");

    // 2. Get current user and device info
    let current_user = peer_conn.get_local_user().await?;
    let current_device = peer_conn.get_local_device().await
        .ok_or_else(|| crate::p2p::errors::P2PError::InvalidState("No device loaded".to_string()))?;

    // 3. Get UCAN public key
    let encrypted_ucan_key = peer_conn.repo_ctx.store_repo.get_ucan_key().await
        .map_err(|e| crate::p2p::errors::P2PError::InvalidState(format!("Failed to get UCAN key: {}", e)))?;

    let crypto = peer_conn.crypto_utils.read().await;
    let ucan_pub_key = crypto.get_public_ucan_key(&encrypted_ucan_key).await
        .map_err(|e| crate::p2p::errors::P2PError::InvalidState(format!("Failed to get UCAN public key: {}", e)))?;

    // 4. Create connection details JSON
    let connection_details = serde_json::json!({
        "user_public_key": current_user.public_key,
        "device_public_key": current_device.device_key,
        "username": current_user.username,
        "ucan_token": viewer_token,
        "ucan_pub_key": ucan_pub_key,
        "folder_id": payload.folder_id,
    });

    // 5. Base64 encode connection string
    use base64::{Engine as _, engine::general_purpose};
    let connection_json = connection_details.to_string();
    let connection_string = general_purpose::STANDARD.encode(connection_json.as_bytes());

    info!("✓ Generated connection string");

    // 6. Send response back to requester
    let response = osvauld_core::models::FolderTokenResponse {
        folder_id: payload.folder_id.clone(),
        connection_string,
    };

    peer_conn.send_message(osvauld_core::models::Message::FolderTokenResponse(response)).await?;

    info!("✅ Sent FolderTokenResponse for folder: {}", payload.folder_id);
    Ok(())
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
