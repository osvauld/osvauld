use std::fmt;

#[derive(Debug)]
pub enum AppError {
    StorageError(String),
    WebSocketError(String),
    SerializationError(String),
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppError::StorageError(msg) => write!(f, "Storage error: {}", msg),
            AppError::WebSocketError(msg) => write!(f, "WebSocket error: {}", msg),
            AppError::SerializationError(msg) => write!(f, "Serialization error: {}", msg),
        }
    }
}

impl std::error::Error for AppError {}

impl From<serde_json::Error> for AppError {
    fn from(err: serde_json::Error) -> Self {
        AppError::SerializationError(err.to_string())
    }
}

impl From<sled::Error> for AppError {
    fn from(err: sled::Error) -> Self {
        AppError::StorageError(format!("Sled error: {}", err))
    }
}
