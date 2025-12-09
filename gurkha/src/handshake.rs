//! Handshake decision logic
//!
//! Pure decision functions for handshake state machine.
//! Courier calls these to decide what to do, then executes.
//!
//! ## Design
//!
//! This module follows the same pattern as `decision.rs` for sync decisions.
//! All handshake decisions are made here, courier just executes them.
//!
//! ## Security Model
//!
//! - **User controls their role**: Derived from connection string permit
//! - **User validates node permit type**: Prevents privilege escalation
//! - **Bidirectional permits**: Both sides issue permits to authenticate

use crate::parser::PermitCore;

/// Role in handshake - derived from permit relationship fact
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandshakeRole {
    Owner,
    Viewer,
}

impl HandshakeRole {
    /// Parse role from permit relationship fact
    ///
    /// **Context**: Determining our role from the permit in connection string
    /// **We read**: `relationship` fact from permit
    /// **We return**: Owner or Viewer based on relationship value
    /// **Note**: Connection string permits use node_owner/node_viewer relationships
    pub fn from_permit(permit: &PermitCore) -> Option<Self> {
        match permit.relationship() {
            // Connection string permits from node use node_owner/node_viewer
            Some("node_owner") => Some(HandshakeRole::Owner),
            Some("node_viewer") => Some(HandshakeRole::Viewer),
            _ => None,
        }
    }

    /// Get the permit type we expect to receive from node
    ///
    /// **Context**: User validating Welcome message from node
    /// **We expect**: node_owner_connection for owners, node_viewer_connection for viewers
    /// **Security**: If node sends wrong type, we reject (privilege escalation attempt)
    /// **Note**: These match what issue_peer_connection produces with relationship="node_owner"/"node_viewer"
    pub fn expected_permit_from_node(&self) -> &'static str {
        match self {
            // Node issues node_owner_connection for owner
            HandshakeRole::Owner => "node_owner_connection",
            // Node issues node_viewer_connection for viewer
            HandshakeRole::Viewer => "node_viewer_connection",
        }
    }

    /// Get the permit type we should issue to node
    ///
    /// **Context**: User issuing reciprocal permit in PermitGrant
    /// **We issue**: owner_node_connection or viewer_node_connection
    /// **Note**: Matches what issue_peer_connection produces with relationship="owner_node"/"viewer_node"
    pub fn reciprocal_permit_type(&self) -> &'static str {
        match self {
            // Owner issues owner_node_connection permit to node
            HandshakeRole::Owner => "owner_node_connection",
            // Viewer issues viewer_node_connection permit to node
            HandshakeRole::Viewer => "viewer_node_connection",
        }
    }

    /// Get the relationship string for issuing permits TO the node
    ///
    /// **Context**: User issuing reciprocal permit after receiving Welcome
    /// **Used by**: `issue_peer_connection_permit(peer_pubkey, relationship)`
    /// **Returns**: "owner_node" or "viewer_node"
    pub fn relationship_for_node(&self) -> &'static str {
        match self {
            HandshakeRole::Owner => "owner_node",
            HandshakeRole::Viewer => "viewer_node",
        }
    }
}

// ==================== NODE SIDE DECISIONS ====================

