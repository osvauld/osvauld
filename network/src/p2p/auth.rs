//! Unified Handshake Authentication Handlers
//!
//! Single entry point for processing all handshake messages.
//! Routes to appropriate handlers based on message type.

use crate::p2p::{errors::P2PResult, PeerConnection};
use osvauld_core::models::{
    Device, FirstConnectionComplete, FirstConnectionResponse, HandshakeMessage, HandshakeRequest,
    Message, ReconnectionResponse, User,
};
use services::{HandshakeType, ParsedHandshakeToken};
use std::sync::Arc;
use tracing::{error, info};

/// Initiate handshake with peer
///
/// Called automatically after P2P connection is established.
/// Builds and sends HandshakeRequest with local user's UCAN token.
///
/// # Arguments
/// * `conn` - Peer connection
/// * `ucan_token` - UCAN token to send (OneTime or persistent connection token)
/// * `local_user` - Local user information
/// * `local_device` - Local device information
pub async fn initiate_handshake(
    conn: &PeerConnection,
    ucan_token: String,
    local_user: User,
    local_device: Device,
) -> P2PResult<()> {
    info!("🤝 Initiating handshake");
    info!("  - Local user: {}", local_user.id);
    info!("  - Local device: {}", local_device.id);

    // Sign local UCAN public key
    let signed_ucan_pub = {
        let crypto = conn.crypto_utils.read().await;
        crypto.sign_message(&local_user.ucan_pub_key).map_err(|e| {
            error!("❌ Failed to sign UCAN public key: {}", e);
            crate::p2p::errors::P2PError::InvalidState(format!("Failed to sign: {}", e))
        })?
    };

    // Build HandshakeRequest
    let request = HandshakeRequest {
        ucan_token,
        peer_user: local_user,
        peer_device: local_device,
        signed_ucan_pub,
    };

    // Send HandshakeRequest
    let message = Message::Handshake(osvauld_core::models::HandshakeMessage::HandshakeRequest(
        request,
    ));

    conn.send_message(message).await?;

    info!("✓ Sent HandshakeRequest");
    Ok(())
}

/// Process incoming handshake message (MAIN ORCHESTRATOR)
///
/// Single entry point for all handshake messages.
/// Routes to appropriate handler based on message variant.
///
/// # Arguments
/// * `conn` - Peer connection
/// * `handshake_msg` - HandshakeMessage enum containing the specific message type
pub async fn process_handshake_message(
    conn: &PeerConnection,
    handshake_msg: &HandshakeMessage,
) -> P2PResult<()> {
    match handshake_msg {
        HandshakeMessage::HandshakeRequest(request) => {
            process_handshake_request(conn, request).await
        }
        HandshakeMessage::FirstConnectionResponse(response) => {
            process_first_connection_response(conn, response).await
        }
        HandshakeMessage::FirstConnectionComplete(complete) => {
            process_first_connection_complete(conn, complete).await
        }
        HandshakeMessage::ReconnectionResponse(response) => {
            process_reconnection_response(conn, response).await
        }
    }
}

/// Process incoming handshake request
///
/// Main entry point called when HandshakeRequest is received.
/// Parses token to determine handshake type, then routes to appropriate handler.
///
/// # Flow
/// 1. Parse and validate UCAN token using auth_service
/// 2. Match on handshake type to route to handler:
///    - PeerFirstConnection -> handle_peer_first_connection
///    - Reconnection -> handle_reconnection
///    - ViewerFirstConnection -> TODO (implement later)
///
/// # Arguments
/// * `conn` - Peer connection with the requesting peer
/// * `request` - HandshakeRequest containing ucan_token, user, device, signed_ucan_pub
pub async fn process_handshake_request(
    conn: &PeerConnection,
    request: &HandshakeRequest,
) -> P2PResult<()> {
    info!("🔐 Processing handshake request");
    info!("  - Peer user: {}", request.peer_user.id);
    info!("  - Peer device: {}", request.peer_device.id);

    // 1. Parse and validate token
    let parsed = services::parse_and_validate_handshake_token(
        &request.ucan_token,
        &request.signed_ucan_pub,
        &request.peer_user.ucan_pub_key,
    )
    .await
    .map_err(|e| {
        error!("❌ Failed to parse/validate handshake token: {}", e);
        crate::p2p::errors::P2PError::InvalidState(format!("Invalid token: {}", e))
    })?;

    info!(
        "✓ Token parsed - Type: {:?}, Role: {:?}",
        parsed.handshake_type, parsed.role
    );

    // 2. Route to appropriate handler based on handshake type
    match parsed.handshake_type {
        HandshakeType::PeerFirstConnection => {
            handle_peer_first_connection(conn, request, parsed).await
        }
        HandshakeType::Reconnection => handle_reconnection(conn, request, parsed).await,
        HandshakeType::ViewerFirstConnection => {
            // TODO: Implement viewer handshake later
            error!("❌ Viewer handshake not yet implemented");
            Err(crate::p2p::errors::P2PError::InvalidState(
                "Viewer handshake not implemented".to_string(),
            ))
        }
    }
}

