//! Website Viewer Connection Handler
//!
//! Central router for all Website message types.
//! Delegates to specific handlers based on message variant.

use crate::p2p::{errors::P2PResult, PeerConnection};
use osvauld_core::models::{WebsiteMessage, WebsiteRequest};
use std::sync::Arc;
use tracing::{error, info};

/// Initiate website connection request from viewer to node
///
/// Called by sync_handler after P2P connection is established.
/// Validates folder existence and first_sync status before sending WebsiteRequest.
///
/// # Arguments
/// * `peer_conn` - The established peer connection with the node
/// * `ucan_token` - UCAN token for folder access (from connection string)
/// * `device_id` - Node's device ID (to look up node user)
/// * `p2p_service` - P2P service for accessing viewer user/device and repo
///
/// # Returns
/// * `Ok(())` - Request sent successfully
/// * `Err` - If validation fails or failed to send request
pub async fn initiate_website_request(
    peer_conn: Arc<PeerConnection>,
    ucan_token: String,
    device_id: String,
    p2p_service: Arc<crate::p2p::P2PService>,
) -> P2PResult<()> {
    info!("🌐 Initiating WebsiteRequest - validating connection");

    // 1. Get viewer's local user and device from P2PService
    let viewer_user = p2p_service.current_user.read().await.clone()
        .ok_or_else(|| crate::p2p::errors::P2PError::InvalidState("No viewer user found".to_string()))?;

    let viewer_device = p2p_service.current_device.read().await.clone()
        .ok_or_else(|| crate::p2p::errors::P2PError::InvalidState("No viewer device found".to_string()))?;

    info!(
        "Viewer: {} (device: {}, first_sync: {})",
        viewer_user.username, viewer_device.id, viewer_user.first_sync
    );

    // 2. Get node device to find user_id
    let node_device = p2p_service.repo_ctx.device_repo.find_by_id(&device_id).await
        .map_err(|e| crate::p2p::errors::P2PError::InvalidState(format!("Failed to get node device: {}", e)))?;

    let node_user_id = &node_device.user_id;

    // 3. Validate folder existence and first_sync status via website_service
    let domain = p2p_service.domain.as_str();
    let (first_sync_done, folder_exists) = services::check_user_and_folder_status(
        &ucan_token,
        node_user_id,
        domain,
        p2p_service.repo_ctx.clone(),
    )
    .await
    .map_err(|e| {
        error!("❌ Failed to validate folder/user status: {}", e);
        crate::p2p::errors::P2PError::InvalidState(format!("Validation failed: {}", e))
    })?;

    info!(
        "✓ Validation complete: first_sync={}, folder_exists={}",
        first_sync_done, folder_exists
    );

    // 4. TODO: Handle different flows based on status
    // - If !first_sync_done: Need to handle first connection flow
    // - If !folder_exists: Folder not found - error or different flow?
    // - If both true: Proceed with sending request

    if !first_sync_done {
        error!("⚠️ Node first_sync not done yet");
        return Err(crate::p2p::errors::P2PError::InvalidState(
            "Node has not completed first sync".to_string()
        ));
    }

    if !folder_exists {
        error!("⚠️ Folder does not exist on node");
        return Err(crate::p2p::errors::P2PError::InvalidState(
            "Folder not found on node".to_string()
        ));
    }

    // 5. Create and send WebsiteRequest
    let request = WebsiteRequest {
        ucan_token,
        viewer_user: viewer_user.clone(),
        viewer_device: viewer_device.clone(),
        first_sync: viewer_user.first_sync,
    };

    info!(
        "📤 Sending WebsiteRequest to node (viewer first_sync={})",
        request.first_sync
    );

    peer_conn
        .send_message(osvauld_core::models::Message::Website(
            WebsiteMessage::WebsiteRequest(request),
        ))
        .await?;

    info!("✅ WebsiteRequest sent successfully");
    Ok(())
}

/// Process all Website messages - central routing function
///
/// This function receives all Website message variants and delegates
/// to the appropriate handler based on message type.
///
/// # Arguments
/// * `peer_conn` - The peer connection
/// * `message` - The WebsiteMessage variant to process
///
/// # Returns
/// * `Ok(())` - Message processed successfully
/// * `Err` - If processing fails
pub async fn process_message(
    peer_conn: Arc<PeerConnection>,
    message: WebsiteMessage,
) -> P2PResult<()> {
    match message {
        WebsiteMessage::WebsiteRequest(payload) => {
            process_website_request(peer_conn, payload).await
        }
        // Future: Add other variants here
        // WebsiteMessage::WebsiteResponse(payload) => { ... }
        // WebsiteMessage::WebsiteReconnectRequest(payload) => { ... }
    }
}

/// Process incoming WebsiteRequest from viewer
///
/// This function is called when a viewer sends a WebsiteRequest message.
/// It validates the viewer's connection by:
/// 1. Getting the node's local user/device from peer connection
/// 2. Extracting repo_ctx and domain from peer connection
/// 3. Calling website_service to check first_sync and folder existence
/// 4. Processing based on the status flags
///
/// # Arguments
/// * `peer_conn` - The peer connection with the viewer
/// * `request` - WebsiteRequest payload containing ucan_token, viewer_user, viewer_device, first_sync
///
/// # Returns
/// * `Ok(())` - Request processed successfully
/// * `Err` - If validation fails or status check fails
async fn process_website_request(
    peer_conn: Arc<PeerConnection>,
    request: WebsiteRequest,
) -> P2PResult<()> {
    info!(
        "Processing WebsiteRequest from viewer: {} (first_sync={})",
        request.viewer_user.username, request.first_sync
    );

    // 1. Get local node user and device from peer connection
    let node_user = peer_conn.get_local_user().await?;
    let node_device = peer_conn
        .get_local_device()
        .await
        .ok_or_else(|| {
            crate::p2p::errors::P2PError::InvalidState("No local device found".to_string())
        })?;

    info!(
        "Node user: {}, device: {}",
        node_user.username, node_device.id
    );

    // 2. Get repo_ctx and domain from peer connection
    let repo_ctx = peer_conn.repo_ctx.clone();
    let domain = peer_conn.domain.as_str();

    // 3. Call website_service to check status
    let (first_sync_done, folder_exists) = services::check_user_and_folder_status(
        &request.ucan_token,
        &node_user.id,
        domain,
        repo_ctx,
    )
    .await
    .map_err(|e| {
        error!("Failed to check user and folder status: {}", e);
        crate::p2p::errors::P2PError::InvalidState(format!(
            "Status check failed: {}",
            e
        ))
    })?;

    info!(
        "Status check complete: first_sync={}, folder_exists={}",
        first_sync_done, folder_exists
    );

    // 4. TODO: Process based on status flags
    // - If !first_sync_done: Return error or wait
    // - If !folder_exists: Different flow (to be discussed)
    // - If both true: Proceed with sending folder and resources

    Ok(())
}
