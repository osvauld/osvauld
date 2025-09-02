use crypto_utils::errors::CryptoError;
use osvauld_core::repositories::RepositoryError;
use services::ServiceError;
use thiserror::Error;

/// Type alias for P2P operation results
pub type P2PResult<T> = Result<T, P2PError>;

/// Main P2P error enum encompassing all P2P-related errors
#[derive(Error, Debug)]
pub enum P2PError {
    #[error("Connection error: {0}")]
    Connection(#[from] ConnectionError),

    #[error("Handshake error: {0}")]
    Handshake(#[from] HandshakeError),

    #[error("Sync error: {0}")]
    Sync(#[from] SyncError),

    #[error("Message error: {0}")]
    Message(#[from] MessageError),

    #[error("Device sync error: {0}")]
    DeviceSync(#[from] DeviceSyncError),

    #[error("Resource sync error: {0}")]
    ResourceSync(#[from] ResourceSyncError),

    #[error("Service layer error: {0}")]
    Service(#[from] ServiceError),

    #[error("Cryptographic operation failed: {0}")]
    Crypto(#[from] CryptoError),

    #[error("Repository operation failed: {0}")]
    Repository(#[from] RepositoryError),

    #[error("P2P service not initialized")]
    NotInitialized,

    #[error("Configuration error: {0}")]
    Configuration(String),

    #[error("Invalid state: {0}")]
    InvalidState(String),

    #[error("Custom error: {0}")]
    Custom(String),
}

/// Connection-related errors
#[derive(Error, Debug)]
pub enum ConnectionError {
    #[error("Failed to establish connection to {node_id}: {reason}")]
    EstablishmentFailed { node_id: String, reason: String },

    #[error("Connection already exists: {connection_id}")]
    AlreadyExists { connection_id: String },

    #[error("Connection not found: {connection_id}")]
    NotFound { connection_id: String },

    #[error("Connection closed: {connection_id}")]
    Closed { connection_id: String },

    #[error("Connection timeout after {seconds} seconds for {connection_id}")]
    Timeout { seconds: u64, connection_id: String },

    #[error("Invalid node ID: {node_id}")]
    InvalidNodeId { node_id: String },

    #[error("Endpoint error: {0}")]
    Endpoint(String),

    #[error("Already connecting to {connection_id}")]
    AlreadyConnecting { connection_id: String },

    #[error("Failed to open bidirectional stream: {reason}")]
    StreamOpenFailed { reason: String },

    #[error("Remote node ID error: {0}")]
    RemoteNodeIdError(String),
}

/// Handshake protocol errors
#[derive(Error, Debug)]
pub enum HandshakeError {
    #[error("Invalid credentials for user {user_id}")]
    InvalidCredentials { user_id: String },

    #[error("Signature verification failed for user {user_id}")]
    SignatureVerificationFailed { user_id: String },

    #[error("UCAN token validation failed: {reason}")]
    UcanValidationFailed { reason: String },

    #[error("Failed to issue UCAN token: {reason}")]
    UcanIssuanceFailed { reason: String },

    #[error("Challenge validation failed")]
    ChallengeValidationFailed,

    #[error("Handshake timeout after {seconds} seconds")]
    Timeout { seconds: u64 },

    #[error("Handshake not complete")]
    NotComplete,

    #[error("Invalid connection type: expected {expected}, got {actual}")]
    InvalidConnectionType { expected: String, actual: String },

    #[error("Missing peer information")]
    MissingPeerInfo,

    #[error("User not found for device {device_id}")]
    UserNotFound { device_id: String },

    #[error("Device not found: {device_id}")]
    DeviceNotFound { device_id: String },

    #[error("Invalid handshake message type")]
    InvalidMessageType,
}

/// Synchronization errors
#[derive(Error, Debug)]
pub enum SyncError {
    #[error("Resource sync failed for {resource_id}: {reason}")]
    ResourceSyncFailed { resource_id: String, reason: String },

    #[error("Device sync failed: {reason}")]
    DeviceSyncFailed { reason: String },

    #[error("User sync failed: {reason}")]
    UserSyncFailed { reason: String },

    #[error("Manifest generation failed: {reason}")]
    ManifestGenerationFailed { reason: String },

    #[error("Manifest comparison failed: {reason}")]
    ManifestComparisonFailed { reason: String },

    #[error("Manifest not found for {entity_type}")]
    ManifestNotFound { entity_type: String },

    #[error("State vector mismatch for resource {resource_id}")]
    StateVectorMismatch { resource_id: String },

    #[error("Update authority validation failed for resource {resource_id}")]
    UpdateAuthorityFailed { resource_id: String },

    #[error("Vector clock merge failed for resource {resource_id}: {reason}")]
    VectorClockMergeFailed { resource_id: String, reason: String },

    #[error("Share record sync failed: {reason}")]
    ShareRecordSyncFailed { reason: String },

    #[error("Resource not found: {resource_id}")]
    ResourceNotFound { resource_id: String },

    #[error("Invalid sync state")]
    InvalidSyncState,
}

/// Message handling errors
#[derive(Error, Debug)]
pub enum MessageError {
    #[error("Failed to serialize message")]
    SerializationFailed(#[from] serde_json::Error),

    #[error("Failed to deserialize message: {0}")]
    DeserializationFailed(String),

    #[error("Failed to send message: {reason}")]
    SendFailed { reason: String },

    #[error("Failed to receive message: {reason}")]
    ReceiveFailed { reason: String },

    #[error("Invalid message type received")]
    InvalidMessageType,

    #[error("Message too large: {size} bytes exceeds maximum of {max}")]
    MessageTooLarge { size: usize, max: usize },

    #[error("Stream error: {0}")]
    StreamError(String),

    #[error("Message handler error: {0}")]
    HandlerError(String),

    #[error("Unexpected end of stream")]
    UnexpectedEndOfStream,
}
#[derive(Error, Debug)]
pub enum DeviceSyncError {
    #[error("Failed to retrieve device manifest")]
    ManifestRetrievalFailed,

    #[error("Failed to send device manifest request")]
    ManifestRequestSendFailed,

    #[error("Failed to process device manifest request")]
    ManifestRequestProcessingFailed,

    #[error("Failed to send device manifest response")]
    ManifestResponseSendFailed,

    #[error("Failed to send device manifest acknowledgment")]
    ManifestAckSendFailed,

    #[error("Device manifest comparison not available")]
    ManifestComparisonNotAvailable,

    #[error("Failed to create network sync payload")]
    NetworkSyncPayloadCreationFailed,

    #[error("Failed to send device network sync")]
    NetworkSyncSendFailed,

    #[error("Failed to process device network sync")]
    NetworkSyncProcessingFailed,

    #[error("Failed to send device network sync acknowledgment")]
    NetworkSyncAckSendFailed,
}
#[derive(Error, Debug)]
pub enum ResourceSyncError {
    #[error("Failed to get resource {resource_id} for remote addition")]
    ResourceRetrievalFailed { resource_id: String },

    #[error("Failed to send resource addition request for {resource_id}")]
    ResourceAdditionRequestFailed { resource_id: String },

    #[error("Failed to add resource {resource_id} to local repository")]
    ResourceAdditionFailed { resource_id: String },

    #[error("Failed to get state vector for resource {resource_id}")]
    StateVectorRetrievalFailed { resource_id: String },

    #[error("Failed to send state vector request for {resource_id}")]
    StateVectorRequestFailed { resource_id: String },

    #[error("Invalid update authority for resource {resource_id}")]
    InvalidUpdateAuthority { resource_id: String },

    #[error("Failed to generate updates for resource {resource_id}")]
    UpdateGenerationFailed { resource_id: String },

    #[error("Failed to send updates response for {resource_id}")]
    UpdatesResponseFailed { resource_id: String },

    #[error("Failed to apply updates for resource {resource_id}")]
    UpdateApplicationFailed { resource_id: String },

    #[error("Failed to get share records for resource {resource_id}")]
    ShareRecordsRetrievalFailed { resource_id: String },

    #[error("Failed to merge share records for resource {resource_id}")]
    ShareRecordsMergeFailed { resource_id: String },

    #[error("Failed to get vector clocks for resource {resource_id}")]
    VectorClocksRetrievalFailed { resource_id: String },

    #[error("Failed to merge vector clocks for resource {resource_id}")]
    VectorClocksMergeFailed { resource_id: String },

    #[error("Failed to update vector clocks")]
    VectorClocksUpdateFailed,

    #[error("Failed to send final update merge for {resource_id}")]
    FinalUpdateMergeFailed { resource_id: String },

    #[error("Failed to send vector clock response for {resource_id}")]
    VectorClockResponseFailed { resource_id: String },

    #[error("Failed to get resource keys for {resource_id}")]
    ResourceKeysRetrievalFailed { resource_id: String },

    #[error("Failed to add resource keys")]
    ResourceKeysAdditionFailed,

    #[error("Failed to get UCAN key for resource {resource_id}")]
    UcanKeyRetrievalFailed { resource_id: String },

    #[error("Failed to send resource addition complete message")]
    ResourceAdditionCompleteFailed,

    #[error("Device manifest not available for resource operations")]
    DeviceManifestNotAvailable,

    #[error("User manifest not available for resource operations")]
    UserManifestNotAvailable,
}

// Conversion implementations for iroh errors
impl From<iroh::endpoint::ConnectionError> for P2PError {
    fn from(err: iroh::endpoint::ConnectionError) -> Self {
        P2PError::Connection(ConnectionError::Endpoint(err.to_string()))
    }
}

impl From<iroh_quinn::WriteError> for P2PError {
    fn from(err: iroh_quinn::WriteError) -> Self {
        P2PError::Message(MessageError::SendFailed {
            reason: err.to_string(),
        })
    }
}

impl From<iroh_quinn::ReadError> for P2PError {
    fn from(err: iroh_quinn::ReadError) -> Self {
        P2PError::Message(MessageError::ReceiveFailed {
            reason: err.to_string(),
        })
    }
}
