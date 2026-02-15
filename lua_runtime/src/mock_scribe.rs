//! MockScribeHandle — in-memory ScribeHandle for testing
//!
//! No Loro, no actors, no network. Pure HashMap-based storage.
//! Provides inspection methods for test assertions.
//!
//! **Reactive**: Mutations notify all registered subscribers via `LuaCommand::LoroChanged`
//! with `full_data` (no delta/ops — uses the Replace fallback path, works for both lists and maps).

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use serde_json::Value as JsonValue;
use tokio::sync::mpsc;

use crate::commands::LuaCommand;
use crate::scribe_handle::ScribeHandle;

/// Layer storage variants
#[derive(Debug, Clone)]
enum MockLayer {
    List(Vec<JsonValue>),
    Map(HashMap<String, JsonValue>),
}

/// Shared state for MockScribeHandle
///
/// Wrapped in Arc<Mutex<>> so multiple handles can share state
/// (simulates multiple peers on the same page).
///
/// **Reactive**: Register `lua_tx` channels via `register_subscriber()`.
/// Every mutation sends `LuaCommand::LoroChanged { full_data }` to all subscribers.
///
/// **Layer normalization**: Like the real Scribe, strips `page_id/` prefix from layer names.
/// Lua calls `scribe:list(page_id .. "/messages")` but layers are stored as `"messages"`.
pub struct MockScribeState {
    layers: HashMap<String, MockLayer>,
    ephemerals: Vec<Vec<u8>>,
    subscriber_count: usize,
    /// Page ID for layer name normalization (strips "page_id/" prefix)
    page_id: Option<String>,
    /// Registered Lua runtime channels — notified on every mutation
    subscribers: Vec<mpsc::Sender<LuaCommand>>,
}

impl MockScribeState {
    fn new() -> Self {
        Self {
            layers: HashMap::new(),
            ephemerals: Vec::new(),
            subscriber_count: 0,
            page_id: None,
            subscribers: Vec::new(),
        }
    }

    /// Set page ID for layer name normalization
    ///
    /// **Context**: Lua calls `scribe:list(page_id .. "/messages")` with a prefix.
    /// Real Scribe strips this prefix. The mock must do the same.
    pub fn set_page_id(&mut self, page_id: &str) {
        self.page_id = Some(page_id.to_string());
    }

    /// Normalize layer name — strip `page_id/` prefix (matches real Scribe behavior)
    fn normalize(&self, name: &str) -> String {
        if let Some(ref page_id) = self.page_id {
            let prefix = format!("{}/", page_id);
            if let Some(bare) = name.strip_prefix(&prefix) {
                return bare.to_string();
            }
        }
        name.to_string()
    }

    /// Register a Lua runtime's command channel for change notifications
    ///
    /// **Context**: Called after `launch_test_slint_app()` returns the `lua_tx`.
    /// All subsequent mutations will send `LoroChanged` to this channel.
    pub fn register_subscriber(&mut self, tx: mpsc::Sender<LuaCommand>) {
        self.subscribers.push(tx);
    }

    /// Clear all subscribers (stale channels from previous reload cycle)
    pub fn clear_subscribers(&mut self) {
        self.subscribers.clear();
    }

    /// Notify all subscribers that a layer changed
    ///
    /// **Sends**: `LuaCommand::LoroChanged` with `full_data` (no delta/ops).
    /// This triggers the Replace fallback path in the binding system,
    /// which works for both list and map layers.
    fn notify_change(&self, layer_name: &str) {
        if self.subscribers.is_empty() {
            return;
        }
        let full_data = match self.get_layer_data(layer_name) {
            Some(data) => data,
            None => return,
        };
        for tx in &self.subscribers {
            let _ = tx.try_send(LuaCommand::LoroChanged {
                layer_name: layer_name.to_string(),
                ops: None,
                delta: None,
                full_data: Some(full_data.clone()),
            });
        }
    }
}

/// In-memory ScribeHandle for testing
///
/// All operations are synchronous and in-memory.
/// Provides inspection methods for assertions.
pub struct MockScribeHandle {
    state: Arc<Mutex<MockScribeState>>,
}

impl MockScribeHandle {
    /// Create a new MockScribeHandle
    pub fn new() -> Arc<dyn ScribeHandle> {
        Arc::new(Self {
            state: Arc::new(Mutex::new(MockScribeState::new())),
        })
    }

