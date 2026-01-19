//! Protocol message types for P2P communication
//!
//! All messages exchanged between peers are defined here.
//! courier2 owns serialization/deserialization.

use serde::{Deserialize, Serialize};

/// Protocol messages for peer-to-peer communication
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Message {
    // ==================== Handshake ====================

    /// Initial handshake from connecting peer
    ///
    /// Contains identity proof (signature) and permit.
    /// - First connection: permit from connection string (first_connection type)
    /// - Subsequent: stored permit from previous Welcome
    Hello {
        /// Decentralized identifier (did:key:...)
        did: String,
        /// Human-readable username
        username: String,
        /// Ed25519 public key for identity verification (verifying key)
        public_key: [u8; 32],
        /// X25519 public key for ECDH encryption
        encryption_key: [u8; 32],
        /// Signature over (did + timestamp) - proves ownership of identity
        signature: Vec<u8>,
        /// Timestamp for replay protection
        timestamp: i64,
        /// UCAN permit (always present)
        permit: String,
    },

    /// Response to Hello - connection accepted
    Welcome {
        /// Node's iroh NodeId as string
        node_id: String,
        /// Node's Ed25519 public key (verifying key)
        node_public_key: [u8; 32],
        /// Node's X25519 public key for ECDH encryption
        node_encryption_key: [u8; 32],
        /// Node's signature over (node_id + timestamp)
        signature: Vec<u8>,
        /// Timestamp for replay protection
        timestamp: i64,
        /// Long-lived UCAN permit for the peer
        permit_for_peer: String,
    },

    /// Owner grants permit to node (first connection only)
    PermitGrant {
        /// Long-lived UCAN permit for the node
        permit_for_node: String,
    },

    /// Generic acknowledgment
    Ack,

    /// Connection/request rejected
    Rejected { reason: String },

    // ==================== Sync Protocol (3-Step) ====================

    /// Sync offer: update data + sender's state vector
    ///
    /// **Context**: First step of 3-step sync protocol
    /// **Sender**: Has update data and their current state vector
    /// **Receiver**: Should apply update, then send SyncAccept with their vector
    ///
    /// **Flow**: SyncOffer → SyncAccept → SyncAck (or resync SyncOffer if diverged)
    SyncOffer {
        /// Page ID (which page this update belongs to)
        page_id: String,
        /// Layer name (e.g., "collaborative_doc", "submissions_doc")
        layer_name: String,
        /// Loro update bytes (transit encrypted)
        data: Vec<u8>,
        /// Sender's state vector for this layer (plaintext - not sensitive)
        state_vector: Vec<u8>,
        /// Ephemeral X25519 public key for ECDH decryption
        ephemeral_public: [u8; 32],
        /// Consent permit (proves authorization to sync)
        permit: String,
    },

    /// Sync accept: receiver's state vector after applying update
    ///
    /// **Context**: Second step of 3-step sync protocol
    /// **Sender**: Applied the update, reporting their state vector
    /// **Receiver**: Compares vectors - if match send SyncAck, if diverged send resync SyncOffer
    SyncAccept {
        /// Page ID
        page_id: String,
        /// Layer name
        layer_name: String,
        /// Receiver's state vector after applying update
        state_vector: Vec<u8>,
    },

    /// Sync acknowledgment: final confirmation
    ///
    /// **Context**: Third step of 3-step sync protocol (happy path)
    /// **Sender**: Vectors matched, sync complete
    /// **Receiver**: Updates cached peer vector, sync complete
    SyncAck {
        /// Page ID
        page_id: String,
        /// Layer name
        layer_name: String,
        /// Sender's final state vector
        state_vector: Vec<u8>,
    },

    // ==================== Publishing (Owner → Node) ====================

    /// Owner publishes a space to node (send first)
    ///
    /// **Context**: Owner manually triggers publish
    /// **Flow**: Send PublishSpace first, then PublishPage for each page
    PublishSpace {
        request_id: String,
        /// Space metadata
        space: PublishedSpace,
        /// Node's space permit (issued from SPACE_TEMPLATE.delegation.node)
        space_permit: String,
    },

    /// Owner publishes a page to node (send one by one after space)
    ///
    /// **Context**: Pages sent separately (can be large)
    /// **Layers**: Encrypted with ephemeral ECDH transit key
    /// **Flow**: Owner generates ephemeral keypair, ECDH with node's pubkey,
    ///          encrypts layers with derived transit key
    PublishPage {
        request_id: String,
        /// Page metadata
        page: PublishedPageMeta,
        /// Node's page permit (delegated from owner's page permit)
        page_permit: String,
        /// Owner's page permit (for node to store for sync authorization)
        owner_permit: String,
        /// Ephemeral X25519 public key for ECDH transit encryption
        ephemeral_public: [u8; 32],
        /// Transit-encrypted layers: layer_name → encrypted bytes
        /// Each layer: nonce (12) || ciphertext || tag (16)
        layers: Vec<(String, Vec<u8>)>,
    },

    /// Acknowledgment for successful page publish
    ///
    /// **Context**: Node echoes back the permit for confirmation
    PublishPageAck {
        request_id: String,
        page_id: String,
        /// Same permit Owner sent - echoed back for integrity check
        permit: String,
    },

    /// Acknowledgment for successful space publish
    ///
    /// **Context**: Node echoes back the permit for confirmation
    /// **Flow**: Owner validates echoed permit, derives space_id from it
    PublishSpaceAck {
        request_id: String,
        /// Same permit Owner sent - echoed back for integrity check
        /// space_id can be derived via gurkha::extract_space_id(permit)
        permit: String,
        /// Page IDs already stored on node (for initial sync)
        pages: Vec<String>,
    },

    /// Error response for failed publish
    PublishError {
        request_id: String,
        error: String,
    },

    // ==================== Shareable Links ====================

    /// Request shareable link for a space (Owner → Node)
    ///
    /// **Context**: Owner wants to share a space with viewers
    /// **Node responds**: ShareableLinkResponse with aud:* permit
    GetShareableLinkRequest {
        request_id: String,
        space_id: String,
    },

    /// Response with shareable link (Node → Owner)
    ///
    /// **Context**: Node generated a viewer permit with aud:* (wildcard audience)
    /// **Owner stores**: This permit for sharing out-of-band with viewers
    GetShareableLinkResponse {
        request_id: String,
        space_id: String,
        /// Viewer permit with aud:* (one-time shareable token)
        permit: String,
    },

    // ==================== Viewer Space Request ====================

    /// Viewer requests space content from node (Viewer → Node)
    ///
    /// **Context**: Viewer received shareable link with aud:* permit
    /// **Viewer sends**: Their DID, public keys, and the aud:* permit
    /// **Node verifies**: Permit is valid aud:* viewer_auth token
    /// **Node responds**: SpaceResponse with delegated permit and content
    SpaceRequest {
        request_id: String,
        /// Space ID from the shareable link
        space_id: String,
        /// Viewer's decentralized identifier (did:key:...)
        viewer_did: String,
        /// Viewer's Ed25519 signing public key (verifying key)
        viewer_public_key: [u8; 32],
        /// Viewer's X25519 encryption public key (for response encryption)
        viewer_encryption_key: [u8; 32],
        /// The aud:* permit from the shareable link
        viewer_permit: String,
    },

    /// Node responds to viewer with space metadata (Node → Viewer)
    ///
    /// **Context**: Node validated aud:* permit, delegated real permit
    /// **Flow**: SpaceRequest → SpaceData → SpaceDataAck → PageData (x N)
    /// **Note**: Pages sent separately after viewer acknowledges
    SpaceData {
        request_id: String,
        space_id: String,
        /// Delegated viewer permit (real permit with viewer's DID as audience)
        delegated_permit: String,
        /// Space metadata
        space: PublishedSpace,
        /// Page IDs in this space (metadata only, layers sent separately)
        page_ids: Vec<String>,
    },

    /// Viewer acknowledges space data receipt (Viewer → Node)
    ///
    /// **Context**: Viewer received SpaceData, ready to receive pages
    /// **Node stores**: Delegated permit in VIEWER_PERMITS table
    /// **Node then**: Streams PageData messages one by one
    SpaceDataAck {
        request_id: String,
        space_id: String,
        /// The delegated permit (for node to verify viewer auth)
        delegated_permit: String,
    },

    /// Node sends individual page to viewer (Node → Viewer)
    ///
    /// **Context**: Viewer acknowledged SpaceData, node streams pages
    /// **Encryption**: Layers are transit-encrypted with ephemeral ECDH
    /// **Viewer stores**: Page with source_node_id for future sync
    PageData {
        request_id: String,
        space_id: String,
        /// Page metadata
        meta: PublishedPageMeta,
        /// Delegated page permit for this viewer
        permit: String,
        /// Ephemeral X25519 public key for ECDH transit decryption
        ephemeral_public: [u8; 32],
        /// Transit-encrypted layers: layer_name → encrypted bytes
        /// Each layer: nonce (12) || ciphertext || tag (16)
        layers: Vec<(String, Vec<u8>)>,
        /// Is this the last page? (for viewer to know sync is complete)
        is_last: bool,
    },

    /// Error response for space request
    SpaceRequestError {
        request_id: String,
        error: String,
    },

    // ==================== Sync Consent (Viewer → Node) ====================

    /// Viewer grants sync consent permits to node (Viewer → Node)
    ///
    /// **Context**: Viewer received space + pages, now issues consent permits
    /// to express consent for receiving sync updates. These are viewer-issued
    /// permits that the node will attach to future sync messages.
    ///
    /// **Flow**: After receiving all PageData messages, viewer issues:
    /// - 1 space consent permit (consent to sync space + accept new pages)
    /// - N page consent permits (consent to sync each page's layers)
    ///
    /// **Permits**: Self-contained with embedded proof chain (`prf_tokens` fact)
    SyncConsentGrant {
        request_id: String,
        space_id: String,
        /// Space consent permit (viewer-issued, for space sync + new pages)
        /// Uses SYNC_SPACE_CONSENT_TEMPLATE, prf = node's space_viewer permit
        space_consent_permit: String,
        /// Page consent permits: (page_id, consent_permit)
        /// Each uses SYNC_PAGE_CONSENT_TEMPLATE, prf = node's page_viewer permit
        page_consent_permits: Vec<(String, String)>,
    },

    /// Node acknowledges receipt of consent permits (Node → Viewer)
    ///
    /// **Context**: Node validated and stored the viewer-issued permits
    /// **Viewer knows**: Node can now send sync updates using these permits
    SyncConsentAck {
        request_id: String,
        space_id: String,
    },

    // ==================== Errors ====================

    /// Error response
    Error {
        /// Request ID if applicable
        id: Option<String>,
        /// Error code
        code: ErrorCode,
        /// Human-readable message
        message: String,
    },
}