/// Handle peer first connection handshake (owner/node/user)
///
/// Called when a peer connects for the first time with a OneTimeConnection token.
///
/// # Flow (THREE-WAY HANDSHAKE - STEP 2)
/// 1. Issue persistent token for peer BEFORE saving
/// 2. Create peer User record with NEW persistent token (NOT one-time token)
/// 3. Save peer user and device to database
/// 4. Send FirstConnectionResponse with issued token
async fn handle_peer_first_connection(
    conn: &PeerConnection,
    request: &HandshakeRequest,
    parsed: ParsedHandshakeToken,
) -> P2PResult<()> {
    info!("🔐 Handling peer first connection (3-way handshake - Step 2)");
    info!("  - Peer user: {}", request.peer_user.id);
    info!("  - Role: {:?}", parsed.role);

    // 1. Convert Role enum to string for token generation
    // Role comes from the parsed OneTimeConnection token
    let role_str = match parsed.role {
        osvauld_core::models::Role::Owner => "owner",
        osvauld_core::models::Role::Node => "node",
        osvauld_core::models::Role::User => "user",
        osvauld_core::models::Role::Viewer => "viewer",
    };

    // 2. Issue persistent connection token BEFORE saving user
    // This ensures we never store the one-time token in the database
    let issued_ucan = services::ucan_service::issue_peer_connection(
        &conn.domain,
        &request.peer_user.ucan_pub_key,
        role_str,
        &conn.crypto_utils,
        &conn.repo_ctx,
    )
    .await
    .map_err(|e| {
        error!("❌ Failed to issue persistent connection token: {}", e);
        crate::p2p::errors::P2PError::InvalidState(format!("Failed to issue token: {}", e))
    })?;

    info!("✓ Issued {} connection token for peer", role_str);

    // 3. Save peer user with the one-time token from request (temporary)
    // This will be updated in step 4 when we receive FirstConnectionComplete
    // with the token THEY issue FOR us
    services::save_first_connection_user(
        &request.peer_user,
        &[request.peer_device.clone()],
        &conn.repo_ctx,
    )
    .await
    .map_err(|e| {
        error!("❌ Failed to save peer user: {}", e);
        crate::p2p::errors::P2PError::InvalidState(format!("Failed to save user: {}", e))
    })?;

    info!("✓ Saved peer user (token will be updated in step 4)");

    // 4. Send FirstConnectionResponse with issued token
    send_first_connection_response(conn, issued_ucan).await?;

    info!("✅ Peer first connection handshake Step 2 complete");
    Ok(())
}

/// Handle reconnection handshake (all roles)
///
/// Called when a user reconnects with a persistent connection token.
///
/// # Flow (TWO-WAY HANDSHAKE - STEP 2)
/// 1. Verify peer user exists in database
/// 2. Send ReconnectionResponse (no issued token - both sides already have persistent tokens)
async fn handle_reconnection(
    conn: &PeerConnection,
    request: &HandshakeRequest,
    _parsed: ParsedHandshakeToken,
) -> P2PResult<()> {
    info!("🔐 Handling reconnection (2-way handshake - Step 2)");
    info!("  - Peer user: {}", request.peer_user.id);

    // 1. Verify user exists in database
    let user_exists = services::user_exists(&request.peer_user.id, &conn.repo_ctx)
        .await
        .map_err(|e| {
            error!("❌ Failed to check user existence: {}", e);
            crate::p2p::errors::P2PError::InvalidState(format!("Failed to verify user: {}", e))
        })?;

    if !user_exists {
        error!("❌ User {} not found in database", request.peer_user.id);
        return Err(crate::p2p::errors::P2PError::InvalidState(
            "User not found - cannot reconnect".to_string(),
        ));
    }

    info!("✓ User exists, reconnection authorized");

    // 2. Update peer user in connection state with their UCAN token
    let mut peer_user = request.peer_user.clone();
    peer_user.ucan_token = request.ucan_token.clone();

    let mut user_guard = conn.user.write().await;
    *user_guard = peer_user;
    drop(user_guard);

    info!("✓ Updated peer user with connection UCAN token");

    // 3. Send ReconnectionResponse (no token exchange needed)
    send_reconnection_response(conn).await?;

    info!("✅ Reconnection handshake Step 2 complete");
    Ok(())
}

