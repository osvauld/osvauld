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

    #[error("User already signed up")]
    AlreadySignedUp,

    #[error("User not signed up")]
    NotSignedUp,

    #[error("Invalid passphrase")]
    InvalidPassphrase,
}

pub type Result<T> = std::result::Result<T, ButlerError>;