    /// Create a new MockScribeHandle with shared state (for multi-peer testing)
    pub fn with_shared_state(state: Arc<Mutex<MockScribeState>>) -> Arc<dyn ScribeHandle> {
        Arc::new(Self { state })
    }

    /// Create shared state for multi-peer scenarios
    pub fn shared_state() -> Arc<Mutex<MockScribeState>> {
        Arc::new(Mutex::new(MockScribeState::new()))
    }

    /// Get the underlying shared state for inspection
    pub fn state(&self) -> Arc<Mutex<MockScribeState>> {
        self.state.clone()
    }
}

// -- Inspection methods (for test assertions) --

impl MockScribeState {
    /// Get all sent ephemerals
    pub fn get_sent_ephemerals(&self) -> Vec<Vec<u8>> {
        self.ephemerals.clone()
    }

    /// Clear sent ephemerals
    pub fn clear_ephemerals(&mut self) {
        self.ephemerals.clear();
    }

    /// Get all layer names
    pub fn layer_names(&self) -> Vec<String> {
        self.layers.keys().cloned().collect()
    }

    /// Get layer data as JSON
    pub fn get_layer_data(&self, name: &str) -> Option<JsonValue> {
        let name = self.normalize(name);
        match self.layers.get(&name) {
            Some(MockLayer::List(items)) => Some(JsonValue::Array(items.clone())),
            Some(MockLayer::Map(map)) => {
                Some(JsonValue::Object(map.iter().map(|(k, v)| (k.clone(), v.clone())).collect()))
            }
            None => None,
        }
    }

    /// Set subscriber count (for testing peer count)
    pub fn set_subscriber_count(&mut self, count: usize) {
        self.subscriber_count = count;
    }

    /// Directly set layer data (for injecting peer writes in tests)
    pub fn inject_list_data(&mut self, name: &str, items: Vec<JsonValue>) {
        let name = self.normalize(name);
        self.layers.insert(name, MockLayer::List(items));
    }

    /// Directly set map data (for injecting peer writes in tests)
    pub fn inject_map_data(&mut self, name: &str, data: HashMap<String, JsonValue>) {
        let name = self.normalize(name);
        self.layers.insert(name, MockLayer::Map(data));
    }
}

impl ScribeHandle for MockScribeHandle {
    fn ensure_list(&self, layer_name: &str) -> Result<(), String> {
        let mut state = self.state.lock().unwrap();
        let name = state.normalize(layer_name);
        state
            .layers
            .entry(name)
            .or_insert_with(|| MockLayer::List(Vec::new()));
        Ok(())
    }

    fn ensure_map(&self, layer_name: &str) -> Result<(), String> {
        let mut state = self.state.lock().unwrap();
        let name = state.normalize(layer_name);
        state
            .layers
            .entry(name)
            .or_insert_with(|| MockLayer::Map(HashMap::new()));
        Ok(())
    }

    fn create_derived_layer(&self, target_layer: &str) -> Result<(), String> {
        self.ensure_list(target_layer)
    }

    fn list_push(&self, layer_name: &str, _path: &str, item: JsonValue) -> Result<(), String> {
        let mut state = self.state.lock().unwrap();
        let name = state.normalize(layer_name);
        match state.layers.get_mut(&name) {
            Some(MockLayer::List(items)) => {
                items.push(item);
                state.notify_change(&name);
                Ok(())
            }
            Some(MockLayer::Map(_)) => Err(format!("Layer '{}' is a map, not a list", name)),
            None => Err(format!("Layer '{}' does not exist", name)),
        }
    }

    fn list_insert(
        &self,
        layer_name: &str,
        _path: &str,
        index: usize,
        item: JsonValue,
    ) -> Result<(), String> {
        let mut state = self.state.lock().unwrap();
        let name = state.normalize(layer_name);
        match state.layers.get_mut(&name) {
            Some(MockLayer::List(items)) => {
                if index > items.len() {
                    return Err(format!("Index {} out of bounds (len={})", index, items.len()));
                }
                items.insert(index, item);
                state.notify_change(&name);
                Ok(())
            }
            Some(MockLayer::Map(_)) => Err(format!("Layer '{}' is a map, not a list", name)),
            None => Err(format!("Layer '{}' does not exist", name)),
        }
    }

