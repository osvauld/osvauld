//! ActorScribeHandle — concrete Scribe actor access for Lua bindings
//!
//! **Pattern**: Wraps `ActorRef<ScribeMessage>` with a synchronous blocking bridge.
//! All methods are synchronous because the Lua runtime runs on an OS thread
//! without a tokio runtime. The async-to-sync bridge is handled internally.
//!
//! **Production**: `ActorScribeHandle` wraps `ActorRef<ScribeMessage>` with `block_in_place()`

use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::sync::Arc;

use ractor::ActorRef;
use scribe::ScribeMessage;
use tokio::sync::oneshot;

/// Helper: block_on an async oneshot that returns `Result<T, ScribeError>`,
/// flattening both channel errors and scribe errors to `String`.
fn rpc<T, E: std::fmt::Display>(rx: oneshot::Receiver<Result<T, E>>) -> Result<T, String> {
    let inner = match tokio::runtime::Handle::try_current() {
        Ok(handle) => tokio::task::block_in_place(|| handle.block_on(rx)),
        Err(_) => futures::executor::block_on(rx),
    };
    inner
        .map_err(|e| format!("Channel error: {}", e))?
        .map_err(|e| format!("{}", e))
}

/// Helper: block_on an async oneshot that returns `T` directly (no inner Result)
fn rpc_direct<T>(rx: oneshot::Receiver<T>) -> Result<T, String> {
    let result = match tokio::runtime::Handle::try_current() {
        Ok(handle) => tokio::task::block_in_place(|| handle.block_on(rx)),
        Err(_) => futures::executor::block_on(rx),
    };
    result.map_err(|e| format!("Channel error: {}", e))
}

/// Concrete Scribe handle wrapping an `ActorRef<ScribeMessage>`
///
/// All operations delegate to the Scribe actor via cast/call with `block_in_place`.
/// Identical behavior to the previous direct `scribe_ref` usage in bindings.
pub struct ActorScribeHandle {
    scribe_ref: ActorRef<ScribeMessage>,
}

impl ActorScribeHandle {
    pub fn new(scribe_ref: ActorRef<ScribeMessage>) -> Arc<Self> {
        Arc::new(Self { scribe_ref })
    }

    /// Get the underlying ActorRef (for operations not covered by the methods,
    /// like SubscribeToPageUpdates)
    pub fn actor_ref(&self) -> &ActorRef<ScribeMessage> {
        &self.scribe_ref
    }

    // -- Layer lifecycle --

    /// Ensure a list layer exists (creates if needed)
    pub fn ensure_list(&self, layer_name: &str) -> Result<(), String> {
        let (tx, rx) = oneshot::channel();
        self.scribe_ref
            .cast(ScribeMessage::EnsureLoroList {
                layer_name: layer_name.to_string(),
                reply: tx,
            })
            .map_err(|e| format!("Failed to ensure list: {}", e))?;
        rpc(rx)
    }

    /// Ensure a map layer exists (creates if needed)
    pub fn ensure_map(&self, layer_name: &str) -> Result<(), String> {
        let (tx, rx) = oneshot::channel();
        self.scribe_ref
            .cast(ScribeMessage::EnsureLoroMap {
                layer_name: layer_name.to_string(),
                reply: tx,
            })
            .map_err(|e| format!("Failed to ensure map: {}", e))?;
        rpc(rx)
    }

    /// Create an empty derived layer (for derivation engine)
    pub fn create_derived_layer(&self, target_layer: &str) -> Result<(), String> {
        self.scribe_ref
            .cast(ScribeMessage::CreateDerivedLayer {
                target_layer: target_layer.to_string(),
            })
            .map_err(|e| format!("Failed to create derived layer: {}", e))
    }

    // -- List operations --

    /// Push item to end of list
    pub fn list_push(&self, layer_name: &str, path: &str, item: JsonValue) -> Result<(), String> {
        self.scribe_ref
            .cast(ScribeMessage::ListPush {
                layer_name: layer_name.to_string(),
                path: path.to_string(),
                item,
            })
            .map_err(|e| format!("Failed to push: {}", e))
    }