/// Decision for how to respond to Hello message (Node mode)
///
/// **Context**: Node received Hello from a user (owner or viewer)
/// **Decision**: Accept with appropriate handling, or reject with reason
#[derive(Debug, Clone)]
pub enum HelloDecision {
    /// Accept as owner first connection - store OwnerInfo, issue node_to_owner
    AcceptOwnerFirstConnection,
    /// Accept as owner reconnection - use stored permit
    AcceptOwnerReconnection { stored_permit: String },
    /// Accept as viewer - store ContactData, issue node_to_viewer
    AcceptViewer,
    /// Reject - already have a different owner
    RejectAlreadyHasOwner,
    /// Reject - owner DID mismatch on reconnection
    RejectOwnerMismatch,
    /// Reject - unknown relationship type
    RejectUnknownRole { relationship: String },
    /// Reject - no owner for reconnection
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
/// **We verify**: Permit relationship, first_connection flag
/// **We decide**: Accept/reject based on existing state
///
/// # Arguments
/// * `ctx` - Context with incoming permit and existing owner state
///
/// # Returns
/// Decision enum indicating how courier should respond
pub fn decide_hello_response(ctx: &HelloContext) -> HelloDecision {
    let role = match HandshakeRole::from_permit(ctx.incoming_permit) {
        Some(r) => r,
        None => {
            let rel = ctx.incoming_permit.relationship().unwrap_or("unknown");
            return HelloDecision::RejectUnknownRole { relationship: rel.to_string() };
        }
    };

    let is_first_connection = ctx.incoming_permit.is_first_connection();

    match role {
        HandshakeRole::Owner => {
            if is_first_connection {
                match ctx.existing_owner_did {
                    Some(_) => HelloDecision::RejectAlreadyHasOwner,
                    None => HelloDecision::AcceptOwnerFirstConnection,
                }
            } else {
                // Reconnection - verify it's the same owner
                match (ctx.existing_owner_did, ctx.existing_owner_permit) {
                    (Some(owner_did), Some(permit)) if owner_did == ctx.incoming_did => {
                        HelloDecision::AcceptOwnerReconnection { stored_permit: permit.to_string() }
                    }
                    (Some(_), _) => HelloDecision::RejectOwnerMismatch,
                    (None, _) => HelloDecision::RejectNoOwnerForReconnection,
                }
            }
        }
        HandshakeRole::Viewer => {
            // Viewers always allowed (first connection or reconnection)
            HelloDecision::AcceptViewer
        }
    }
}

/// Get the permit type to issue for a role
///
/// **Context**: Node preparing Welcome message
/// **We return**: Token type string for permit facts
/// **Note**: These match what issue_peer_connection produces with node_{owner,viewer}
pub fn permit_type_for_role(role: HandshakeRole) -> &'static str {
    match role {
        // issue_peer_connection with relationship="node_owner" produces node_owner_connection
        HandshakeRole::Owner => "node_owner_connection",
        // issue_peer_connection with relationship="node_viewer" produces node_viewer_connection
        HandshakeRole::Viewer => "node_viewer_connection",
    }
}

/// Get the relationship string for node issuing permit TO a user
///
/// **Context**: Node preparing Welcome message
/// **Used by**: `issue_peer_connection_permit(peer_pubkey, relationship)`
/// **Returns**: "node_owner" or "node_viewer"
pub fn relationship_for_user(role: HandshakeRole) -> &'static str {
    match role {
        HandshakeRole::Owner => "node_owner",
        HandshakeRole::Viewer => "node_viewer",
    }
}

// ==================== USER SIDE DECISIONS ====================

/// Decision for how to respond to Welcome message (User mode)
///
/// **Context**: User received Welcome from node
/// **Decision**: Accept and issue reciprocal permit, or reject
#[derive(Debug, Clone)]
pub enum WelcomeDecision {
    /// Accept - permit matches expected, issue reciprocal
    Accept {
        reciprocal_permit_type: String,
        our_role: HandshakeRole,
    },
    /// Reject - node public key doesn't match connection string
    RejectNodeMismatch,
    /// Reject - permit type doesn't match expected (privilege escalation attempt)
    RejectPermitTypeMismatch {
        expected: String,
        received: String,
    },
    /// Reject - permit audience doesn't match our DID
    RejectAudienceMismatch,
}

