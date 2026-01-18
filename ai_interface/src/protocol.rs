//! Protocol types for AI interface communication
//!
//! JSON-RPC style protocol for routing commands to debug servers.

use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

/// Request wrapper with id for JSON-RPC style
#[derive(Debug, Serialize, Deserialize)]
pub struct Request {
    /// Target instance (e.g., "owner", "customer1", "kunki", "all")
    #[serde(default)]
    pub target: Option<String>,
    /// Action to perform
    pub action: String,
    /// Additional parameters
    #[serde(default)]
    pub params: JsonValue,
    /// Request ID for response correlation
    #[serde(default)]
    pub id: Option<u64>,
}

/// Response wrapper
#[derive(Debug, Serialize, Deserialize)]
pub struct Response {
    /// Result on success
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<JsonValue>,
    /// Error message on failure
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Target instance that responded
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instance: Option<String>,
    /// Request ID for correlation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<u64>,
}

impl Response {
    pub fn success(result: JsonValue, instance: Option<String>, id: Option<u64>) -> Self {
        Self {
            result: Some(result),
            error: None,
            instance,
            id,
        }
    }

    pub fn error(error: String, instance: Option<String>, id: Option<u64>) -> Self {
        Self {
            result: None,
            error: Some(error),
            instance,
            id,
        }
    }
}
