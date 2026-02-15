//! UI Types - Serializable UI mutations
//!
//! Types for sending mutations from Lua runtime to UI layer.
//!
//! **Threading**: These types are Send + Clone, safe for mpsc channels
//! **Serializable**: All fields are JSON-compatible for cross-thread communication

use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

/// Operations for VecModel mutation
///
/// **Usage**: Lua runtime computes these, UI thread applies them to VecModels
/// **Serializable**: All fields are JSON-compatible for cross-thread communication
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "op")]
pub enum VecModelOp {
    /// Append item to end of model
    ///
    /// **Triggers**: ModelNotify::row_added(model.len())
    Push { model_name: String, item: JsonValue },

    /// Insert item at specific index
    ///
    /// **Triggers**: ModelNotify::row_added(index)
    Insert {
        model_name: String,
        index: usize,
        item: JsonValue,
    },

    /// Remove item at index
    ///
    /// **Triggers**: ModelNotify::row_removed(index)
    Remove { model_name: String, index: usize },

    /// Update item at index
    ///
    /// **Triggers**: ModelNotify::row_changed(index)
    Set {
        model_name: String,
        index: usize,
        item: JsonValue,
    },

    /// Remove all items from model
    ///
    /// **Triggers**: Multiple row_removed events
    Clear { model_name: String },

    /// Replace all items in model atomically
    ///
    /// **Triggers**: Single reset event (more efficient than Clear + Push)
    /// **Usage**: For complete array replacement without flickering
    Replace {
        model_name: String,
        items: Vec<JsonValue>,
    },
}

/// UI mutation message (Lua runtime -> UI thread)
///
/// **Channel**: mpsc::Sender<UiMutation> from Lua runtime
/// **Receiver**: UI layer's process_ui_mutations() on main thread
#[derive(Debug, Clone)]
pub struct UiMutation {
    /// App identifier (for multi-app scenarios)
    pub app_id: String,

    /// Scalar property updates (e.g., "draft", "message_count")
    pub properties: Vec<PropertyUpdate>,

    /// VecModel operations (fine-grained array updates)
    pub model_ops: Vec<VecModelOp>,
}

/// Scalar property update
///
/// **Usage**: Update non-array properties (strings, numbers, bools)
/// **Applied via**: ComponentInstance::set_property()
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PropertyUpdate {
    pub key: String,
    pub value: JsonValue,
}

/// Query for reading UI property values (Lua -> UI -> Lua)
///
/// **Usage**: Lua calls ui:get("property_name"), blocks for response
/// **Threading**: Sent via channel, response via oneshot
#[derive(Debug)]
pub struct UiQuery {
    /// Property name to read
    pub prop_name: String,
    /// Response channel (UI sends value back)
    pub response_tx: tokio::sync::oneshot::Sender<Option<JsonValue>>,
}

impl UiMutation {
    /// Create new mutation
    pub fn new(
        app_id: String,
        properties: Vec<PropertyUpdate>,
        model_ops: Vec<VecModelOp>,
    ) -> Self {
        Self {
            app_id,
            properties,
            model_ops,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vecmodel_op_serialization() {
        let op = VecModelOp::Push {
            model_name: "messages".to_string(),
            item: serde_json::json!({
                "id": "msg_1",
                "content": "Hello"
            }),
        };

        let json = serde_json::to_string(&op).unwrap();
        let deserialized: VecModelOp = serde_json::from_str(&json).unwrap();

        match deserialized {
            VecModelOp::Push { model_name, item } => {
                assert_eq!(model_name, "messages");
                assert_eq!(item["id"], "msg_1");
            }
            _ => panic!("Wrong variant"),
        }
    }
}
