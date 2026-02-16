//! PeerState - Explicit state machine for handshake protocol
//!
//! ```text
//! Connected → HelloReceived → AwaitingPermitGrant → Authenticated
//!     │                                                   │
//!     └──────────────────────────────────────────────────→ Failed
//! ```
//!
//! ## Design (Capability-Based)
//!
//! The protocol is completely role-agnostic. State tracks capabilities
//! derived directly from the permit structure:
//!
//! - `can_publish: true` → Peer can publish spaces to this node

/// Type of peer connection
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PeerType {
    /// Peer can publish spaces (has accept_publish capability)
    Owner,
    /// Peer is our node (we connected to this Node)
    MyNode,
    /// Peer cannot publish, sync only
    Viewer,
}

impl PeerType {
    /// Derive PeerType from capability
    pub fn from_can_publish(can_publish: bool) -> Self {
        if can_publish {
            PeerType::Owner
        } else {
            PeerType::Viewer
        }
    }
}

/// Peer connection state machine
///
/// Tracks the handshake progress for a single peer connection.
/// Each PeerActor owns one PeerState instance.
#[derive(Debug, Clone)]
pub enum PeerState {
    /// Initial state after connection established
    Connected,

    /// Received Hello from peer (Node mode)
    /// Next: We send Welcome, transition to AwaitingPermitGrant
    HelloReceived {
        their_did: String,
        their_username: String,
        their_public_key: Vec<u8>,
        their_permit: String,
        is_first_connection: bool,
    },

    /// Sent Hello, waiting for Welcome (User mode)
    /// Next: Receive Welcome, send PermitGrant, transition to Authenticated
    AwaitingWelcome {
        our_did: String,
        our_username: String,
        sent_at: i64,
        /// Whether we can publish spaces to this node (from our permit)
        can_publish: bool,
        /// Expected node public key from connection string (for validation)
        expected_node_pubkey: Vec<u8>,
        /// Our original permit (for capability extraction in WelcomeContext)
        our_permit: String,
    },

    /// Sent Welcome, waiting for PermitGrant (Node mode)
    /// Or: Received Welcome, sent PermitGrant, waiting for Ack (User mode)
    AwaitingPermitGrant {
        their_did: String,
        their_username: String,
        is_first_connection: bool,
        /// Whether peer can publish spaces (determined from their permit)
        can_publish: bool,
    },

    /// Handshake complete, ready for sync operations
    Authenticated {
        peer_type: PeerType,
        did: String,
        username: String,
    },

    /// Connection failed - terminal state
    Failed { reason: String },
}

impl PeerState {
    /// Check if peer is authenticated and ready for sync
    pub fn is_authenticated(&self) -> bool {
        matches!(self, PeerState::Authenticated { .. })
    }

    /// Check if connection has failed
    pub fn is_failed(&self) -> bool {
        matches!(self, PeerState::Failed { .. })
    }

    /// Check if we received Hello
    pub fn is_hello_received(&self) -> bool {
        matches!(self, PeerState::HelloReceived { .. })
    }

    /// Get peer DID if authenticated
    pub fn did(&self) -> Option<&str> {
        match self {
            PeerState::Authenticated { did, .. } => Some(did),
            _ => None,
        }
    }

    /// Get peer type if authenticated
    pub fn peer_type(&self) -> Option<PeerType> {
        match self {
            PeerState::Authenticated { peer_type, .. } => Some(*peer_type),
            _ => None,
        }
    }

    /// Transition to failed state
    pub fn fail(reason: impl Into<String>) -> Self {
        PeerState::Failed {
            reason: reason.into(),
        }
    }

    /// Get state name for logging
    pub fn name(&self) -> &'static str {
        match self {
            PeerState::Connected => "Connected",
            PeerState::HelloReceived { .. } => "HelloReceived",
            PeerState::AwaitingWelcome { .. } => "AwaitingWelcome",
            PeerState::AwaitingPermitGrant { .. } => "AwaitingPermitGrant",
            PeerState::Authenticated { .. } => "Authenticated",
            PeerState::Failed { .. } => "Failed",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_auth(peer_type: PeerType) -> PeerState {
        PeerState::Authenticated {
            peer_type,
            did: "did:key:test".to_string(),
            username: "alice".to_string(),
        }
    }

    #[test]
    fn test_is_authenticated() {
        assert!(make_auth(PeerType::Owner).is_authenticated());
        assert!(!PeerState::Connected.is_authenticated());
        assert!(!PeerState::fail("reason").is_authenticated());
    }

    #[test]
    fn test_is_failed() {
        assert!(PeerState::fail("reason").is_failed());
        assert!(!PeerState::Connected.is_failed());
        assert!(!make_auth(PeerType::Owner).is_failed());
    }

    #[test]
    fn test_did_extraction() {
        assert_eq!(make_auth(PeerType::Owner).did(), Some("did:key:test"));
        assert_eq!(PeerState::Connected.did(), None);
        assert_eq!(PeerState::fail("reason").did(), None);
    }

    #[test]
    fn test_peer_type_extraction() {
        assert_eq!(
            make_auth(PeerType::Owner).peer_type(),
            Some(PeerType::Owner)
        );
        assert_eq!(
            make_auth(PeerType::Viewer).peer_type(),
            Some(PeerType::Viewer)
        );
        assert_eq!(PeerState::Connected.peer_type(), None);
        assert_eq!(PeerState::fail("reason").peer_type(), None);
    }

    #[test]
    fn test_state_names_are_distinct() {
        let states = [
            PeerState::Connected,
            PeerState::HelloReceived {
                their_did: "did".to_string(),
                their_username: "user".to_string(),
                their_public_key: vec![],
                their_permit: "permit".to_string(),
                is_first_connection: false,
            },
            PeerState::AwaitingWelcome {
                our_did: "did".to_string(),
                our_username: "user".to_string(),
                sent_at: 0,
                can_publish: false,
                expected_node_pubkey: vec![],
                our_permit: "permit".to_string(),
            },
            PeerState::AwaitingPermitGrant {
                their_did: "did".to_string(),
                their_username: "user".to_string(),
                is_first_connection: false,
                can_publish: false,
            },
            PeerState::Authenticated {
                peer_type: PeerType::Owner,
                did: "did".to_string(),
                username: "user".to_string(),
            },
            PeerState::fail("reason"),
        ];

        let names: Vec<&str> = states.iter().map(|s| s.name()).collect();

        // Verify all names are unique
        for (i, name) in names.iter().enumerate() {
            for (j, other) in names.iter().enumerate() {
                if i != j {
                    assert_ne!(name, other, "State names should be unique");
                }
            }
        }
    }
}