impl Message {
    /// Get the message variant name (for logging without binary data)
    pub fn name(&self) -> &'static str {
        match self {
            Message::Hello { .. } => "Hello",
            Message::Welcome { .. } => "Welcome",
            Message::PermitGrant { .. } => "PermitGrant",
            Message::Ack => "Ack",
            Message::Rejected { .. } => "Rejected",
            Message::SyncOffer { .. } => "SyncOffer",
            Message::SyncAccept { .. } => "SyncAccept",
            Message::SyncAck { .. } => "SyncAck",
            Message::PublishSpace { .. } => "PublishSpace",
            Message::PublishSpaceAck { .. } => "PublishSpaceAck",
            Message::PublishPage { .. } => "PublishPage",
            Message::PublishPageAck { .. } => "PublishPageAck",
            Message::PublishError { .. } => "PublishError",
            Message::GetShareableLinkRequest { .. } => "GetShareableLinkRequest",
            Message::GetShareableLinkResponse { .. } => "GetShareableLinkResponse",
            Message::SpaceRequest { .. } => "SpaceRequest",
            Message::SpaceData { .. } => "SpaceData",
            Message::SpaceDataAck { .. } => "SpaceDataAck",
            Message::SpaceRequestError { .. } => "SpaceRequestError",
            Message::PageData { .. } => "PageData",
            Message::SyncConsentGrant { .. } => "SyncConsentGrant",
            Message::SyncConsentAck { .. } => "SyncConsentAck",
            Message::Error { .. } => "Error",
        }
    }

    /// Serialize message to bytes using bincode
    pub fn to_bytes(&self) -> Result<Vec<u8>, bincode::Error> {
        bincode::serialize(self)
    }

    /// Deserialize message from bytes using bincode
    pub fn from_bytes(data: &[u8]) -> Result<Self, bincode::Error> {
        bincode::deserialize(data)
    }
}

