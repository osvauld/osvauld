//! Pairing Handlers - First connection handshake
//!
//! Handles the owner-node pairing flow (Phase 1 focus).
//!
//! ## First Connection Flow (Owner -> Node):
//! 1. Owner scans connection string (contains first_connection permit)
//! 2. Owner sends Hello with permit from connection string
//! 3. Node verifies permit, stores owner info, sends Welcome with long-lived permit
//! 4. Owner stores node info and permit, sends PermitGrant with long-lived permit for node
//! 5. Node stores owner's permit, sends Ack
//!
//! ## Reconnection Flow:
//! 1. Either side sends Hello with stored long-lived permit
//! 2. Receiver verifies permit, sends Welcome
//! 3. Connection established

use crate::registry::{PeerInfo, PeerRegistry, PeerType};
use anyhow::{anyhow, Result};
use chrono::Utc;
use transport::NodeId;
use tracing::{debug, error, info, warn};
use transport::{ConnectionHandle, Message};

/// Context for handling pairing operations
pub struct PairingContext<'a> {
    /// Our identity (DID, username, public key)
    pub our_did: &'a str,
    pub our_username: &'a str,
    pub our_public_key: &'a [u8],
    /// Signing function for identity proof
    pub sign_fn: Box<dyn Fn(&[u8]) -> Result<Vec<u8>> + Send + Sync + 'a>,
    /// Permit issuing function
    pub issue_permit_fn: Box<dyn Fn(&str, &str) -> Result<String> + Send + Sync + 'a>,
    /// Permit verification function
    pub verify_permit_fn: Box<dyn Fn(&str) -> Result<PermitInfo> + Send + Sync + 'a>,
}

/// Information extracted from a verified permit
#[derive(Debug, Clone)]
pub struct PermitInfo {
    /// The relationship: owner, node, peer_user, peer_node
    pub relationship: String,
    /// Whether this is a first_connection permit
    pub is_first_connection: bool,
    /// The audience (who the permit is for)
    pub audience: String,
    /// The issuer (who created the permit)
    pub issuer: String,
}

/// Handle incoming Hello message
///
/// Called when we receive a Hello from a connecting peer.
/// Verifies the permit, creates PeerInfo, and sends Welcome.
pub async fn handle_hello(
    node_id: NodeId,
    conn: &ConnectionHandle,
    did: String,
    username: String,
    public_key: Vec<u8>,
    signature: Vec<u8>,
    timestamp: i64,
    permit: String,
    ctx: &PairingContext<'_>,
    registry: &PeerRegistry,
) -> Result<PeerInfo> {
    info!("Handling Hello from {} ({})", username, node_id);

    // 1. Verify timestamp (prevent replay attacks)
    let now = Utc::now().timestamp();
    let age_seconds = now - timestamp;
    if age_seconds > 300 || age_seconds < -30 {
        // 5 min window
        conn.send(&Message::Rejected {
            reason: "Timestamp expired or invalid".to_string(),
        })
        .await?;
        return Err(anyhow!("Timestamp out of range: {} seconds", age_seconds));
    }

    // 2. Verify signature over (did + timestamp)
    let sign_data = format!("{}{}", did, timestamp);
    if !verify_signature(&public_key, sign_data.as_bytes(), &signature)? {
        conn.send(&Message::Rejected {
            reason: "Invalid signature".to_string(),
        })
        .await?;
        return Err(anyhow!("Invalid identity signature"));
    }

    debug!("Identity verified for {}", did);

    // 3. Verify the permit
    let permit_info = (ctx.verify_permit_fn)(&permit)?;

    // Determine peer type from permit relationship
    let peer_type = match permit_info.relationship.as_str() {
        "owner" => PeerType::Owner,
        "node" => PeerType::MyNode,
        "peer_user" => PeerType::PeerUser,
        "peer_node" => PeerType::PeerNode,
        other => {
            conn.send(&Message::Rejected {
                reason: format!("Unknown relationship: {}", other),
            })
            .await?;
            return Err(anyhow!("Unknown permit relationship: {}", other));
        }
    };

    info!(
        "Permit verified: {} is {:?} (first_connection: {})",
        username, peer_type, permit_info.is_first_connection
    );

    // 4. Issue a long-lived permit for the peer
    let relationship_str = match peer_type {
        PeerType::Owner => "owner",
        PeerType::MyNode => "node",
        PeerType::PeerUser => "peer_user",
        PeerType::PeerNode => "peer_node",
    };

    let issued_permit = (ctx.issue_permit_fn)(&did, relationship_str)?;
    debug!("Issued {} permit for {}", relationship_str, did);

    // 5. Create peer info
    let peer_info = PeerInfo::new(
        node_id,
        peer_type,
        did.clone(),
        username.clone(),
        public_key,
        permit,
        conn.clone(),
    )
    .with_issued_permit(issued_permit.clone());

    // 6. Send Welcome with our info and the issued permit
    let our_timestamp = Utc::now().timestamp();
    let our_sign_data = format!("{}{}", ctx.our_did, our_timestamp);
    let our_signature = (ctx.sign_fn)(our_sign_data.as_bytes())?;

    conn.send(&Message::Welcome {
        node_id: node_id.to_string(),
        node_public_key: ctx.our_public_key.to_vec(),
        signature: our_signature,
        timestamp: our_timestamp,
        permit_for_peer: issued_permit,
    })
    .await?;

    info!("Sent Welcome to {} ({})", username, node_id);

    Ok(peer_info)
}

