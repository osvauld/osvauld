use crate::models::LayerError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ButlerError {
    #[error("Storage error: {0}")]
    Storage(String),

    #[error("Database error: {0}")]
    Database(#[from] redb::DatabaseError),

    #[error("Transaction error: {0}")]
    Transaction(#[from] redb::TransactionError),

    #[error("Table error: {0}")]
    Table(#[from] redb::TableError),

    #[error("Commit error: {0}")]
    Commit(#[from] redb::CommitError),

    #[error("Storage error: {0}")]
    StorageError(#[from] redb::StorageError),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("Encryption error: {0}")]
    Encryption(String),

    #[error("Layer not found: {0}")]
    LayerNotFound(String),

    #[error("Space not found: {0}")]
    SpaceNotFound(String),

    #[error("Page not found: {0}")]
    PageNotFound(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Loro error: {0}")]
    Loro(String),

    #[error("Layer error: {0}")]
    Layer(String),

    #[error("Crypto error: {0}")]
    Crypto(String),

    #[error("Permit not found: {0}")]
    PermitNotFound(String),

    #[error("Permit error: {0}")]
    Permit(String),

    #[error("User already signed up")]
    AlreadySignedUp,

    #[error("User not signed up")]
    NotSignedUp,

    #[error("Invalid passphrase")]
    InvalidPassphrase,

    #[error("Not logged in")]
    NotLoggedIn,

    #[error("Permit error: {0}")]
    PermitError(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Asset not found: {0}")]
    AssetNotFound(String),
}

pub type Result<T> = std::result::Result<T, ButlerError>;

impl From<LayerError> for ButlerError {
    fn from(err: LayerError) -> Self {
        ButlerError::Layer(err.to_string())
    }
}