/// Process FirstConnectionResponse (THREE-WAY HANDSHAKE - STEP 3)
///
/// Called when initiator receives FirstConnectionResponse with issued token from responder.
///
/// # Flow
/// 1. Save peer's issued token to database (replace one-time token)
/// 2. Update peer user and device info in connection state
/// 3. Issue persistent token for responder
/// 4. Send FirstConnectionComplete with our issued token
async fn process_first_connection_response(
    conn: &PeerConnection,
    response: &FirstConnectionResponse,
) -> P2PResult<()> {
    info!("🔐 Processing FirstConnectionResponse (3-way handshake - Step 3)");
    info!("  - Peer user: {}", response.peer_user.id);
    info!("  - Peer device: {}", response.peer_device.id);

    // 1. Save peer user with the token THEY issued FOR us
    // When we connect TO them, we'll present this token
    // When they connect TO us, they'll present the token we issue below
    let mut peer_user_with_their_token = response.peer_user.clone();
    peer_user_with_their_token.ucan_token = response.issued_ucan.clone();

    services::save_first_connection_user(
        &peer_user_with_their_token,
        &response.devices,
        &conn.repo_ctx,
    )
    .await
    .map_err(|e| {
        error!("❌ Failed to save peer user: {}", e);
        crate::p2p::errors::P2PError::InvalidState(format!("Failed to save user: {}", e))
    })?;

    info!("✓ Saved peer user with token they issued for us");

    // 2. Update peer user and device info in connection state
    conn.set_peer_user_and_device(response.peer_user.clone(), response.peer_device.clone())
        .await;

    // 3. Determine role for the peer (inverse of our role typically)
    let local_user = conn.user.read().await.clone();
    let role_str = if local_user.owner {
        "node" // If we're owner, peer is node
    } else {
        "owner" // If we're node, peer is owner
    };

    // 4. Issue NEW persistent connection token FOR the peer (node)
    // They will save this and present it when connecting TO us
    let issued_ucan = services::ucan_service::issue_peer_connection(
        &conn.domain,
        &response.peer_user.ucan_pub_key,
        role_str,
        &conn.crypto_utils,
        &conn.repo_ctx,
    )
    .await
    .map_err(|e| {
        error!("❌ Failed to issue persistent connection token: {}", e);
        crate::p2p::errors::P2PError::InvalidState(format!("Failed to issue token: {}", e))
    })?;

    info!("✓ Issued {} connection token for peer", role_str);

    // 5. Get local user ID to send with FirstConnectionComplete
    let local_user = conn.get_local_user().await?;

    // 6. Send FirstConnectionComplete with the NEW token we issued FOR node
    send_first_connection_complete(conn, issued_ucan, local_user.id).await?;

    info!("✅ FirstConnectionResponse processed, Step 3 complete");
    Ok(())
}

/// Process FirstConnectionComplete (THREE-WAY HANDSHAKE - FINAL STEP)
///
/// Called when responder receives FirstConnectionComplete with issued token from initiator.
///
/// # Flow
/// 1. Save initiator's issued token to database
/// 2. Mark handshake as complete
async fn process_first_connection_complete(
    conn: &PeerConnection,
    complete: &FirstConnectionComplete,
) -> P2PResult<()> {
    info!("🔐 Processing FirstConnectionComplete (3-way handshake - Final)");
    info!("  - Peer user: {}", complete.peer_user_id);

    // 1. Update the peer user's token (the initiator who sent us this token)
    services::update_user_token(
        &complete.peer_user_id,
        &complete.issued_ucan,
        &conn.repo_ctx,
    )
    .await
    .map_err(|e| {
        error!("❌ Failed to update peer's token: {}", e);
        crate::p2p::errors::P2PError::InvalidState(format!("Failed to update token: {}", e))
    })?;

    info!(
        "✓ Saved persistent token from peer {}",
        complete.peer_user_id
    );
    info!("✅ Three-way handshake COMPLETE - both sides have persistent tokens");
    Ok(())
}

