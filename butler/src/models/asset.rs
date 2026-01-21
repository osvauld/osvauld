//! Asset metadata for encrypted static assets
//!
//! Assets are stored encrypted at rest with metadata tracked in Loro CRDT layers.
//! Signatures provide integrity verification for sync.

use serde::{Deserialize, Serialize};

/// Metadata for an encrypted asset
///
/// Stored in Loro `{page_id}/assets` layer for sync.
/// The actual encrypted file is stored in AssetStore at `{hash}.enc`.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AssetMetadata {
    /// Blake3 hash of the plaintext content (also used as asset ID)
    pub hash: String,

    /// Ed25519 signature of the hash by the uploader
    ///
    /// **Verification**: `verify(signature, hash, created_by_pubkey)`
    /// **Re-signing**: Node replaces this when relaying to viewers
    pub signature: String,

    /// Original filename (for display/download)
    pub filename: String,

    /// MIME type (e.g., "image/png", "application/pdf")
    pub mime_type: String,

    /// Plaintext size in bytes
    pub size: u64,

    /// DID of the original uploader
    pub created_by: String,

    /// Unix timestamp (seconds) when the asset was created
    pub created_at: i64,
}

impl AssetMetadata {
    /// Create new asset metadata
    pub fn new(
        hash: String,
        signature: String,
        filename: String,
        mime_type: String,
        size: u64,
        created_by: String,
    ) -> Self {
        Self {
            hash,
            signature,
            filename,
            mime_type,
            size,
            created_by,
            created_at: chrono::Utc::now().timestamp(),
        }
    }

    /// Get the asset ID (alias for hash)
    pub fn id(&self) -> &str {
        &self.hash
    }
}

/// Asset sync request (sent when requesting asset transfer)
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AssetRequest {
    /// Page containing the asset
    pub page_id: String,

    /// Blake3 hash of the asset
    pub hash: String,
}

/// Asset ready notification (sent when blob is available for download)
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AssetReady {
    /// Page containing the asset
    pub page_id: String,

    /// Blake3 hash of the asset
    pub hash: String,

    /// iroh-blobs hash (for download)
    pub iroh_hash: Vec<u8>,
}

/// Asset transfer acknowledgment
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AssetAck {
    /// Page containing the asset
    pub page_id: String,

    /// Blake3 hash of the asset
    pub hash: String,

    /// Whether the transfer was successful
    pub success: bool,

    /// Optional error message if failed
    pub error: Option<String>,
}
