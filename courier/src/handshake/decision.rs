//! Handshake decision logic
//!
//! Pure decision functions for handshake state machine.
//! PeerActor calls these to decide what to do, then executes.
//!
//! ## Design (Capability-Based)
//!
//! The protocol is completely role-agnostic. All decisions are based on
//! capabilities derived directly from the permit structure:
//!
//! - `accept_publish: true` → Peer can publish spaces to this node
//! - `first_connection: true` → First time connecting (needs OwnerInfo stored)
//! - `relay: true` → Can forward data to other peers
//! - `share: true` → Can issue delegated permits
//!
//! ## Security Model
//!
//! - **Capabilities in permit**: No hardcoded role names
//! - **Uniform handshake**: Same flow for all peers, capabilities determine behavior
//! - **Bidirectional permits**: Both sides issue permits to authenticate

use gurkha::{PermitCore, PeerCapabilities};

// ==================== CAPABILITY EXTRACTION ====================

/// Extract peer capabilities from permit
///
/// **Context**: Determining behavior from the permit in connection string
/// **We read**: `peer_capabilities` from permit facts
/// **We return**: PeerCapabilities struct with all capability flags
pub fn extract_capabilities(permit: &PermitCore) -> PeerCapabilities {
    if let Some(caps) = permit.get_fact("peer_capabilities") {
        if let Some(obj) = caps.as_object() {
            return PeerCapabilities::from_json(obj);
        }
    }
    // Default: no special capabilities
    PeerCapabilities::default()
}

/// Check if permit indicates first connection
pub fn is_first_connection(permit: &PermitCore) -> bool {
    permit.is_first_connection()
}

/// Check if permit allows publishing spaces
pub fn can_publish(permit: &PermitCore) -> bool {
    extract_capabilities(permit).accept_publish
}

// ==================== NODE SIDE DECISIONS ====================

/// Decision for how to respond to Hello message (Node mode)
///
/// **Context**: Node received Hello from a peer
/// **Decision**: Accept with appropriate handling, or reject with reason
///
/// Note: Decisions are capability-based, not role-based.
#[derive(Debug, Clone)]
pub enum HelloDecision {
    /// Accept first connection from peer who can publish
    /// - Store OwnerInfo
    /// - Issue peer_connection permit
    AcceptFirstConnection {
        /// Peer can publish spaces to this node
        can_publish: bool,
    },
    /// Accept reconnection from known peer
    /// - Use stored permit
    AcceptReconnection {
        stored_permit: String,
        can_publish: bool,
    },
    /// Accept connection from peer (not first, not owner reconnection)
    AcceptPeer,
    /// Reject - already have a different owner
    RejectAlreadyHasOwner,
    /// Reject - owner DID mismatch on reconnection
    RejectOwnerMismatch,
    /// Reject - no owner for reconnection attempt
    RejectNoOwnerForReconnection,
}

/// Context for Hello decision
///
/// **Context**: Node building decision context from incoming Hello
/// **Courier provides**: Parsed permit, incoming DID, existing owner info
pub struct HelloContext<'a> {
    pub incoming_permit: &'a PermitCore,
    pub incoming_did: &'a str,
    pub existing_owner_did: Option<&'a str>,
    pub existing_owner_permit: Option<&'a str>,
}

/// Decide how to respond to Hello (Node mode)
///
/// **Context**: Node received Hello with permit, needs to decide response
/// **We check**: Capabilities (can_publish), first_connection flag
/// **We decide**: Accept/reject based on existing state and capabilities
///
/// # Arguments
/// * `ctx` - Context with incoming permit and existing owner state
///
/// # Returns
/// Decision enum indicating how courier should respond
pub fn decide_hello_response(ctx: &HelloContext) -> HelloDecision {
    let caps = extract_capabilities(ctx.incoming_permit);
    let is_first = is_first_connection(ctx.incoming_permit);

    if caps.accept_publish {
        // Peer wants to publish - check owner state
        if is_first {
            match ctx.existing_owner_did {
                Some(_) => HelloDecision::RejectAlreadyHasOwner,
                None => HelloDecision::AcceptFirstConnection { can_publish: true },
            }
        } else {
            // Reconnection - verify it's the same owner
            match (ctx.existing_owner_did, ctx.existing_owner_permit) {
                (Some(owner_did), Some(permit)) if owner_did == ctx.incoming_did => {
                    HelloDecision::AcceptReconnection {
                        stored_permit: permit.to_string(),
                        can_publish: true,
                    }
                }
                (Some(_), _) => HelloDecision::RejectOwnerMismatch,
                (None, _) => HelloDecision::RejectNoOwnerForReconnection,
            }
        }
    } else {
        // Regular peer connection (no publish capability)
        HelloDecision::AcceptPeer
    }
}

// ==================== USER SIDE DECISIONS ====================