    fn list_delete(&self, layer_name: &str, _path: &str, index: usize) -> Result<(), String> {
        let mut state = self.state.lock().unwrap();
        let name = state.normalize(layer_name);
        match state.layers.get_mut(&name) {
            Some(MockLayer::List(items)) => {
                if index >= items.len() {
                    return Err(format!("Index {} out of bounds (len={})", index, items.len()));
                }
                items.remove(index);
                state.notify_change(&name);
                Ok(())
            }
            Some(MockLayer::Map(_)) => Err(format!("Layer '{}' is a map, not a list", name)),
            None => Err(format!("Layer '{}' does not exist", name)),
        }
    }

    fn list_get(&self, layer_name: &str, index: usize) -> Result<Option<JsonValue>, String> {
        let state = self.state.lock().unwrap();
        let name = state.normalize(layer_name);
        match state.layers.get(&name) {
            Some(MockLayer::List(items)) => Ok(items.get(index).cloned()),
            Some(MockLayer::Map(_)) => Err(format!("Layer '{}' is a map, not a list", name)),
            None => Err(format!("Layer '{}' does not exist", name)),
        }
    }

    fn list_length(&self, layer_name: &str) -> Result<usize, String> {
        let state = self.state.lock().unwrap();
        let name = state.normalize(layer_name);
        match state.layers.get(&name) {
            Some(MockLayer::List(items)) => Ok(items.len()),
            Some(MockLayer::Map(_)) => Err(format!("Layer '{}' is a map, not a list", name)),
            None => Err(format!("Layer '{}' does not exist", name)),
        }
    }

    fn map_insert(
        &self,
        layer_name: &str,
        _path: &str,
        key: &str,
        value: JsonValue,
    ) -> Result<(), String> {
        let mut state = self.state.lock().unwrap();
        let name = state.normalize(layer_name);
        match state.layers.get_mut(&name) {
            Some(MockLayer::Map(map)) => {
                map.insert(key.to_string(), value);
                state.notify_change(&name);
                Ok(())
            }
            Some(MockLayer::List(_)) => {
                Err(format!("Layer '{}' is a list, not a map", name))
            }
            None => Err(format!("Layer '{}' does not exist", name)),
        }
    }

    fn map_get(&self, layer_name: &str, key: &str) -> Result<Option<JsonValue>, String> {
        let state = self.state.lock().unwrap();
        let name = state.normalize(layer_name);
        match state.layers.get(&name) {
            Some(MockLayer::Map(map)) => Ok(map.get(key).cloned()),
            Some(MockLayer::List(_)) => {
                Err(format!("Layer '{}' is a list, not a map", name))
            }
            None => Err(format!("Layer '{}' does not exist", name)),
        }
    }

    fn map_delete(&self, layer_name: &str, _path: &str, key: &str) -> Result<(), String> {
        let mut state = self.state.lock().unwrap();
        let name = state.normalize(layer_name);
        match state.layers.get_mut(&name) {
            Some(MockLayer::Map(map)) => {
                map.remove(key);
                state.notify_change(&name);
                Ok(())
            }
            Some(MockLayer::List(_)) => {
                Err(format!("Layer '{}' is a list, not a map", name))
            }
            None => Err(format!("Layer '{}' does not exist", name)),
        }
    }

    fn map_length(&self, layer_name: &str) -> Result<usize, String> {
        let state = self.state.lock().unwrap();
        let name = state.normalize(layer_name);
        match state.layers.get(&name) {
            Some(MockLayer::Map(map)) => Ok(map.len()),
            Some(MockLayer::List(_)) => {
                Err(format!("Layer '{}' is a list, not a map", name))
            }
            None => Err(format!("Layer '{}' does not exist", name)),
        }
    }

    fn map_keys(&self, layer_name: &str) -> Result<Vec<String>, String> {
        let state = self.state.lock().unwrap();
        let name = state.normalize(layer_name);
        match state.layers.get(&name) {
            Some(MockLayer::Map(map)) => Ok(map.keys().cloned().collect()),
            Some(MockLayer::List(_)) => {
                Err(format!("Layer '{}' is a list, not a map", name))
            }
            None => Err(format!("Layer '{}' does not exist", name)),
        }
    }

    fn list_layers(&self, pattern: &str) -> Result<Vec<String>, String> {
        let state = self.state.lock().unwrap();
        let names: Vec<String> = state
            .layers
            .keys()
            .filter(|name| matches_glob(name, pattern))
            .cloned()
            .collect();
        Ok(names)
    }

    fn get_layer_json(&self, layer_name: &str) -> Result<Option<JsonValue>, String> {
        let state = self.state.lock().unwrap();
        let name = state.normalize(layer_name);
        Ok(state.get_layer_data(&name))
    }

