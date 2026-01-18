//! VecModel Operations - Serializable UI mutations
//!
//! Operations sent from Lua worker threads to Slint thread via channels.
//!
//! **Pattern**: Similar to LoroChangeEvent in butler/scribe.rs
//! **Threading**: These types are Send + Clone, safe for mpsc channels
//! **Memory**: Operations contain JSON values, not Slint types (serializable)

use serde::{Serialize, Deserialize};
use serde_json::Value as JsonValue;

/// Operations for VecModel mutation
///
/// **Usage**: Lua worker computes these, Slint thread applies them to VecModels
/// **Serializable**: All fields are JSON-compatible for cross-thread communication
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "op")]
pub enum VecModelOp {
    /// Append item to end of model
    ///
    /// **Triggers**: ModelNotify::row_added(model.len())
    Push {
        model_name: String,
        item: JsonValue,
    },

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
    Remove {
        model_name: String,
        index: usize,
    },

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
    Clear {
        model_name: String,
    },
}

/// UI mutation message (Lua thread → Slint thread)
///
/// **Channel**: mpsc::Sender<UiMutation> from Lua worker
/// **Receiver**: SlintRuntime::process_ui_mutations() on main thread
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

/// Query for reading UI property values (Lua → Slint → Lua)
///
/// **Usage**: Lua calls ui:get("property_name"), blocks for response
/// **Threading**: Sent via channel, response via oneshot
#[derive(Debug)]
pub struct UiQuery {
    /// Property name to read
    pub prop_name: String,
    /// Response channel (Slint sends value back)
    pub response_tx: tokio::sync::oneshot::Sender<Option<JsonValue>>,
}

impl UiMutation {
    /// Create empty mutation (no-op)
    pub fn empty(app_id: String) -> Self {
        Self {
            app_id,
            properties: vec![],
            model_ops: vec![],
        }
    }

    /// Create mutation with only property updates
    pub fn properties_only(app_id: String, properties: Vec<PropertyUpdate>) -> Self {
        Self {
            app_id,
            properties,
            model_ops: vec![],
        }
    }

    /// Create mutation with only model operations
    pub fn models_only(app_id: String, model_ops: Vec<VecModelOp>) -> Self {
        Self {
            app_id,
            properties: vec![],
            model_ops,
        }
    }

    /// Check if mutation has any operations
    pub fn is_empty(&self) -> bool {
        self.properties.is_empty() && self.model_ops.is_empty()
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

    #[test]
    fn test_ui_mutation_empty() {
        let mutation = UiMutation::empty("chat_app".to_string());
        assert!(mutation.is_empty());
    }

    #[test]
    fn test_ui_mutation_properties() {
        let mutation = UiMutation::properties_only(
            "chat_app".to_string(),
            vec![PropertyUpdate {
                key: "draft".to_string(),
                value: serde_json::json!(""),
            }],
        );
        assert!(!mutation.is_empty());
        assert_eq!(mutation.properties.len(), 1);
        assert_eq!(mutation.model_ops.len(), 0);
    }
}
