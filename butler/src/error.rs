//! Butler error types - consolidated to core categories
//!
//! **Design**: 8 core error types that cover all failure cases.
//! Error messages provide specific context via string contents.

use crate::models::LayerError;
use thiserror::Error;

/// Consolidated Butler error type
///
/// **Categories**:
/// - `NotFound` - Resource doesn't exist (space, page, layer, permit, asset)
/// - `Database` - Storage/persistence failures
/// - `Serialization` - JSON/bincode encode/decode failures
/// - `Encryption` - Crypto operation failures
/// - `Invalid` - Bad input (invalid passphrase, already exists, etc.)
/// - `Unauthorized` - Permission denied (not logged in, not authorized)
/// - `Io` - File system operations
/// - `Internal` - Unexpected errors (Loro, crypto libraries, etc.)
#[derive(Error, Debug)]
pub enum ButlerError {
    /// Resource not found
    ///
    /// Examples: "space: abc123", "page: xyz789", "layer: messages", "asset: hash123"
    #[error("Not found: {0}")]
    NotFound(String),

    /// Database/storage operation failed
    ///
    /// Wraps redb errors and other storage failures
    #[error("Database error: {0}")]
    Database(String),

    /// Serialization/deserialization failed
    ///
    /// JSON parsing, bincode encoding, etc.
    #[error("Serialization error: {0}")]
    Serialization(String),

    /// Encryption/decryption operation failed
    ///
    /// Key derivation, encrypt, decrypt, sign, verify
    #[error("Encryption error: {0}")]
    Encryption(String),

    /// Invalid input or state
    ///
    /// Examples: "already signed up", "invalid passphrase", "invalid permit"
    #[error("Invalid: {0}")]
    Invalid(String),

    /// Authorization failed
    ///
    /// Examples: "not logged in", "permit required"
    #[error("Unauthorized: {0}")]
    Unauthorized(String),

    /// File system operation failed
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Internal/unexpected error
    ///
    /// Loro errors, crypto library errors, etc.
    #[error("Internal error: {0}")]
    Internal(String),
}

pub type Result<T> = std::result::Result<T, ButlerError>;

// Error Conversions

impl From<LayerError> for ButlerError {
    fn from(err: LayerError) -> Self {
        ButlerError::Internal(format!("Layer: {}", err))
    }
}

impl From<redb::DatabaseError> for ButlerError {
    fn from(err: redb::DatabaseError) -> Self {
        ButlerError::Database(err.to_string())
    }
}

impl From<redb::TransactionError> for ButlerError {
    fn from(err: redb::TransactionError) -> Self {
        ButlerError::Database(format!("Transaction: {}", err))
    }
}

impl From<redb::TableError> for ButlerError {
    fn from(err: redb::TableError) -> Self {
        ButlerError::Database(format!("Table: {}", err))
    }
}

impl From<redb::CommitError> for ButlerError {
    fn from(err: redb::CommitError) -> Self {
        ButlerError::Database(format!("Commit: {}", err))
    }
}

impl From<redb::StorageError> for ButlerError {
    fn from(err: redb::StorageError) -> Self {
        ButlerError::Database(format!("Storage: {}", err))
    }
}

// Helper Constructors for Common Cases

impl ButlerError {
    /// Create a NotFound error for a space
    pub fn space_not_found(id: &str) -> Self {
        ButlerError::NotFound(format!("space: {}", id))
    }

    /// Create a NotFound error for a page
    pub fn page_not_found(id: &str) -> Self {
        ButlerError::NotFound(format!("page: {}", id))
    }

    /// Create a NotFound error for a layer
    pub fn layer_not_found(name: &str) -> Self {
        ButlerError::NotFound(format!("layer: {}", name))
    }

    /// Create a NotFound error for an asset
    pub fn asset_not_found(hash: &str) -> Self {
        ButlerError::NotFound(format!("asset: {}", hash))
    }

    /// Create a NotFound error for a permit
    pub fn permit_not_found(id: &str) -> Self {
        ButlerError::NotFound(format!("permit: {}", id))
    }

    /// Create an Invalid error for permit parsing
    pub fn permit_error(msg: impl Into<String>) -> Self {
        ButlerError::Invalid(format!("permit: {}", msg.into()))
    }

    /// Create an Unauthorized error for not being logged in
    pub fn not_logged_in() -> Self {
        ButlerError::Unauthorized("not logged in".to_string())
    }

    /// Create an Invalid error for already signed up
    pub fn already_signed_up() -> Self {
        ButlerError::Invalid("already signed up".to_string())
    }

    /// Create an Invalid error for not signed up
    pub fn not_signed_up() -> Self {
        ButlerError::Invalid("not signed up".to_string())
    }

    /// Create an Invalid error for invalid passphrase
    pub fn invalid_passphrase() -> Self {
        ButlerError::Invalid("invalid passphrase".to_string())
    }

    /// Create a verification error
    pub fn verification_failed(msg: impl Into<String>) -> Self {
        ButlerError::Invalid(format!("verification: {}", msg.into()))
    }

    /// Create a Loro/CRDT error
    pub fn loro_error(msg: impl Into<String>) -> Self {
        ButlerError::Internal(format!("Loro: {}", msg.into()))
    }

    /// Create a crypto error
    pub fn crypto_error(msg: impl Into<String>) -> Self {
        ButlerError::Encryption(msg.into())
    }
}