    fn get_layer_data(&self, layer_name: &str) -> Result<JsonValue, String> {
        let state = self.state.lock().unwrap();
        let name = state.normalize(layer_name);
        state
            .get_layer_data(&name)
            .ok_or_else(|| format!("Layer '{}' does not exist", name))
    }

    fn get_subscriber_count(&self) -> usize {
        self.state.lock().unwrap().subscriber_count
    }

    fn create_layer(&self, schema_key: &str, layer_id: &str, _authorized_peers: Option<Vec<String>>) -> Result<String, String> {
        // Mock: generate a simple layer name from schema_key and layer_id
        // In production, Scribe generates the full path with DID
        let layer_name = format!("mock-page/{}/{}", schema_key.replace("{id}", layer_id), layer_id);
        Ok(layer_name)
    }

    fn add_layer_access(&self, _layer_name: &str, _dids: &[String]) -> Result<(), String> {
        Ok(())
    }

    fn remove_layer_access(&self, _layer_name: &str, _did: &str) -> Result<(), String> {
        Ok(())
    }

    fn send_ephemeral(&self, payload: Vec<u8>) -> Result<(), String> {
        self.state.lock().unwrap().ephemerals.push(payload);
        Ok(())
    }
}

/// Simple glob matching for layer names
///
/// Supports `*` as wildcard for any characters within a segment.
fn matches_glob(name: &str, pattern: &str) -> bool {
    if !pattern.contains('*') {
        return name == pattern;
    }

    // Split on '*' and match greedily
    let parts: Vec<&str> = pattern.split('*').collect();
    if parts.is_empty() {
        return true;
    }

    let mut pos = 0;
    for (i, part) in parts.iter().enumerate() {
        if part.is_empty() {
            continue;
        }
        match name[pos..].find(part) {
            Some(idx) => {
                // First part must match at start
                if i == 0 && idx != 0 {
                    return false;
                }
                pos += idx + part.len();
            }
            None => return false,
        }
    }

    // Last part must match at end (unless pattern ends with *)
    if !pattern.ends_with('*') {
        name.ends_with(parts.last().unwrap_or(&""))
    } else {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_list_operations() {
        let handle = MockScribeHandle::new();

        // Ensure list and push
        handle.ensure_list("messages").unwrap();
        handle
            .list_push("messages", "", serde_json::json!({"text": "hello"}))
            .unwrap();
        handle
            .list_push("messages", "", serde_json::json!({"text": "world"}))
            .unwrap();

        assert_eq!(handle.list_length("messages").unwrap(), 2);
        assert_eq!(
            handle.list_get("messages", 0).unwrap(),
            Some(serde_json::json!({"text": "hello"}))
        );

        // Insert at index
        handle
            .list_insert("messages", "", 1, serde_json::json!({"text": "middle"}))
            .unwrap();
        assert_eq!(handle.list_length("messages").unwrap(), 3);
        assert_eq!(
            handle.list_get("messages", 1).unwrap(),
            Some(serde_json::json!({"text": "middle"}))
        );

        // Delete
        handle.list_delete("messages", "", 1).unwrap();
        assert_eq!(handle.list_length("messages").unwrap(), 2);
        assert_eq!(
            handle.list_get("messages", 1).unwrap(),
            Some(serde_json::json!({"text": "world"}))
        );
    }

    #[test]
    fn test_mock_map_operations() {
        let handle = MockScribeHandle::new();

        handle.ensure_map("users").unwrap();
        handle
            .map_insert("users", "", "alice", serde_json::json!({"name": "Alice"}))
            .unwrap();
        handle
            .map_insert("users", "", "bob", serde_json::json!({"name": "Bob"}))
            .unwrap();

        assert_eq!(handle.map_length("users").unwrap(), 2);
        assert_eq!(
            handle.map_get("users", "alice").unwrap(),
            Some(serde_json::json!({"name": "Alice"}))
        );

        let mut keys = handle.map_keys("users").unwrap();
        keys.sort();
        assert_eq!(keys, vec!["alice", "bob"]);

        handle.map_delete("users", "", "alice").unwrap();
        assert_eq!(handle.map_length("users").unwrap(), 1);
        assert_eq!(handle.map_get("users", "alice").unwrap(), None);
    }

    #[test]
    fn test_mock_layer_listing() {
        let handle = MockScribeHandle::new();

        handle.ensure_list("page1/messages").unwrap();
        handle.ensure_list("page1/orders").unwrap();
        handle.ensure_map("page1/users").unwrap();
        handle.ensure_list("page2/messages").unwrap();

        let mut names = handle.list_layers("page1/*").unwrap();
        names.sort();
        assert_eq!(names, vec!["page1/messages", "page1/orders", "page1/users"]);

        let names = handle.list_layers("page2/*").unwrap();
        assert_eq!(names, vec!["page2/messages"]);

        let names = handle.list_layers("*/messages").unwrap();
        assert_eq!(names.len(), 2);
    }

    #[test]
    fn test_mock_ephemerals() {
        let handle = MockScribeHandle::new();

        handle.send_ephemeral(b"hello".to_vec()).unwrap();
        handle.send_ephemeral(b"world".to_vec()).unwrap();

        // Need to access inner state for inspection
        // Use get_layer_json as a proxy - ephemerals need state access
        // For now, verify send doesn't error
        assert_eq!(handle.get_subscriber_count(), 0);
    }

    #[test]
    fn test_mock_get_layer_json() {
        let handle = MockScribeHandle::new();

        // Non-existent layer returns None
        assert_eq!(handle.get_layer_json("nonexistent").unwrap(), None);

        // List layer
        handle.ensure_list("items").unwrap();
        handle
            .list_push("items", "", serde_json::json!("a"))
            .unwrap();
        let data = handle.get_layer_json("items").unwrap().unwrap();
        assert_eq!(data, serde_json::json!(["a"]));

        // Map layer
        handle.ensure_map("config").unwrap();
        handle
            .map_insert("config", "", "key", serde_json::json!("val"))
            .unwrap();
        let data = handle.get_layer_json("config").unwrap().unwrap();
        assert_eq!(data, serde_json::json!({"key": "val"}));
    }

    #[test]
    fn test_mock_shared_state() {
        let shared = MockScribeHandle::shared_state();
        let handle1 = MockScribeHandle::with_shared_state(shared.clone());
        let handle2 = MockScribeHandle::with_shared_state(shared.clone());

        // Write through handle1, read through handle2
        handle1.ensure_list("shared").unwrap();
        handle1
            .list_push("shared", "", serde_json::json!("from_peer1"))
            .unwrap();

        assert_eq!(handle2.list_length("shared").unwrap(), 1);
        assert_eq!(
            handle2.list_get("shared", 0).unwrap(),
            Some(serde_json::json!("from_peer1"))
        );
    }

    #[test]
    fn test_reactive_notifications() {
        let shared = MockScribeHandle::shared_state();
        let handle = MockScribeHandle::with_shared_state(shared.clone());

        // Register a subscriber
        let (tx, mut rx) = mpsc::channel::<LuaCommand>(64);
        shared.lock().unwrap().register_subscriber(tx);

        // Mutate — should notify
        handle.ensure_list("messages").unwrap();
        handle
            .list_push("messages", "", serde_json::json!({"text": "hello"}))
            .unwrap();

        // Check notification arrived
        let cmd = rx.try_recv().unwrap();
        match cmd {
            LuaCommand::LoroChanged { layer_name, full_data, ops, delta } => {
                assert_eq!(layer_name, "messages");
                assert!(ops.is_none());
                assert!(delta.is_none());
                assert_eq!(full_data, Some(serde_json::json!([{"text": "hello"}])));
            }
            other => panic!("Expected LoroChanged, got {:?}", other),
        }
    }

    #[test]
    fn test_type_mismatch_errors() {
        let handle = MockScribeHandle::new();

        handle.ensure_list("my_list").unwrap();
        handle.ensure_map("my_map").unwrap();

        // List ops on map
        assert!(handle.list_push("my_map", "", serde_json::json!(1)).is_err());
        assert!(handle.list_get("my_map", 0).is_err());

        // Map ops on list
        assert!(handle
            .map_insert("my_list", "", "k", serde_json::json!(1))
            .is_err());
        assert!(handle.map_get("my_list", "k").is_err());
    }

    #[test]
    fn test_matches_glob() {
        assert!(matches_glob("page1/messages", "page1/*"));
        assert!(matches_glob("page1/orders", "page1/*"));
        assert!(!matches_glob("page2/orders", "page1/*"));
        assert!(matches_glob("page1/messages", "*/messages"));
        assert!(matches_glob("a/b/c", "a/*/c"));
        assert!(matches_glob("exact", "exact"));
        assert!(!matches_glob("exact", "other"));
        assert!(matches_glob("anything", "*"));
    }
}