// ==================== Publishing Types ====================

/// Space metadata for publishing (transport-layer representation)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublishedSpace {
    pub id: String,
    pub name: String,
    pub parent_space_id: Option<String>,
    pub owner_did: String,
    pub description: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

/// Page metadata for publishing (without encrypted_key - that's local)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublishedPageMeta {
    pub id: String,
    pub space_id: String,
    pub name: String,
    pub owner_did: String,
    pub is_private: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

/// Error codes for protocol errors
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ErrorCode {
    /// Authentication/authorization failed
    Unauthorized,
    /// Resource not found
    NotFound,
    /// Access denied by permit
    AccessDenied,
    /// Malformed message
    InvalidMessage,
    /// Internal error
    Internal,
    /// Timestamp too old (replay attack)
    TimestampExpired,
    /// Permit invalid or expired
    PermitInvalid,
}

/// Connection string structure
///
/// This is what the node prints on startup for the owner to scan/enter.
/// Contains everything needed to establish first connection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionString {
    /// Node's Ed25519 signing public key (verifying key = ID)
    pub node_public_key: [u8; 32],
    /// Node's X25519 encryption public key (for ECDH)
    pub node_encryption_key: [u8; 32],
    /// Iroh device key for P2P connection
    pub device_public_key: [u8; 32],
    /// First-connection UCAN permit
    pub permit: String,
}

