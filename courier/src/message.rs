//! Protocol message types for P2P communication
//!
//! All messages exchanged between peers are defined here.
//! courier2 owns serialization/deserialization.

use serde::{Deserialize, Serialize};

/// Protocol messages for peer-to-peer communication
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Message {
    // --- Handshake ---

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

    // --- Sync Protocol (3-Step) ---

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

    /// Request full snapshot replacement after max resyncs exceeded
    ///
    /// **Context**: Sync divergence couldn't be resolved after MAX_RESYNC_ATTEMPTS
    /// **User mode**: Sends this to node to request authoritative snapshot
    /// **Node mode**: Responds with SyncSnapshot containing full layer state
    ///
    /// **Invariant**: Node is source of truth - user accepts node's full state
    SyncReset {
        /// Page ID
        page_id: String,
        /// Layer name
        layer_name: String,
    },

    /// Response to SyncReset - full layer snapshot
    ///
    /// **Context**: Node sends authoritative layer snapshot to resolve divergence
    /// **User mode**: Replaces local layer entirely with this snapshot
    /// **Result**: Vectors guaranteed to match after apply
    SyncSnapshot {
        /// Page ID
        page_id: String,
        /// Layer name
        layer_name: String,
        /// Full Loro snapshot (not incremental update)
        snapshot: Vec<u8>,
        /// State vector after snapshot (for verification)
        state_vector: Vec<u8>,
        /// Ephemeral X25519 public key for ECDH decryption
        ephemeral_public: [u8; 32],
    },

    // --- Asset Sync (iroh-blobs) ---

    /// Request peer to prepare an asset for transfer
    ///
    /// **Context**: After syncing `{page_id}/assets` layer via Loro, receiver
    /// finds assets in metadata that are missing locally
    /// **Flow**: AssetPrepare → (peer decrypts, adds to blob store) → AssetReady
    AssetPrepare {
        /// Page containing the asset
        page_id: String,
        /// Blake3 hash of the asset (from AssetMetadata)
        hash: String,
    },

    /// Notification that asset blob is ready for download
    ///
    /// **Context**: Peer decrypted asset from local storage, added to iroh-blobs store
    /// **Receiver**: Downloads via iroh-blobs using iroh_hash, verifies, encrypts, stores
    AssetReady {
        /// Page containing the asset
        page_id: String,
        /// Blake3 hash of the asset (matches AssetPrepare request)
        hash: String,
        /// iroh-blobs hash for download (32 bytes)
        iroh_hash: [u8; 32],
    },

    /// Asset transfer acknowledgment
    ///
    /// **Context**: Receiver downloaded, verified, and stored the asset
    /// **Sender**: Can cleanup temporary blob from iroh-blobs store
    AssetAck {
        /// Page containing the asset
        page_id: String,
        /// Blake3 hash of the asset
        hash: String,
        /// Whether transfer was successful
        success: bool,
        /// Error message if failed
        error: Option<String>,
    },

    // --- Publishing (Owner → Node) ---

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

    // --- Shareable Links ---

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

    // --- Viewer Space Request ---

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

    // --- Sync Consent (Viewer → Node) ---

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

    // --- Errors ---

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
            Message::SyncReset { .. } => "SyncReset",
            Message::SyncSnapshot { .. } => "SyncSnapshot",
            Message::AssetPrepare { .. } => "AssetPrepare",
            Message::AssetReady { .. } => "AssetReady",
            Message::AssetAck { .. } => "AssetAck",
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

// --- Publishing Types ---

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

// --- Ephemeral Datagram (Simplified) ---

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
    use proptest::prelude::*;

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

    // --- Property-Based Tests ---

    prop_compose! {
        fn arb_ephemeral_datagram()(
            page_id in "[a-z0-9]{8,16}",
            payload in prop::collection::vec(any::<u8>(), 0..100),
        ) -> EphemeralDatagram {
            EphemeralDatagram { page_id, payload }
        }
    }

    proptest! {
        #[test]
        fn test_ephemeral_roundtrip(datagram in arb_ephemeral_datagram()) {
            let serialized = datagram.to_bytes().unwrap();
            let deserialized = EphemeralDatagram::from_bytes(&serialized).unwrap();
            prop_assert_eq!(datagram.page_id, deserialized.page_id);
            prop_assert_eq!(datagram.payload, deserialized.payload);
        }
    }

    prop_compose! {
        fn arb_hello()(
            did in "[a-z0-9:]{10,30}",
            username in "[a-zA-Z]{3,12}",
            public_key in prop::array::uniform32(any::<u8>()),
            encryption_key in prop::array::uniform32(any::<u8>()),
            signature in prop::collection::vec(any::<u8>(), 32..64),
            timestamp in any::<i64>(),
            permit in "[a-zA-Z0-9]{20,50}",
        ) -> Message {
            Message::Hello { did, username, public_key, encryption_key, signature, timestamp, permit }
        }
    }

    prop_compose! {
        fn arb_sync_offer()(
            page_id in "[a-z0-9]{8,16}",
            layer_name in "[a-z0-9/_]{4,20}",
            data in prop::collection::vec(any::<u8>(), 0..100),
            state_vector in prop::collection::vec(any::<u8>(), 0..50),
            ephemeral_public in prop::array::uniform32(any::<u8>()),
            permit in "[a-zA-Z0-9]{20,50}",
        ) -> Message {
            Message::SyncOffer { page_id, layer_name, data, state_vector, ephemeral_public, permit }
        }
    }

    prop_compose! {
        fn arb_sync_accept()(
            page_id in "[a-z0-9]{8,16}",
            layer_name in "[a-z0-9/_]{4,20}",
            state_vector in prop::collection::vec(any::<u8>(), 0..50),
        ) -> Message {
            Message::SyncAccept { page_id, layer_name, state_vector }
        }
    }

    proptest! {
        #[test]
        fn test_hello_roundtrip(msg in arb_hello()) {
            let serialized = bincode::serialize(&msg).unwrap();
            let deserialized: Message = bincode::deserialize(&serialized).unwrap();
            if let (
                Message::Hello { did: d1, username: u1, permit: p1, public_key: pk1, encryption_key: ek1, timestamp: t1, .. },
                Message::Hello { did: d2, username: u2, permit: p2, public_key: pk2, encryption_key: ek2, timestamp: t2, .. }
            ) = (&msg, &deserialized) {
                prop_assert_eq!(d1, d2);
                prop_assert_eq!(u1, u2);
                prop_assert_eq!(p1, p2);
                prop_assert_eq!(pk1, pk2);
                prop_assert_eq!(ek1, ek2);
                prop_assert_eq!(t1, t2);
            } else {
                prop_assert!(false, "Deserialization changed message type");
            }
        }

        #[test]
        fn test_sync_offer_roundtrip(msg in arb_sync_offer()) {
            let serialized = bincode::serialize(&msg).unwrap();
            let deserialized: Message = bincode::deserialize(&serialized).unwrap();
            if let (
                Message::SyncOffer { page_id: p1, layer_name: l1, data: d1, state_vector: sv1, ephemeral_public: ep1, permit: pmt1 },
                Message::SyncOffer { page_id: p2, layer_name: l2, data: d2, state_vector: sv2, ephemeral_public: ep2, permit: pmt2 }
            ) = (&msg, &deserialized) {
                prop_assert_eq!(p1, p2);
                prop_assert_eq!(l1, l2);
                prop_assert_eq!(d1, d2);
                prop_assert_eq!(sv1, sv2);
                prop_assert_eq!(ep1, ep2);
                prop_assert_eq!(pmt1, pmt2);
            } else {
                prop_assert!(false, "Deserialization changed message type");
            }
        }

        #[test]
        fn test_sync_accept_roundtrip(msg in arb_sync_accept()) {
            let serialized = bincode::serialize(&msg).unwrap();
            let deserialized: Message = bincode::deserialize(&serialized).unwrap();
            if let (
                Message::SyncAccept { page_id: p1, layer_name: l1, state_vector: sv1 },
                Message::SyncAccept { page_id: p2, layer_name: l2, state_vector: sv2 }
            ) = (&msg, &deserialized) {
                prop_assert_eq!(p1, p2);
                prop_assert_eq!(l1, l2);
                prop_assert_eq!(sv1, sv2);
            } else {
                prop_assert!(false, "Deserialization changed message type");
            }
        }
    }

    // --- Message Name Tests ---

    #[test]
    fn test_message_names_are_unique() {
        // Create representative messages for each variant
        let messages: Vec<Message> = vec![
            Message::Hello {
                did: "did".to_string(),
                username: "u".to_string(),
                public_key: [0; 32],
                encryption_key: [0; 32],
                signature: vec![],
                timestamp: 0,
                permit: "p".to_string(),
            },
            Message::Welcome {
                node_id: "n".to_string(),
                node_public_key: [0; 32],
                node_encryption_key: [0; 32],
                signature: vec![],
                timestamp: 0,
                permit_for_peer: "p".to_string(),
            },
            Message::PermitGrant { permit_for_node: "p".to_string() },
            Message::Ack,
            Message::Rejected { reason: "r".to_string() },
            Message::SyncOffer {
                page_id: "p".to_string(),
                layer_name: "l".to_string(),
                data: vec![],
                state_vector: vec![],
                ephemeral_public: [0; 32],
                permit: "p".to_string(),
            },
            Message::SyncAccept {
                page_id: "p".to_string(),
                layer_name: "l".to_string(),
                state_vector: vec![],
            },
            Message::SyncAck {
                page_id: "p".to_string(),
                layer_name: "l".to_string(),
                state_vector: vec![],
            },
            Message::Error {
                id: None,
                code: ErrorCode::Internal,
                message: "e".to_string(),
            },
        ];

        let names: Vec<&str> = messages.iter().map(|m| m.name()).collect();
        let unique_names: std::collections::HashSet<&str> = names.iter().cloned().collect();
        assert_eq!(names.len(), unique_names.len(), "Message names should be unique");
    }
}
