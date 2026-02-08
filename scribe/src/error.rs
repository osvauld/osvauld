//! Scribe error types
//!
//! Simple error types for the Scribe crate.

use std::fmt;

/// Scribe error type
#[derive(Debug)]
pub enum ScribeError {
    /// Layer not found
    LayerNotFound(String),
    /// CRDT operation failed
    CrdtError(String),
    /// Validation failed
    ValidationError(String),
    /// Permission denied
    PermissionDenied(String),
    /// Permit parsing/validation error
    PermitError(String),
    /// IO error
    IoError(String),
    /// General error
    Other(String),
}

impl fmt::Display for ScribeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ScribeError::LayerNotFound(name) => write!(f, "Layer not found: {}", name),
            ScribeError::CrdtError(msg) => write!(f, "CRDT error: {}", msg),
            ScribeError::ValidationError(msg) => write!(f, "Validation error: {}", msg),
            ScribeError::PermissionDenied(msg) => write!(f, "Permission denied: {}", msg),
            ScribeError::PermitError(msg) => write!(f, "Permit error: {}", msg),
            ScribeError::IoError(msg) => write!(f, "IO error: {}", msg),
            ScribeError::Other(msg) => write!(f, "{}", msg),
        }
    }
}

impl std::error::Error for ScribeError {}

impl From<String> for ScribeError {
    fn from(s: String) -> Self {
        ScribeError::Other(s)
    }
}

impl From<&str> for ScribeError {
    fn from(s: &str) -> Self {
        ScribeError::Other(s.to_string())
    }
}

impl From<std::io::Error> for ScribeError {
    fn from(e: std::io::Error) -> Self {
        ScribeError::IoError(e.to_string())
    }
}

impl From<loro::LoroError> for ScribeError {
    fn from(e: loro::LoroError) -> Self {
        ScribeError::CrdtError(e.to_string())
    }
}

/// Result type for Scribe operations
pub type Result<T> = std::result::Result<T, ScribeError>;
