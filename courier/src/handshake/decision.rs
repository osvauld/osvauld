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

use gurkha::{Permit, PeerCapabilities};

/// Extract peer capabilities from permit
///
/// **Context**: Determining behavior from the permit in connection string
/// **We read**: `peer_capabilities` from permit facts
/// **We return**: PeerCapabilities struct with all capability flags
pub fn extract_capabilities(permit: &Permit) -> PeerCapabilities {
    permit.peer_capabilities().clone()
}

/// Check if permit indicates first connection
pub fn is_first_connection(permit: &Permit) -> bool {
    permit.is_first_connection()
}

/// Check if permit allows publishing spaces
pub fn can_publish(permit: &Permit) -> bool {
    permit.peer_capabilities().accept_publish
}

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
    pub incoming_permit: &'a Permit,
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
    pub our_permit: &'a Permit,
    pub our_did: &'a str,
    /// Our public key in base64 - permits use this as audience, not DID
    pub our_pubkey_b64: &'a str,
    pub expected_node_pubkey: &'a [u8],
    pub received_node_pubkey: &'a [u8],
    pub received_permit: &'a Permit,
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
    pub received_permit: &'a Permit,
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
    use gurkha::test_fixtures;

    #[test]
    fn test_hello_first_connection_no_existing_owner() {
        let permit = test_fixtures::handshake_owner_first("page1", "did:key:alice");
        let ctx = HelloContext {
            incoming_permit: &permit,
            incoming_did: "did:key:alice",
            existing_owner_did: None,
            existing_owner_permit: None,
        };

        match decide_hello_response(&ctx) {
            HelloDecision::AcceptFirstConnection { can_publish } => {
                assert!(can_publish);
            }
            other => panic!("Expected AcceptFirstConnection, got {:?}", other),
        }
    }

    #[test]
    fn test_hello_first_connection_already_has_owner() {
        let permit = test_fixtures::handshake_owner_first("page1", "did:key:bob");
        let ctx = HelloContext {
            incoming_permit: &permit,
            incoming_did: "did:key:bob",
            existing_owner_did: Some("did:key:alice"),
            existing_owner_permit: Some("existing_permit"),
        };

        assert!(matches!(
            decide_hello_response(&ctx),
            HelloDecision::RejectAlreadyHasOwner
        ));
    }

    #[test]
    fn test_hello_reconnection_same_owner() {
        let permit = test_fixtures::handshake_owner_reconnect("page1", "did:key:alice");
        let ctx = HelloContext {
            incoming_permit: &permit,
            incoming_did: "did:key:alice",
            existing_owner_did: Some("did:key:alice"),
            existing_owner_permit: Some("stored_permit_token"),
        };

        match decide_hello_response(&ctx) {
            HelloDecision::AcceptReconnection { stored_permit, can_publish } => {
                assert_eq!(stored_permit, "stored_permit_token");
                assert!(can_publish);
            }
            other => panic!("Expected AcceptReconnection, got {:?}", other),
        }
    }

    #[test]
    fn test_hello_reconnection_different_owner() {
        let permit = test_fixtures::handshake_owner_reconnect("page1", "did:key:bob");
        let ctx = HelloContext {
            incoming_permit: &permit,
            incoming_did: "did:key:bob",
            existing_owner_did: Some("did:key:alice"),
            existing_owner_permit: Some("stored_permit"),
        };

        assert!(matches!(
            decide_hello_response(&ctx),
            HelloDecision::RejectOwnerMismatch
        ));
    }

    #[test]
    fn test_hello_reconnection_no_existing_owner() {
        let permit = test_fixtures::handshake_owner_reconnect("page1", "did:key:alice");
        let ctx = HelloContext {
            incoming_permit: &permit,
            incoming_did: "did:key:alice",
            existing_owner_did: None,
            existing_owner_permit: None,
        };

        assert!(matches!(
            decide_hello_response(&ctx),
            HelloDecision::RejectNoOwnerForReconnection
        ));
    }

    #[test]
    fn test_hello_viewer_accepts_as_peer() {
        let permit = test_fixtures::handshake_viewer("page1", "did:key:viewer");
        let ctx = HelloContext {
            incoming_permit: &permit,
            incoming_did: "did:key:viewer",
            existing_owner_did: Some("did:key:owner"),
            existing_owner_permit: Some("owner_permit"),
        };

        assert!(matches!(
            decide_hello_response(&ctx),
            HelloDecision::AcceptPeer
        ));
    }

    #[test]
    fn test_hello_viewer_first_connection_accepts_as_peer() {
        let permit = test_fixtures::handshake_viewer_first("page1", "did:key:viewer");
        let ctx = HelloContext {
            incoming_permit: &permit,
            incoming_did: "did:key:viewer",
            existing_owner_did: None,
            existing_owner_permit: None,
        };

        // Viewers without accept_publish capability are accepted as peers
        assert!(matches!(
            decide_hello_response(&ctx),
            HelloDecision::AcceptPeer
        ));
    }

    #[test]
    fn test_welcome_accept_matching_audience() {
        let our_permit = test_fixtures::handshake_owner_first("page1", "did:key:alice");
        let received_permit = test_fixtures::handshake_owner_reconnect("page1", "alice_pubkey_b64");

        let ctx = WelcomeContext {
            our_permit: &our_permit,
            our_did: "did:key:alice",
            our_pubkey_b64: "alice_pubkey_b64",
            expected_node_pubkey: b"node_pubkey",
            received_node_pubkey: b"node_pubkey",
            received_permit: &received_permit,
        };

        match decide_welcome_response(&ctx) {
            WelcomeDecision::Accept { can_publish } => {
                assert!(can_publish);
            }
            other => panic!("Expected Accept, got {:?}", other),
        }
    }

    #[test]
    fn test_welcome_reject_node_mismatch() {
        let our_permit = test_fixtures::handshake_owner_first("page1", "did:key:alice");
        let received_permit = test_fixtures::handshake_owner_reconnect("page1", "alice_pubkey_b64");

        let ctx = WelcomeContext {
            our_permit: &our_permit,
            our_did: "did:key:alice",
            our_pubkey_b64: "alice_pubkey_b64",
            expected_node_pubkey: b"expected_node",
            received_node_pubkey: b"different_node",
            received_permit: &received_permit,
        };

        assert!(matches!(
            decide_welcome_response(&ctx),
            WelcomeDecision::RejectNodeMismatch
        ));
    }

    #[test]
    fn test_welcome_accept_empty_expected_pubkey() {
        // When expected_node_pubkey is empty, we skip the check
        let our_permit = test_fixtures::handshake_owner_first("page1", "did:key:alice");
        let received_permit = test_fixtures::handshake_owner_reconnect("page1", "alice_pubkey_b64");

        let ctx = WelcomeContext {
            our_permit: &our_permit,
            our_did: "did:key:alice",
            our_pubkey_b64: "alice_pubkey_b64",
            expected_node_pubkey: b"",
            received_node_pubkey: b"any_node",
            received_permit: &received_permit,
        };

        assert!(matches!(
            decide_welcome_response(&ctx),
            WelcomeDecision::Accept { .. }
        ));
    }

    #[test]
    fn test_welcome_viewer_cannot_publish() {
        let our_permit = test_fixtures::handshake_viewer("page1", "did:key:viewer");
        let received_permit = test_fixtures::handshake_viewer("page1", "viewer_pubkey_b64");

        let ctx = WelcomeContext {
            our_permit: &our_permit,
            our_did: "did:key:viewer",
            our_pubkey_b64: "viewer_pubkey_b64",
            expected_node_pubkey: b"node",
            received_node_pubkey: b"node",
            received_permit: &received_permit,
        };

        match decide_welcome_response(&ctx) {
            WelcomeDecision::Accept { can_publish } => {
                assert!(!can_publish);
            }
            other => panic!("Expected Accept, got {:?}", other),
        }
    }

    #[test]
    fn test_permit_grant_accept() {
        let received_permit = test_fixtures::handshake_owner_reconnect("page1", "node_pubkey_b64");
        // Use the actual issuer from the permit (test fixtures sign with TEST_KEY)
        let their_did = received_permit.issuer().expect("permit should have issuer");

        let ctx = PermitGrantContext {
            peer_can_publish: true,
            our_pubkey_b64: "node_pubkey_b64",
            their_did,
            received_permit: &received_permit,
        };

        match decide_permit_grant_response(&ctx) {
            PermitGrantDecision::Accept { can_publish } => {
                assert!(can_publish);
            }
            other => panic!("Expected Accept, got {:?}", other),
        }
    }

    #[test]
    fn test_permit_grant_viewer_cannot_publish() {
        let received_permit = test_fixtures::handshake_viewer("page1", "node_pubkey_b64");
        // Use the actual issuer from the permit (test fixtures sign with TEST_KEY)
        let their_did = received_permit.issuer().expect("permit should have issuer");

        let ctx = PermitGrantContext {
            peer_can_publish: false,
            our_pubkey_b64: "node_pubkey_b64",
            their_did,
            received_permit: &received_permit,
        };

        match decide_permit_grant_response(&ctx) {
            PermitGrantDecision::Accept { can_publish } => {
                assert!(!can_publish);
            }
            other => panic!("Expected Accept, got {:?}", other),
        }
    }

    #[test]
    fn test_extract_capabilities() {
        // Owner: can publish + share, not relay
        let owner = test_fixtures::handshake_owner_first("page1", "did:key:alice");
        let owner_caps = extract_capabilities(&owner);
        assert!(owner_caps.accept_publish);
        assert!(owner_caps.share);
        assert!(!owner_caps.relay);

        // Viewer: no capabilities
        let viewer = test_fixtures::handshake_viewer("page1", "did:key:viewer");
        let viewer_caps = extract_capabilities(&viewer);
        assert!(!viewer_caps.accept_publish);
        assert!(!viewer_caps.share);
        assert!(!viewer_caps.relay);

        // First connection vs reconnection
        assert!(is_first_connection(&owner));
        let reconnect = test_fixtures::handshake_owner_reconnect("page1", "did:key:alice");
        assert!(!is_first_connection(&reconnect));

        // Can publish
        assert!(can_publish(&owner));
        assert!(!can_publish(&viewer));
    }
}