    /// Insert item at index
    pub fn list_insert(
        &self,
        layer_name: &str,
        path: &str,
        index: usize,
        item: JsonValue,
    ) -> Result<(), String> {
        self.scribe_ref
            .cast(ScribeMessage::ListInsert {
                layer_name: layer_name.to_string(),
                path: path.to_string(),
                index,
                item,
            })
            .map_err(|e| format!("Failed to insert: {}", e))
    }

    /// Delete item at index
    pub fn list_delete(&self, layer_name: &str, path: &str, index: usize) -> Result<(), String> {
        self.scribe_ref
            .cast(ScribeMessage::ListDelete {
                layer_name: layer_name.to_string(),
                path: path.to_string(),
                index,
            })
            .map_err(|e| format!("Failed to delete: {}", e))
    }

    /// Get item at index
    pub fn list_get(&self, layer_name: &str, index: usize) -> Result<Option<JsonValue>, String> {
        let (tx, rx) = oneshot::channel();
        self.scribe_ref
            .cast(ScribeMessage::ListGet {
                layer_name: layer_name.to_string(),
                index,
                reply: tx,
            })
            .map_err(|e| format!("Failed to get: {}", e))?;
        rpc(rx)
    }

    /// Get list length
    pub fn list_length(&self, layer_name: &str) -> Result<usize, String> {
        let (tx, rx) = oneshot::channel();
        self.scribe_ref
            .cast(ScribeMessage::ListLength {
                layer_name: layer_name.to_string(),
                reply: tx,
            })
            .map_err(|e| format!("Failed to get length: {}", e))?;
        rpc(rx)
    }

    // -- Map operations --

    /// Insert or update a key-value pair
    pub fn map_insert(
        &self,
        layer_name: &str,
        path: &str,
        key: &str,
        value: JsonValue,
    ) -> Result<(), String> {
        self.scribe_ref
            .cast(ScribeMessage::MapInsert {
                layer_name: layer_name.to_string(),
                path: path.to_string(),
                key: key.to_string(),
                value,
            })
            .map_err(|e| format!("Failed to set: {}", e))
    }

    /// Get value by key
    pub fn map_get(&self, layer_name: &str, key: &str) -> Result<Option<JsonValue>, String> {
        let (tx, rx) = oneshot::channel();
        self.scribe_ref
            .cast(ScribeMessage::MapGet {
                layer_name: layer_name.to_string(),
                key: key.to_string(),
                reply: tx,
            })
            .map_err(|e| format!("Failed to get: {}", e))?;
        rpc(rx)
    }

    /// Delete key from map
    pub fn map_delete(&self, layer_name: &str, path: &str, key: &str) -> Result<(), String> {
        self.scribe_ref
            .cast(ScribeMessage::MapDelete {
                layer_name: layer_name.to_string(),
                path: path.to_string(),
                key: key.to_string(),
            })
            .map_err(|e| format!("Failed to delete: {}", e))
    }

    /// Get map length
    pub fn map_length(&self, layer_name: &str) -> Result<usize, String> {
        let (tx, rx) = oneshot::channel();
        self.scribe_ref
            .cast(ScribeMessage::MapLength {
                layer_name: layer_name.to_string(),
                reply: tx,
            })
            .map_err(|e| format!("Failed to get length: {}", e))?;
        rpc(rx)
    }

    /// Get all keys from map
    pub fn map_keys(&self, layer_name: &str) -> Result<Vec<String>, String> {
        let (tx, rx) = oneshot::channel();
        self.scribe_ref
            .cast(ScribeMessage::MapKeys {
                layer_name: layer_name.to_string(),
                reply: tx,
            })
            .map_err(|e| format!("Failed to get keys: {}", e))?;
        rpc(rx)
    }

    // -- Query --

