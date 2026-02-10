//! ScribeHandle trait — abstracts Scribe actor access for testability
//!
//! **Pattern**: Follows the same trait extraction pattern as `LayerStorage`/`PeerVectorStorage`
//! in `scribe/src/storage.rs`.
//!
//! **Production**: `ActorScribeHandle` wraps `ActorRef<ScribeMessage>` with `block_on_async()`
//! **Testing**: `MockScribeHandle` (in `mock_scribe.rs`) uses in-memory HashMap storage

use serde_json::Value as JsonValue;
use std::sync::Arc;

/// Abstraction over Scribe actor operations used by Lua bindings
///
/// All methods are synchronous (blocking) because Lua runtime runs on an OS thread
/// without a tokio runtime. Implementations handle the async-to-sync bridge internally.
pub trait ScribeHandle: Send + Sync {
    // -- Layer lifecycle --

    /// Ensure a list layer exists (creates if needed)
    fn ensure_list(&self, layer_name: &str) -> Result<(), String>;

    /// Ensure a map layer exists (creates if needed)
    fn ensure_map(&self, layer_name: &str) -> Result<(), String>;

    /// Create an empty derived layer (for derivation engine)
    fn create_derived_layer(&self, target_layer: &str) -> Result<(), String>;

    // -- List operations --

    /// Push item to end of list
    fn list_push(&self, layer_name: &str, path: &str, item: JsonValue) -> Result<(), String>;

    /// Insert item at index
    fn list_insert(
        &self,
        layer_name: &str,
        path: &str,
        index: usize,
        item: JsonValue,
    ) -> Result<(), String>;

    /// Delete item at index
    fn list_delete(&self, layer_name: &str, path: &str, index: usize) -> Result<(), String>;

    /// Get item at index
    fn list_get(&self, layer_name: &str, index: usize) -> Result<Option<JsonValue>, String>;

    /// Get list length
    fn list_length(&self, layer_name: &str) -> Result<usize, String>;

    // -- Map operations --

    /// Insert or update a key-value pair
    fn map_insert(
        &self,
        layer_name: &str,
        path: &str,
        key: &str,
        value: JsonValue,
    ) -> Result<(), String>;

    /// Get value by key
    fn map_get(&self, layer_name: &str, key: &str) -> Result<Option<JsonValue>, String>;

    /// Delete key from map
    fn map_delete(&self, layer_name: &str, path: &str, key: &str) -> Result<(), String>;

    /// Get map length
    fn map_length(&self, layer_name: &str) -> Result<usize, String>;

    /// Get all keys from map
    fn map_keys(&self, layer_name: &str) -> Result<Vec<String>, String>;

    // -- Query --

    /// List layer names matching a glob pattern
    fn list_layers(&self, pattern: &str) -> Result<Vec<String>, String>;

    /// Get layer content as JSON (returns None if layer doesn't exist)
    fn get_layer_json(&self, layer_name: &str) -> Result<Option<JsonValue>, String>;

    /// Get layer data as JSON (returns error if layer doesn't exist)
    fn get_layer_data(&self, layer_name: &str) -> Result<JsonValue, String>;

    // -- Peers & Ephemeral --

    /// Get count of subscribers to this page
    fn get_subscriber_count(&self) -> usize;

    /// Send ephemeral data to all peers
    fn send_ephemeral(&self, payload: Vec<u8>) -> Result<(), String>;
}

// -- Production implementation: ActorScribeHandle --

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

/// Production ScribeHandle wrapping an `ActorRef<ScribeMessage>`
///
/// All operations delegate to the Scribe actor via cast/call with `block_on_async`.
/// Identical behavior to the previous direct `scribe_ref` usage in bindings.
pub struct ActorScribeHandle {
    scribe_ref: ActorRef<ScribeMessage>,
}

impl ActorScribeHandle {
    pub fn new(scribe_ref: ActorRef<ScribeMessage>) -> Arc<dyn ScribeHandle> {
        Arc::new(Self { scribe_ref })
    }

    /// Get the underlying ActorRef (for operations not covered by the trait,
    /// like SubscribeToPageUpdates)
    pub fn actor_ref(&self) -> &ActorRef<ScribeMessage> {
        &self.scribe_ref
    }
}

impl ScribeHandle for ActorScribeHandle {
    fn ensure_list(&self, layer_name: &str) -> Result<(), String> {
        let (tx, rx) = oneshot::channel();
        self.scribe_ref
            .cast(ScribeMessage::EnsureLoroList {
                layer_name: layer_name.to_string(),
                reply: tx,
            })
            .map_err(|e| format!("Failed to ensure list: {}", e))?;
        rpc(rx)
    }