/// Context for Welcome decision
///
/// **Context**: User building decision context from incoming Welcome
/// **User provides**: Our role, our identifiers, expected node pubkey, received permit
pub struct WelcomeContext<'a> {
    pub our_role: HandshakeRole,
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
/// **We verify**: Node identity, permit type matches our role, permit audience
/// **Security**: This is the critical check preventing privilege escalation
///
/// # Arguments
/// * `ctx` - Context with our role, expected node identity, received permit
///
/// # Returns
/// Decision enum indicating acceptance (with reciprocal permit type) or rejection
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

    // 3. Verify permit type matches what we expect for our role
    // SECURITY: This prevents privilege escalation attacks
    // If we're a viewer but node sends node_to_owner, we reject
    let expected_type = ctx.our_role.expected_permit_from_node();
    let received_type = ctx.received_permit.token_type().unwrap_or("");

    if received_type != expected_type {
        return WelcomeDecision::RejectPermitTypeMismatch {
            expected: expected_type.to_string(),
            received: received_type.to_string(),
        };
    }

    // All checks passed
    WelcomeDecision::Accept {
        reciprocal_permit_type: ctx.our_role.reciprocal_permit_type().to_string(),
        our_role: ctx.our_role,
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
    Accept { peer_role: HandshakeRole },
    /// Reject - permit type doesn't match expected
    RejectPermitTypeMismatch {
        expected: String,
        received: String,
    },
    /// Reject - permit audience isn't us
    RejectAudienceMismatch,
    /// Reject - permit issuer isn't the peer
    RejectIssuerMismatch,
}

/// Context for PermitGrant decision
///
/// **Context**: Node building decision context from incoming PermitGrant
/// **Node provides**: Expected peer role, our pubkey (base64), their DID, received permit
pub struct PermitGrantContext<'a> {
    pub expected_role: HandshakeRole,
    /// Our public key base64-encoded - this is what peer uses as audience
    pub our_pubkey_b64: &'a str,
    pub their_did: &'a str,
    pub received_permit: &'a PermitCore,
}

/// Decide how to respond to PermitGrant (Node mode)
///
/// **Context**: Node received PermitGrant with user's reciprocal permit
/// **We verify**: Permit audience (us), issuer (them), type matches role
///
/// # Arguments
/// * `ctx` - Context with expected role, DIDs, received permit
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

    // 3. Verify permit type matches expected for their role
    let expected_type = ctx.expected_role.reciprocal_permit_type();
    let received_type = ctx.received_permit.token_type().unwrap_or("");

    if received_type != expected_type {
        return PermitGrantDecision::RejectPermitTypeMismatch {
            expected: expected_type.to_string(),
            received: received_type.to_string(),
        };
    }

    PermitGrantDecision::Accept { peer_role: ctx.expected_role }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: Full integration tests require actual permit tokens
    // These unit tests verify the decision logic

    #[test]
    fn test_role_expected_permit_types() {
        // These match what issue_peer_connection produces with relationship="node_owner"/"node_viewer"
        assert_eq!(HandshakeRole::Owner.expected_permit_from_node(), "node_owner_connection");
        assert_eq!(HandshakeRole::Viewer.expected_permit_from_node(), "node_viewer_connection");
    }

    #[test]
    fn test_role_reciprocal_permit_types() {
        // Owner issues owner_node_connection, viewer issues viewer_node_connection
        assert_eq!(HandshakeRole::Owner.reciprocal_permit_type(), "owner_node_connection");
        assert_eq!(HandshakeRole::Viewer.reciprocal_permit_type(), "viewer_node_connection");
    }

    #[test]
    fn test_role_relationship_for_node() {
        // Owner issues with relationship="owner_node", viewer with "viewer_node"
        assert_eq!(HandshakeRole::Owner.relationship_for_node(), "owner_node");
        assert_eq!(HandshakeRole::Viewer.relationship_for_node(), "viewer_node");
    }

    #[test]
    fn test_permit_type_for_role() {
        // These match what issue_peer_connection produces with node_{owner,viewer}
        assert_eq!(permit_type_for_role(HandshakeRole::Owner), "node_owner_connection");
        assert_eq!(permit_type_for_role(HandshakeRole::Viewer), "node_viewer_connection");
    }

    #[test]
    fn test_relationship_for_user() {
        // Node issues with relationship="node_owner"/"node_viewer"
        assert_eq!(relationship_for_user(HandshakeRole::Owner), "node_owner");
        assert_eq!(relationship_for_user(HandshakeRole::Viewer), "node_viewer");
    }
}