    /// List layer names matching a glob pattern
    pub fn list_layers(&self, pattern: &str) -> Result<Vec<String>, String> {
        let (tx, rx) = oneshot::channel();
        self.scribe_ref
            .cast(ScribeMessage::ListLayers {
                pattern: pattern.to_string(),
                reply: tx,
            })
            .map_err(|e| format!("Failed to list layers: {}", e))?;
        rpc_direct(rx)
    }

    /// Get layer content as JSON (returns None if layer doesn't exist)
    pub fn get_layer_json(&self, layer_name: &str) -> Result<Option<JsonValue>, String> {
        let (tx, rx) = oneshot::channel();
        self.scribe_ref
            .cast(ScribeMessage::GetLayerJson {
                layer_name: layer_name.to_string(),
                reply: tx,
            })
            .map_err(|e| format!("Failed to request layer data: {}", e))?;
        rpc_direct(rx)
    }

    /// Get layer data as JSON (returns error if layer doesn't exist)
    pub fn get_layer_data(&self, layer_name: &str) -> Result<JsonValue, String> {
        let (tx, rx) = oneshot::channel();
        self.scribe_ref
            .cast(ScribeMessage::GetLayerData {
                layer_name: layer_name.to_string(),
                reply: tx,
            })
            .map_err(|e| format!("Failed to request layer data: {}", e))?;
        rpc(rx)
    }

    // -- Dynamic layers --

    /// Create a dynamic layer from a schema pattern
    ///
    /// **Context**: Lua app calls
    /// `scribe:create_layer("channels/{channel}/messages/{period}", { channel = "general", period = "2025-02" })`
    /// **Returns**: Full layer name (e.g., "{page_id}/channels/{our_did}/general/messages/2025-02")
    pub fn create_layer(
        &self,
        schema_key: &str,
        placeholders: HashMap<String, String>,
        authorized_peers: Option<Vec<String>>,
    ) -> Result<String, String> {
        let (tx, rx) = oneshot::channel();
        self.scribe_ref
            .cast(ScribeMessage::CreateDynamicLayer {
                schema_key: schema_key.to_string(),
                placeholders,
                authorized_peers,
                reply: tx,
            })
            .map_err(|e| format!("Failed to create dynamic layer: {}", e))?;
        rpc(rx)
    }

    /// Backward-compatible helper for `{id}`-only schemas.
    pub fn create_layer_with_id(
        &self,
        schema_key: &str,
        layer_id: &str,
        authorized_peers: Option<Vec<String>>,
    ) -> Result<String, String> {
        let mut placeholders = HashMap::new();
        placeholders.insert("id".to_string(), layer_id.to_string());
        self.create_layer(schema_key, placeholders, authorized_peers)
    }

    // -- Layer access --

    /// Add DIDs as participants to an explicit dynamic layer
    pub fn add_layer_access(&self, layer_name: &str, dids: &[String]) -> Result<(), String> {
        let (tx, rx) = oneshot::channel();
        self.scribe_ref
            .cast(ScribeMessage::AddLayerAccess {
                layer_name: layer_name.to_string(),
                dids: dids.to_vec(),
                reply: tx,
            })
            .map_err(|e| format!("Failed to add layer access: {}", e))?;
        rpc(rx)
    }

    // -- Peers & Ephemeral --

    /// Get count of subscribers to this page
    pub fn get_subscriber_count(&self) -> usize {
        let (tx, rx) = oneshot::channel();
        if self
            .scribe_ref
            .cast(ScribeMessage::GetSubscriberCount { reply: tx })
            .is_err()
        {
            return 0;
        }
        let result = match tokio::runtime::Handle::try_current() {
            Ok(handle) => tokio::task::block_in_place(|| handle.block_on(rx)),
            Err(_) => futures::executor::block_on(rx),
        };
        result.unwrap_or(0)
    }

    /// Send ephemeral data to all peers
    pub fn send_ephemeral(&self, payload: Vec<u8>) -> Result<(), String> {
        self.scribe_ref
            .cast(ScribeMessage::SendEphemeral { payload })
            .map_err(|e| format!("Failed to send ephemeral: {}", e))
    }
}