/// Process ReconnectionResponse (TWO-WAY HANDSHAKE - FINAL STEP)
///
/// Called when initiator receives ReconnectionResponse.
///
/// # Flow
/// 1. Update peer user and device info in connection state
/// 2. Mark handshake as complete (no token exchange needed)
async fn process_reconnection_response(
    conn: &PeerConnection,
    response: &ReconnectionResponse,
) -> P2PResult<()> {
    info!("🔐 Processing ReconnectionResponse (2-way handshake - Final)");
    info!("  - Peer user: {}", response.peer_user.id);
    info!("  - Peer device: {}", response.peer_device.id);

    // Update peer user and device info in connection state
    conn.set_peer_user_and_device(response.peer_user.clone(), response.peer_device.clone())
        .await;

    info!("✅ Reconnection handshake COMPLETE");
    Ok(())
}

// ==================== Helper Functions ====================

/// Helper: Send FirstConnectionResponse to peer
async fn send_first_connection_response(
    conn: &PeerConnection,
    issued_ucan: String,
) -> P2PResult<()> {
    let local_user = conn.user.read().await.clone();
    let local_device = conn.device.read().await.clone();

    let local_devices = conn
        .repo_ctx
        .device_repo
        .get_devices_by_user_id(&local_user.id)
        .await
        .map_err(|e| {
            error!("❌ Failed to get local devices: {}", e);
            crate::p2p::errors::P2PError::InvalidState(format!("Failed to get devices: {}", e))
        })?;

    let signed_ucan_pub = {
        let crypto = conn.crypto_utils.read().await;
        crypto.sign_message(&local_user.ucan_pub_key).map_err(|e| {
            error!("❌ Failed to sign UCAN public key: {}", e);
            crate::p2p::errors::P2PError::InvalidState(format!("Failed to sign: {}", e))
        })?
    };

    let response = FirstConnectionResponse {
        issued_ucan,
        peer_user: local_user,
        peer_device: local_device,
        devices: local_devices,
        signed_ucan_pub,
    };

    let message = Message::Handshake(HandshakeMessage::FirstConnectionResponse(response));

    conn.send_message(message).await?;
    info!("✓ Sent FirstConnectionResponse");
    Ok(())
}

/// Helper: Send FirstConnectionComplete to peer
async fn send_first_connection_complete(
    conn: &PeerConnection,
    issued_ucan: String,
    peer_user_id: String,
) -> P2PResult<()> {
    let complete = FirstConnectionComplete {
        issued_ucan,
        peer_user_id,
    };

    let message = Message::Handshake(HandshakeMessage::FirstConnectionComplete(complete));

    conn.send_message(message).await?;
    info!("✓ Sent FirstConnectionComplete");
    Ok(())
}

/// Helper: Send ReconnectionResponse to peer
async fn send_reconnection_response(conn: &PeerConnection) -> P2PResult<()> {
    let local_user = conn.user.read().await.clone();
    let local_device = conn.device.read().await.clone();

    let local_devices = conn
        .repo_ctx
        .device_repo
        .get_devices_by_user_id(&local_user.id)
        .await
        .map_err(|e| {
            error!("❌ Failed to get local devices: {}", e);
            crate::p2p::errors::P2PError::InvalidState(format!("Failed to get devices: {}", e))
        })?;

    let signed_ucan_pub = {
        let crypto = conn.crypto_utils.read().await;
        crypto.sign_message(&local_user.ucan_pub_key).map_err(|e| {
            error!("❌ Failed to sign UCAN public key: {}", e);
            crate::p2p::errors::P2PError::InvalidState(format!("Failed to sign: {}", e))
        })?
    };

    let response = ReconnectionResponse {
        peer_user: local_user,
        peer_device: local_device,
        devices: local_devices,
        signed_ucan_pub,
    };

    let message = Message::Handshake(HandshakeMessage::ReconnectionResponse(response));

    conn.send_message(message).await?;
    info!("✓ Sent ReconnectionResponse");
    Ok(())
}