    fn ensure_map(&self, layer_name: &str) -> Result<(), String> {
        let (tx, rx) = oneshot::channel();
        self.scribe_ref
            .cast(ScribeMessage::EnsureLoroMap {
                layer_name: layer_name.to_string(),
                reply: tx,
            })
            .map_err(|e| format!("Failed to ensure map: {}", e))?;
        rpc(rx)
    }

    fn create_derived_layer(&self, target_layer: &str) -> Result<(), String> {
        self.scribe_ref
            .cast(ScribeMessage::CreateDerivedLayer {
                target_layer: target_layer.to_string(),
            })
            .map_err(|e| format!("Failed to create derived layer: {}", e))
    }

    fn list_push(&self, layer_name: &str, path: &str, item: JsonValue) -> Result<(), String> {
        self.scribe_ref
            .cast(ScribeMessage::ListPush {
                layer_name: layer_name.to_string(),
                path: path.to_string(),
                item,
            })
            .map_err(|e| format!("Failed to push: {}", e))
    }

    fn list_insert(
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

    fn list_delete(&self, layer_name: &str, path: &str, index: usize) -> Result<(), String> {
        self.scribe_ref
            .cast(ScribeMessage::ListDelete {
                layer_name: layer_name.to_string(),
                path: path.to_string(),
                index,
            })
            .map_err(|e| format!("Failed to delete: {}", e))
    }

    fn list_get(&self, layer_name: &str, index: usize) -> Result<Option<JsonValue>, String> {
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

    fn list_length(&self, layer_name: &str) -> Result<usize, String> {
        let (tx, rx) = oneshot::channel();
        self.scribe_ref
            .cast(ScribeMessage::ListLength {
                layer_name: layer_name.to_string(),
                reply: tx,
            })
            .map_err(|e| format!("Failed to get length: {}", e))?;
        rpc(rx)
    }

    fn map_insert(
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

    fn map_get(&self, layer_name: &str, key: &str) -> Result<Option<JsonValue>, String> {
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

    fn map_delete(&self, layer_name: &str, path: &str, key: &str) -> Result<(), String> {
        self.scribe_ref
            .cast(ScribeMessage::MapDelete {
                layer_name: layer_name.to_string(),
                path: path.to_string(),
                key: key.to_string(),
            })
            .map_err(|e| format!("Failed to delete: {}", e))
    }

    fn map_length(&self, layer_name: &str) -> Result<usize, String> {
        let (tx, rx) = oneshot::channel();
        self.scribe_ref
            .cast(ScribeMessage::MapLength {
                layer_name: layer_name.to_string(),
                reply: tx,
            })
            .map_err(|e| format!("Failed to get length: {}", e))?;
        rpc(rx)
    }

    fn map_keys(&self, layer_name: &str) -> Result<Vec<String>, String> {
        let (tx, rx) = oneshot::channel();
        self.scribe_ref
            .cast(ScribeMessage::MapKeys {
                layer_name: layer_name.to_string(),
                reply: tx,
            })
            .map_err(|e| format!("Failed to get keys: {}", e))?;
        rpc(rx)
    }

    fn list_layers(&self, pattern: &str) -> Result<Vec<String>, String> {
        let (tx, rx) = oneshot::channel();
        self.scribe_ref
            .cast(ScribeMessage::ListLayers {
                pattern: pattern.to_string(),
                reply: tx,
            })
            .map_err(|e| format!("Failed to list layers: {}", e))?;
        rpc_direct(rx)
    }

    fn get_layer_json(&self, layer_name: &str) -> Result<Option<JsonValue>, String> {
        let (tx, rx) = oneshot::channel();
        self.scribe_ref
            .cast(ScribeMessage::GetLayerJson {
                layer_name: layer_name.to_string(),
                reply: tx,
            })
            .map_err(|e| format!("Failed to request layer data: {}", e))?;
        rpc_direct(rx)
    }

    fn get_layer_data(&self, layer_name: &str) -> Result<JsonValue, String> {
        let (tx, rx) = oneshot::channel();
        self.scribe_ref
            .cast(ScribeMessage::GetLayerData {
                layer_name: layer_name.to_string(),
                reply: tx,
            })
            .map_err(|e| format!("Failed to request layer data: {}", e))?;
        rpc(rx)
    }

    fn get_subscriber_count(&self) -> usize {
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

    fn send_ephemeral(&self, payload: Vec<u8>) -> Result<(), String> {
        self.scribe_ref
            .cast(ScribeMessage::SendEphemeral { payload })
            .map_err(|e| format!("Failed to send ephemeral: {}", e))
    }
}
