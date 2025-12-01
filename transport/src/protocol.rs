//! Protocol message types for P2P communication
//!
//! All messages exchanged between peers are defined here.
//! Transport layer just sends/receives these - business logic is in Courier.

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
        /// Ed25519 public key for identity verification
        public_key: Vec<u8>,
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
        /// Node's Ed25519 public key
        node_public_key: Vec<u8>,
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
    Rejected {
        reason: String,
    },

    // ==================== Resource Sync (Phase 2) ====================

    /// Request sync for a resource
    SyncRequest {
        /// Request ID for correlation
        id: String,
        /// Resource (page) ID to sync
        resource_id: String,
        /// Loro state vector for incremental sync
        state_vector: Vec<u8>,
    },

    /// Response with updates
    SyncResponse {
        /// Matches request ID
        id: String,
        /// Loro updates (encrypted)
        updates: Vec<u8>,
    },

    /// Push updates (fire-and-forget)
    SyncPush {
        /// Resource ID
        resource_id: String,
        /// Loro updates (encrypted)
        updates: Vec<u8>,
    },

    // ==================== Folder Sync (Phase 2) ====================

    /// Request folder contents
    FolderRequest {
        id: String,
        folder_id: String,
    },

    /// Response with folder pages
    FolderResponse {
        id: String,
        folder_id: String,
        /// Serialized page metadata
        pages: Vec<u8>,
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
        /// Node's folder permit (issued from FOLDER_TEMPLATE.delegation.node)
        space_permit: String,
    },

    /// Owner publishes a page to node (send one by one after space)
    ///
    /// **Context**: Pages sent separately (can be large)
    /// **Layers**: Decrypted by owner, node re-encrypts with own key
    PublishPage {
        request_id: String,
        /// Page metadata
        page: PublishedPageMeta,
        /// Node's resource permit (issued from RESOURCE_TEMPLATE.delegation.node)
        page_permit: String,
        /// Decrypted layer data: layer_name → raw bytes
        layers: std::collections::HashMap<String, Vec<u8>>,
    },

    // ==================== Live Data (Future) ====================

    /// Continuous data stream (games, video, audio)
    LiveData {
        stream_id: String,
        data: Vec<u8>,
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
    pub page_type: String,
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
    /// Node's Ed25519 signing public key
    pub node_public_key: Vec<u8>,
    /// Iroh device key for P2P connection
    pub device_public_key: Vec<u8>,
    /// First-connection UCAN permit
    pub permit: String,
}

impl ConnectionString {
    /// Create a new connection string
    pub fn new(node_public_key: Vec<u8>, device_public_key: Vec<u8>, permit: String) -> Self {
        Self {
            node_public_key,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_serialization() {
        let msg = Message::Hello {
            did: "did:key:test".to_string(),
            username: "alice".to_string(),
            public_key: vec![1, 2, 3],
            signature: vec![4, 5, 6],
            timestamp: 1234567890,
            permit: "ucan_token_here".to_string(),
        };

        let serialized = bincode::serialize(&msg).unwrap();
        let deserialized: Message = bincode::deserialize(&serialized).unwrap();

        match deserialized {
            Message::Hello { did, username, .. } => {
                assert_eq!(did, "did:key:test");
                assert_eq!(username, "alice");
            }
            _ => panic!("Wrong message type"),
        }
    }

    #[test]
    fn test_connection_string_encode_decode() {
        let conn = ConnectionString::new(
            vec![1, 2, 3],
            vec![4, 5, 6],
            "test_permit".to_string(),
        );

        let encoded = conn.encode().unwrap();
        let decoded = ConnectionString::decode(&encoded).unwrap();

        assert_eq!(decoded.node_public_key, vec![1, 2, 3]);
        assert_eq!(decoded.device_public_key, vec![4, 5, 6]);
        assert_eq!(decoded.permit, "test_permit");
    }
}