impl ConnectionString {
    /// Create a new connection string
    pub fn new(
        node_public_key: [u8; 32],
        node_encryption_key: [u8; 32],
        device_public_key: [u8; 32],
        permit: String,
    ) -> Self {
        Self {
            node_public_key,
            node_encryption_key,
            device_public_key,
            permit,
        }
    }

    /// Encode to base64 string for display/QR
    pub fn encode(&self) -> Result<String, serde_json::Error> {
        let json = serde_json::to_string(self)?;
        Ok(base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            json.as_bytes(),
        ))
    }

    /// Decode from base64 string
    pub fn decode(encoded: &str) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let bytes = base64::Engine::decode(
            &base64::engine::general_purpose::STANDARD,
            encoded,
        )?;
        let json = String::from_utf8(bytes)?;
        let conn: Self = serde_json::from_str(&json)?;
        Ok(conn)
    }
}

// ==================== Ephemeral Datagram (Simplified) ====================

/// Ephemeral datagram - opaque payload routed by page_id
///
/// **Properties:**
/// - Unreliable, unordered (QUIC datagram)
/// - ~1200 byte limit (MTU)
/// - Fire-and-forget
///
/// **Design**: Protocol layer is dumb - just routes opaque bytes by page_id.
/// App layer (Lua) defines the meaning of payload (cursor, typing, presence, etc.)
///
/// **Note:** user_did is derived from connection (PeerActor knows peer identity),
/// not included in payload to minimize size.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EphemeralDatagram {
    /// Page ID this datagram is for (routing key)
    pub page_id: String,
    /// Opaque payload - app defines format (JSON, msgpack, etc.)
    pub payload: Vec<u8>,
}

impl EphemeralDatagram {
    /// Serialize to bytes for sending
    pub fn to_bytes(&self) -> Result<Vec<u8>, bincode::Error> {
        bincode::serialize(self)
    }

    /// Deserialize from received bytes
    pub fn from_bytes(data: &[u8]) -> Result<Self, bincode::Error> {
        bincode::deserialize(data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ephemeral_datagram_serialization() {
        let datagram = EphemeralDatagram {
            page_id: "page-123".to_string(),
            payload: b"{\"type\":\"cursor\",\"x\":100.5,\"y\":200.5}".to_vec(),
        };

        let bytes = datagram.to_bytes().unwrap();
        let deserialized = EphemeralDatagram::from_bytes(&bytes).unwrap();

        assert_eq!(deserialized.page_id, "page-123");
        assert_eq!(deserialized.payload, datagram.payload);
    }

    #[test]
    fn test_message_serialization() {
        let msg = Message::Hello {
            did: "did:key:test".to_string(),
            username: "alice".to_string(),
            public_key: [1u8; 32],
            encryption_key: [2u8; 32],
            signature: vec![4, 5, 6],
            timestamp: 1234567890,
            permit: "ucan_token_here".to_string(),
        };

        let bytes = msg.to_bytes().unwrap();
        let deserialized = Message::from_bytes(&bytes).unwrap();

        match deserialized {
            Message::Hello { did, username, .. } => {
                assert_eq!(did, "did:key:test");
                assert_eq!(username, "alice");
            }
            _ => panic!("Wrong message type"),
        }
    }
}
