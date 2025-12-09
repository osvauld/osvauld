//! PeerState - Explicit state machine for handshake protocol
//!
//! ```text
//! Connected → HelloReceived → AwaitingPermitGrant → Authenticated
//!     │                                                   │
//!     └──────────────────────────────────────────────────→ Failed
//! ```

use gurkha::HandshakeRole;

/// Type of peer connection
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PeerType {
    /// Peer is the owner (Node receives connection from Owner)
    Owner,
    /// Peer is our node (Owner connected to this Node)
    MyNode,
    /// Peer is a viewer with delegated permit
    Viewer,
    /// Peer is a submitter with push-only access
    Submitter,
}

impl From<HandshakeRole> for PeerType {
    fn from(role: HandshakeRole) -> Self {
        match role {
            HandshakeRole::Owner => PeerType::Owner,
            HandshakeRole::Viewer => PeerType::Viewer,
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
        /// Our role in this connection (Owner or Viewer) - derived from our permit
        our_role: HandshakeRole,
        /// Expected node public key from connection string (for validation)
        expected_node_pubkey: Vec<u8>,
    },

    /// Sent Welcome, waiting for PermitGrant (Node mode)
    /// Or: Received Welcome, sent PermitGrant, waiting for Ack (User mode)
    AwaitingPermitGrant {
        their_did: String,
        their_username: String,
        is_first_connection: bool,
        /// Their role in this connection (Owner or Viewer) - determined from their permit
        their_role: HandshakeRole,
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
