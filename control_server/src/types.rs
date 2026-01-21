//! Common types for control server protocol

use serde::{Deserialize, Serialize};

/// JSON-RPC style request
#[derive(Debug, Clone, Deserialize)]
pub struct Request {
    /// Method name (e.g., "ping", "eval", "login")
    pub method: String,
    /// Optional parameters
    #[serde(default)]
    pub params: Option<serde_json::Value>,
    /// Request ID for correlation
    pub id: u64,
}

/// JSON-RPC style response
#[derive(Debug, Clone, Serialize)]
pub struct Response {
    /// Result on success
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    /// Error on failure
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorResponse>,
    /// Request ID for correlation
    pub id: u64,
}

impl Response {
    /// Create a success response
    pub fn ok<T: Serialize>(id: u64, result: T) -> Self {
        Self {
            result: Some(serde_json::to_value(result).unwrap_or(serde_json::Value::Null)),
            error: None,
            id,
        }
    }

    /// Create an error response
    pub fn err(id: u64, code: i32, message: impl Into<String>) -> Self {
        Self {
            result: None,
            error: Some(ErrorResponse {
                code,
                message: message.into(),
            }),
            id,
        }
    }

    /// Create a simple success response with status: "ok"
    pub fn ok_status(id: u64) -> Self {
        Self::ok(id, serde_json::json!({"status": "ok"}))
    }
}

/// Error response structure
#[derive(Debug, Clone, Serialize)]
pub struct ErrorResponse {
    /// Error code
    pub code: i32,
    /// Error message
    pub message: String,
}

/// Common error codes
pub mod error_codes {
    pub const PARSE_ERROR: i32 = -32700;
    pub const INVALID_REQUEST: i32 = -32600;
    pub const METHOD_NOT_FOUND: i32 = -32601;
    pub const INVALID_PARAMS: i32 = -32602;
    pub const INTERNAL_ERROR: i32 = -32603;

    // Application-specific errors (-32000 to -32099)
    pub const NOT_AUTHENTICATED: i32 = -32001;
    pub const NOT_FOUND: i32 = -32002;
    pub const PERMISSION_DENIED: i32 = -32003;
    pub const OPERATION_FAILED: i32 = -32004;
}
