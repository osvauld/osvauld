use thiserror::Error;
/// Comprehensive error type for P2P operations
#[derive(Error, Debug)]
pub enum P2PError {
    #[error("Failed to initialize P2P service: {0}")]
    Initialization(String),

    #[error("Connection error: {0}")]
    Connection(String),

    #[error("Handshake error: {0}")]
    Handshake(#[from] HandshakeError),

    #[error("Message error: {0}")]
    Message(String),

    #[error("Peer connection error: {0}")]
    PeerConnection(String),

    #[error("Authentication service error: {0}")]
    AuthService(String),

    #[error("User service error: {0}")]
    UserService(String),

    #[error("Sync service error: {0}")]
    SyncService(String),

    #[error("Share service error: {0}")]
    ShareService(String),

    #[error("Timeout error: {0}")]
    Timeout(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("Deserialization error: {0}")]
    Deserialization(String),

    #[error("P2P service not initialized")]
    NotInitialized,

    #[error("P2P service not configured: {0}")]
    Configuration(String),
}

#[derive(Error, Debug)]
pub enum HandshakeError {
    #[error("Invalid signature: {0}")]
    InvalidSignature(String),

    #[error("signature failure: {0}")]
    SignatureFailure(String),
    #[error("Invalid challenge: {0}")]
    InvalidChallenge(String),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("Auth service error: {0}")]
    AuthService(String),

    #[error("Connection error: {0}")]
    Connection(String),

    #[error("Timeout error: {0}")]
    Timeout(String),

    #[error("Deserialization error: {0}")]
    Deserialization(#[from] serde_json::Error),
}
// Convert String errors to P2PError for backward compatibility
impl From<String> for P2PError {
    fn from(error: String) -> Self {
        P2PError::Message(error)
    }
}

// Convert &str errors to P2PError for backward compatibility
impl From<&str> for P2PError {
    fn from(error: &str) -> Self {
        P2PError::Message(error.to_string())
    }
}
impl From<P2PError> for String {
    fn from(error: P2PError) -> Self {
        error.to_string()
    }
}
