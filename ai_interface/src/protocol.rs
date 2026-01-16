//! Protocol types for debug server communication
//!
//! JSON-RPC over Unix socket protocol shared between test harness and debug console.

use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

/// Command sent to debug server
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "method", content = "params")]
pub enum DebugCommand {
    /// Execute Lua code and return result
    #[serde(rename = "eval")]
    Eval { code: String },

    /// Get state snapshot
    #[serde(rename = "state")]
    State,

    /// Get logs (with optional filters)
    #[serde(rename = "logs")]
    Logs {
        #[serde(default)]
        last: Option<usize>,
        #[serde(default)]
        offset: Option<usize>,
        #[serde(default)]
        count: Option<usize>,
        #[serde(default)]
        instance: Option<String>,
        #[serde(default)]
        level: Option<String>,
        #[serde(default)]
        search: Option<String>,
    },

    /// Subscribe to log stream
    #[serde(rename = "subscribe_logs")]
    SubscribeLogs {
        #[serde(default)]
        level: Option<String>,
    },

    /// UI automation: click element by accessible-label
    #[serde(rename = "ui_click")]
    UiClick { label: String },

    /// UI automation: type text into element
    #[serde(rename = "ui_type")]
    UiType { label: String, text: String },

    /// UI automation: get text from element
    #[serde(rename = "ui_get_text")]
    UiGetText { label: String },

    /// Ping (health check)
    #[serde(rename = "ping")]
    Ping,
}

/// Response from debug server
#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum DebugResponse {
    /// Success result
    Success { result: JsonValue, id: Option<u64> },
    /// Error result
    Error { error: String, id: Option<u64> },
    /// Log stream entry
    LogStream { stream: String, data: LogEntry },
}

/// Log entry from an instance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    /// Timestamp (RFC3339)
    pub ts: String,
    /// Log level
    pub level: String,
    /// Target (module path)
    pub target: String,
    /// Message
    pub msg: String,
    /// Instance name (owner, customer1, kunki, etc.)
    #[serde(default)]
    pub instance: Option<String>,
}

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
