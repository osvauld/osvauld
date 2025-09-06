use crypto_utils::errors::CryptoError;
use osvauld_core::repositories::RepositoryError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ServiceError {
    #[error("Authentication error: {0}")]
    Auth(#[from] AuthServiceError),

    #[error("Resource error: {0}")]
    Resource(#[from] ResourceServiceError),

    #[error("Folder error: {0}")]
    Folder(#[from] FolderServiceError),

    #[error("User error: {0}")]
    User(#[from] UserServiceError),

    #[error("Sync error: {0}")]
    Sync(#[from] SyncServiceError),

    #[error("Crypto error: {0}")]
    Crypto(#[from] CryptoError),

    #[error("Repository error: {0}")]
    Repository(#[from] RepositoryError),

    #[error("Validation error in {field}: {reason}")]
    Validation { field: String, reason: String },

    #[error("Internal error: {message}")]
    Internal { message: String },
}

#[derive(Error, Debug)]
pub enum AuthServiceError {
    #[error("Invalid credentials provided")]
    InvalidCredentials,

    #[error("User already exists: {username}")]
    UserAlreadyExists { username: String },

    #[error("User not found: {user_id}")]
    UserNotFound { user_id: String },

    #[error("Device not found: {device_id}")]
    DeviceNotFound { device_id: String },

    #[error("Certificate not found")]
    CertificateNotFound,

    #[error("Invalid passphrase")]
    InvalidPassphrase,

    #[error("Challenge generation failed")]
    ChallengeGenerationFailed,

    #[error("UCAN token generation failed: {reason}")]
    UcanTokenGenerationFailed { reason: String },

    #[error("Certificate import failed: {reason}")]
    CertificateImportFailed { reason: String },

    #[error("Certificate export failed: {reason}")]
    CertificateExportFailed { reason: String },

    #[error("Password change failed: {reason}")]
    PasswordChangeFailed { reason: String },

    #[error("User signup failed: {reason}")]
    SignupFailed { reason: String },

    #[error("Certificate loading failed: {reason}")]
    CertificateLoadingFailed { reason: String },

    #[error("Already signed up")]
    AlreadySignedUp,

    #[error("Not signed up")]
    NotSignedUp,

    #[error(transparent)]
    Crypto(#[from] CryptoError),

    #[error(transparent)]
    Repository(#[from] RepositoryError),
}

#[derive(Error, Debug)]
pub enum ResourceServiceError {
    #[error("Parse error: {0}")]
    ParseError(String),

    #[error("Resource not found: {resource_id}")]
    ResourceNotFound { resource_id: String },

    #[error("Invalid resource type: {resource_type}")]
    InvalidResourceType { resource_type: String },

    #[error("Resource access denied: {resource_id}")]
    AccessDenied { resource_id: String },

    #[error("Resource already exists: {resource_id}")]
    ResourceAlreadyExists { resource_id: String },

    #[error("Share operation failed: {reason}")]
    ShareFailed { reason: String },

    #[error("Sync operation failed for resource {resource_id}: {reason}")]
    SyncFailed { resource_id: String, reason: String },

    #[error("Vector clock merge failed: {reason}")]
    VectorClockMergeFailed { reason: String },

    #[error("Update authority validation failed: {reason}")]
    AuthorityValidationFailed { reason: String },

    #[error(transparent)]
    Crypto(#[from] CryptoError),

    #[error(transparent)]
    Repository(#[from] RepositoryError),
}

#[derive(Error, Debug)]
pub enum FolderServiceError {
    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Folder not found: {folder_id}")]
    FolderNotFound { folder_id: String },

    #[error("Folder name cannot be empty")]
    EmptyFolderName,

    #[error("Folder already exists: {name}")]
    FolderAlreadyExists { name: String },

    #[error("Cannot delete default folder")]
    CannotDeleteDefaultFolder,

    #[error("Folder contains resources and cannot be deleted")]
    FolderNotEmpty,

    #[error("PermissionDenied")]
    InsufficientPermissions,
    #[error(transparent)]
    Repository(#[from] RepositoryError),
}

#[derive(Error, Debug)]
pub enum UserServiceError {
    #[error("User not found: {user_id}")]
    UserNotFound { user_id: String },

    #[error("Device not found: {device_id}")]
    DeviceNotFound { device_id: String },

    #[error("Invalid user data: {field}: {reason}")]
    InvalidUserData { field: String, reason: String },

    #[error("User already connected: {user_id}")]
    UserAlreadyConnected { user_id: String },

    #[error("UCAN token invalid or expired")]
    InvalidUcanToken,

    #[error("Signature verification failed")]
    SignatureVerificationFailed,

    #[error("User connection failed: {reason}")]
    ConnectionFailed { reason: String },

    #[error("Permission denied for operation: {operation}")]
    PermissionDenied { operation: String },

    #[error("Failed to issue UCAN token: {reason}")]
    UcanTokenIssueFailed { reason: String },

    #[error("Failed to sign UCAN public key: {reason}")]
    UcanSigningFailed { reason: String },

    #[error(transparent)]
    Crypto(#[from] CryptoError),

    #[error(transparent)]
    Repository(#[from] RepositoryError),
}

#[derive(Error, Debug)]
pub enum SyncServiceError {
    #[error("Device manifest generation failed: {reason}")]
    DeviceManifestFailed { reason: String },

    #[error("User manifest generation failed: {reason}")]
    UserManifestFailed { reason: String },

    #[error("Resource sync failed for {resource_id}: {reason}")]
    ResourceSyncFailed { resource_id: String, reason: String },

    #[error("Network sync failed: {reason}")]
    NetworkSyncFailed { reason: String },

    #[error("Vector clock operation failed: {reason}")]
    VectorClockFailed { reason: String },

    #[error("Share record sync failed: {reason}")]
    ShareRecordSyncFailed { reason: String },

    #[error("Device sync failed: {reason}")]
    DeviceSyncFailed { reason: String },

    #[error("User sync failed: {reason}")]
    UserSyncFailed { reason: String },

    #[error("Folder sync failed: {reason}")]
    FolderSyncFailed { reason: String },

    #[error("UCAN token delegation failed: {reason}")]
    UcanDelegationFailed { reason: String },

    #[error("Default folder not found")]
    DefaultFolderNotFound,

    #[error(transparent)]
    Crypto(#[from] CryptoError),

    #[error(transparent)]
    Repository(#[from] RepositoryError),
}

impl ServiceError {
    pub fn validation(field: &str, reason: &str) -> Self {
        ServiceError::Validation {
            field: field.to_string(),
            reason: reason.to_string(),
        }
    }

    pub fn internal(message: &str) -> Self {
        ServiceError::Internal {
            message: message.to_string(),
        }
    }
}

pub type ServiceResult<T> = Result<T, ServiceError>;