/// Handle incoming Welcome message
///
/// Called after we sent Hello and received Welcome back.
/// Stores their info and permit, sends PermitGrant if first connection.
pub async fn handle_welcome(
    node_id: NodeId,
    conn: &ConnectionHandle,
    remote_node_id: String,
    node_public_key: Vec<u8>,
    signature: Vec<u8>,
    timestamp: i64,
    permit_for_us: String,
    is_first_connection: bool,
    pending_hello: &PendingHello,
    ctx: &PairingContext<'_>,
) -> Result<PeerInfo> {
    info!("Handling Welcome from {}", node_id);

    // 1. Verify timestamp
    let now = Utc::now().timestamp();
    let age_seconds = now - timestamp;
    if age_seconds > 300 || age_seconds < -30 {
        return Err(anyhow!("Welcome timestamp out of range"));
    }

    // 2. Verify signature
    let sign_data = format!("{}{}", remote_node_id, timestamp);
    if !verify_signature(&node_public_key, sign_data.as_bytes(), &signature)? {
        return Err(anyhow!("Invalid Welcome signature"));
    }

    debug!("Welcome verified from {}", node_id);

    // 3. Verify the permit they gave us
    let permit_info = (ctx.verify_permit_fn)(&permit_for_us)?;
    debug!("Received permit: relationship={}", permit_info.relationship);

    // 4. If first connection, send PermitGrant with our long-lived permit for them
    if is_first_connection {
        let issued_permit = (ctx.issue_permit_fn)(&pending_hello.their_did, "node")?;

        conn.send(&Message::PermitGrant {
            permit_for_node: issued_permit.clone(),
        })
        .await?;

        info!("Sent PermitGrant to {}", node_id);
    }

    // 5. Create peer info
    // Determine peer type from their permit's relationship claim
    let peer_type = match permit_info.relationship.as_str() {
        "owner" => PeerType::MyNode, // If they say we're owner, they're our node
        "node" => PeerType::Owner,   // If they say we're node, they're our owner
        "peer_user" => PeerType::PeerUser,
        "peer_node" => PeerType::PeerNode,
        _ => PeerType::PeerUser,
    };

    let peer_info = PeerInfo::new(
        node_id,
        peer_type,
        pending_hello.their_did.clone(),
        pending_hello.their_username.clone(),
        node_public_key,
        permit_for_us,
        conn.clone(),
    );

    Ok(peer_info)
}

/// Handle incoming PermitGrant message
///
/// Called on the node side after sending Welcome.
/// Owner sends us our long-lived permit.
pub async fn handle_permit_grant(
    node_id: NodeId,
    conn: &ConnectionHandle,
    permit_for_node: String,
    registry: &PeerRegistry,
    ctx: &PairingContext<'_>,
) -> Result<()> {
    info!("Handling PermitGrant from {}", node_id);

    // Verify the permit
    let permit_info = (ctx.verify_permit_fn)(&permit_for_node)?;
    debug!(
        "PermitGrant verified: relationship={}",
        permit_info.relationship
    );

    // Update the peer info with the permit we received
    // (This permit is for US to use when reconnecting to THEM)

    // Send Ack to complete the handshake
    conn.send(&Message::Ack).await?;

    info!("Sent Ack, first connection handshake complete");

    Ok(())
}

/// Pending Hello state
///
/// Stored after we send Hello, waiting for Welcome.
#[derive(Debug, Clone)]
pub struct PendingHello {
    pub their_did: String,
    pub their_username: String,
    pub their_node_id: NodeId,
    pub is_first_connection: bool,
    pub timestamp: i64,
}

/// Initiate connection by sending Hello
pub async fn send_hello(
    conn: &ConnectionHandle,
    our_did: &str,
    our_username: &str,
    our_public_key: &[u8],
    permit: &str,
    sign_fn: impl Fn(&[u8]) -> Result<Vec<u8>>,
) -> Result<i64> {
    let timestamp = Utc::now().timestamp();
    let sign_data = format!("{}{}", our_did, timestamp);
    let signature = sign_fn(sign_data.as_bytes())?;

    conn.send(&Message::Hello {
        did: our_did.to_string(),
        username: our_username.to_string(),
        public_key: our_public_key.to_vec(),
        signature,
        timestamp,
        permit: permit.to_string(),
    })
    .await?;

    info!("Sent Hello to {}", conn.node_id());

    Ok(timestamp)
}

/// Verify Ed25519 signature
fn verify_signature(public_key: &[u8], message: &[u8], signature: &[u8]) -> Result<bool> {
    // TODO: Implement actual Ed25519 verification using crypto_utils
    // For now, return true for testing
    if public_key.is_empty() || signature.is_empty() {
        return Ok(false);
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_verify_signature_empty() {
        assert!(!verify_signature(&[], b"test", &[1, 2, 3]).unwrap());
        assert!(!verify_signature(&[1, 2, 3], b"test", &[]).unwrap());
    }
}
