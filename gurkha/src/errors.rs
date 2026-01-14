//! Error types for Gurkha Permit domain

use thiserror::Error;

/// Gurkha error type
#[derive(Error, Debug)]
pub enum GurkhaError {
    #[error("Permit error: {0}")]
    Permit(String),

    #[error("Invalid token: {0}")]
    InvalidToken(String),

    #[error("Invalid capability: {0}")]
    InvalidCapability(String),

    #[error("Capability not found")]
    CapabilityNotFound,

    #[error("Keys not loaded - please login first")]
    KeysNotLoaded,

    #[error("Invalid permit: {0}")]
    InvalidPermit(String),

    #[error("Invalid template: {0}")]
    InvalidTemplate(String),

    #[error("Parsing error: {0}")]
    ParseError(String),

    #[error("Validation error: {0}")]
    ValidationError(String),

    #[error("Signing error: {0}")]
    SigningError(String),

    #[error("Creation error: {0}")]
    CreationError(String),

    #[error("Signature error: {0}")]
    SignatureError(String),

    #[error("Permit CID conversion failed: {0}")]
    PermitCidConversionFailed(String),

    #[error("Encoding error: {0}")]
    EncodingError(String),

    #[error("Format error: {0}")]
    FormatError(String),

    #[error("Missing template: {0}")]
    MissingTemplate(String),

    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("Base64 decode error: {0}")]
    Base64Error(#[from] base64::DecodeError),

    #[error("UTF-8 error: {0}")]
    Utf8Error(#[from] std::string::FromUtf8Error),

    #[error("Other error: {0}")]
    Other(String),
}

/// Service-level result type (compatible with service layer)
pub type ServiceError = GurkhaError;
pub type ServiceResult<T> = std::result::Result<T, ServiceError>;

/// Result type for Gurkha operations
pub type Result<T> = std::result::Result<T, GurkhaError>;

/// Convert PermitError to GurkhaError
impl From<crate::parser::PermitError> for GurkhaError {
    fn from(err: crate::parser::PermitError) -> Self {
        match err {
            crate::parser::PermitError::InvalidTokenType(msg) => GurkhaError::InvalidToken(msg),
            crate::parser::PermitError::ParsingFailed(msg) => GurkhaError::ParseError(msg),
            crate::parser::PermitError::MissingField(msg) => GurkhaError::ParseError(format!("Missing field: {}", msg)),
            crate::parser::PermitError::ValidationFailed(msg) => GurkhaError::ValidationError(msg),
        }
    }
}