/// Decision for how to respond to Welcome message (User mode)
///
/// **Context**: User received Welcome from node
/// **Decision**: Accept and issue reciprocal permit, or reject
#[derive(Debug, Clone)]
pub enum WelcomeDecision {
    /// Accept - permit valid, issue reciprocal
    Accept {
        /// Whether we can publish to this node
        can_publish: bool,
    },
    /// Reject - node public key doesn't match connection string
    RejectNodeMismatch,
    /// Reject - permit audience doesn't match our DID
    RejectAudienceMismatch,
}

/// Context for Welcome decision
///
/// **Context**: User building decision context from incoming Welcome
/// **User provides**: Our permit (for capabilities), our identifiers, expected node pubkey, received permit
pub struct WelcomeContext<'a> {
    /// Our outgoing permit (to extract capabilities)
    pub our_permit: &'a PermitCore,
    pub our_did: &'a str,
    /// Our public key in base64 - permits use this as audience, not DID
    pub our_pubkey_b64: &'a str,
    pub expected_node_pubkey: &'a [u8],
    pub received_node_pubkey: &'a [u8],
    pub received_permit: &'a PermitCore,
}

/// Decide how to respond to Welcome (User mode)
///
/// **Context**: User received Welcome with node's permit
/// **We verify**: Node identity, permit audience
/// **Security**: Uniform validation for all peers
///
/// # Arguments
/// * `ctx` - Context with our permit, expected node identity, received permit
///
/// # Returns
/// Decision enum indicating acceptance or rejection
pub fn decide_welcome_response(ctx: &WelcomeContext) -> WelcomeDecision {
    // 1. Verify node identity matches connection string
    if !ctx.expected_node_pubkey.is_empty() && ctx.expected_node_pubkey != ctx.received_node_pubkey {
        return WelcomeDecision::RejectNodeMismatch;
    }

    // 2. Verify permit audience is us (or wildcard)
    // Note: Permits use base64 pubkey as audience, not DID
    if let Some(aud) = ctx.received_permit.audience() {
        if aud != "*" && aud != ctx.our_pubkey_b64 && aud != ctx.our_did {
            return WelcomeDecision::RejectAudienceMismatch;
        }
    }

    // Extract our capabilities to pass through
    let caps = extract_capabilities(ctx.our_permit);

    // All checks passed
    WelcomeDecision::Accept {
        can_publish: caps.accept_publish,
    }
}

// ==================== NODE SIDE: PERMITGRANT ====================

/// Decision for how to respond to PermitGrant message (Node mode)
///
/// **Context**: Node received PermitGrant from user with their permit
/// **Decision**: Accept and complete handshake, or reject
#[derive(Debug, Clone)]
pub enum PermitGrantDecision {
    /// Accept - permit valid, complete handshake
    Accept {
        /// Whether peer can publish to this node
        can_publish: bool,
    },
    /// Reject - permit audience isn't us
    RejectAudienceMismatch,
    /// Reject - permit issuer isn't the peer
    RejectIssuerMismatch,
}

/// Context for PermitGrant decision
///
/// **Context**: Node building decision context from incoming PermitGrant
/// **Node provides**: Peer's capabilities (from Hello), our pubkey (base64), their DID, received permit
pub struct PermitGrantContext<'a> {
    /// Capabilities we determined from their Hello permit
    pub peer_can_publish: bool,
    /// Our public key base64-encoded - this is what peer uses as audience
    pub our_pubkey_b64: &'a str,
    pub their_did: &'a str,
    pub received_permit: &'a PermitCore,
}

/// Decide how to respond to PermitGrant (Node mode)
///
/// **Context**: Node received PermitGrant with user's reciprocal permit
/// **We verify**: Permit audience (us), issuer (them)
///
/// # Arguments
/// * `ctx` - Context with peer capabilities, DIDs, received permit
///
/// # Returns
/// Decision enum indicating acceptance or rejection
pub fn decide_permit_grant_response(ctx: &PermitGrantContext) -> PermitGrantDecision {
    // 1. Verify permit audience is our base64-encoded public key
    if let Some(aud) = ctx.received_permit.audience() {
        if aud != ctx.our_pubkey_b64 {
            return PermitGrantDecision::RejectAudienceMismatch;
        }
    }

    // 2. Verify permit issuer is them
    if let Some(iss) = ctx.received_permit.issuer() {
        if iss != ctx.their_did {
            return PermitGrantDecision::RejectIssuerMismatch;
        }
    }

    // Accept with capability info
    PermitGrantDecision::Accept {
        can_publish: ctx.peer_can_publish,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: Full integration tests require actual permit tokens
    // These unit tests verify the decision logic

    #[test]
    fn test_hello_decision_first_connection_can_publish() {
        // Simulating a permit with accept_publish: true, first_connection: true
        // Since we can't easily create a PermitCore in tests, this is more of a
        // documentation of expected behavior
    }
}
