//! Layer - CRDT data container within a Page
//!
//! Terminology:
//! - Space: Container that groups Pages
//! - Page: A sub-application instance with its own template
//! - Layer: CRDT data containers (the actual collaborative state)
//!
//! Layers are the actual Loro documents that store collaborative data.
//! Each Page can have multiple Layers (e.g., content_layer, comments_layer).

use loro::{LoroDoc, ExportMode};

/// Layer - Wrapper around LoroDoc for collaborative data
///
/// Consumers (like Butler) use this type, never LoroDoc directly.
/// All loro operations go through this wrapper.
#[derive(Clone)]
pub struct Layer {
    inner: LoroDoc,
}

impl Layer {
    /// Create a new empty layer
    pub fn new() -> Self {
        Self {
            inner: LoroDoc::new(),
        }
    }

    /// Create a layer from snapshot bytes
    pub fn from_snapshot(snapshot_bytes: &[u8]) -> Result<Self, LayerError> {
        let doc = LoroDoc::new();
        doc.import(snapshot_bytes)
            .map_err(|e| LayerError::Import(e.to_string()))?;
        Ok(Self { inner: doc })
    }

    /// Apply updates/snapshot bytes to this layer (merge)
    pub fn apply(&self, bytes: &[u8]) -> Result<(), LayerError> {
        self.inner
            .import(bytes)
            .map(|_| ())
            .map_err(|e| LayerError::Import(e.to_string()))
    }

    /// Export full snapshot with complete history (for owner/node storage)
    pub fn export_snapshot(&self) -> Vec<u8> {
        self.inner
            .export(ExportMode::Snapshot)
            .expect("Failed to export snapshot")
    }

    /// Export shallow snapshot without history (for viewers)
    pub fn export_shallow_snapshot(&self) -> Vec<u8> {
        let frontiers = self.inner.state_frontiers();
        self.inner
            .export(ExportMode::shallow_snapshot(&frontiers))
            .expect("Failed to export shallow snapshot")
    }

    /// Export updates since a version vector (for incremental sync)
    pub fn export_updates(&self, from_version: &[u8]) -> Result<Vec<u8>, LayerError> {
        let vv = loro::VersionVector::decode(from_version)
            .map_err(|e| LayerError::Decode(e.to_string()))?;
        Ok(self.inner
            .export(ExportMode::updates(&vv))
            .expect("Failed to export updates"))
    }

    /// Get version vector (for owner/node - full history tracking)
    pub fn version_vector(&self) -> Vec<u8> {
        self.inner.oplog_vv().encode()
    }

    /// Get state frontiers (for viewers - current state)
    pub fn frontiers(&self) -> Vec<u8> {
        self.inner.state_frontiers().encode()
    }

    /// Commit the current transaction
    ///
    /// This must be called after direct modifications via Loro FFI to trigger observers.
    /// Example: After Lua modifies a LoroList/LoroMap, call this to fire change events.
    pub fn commit(&self) {
        self.inner.commit();
    }

    /// Access the underlying LoroDoc for container operations (text, map, list, etc.)
    ///
    /// Use this to get/create containers and perform actual layer edits.
    /// Example: `layer.loro().get_text("content")`
    pub fn loro(&self) -> &LoroDoc {
        &self.inner
    }

    /// Export the layer state as a JSON value
    ///
    /// Returns the deep value representation of the document state.
    /// This is useful for reading the current state as JSON.
    pub fn to_json_value(&self) -> serde_json::Value {
        let loro_value = self.inner.get_deep_value();
        // Convert LoroValue to serde_json::Value
        loro_value_to_json(loro_value)
    }

    /// Alias for to_json_value (for consistency)
    pub fn to_json(&self) -> serde_json::Value {
        self.to_json_value()
    }

    // =========================================================================
    // CRDT-Specific Operations
    // =========================================================================

    /// Push an item to a list at the given path
    ///
    /// **Context**: Template action `append(item)` triggers this
    /// **We do**: Find/create the list, push the item
    pub fn list_push(&self, path: &str, item: &serde_json::Value) -> Result<(), LayerError> {
        let root = self.inner.get_map("root");

        // Get or create the list at this path
        let list = match root.get(path) {
            Some(loro::ValueOrContainer::Container(loro::Container::List(list))) => list,
            Some(_) => return Err(LayerError::Import(format!("Path '{}' is not a list", path))),
            None => {
                // Create new list
                root.insert_container(path, loro::LoroList::new())
                    .map_err(|e| LayerError::Import(e.to_string()))?
            }
        };

        // Push the item
        let loro_value = json_to_loro_value(item);
        list.push(loro_value)
            .map_err(|e| LayerError::Import(e.to_string()))?;

        self.inner.commit();
        Ok(())
    }

    /// Insert an item at a specific index in a list
    pub fn list_insert(&self, path: &str, index: usize, item: &serde_json::Value) -> Result<(), LayerError> {
        let root = self.inner.get_map("root");

        let list = match root.get(path) {
            Some(loro::ValueOrContainer::Container(loro::Container::List(list))) => list,
            Some(_) => return Err(LayerError::Import(format!("Path '{}' is not a list", path))),
            None => {
                root.insert_container(path, loro::LoroList::new())
                    .map_err(|e| LayerError::Import(e.to_string()))?
            }
        };

        let loro_value = json_to_loro_value(item);
        list.insert(index, loro_value)
            .map_err(|e| LayerError::Import(e.to_string()))?;

        self.inner.commit();
        Ok(())
    }

    /// Get an item at a specific index from a list by layer name
    ///
    /// **Context**: Used by derivation to fetch full item for field-level updates
    /// **Note**: This uses the layer_name as the list container name (not path in root map)
    pub fn list_get(&self, layer_name: &str, index: usize) -> Result<serde_json::Value, LayerError> {
        let list = self.inner.get_list(layer_name);

        let value = list.get(index)
            .ok_or_else(|| LayerError::Import(format!("Index {} out of bounds", index)))?;

        // Convert LoroValue to JSON
        Ok(loro_value_to_json(value.into_value().unwrap_or(loro::LoroValue::Null)))
    }

    /// Delete an item at a specific index from a list
    pub fn list_delete(&self, path: &str, index: usize) -> Result<(), LayerError> {
        let root = self.inner.get_map("root");

        let list = match root.get(path) {
            Some(loro::ValueOrContainer::Container(loro::Container::List(list))) => list,
            Some(_) => return Err(LayerError::Import(format!("Path '{}' is not a list", path))),
            None => return Ok(()), // Nothing to delete
        };

        list.delete(index, 1)
            .map_err(|e| LayerError::Import(e.to_string()))?;

        self.inner.commit();
        Ok(())
    }

    /// Insert a key-value pair into a map at the given path
    pub fn map_insert(&self, path: &str, key: &str, value: &serde_json::Value) -> Result<(), LayerError> {
        let root = self.inner.get_map("root");

        let map = match root.get(path) {
            Some(loro::ValueOrContainer::Container(loro::Container::Map(map))) => map,
            Some(_) => return Err(LayerError::Import(format!("Path '{}' is not a map", path))),
            None => {
                root.insert_container(path, loro::LoroMap::new())
                    .map_err(|e| LayerError::Import(e.to_string()))?
            }
        };

        let loro_value = json_to_loro_value(value);
        map.insert(key, loro_value)
            .map_err(|e| LayerError::Import(e.to_string()))?;

        self.inner.commit();
        Ok(())
    }

    /// Delete a key from a map at the given path
    pub fn map_delete(&self, path: &str, key: &str) -> Result<(), LayerError> {
        let root = self.inner.get_map("root");

        let map = match root.get(path) {
            Some(loro::ValueOrContainer::Container(loro::Container::Map(map))) => map,
            Some(_) => return Err(LayerError::Import(format!("Path '{}' is not a map", path))),
            None => return Ok(()), // Nothing to delete
        };

        map.delete(key).ok(); // Ignore if key doesn't exist

        self.inner.commit();
        Ok(())
    }

    /// Increment a counter value
    ///
    /// **Note**: Counter support requires loro "counter" feature.
    /// For now, we emulate with a numeric value in the root map.
    pub fn counter_inc(&self, path: &str, amount: i64) -> Result<(), LayerError> {
        let root = self.inner.get_map("root");

        // Get current value or default to 0
        let current: i64 = match root.get(path) {
            Some(loro::ValueOrContainer::Value(loro::LoroValue::I64(n))) => n,
            Some(loro::ValueOrContainer::Value(loro::LoroValue::Double(f))) => f as i64,
            _ => 0,
        };

        // Update with new value
        let new_value = current + amount;
        root.insert(path, new_value)
            .map_err(|e| LayerError::Import(e.to_string()))?;

        self.inner.commit();
        Ok(())
    }

    // =========================================================================
    // Legacy JSON Operations
    // =========================================================================

    /// Set a value at a path from JSON (for UI commits)
    ///
    /// **Context**: CEL `commit()` returns JSON values that need to be stored in Loro.
    /// **We do**: Convert JSON to Loro containers and update the document.
    ///
    /// **Path**: Currently supports root-level keys only (e.g., "messages")
    /// **Note**: Prefer typed CRDT operations (list_push, map_insert) for proper merging
    pub fn set_from_json(&self, path: &str, value: &serde_json::Value) -> Result<(), LayerError> {
        // Get or create the root map
        let root = self.inner.get_map("root");

        // Set the value based on type
        match value {
            serde_json::Value::Array(arr) => {
                // For arrays, we need to clear and repopulate the list
                // First, delete the existing list if any
                root.delete(path).ok();

                // Create a new list at this path
                let list = root.insert_container(path, loro::LoroList::new())
                    .map_err(|e| LayerError::Import(e.to_string()))?;

                // Insert each item
                for item in arr {
                    let loro_value = json_to_loro_value(item);
                    list.push(loro_value)
                        .map_err(|e| LayerError::Import(e.to_string()))?;
                }
            }
            serde_json::Value::Object(_) => {
                // For objects, insert as a map
                let loro_value = json_to_loro_value(value);
                root.insert(path, loro_value)
                    .map_err(|e| LayerError::Import(e.to_string()))?;
            }
            serde_json::Value::String(s) => {
                root.insert(path, s.clone())
                    .map_err(|e| LayerError::Import(e.to_string()))?;
            }
            serde_json::Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    root.insert(path, i)
                        .map_err(|e| LayerError::Import(e.to_string()))?;
                } else if let Some(f) = n.as_f64() {
                    root.insert(path, f)
                        .map_err(|e| LayerError::Import(e.to_string()))?;
                }
            }
            serde_json::Value::Bool(b) => {
                root.insert(path, *b)
                    .map_err(|e| LayerError::Import(e.to_string()))?;
            }
            serde_json::Value::Null => {
                root.delete(path).ok();
            }
        }

        // Commit the transaction
        self.inner.commit();

        Ok(())
    }

    // =========================================================================
    // App Layer Operations (LoroMap of file paths -> content)
    // =========================================================================

    /// Get a file's content from an app layer
    ///
    /// **Context**: App layers store files as LoroMap { "path/to/file.lua": "content" }
    /// **Returns**: File content as string, or None if not found
    pub fn get_file(&self, file_path: &str) -> Option<String> {
        let root = self.inner.get_map("files");
        match root.get(file_path) {
            Some(loro::ValueOrContainer::Value(loro::LoroValue::String(s))) => Some(s.to_string()),
            _ => None,
        }
    }

    /// Set a file's content in an app layer
    ///
    /// **Context**: Store or update a file in the app layer
    /// **We do**: Insert/update the file path -> content mapping
    pub fn set_file(&self, file_path: &str, content: &str) -> Result<(), LayerError> {
        let root = self.inner.get_map("files");
        root.insert(file_path, content.to_string())
            .map_err(|e| LayerError::Import(e.to_string()))?;
        self.inner.commit();
        Ok(())
    }

    /// List all file paths in an app layer
    ///
    /// **Context**: Get list of all files stored in this app layer
    /// **Returns**: Vector of file paths
    pub fn list_files(&self) -> Vec<String> {
        let root = self.inner.get_map("files");
        root.keys().map(|k| k.to_string()).collect()
    }

    /// Delete a file from an app layer
    ///
    /// **Context**: Remove a file from the app layer
    pub fn delete_file(&self, file_path: &str) -> Result<(), LayerError> {
        let root = self.inner.get_map("files");
        root.delete(file_path).ok(); // Ignore if doesn't exist
        self.inner.commit();
        Ok(())
    }

    /// Get all files as a HashMap (for bulk operations)
    ///
    /// **Context**: Load all app files at once
    /// **Returns**: HashMap of file_path -> content
    pub fn get_all_files(&self) -> std::collections::HashMap<String, String> {
        let root = self.inner.get_map("files");
        let mut files = std::collections::HashMap::new();
        for key in root.keys() {
            if let Some(loro::ValueOrContainer::Value(loro::LoroValue::String(s))) = root.get(&key) {
                files.insert(key.to_string(), s.to_string());
            }
        }
        files
    }

    /// Set multiple files at once (for bulk import)
    ///
    /// **Context**: Import all app files in one operation
    /// **We do**: Clear existing files and set new ones
    pub fn set_all_files(&self, files: &std::collections::HashMap<String, String>) -> Result<(), LayerError> {
        let root = self.inner.get_map("files");

        // Clear existing files
        for key in root.keys() {
            root.delete(&key).ok();
        }

        // Insert new files
        for (path, content) in files {
            root.insert(path, content.clone())
                .map_err(|e| LayerError::Import(e.to_string()))?;
        }

        self.inner.commit();
        Ok(())
    }

    // =========================================================================
    // Observer/Subscription Methods
    // =========================================================================

    /// Subscribe to Loro changes and get delta information
    ///
    /// **Context**: Subscribe to all changes in this layer's CRDT
    /// **Callback receives**: DiffEvent containing delta operations
    /// **Returns**: Subscription handle that must be kept alive
    ///
    /// **Important**: The returned Subscription must be stored! If dropped, the subscription is cancelled.
    pub fn subscribe_root<F>(&self, callback: F) -> loro::Subscription
    where
        F: (for<'a> Fn(loro::event::DiffEvent<'a>)) + 'static + Send + Sync,
    {
        use std::sync::Arc;
        self.inner.subscribe_root(Arc::new(callback))
    }
}

/// Convert LoroValue to serde_json::Value
fn loro_value_to_json(value: loro::LoroValue) -> serde_json::Value {
    match value {
        loro::LoroValue::Null => serde_json::Value::Null,
        loro::LoroValue::Bool(b) => serde_json::Value::Bool(b),
        loro::LoroValue::I64(n) => serde_json::json!(n),
        loro::LoroValue::Double(f) => serde_json::json!(f),
        loro::LoroValue::String(s) => serde_json::Value::String(s.to_string()),
        loro::LoroValue::Binary(b) => {
            // Convert binary to base64 string
            use base64::Engine;
            serde_json::Value::String(base64::engine::general_purpose::STANDARD.encode(&*b))
        }
        loro::LoroValue::List(arr) => {
            serde_json::Value::Array(arr.iter().cloned().map(loro_value_to_json).collect())
        }
        loro::LoroValue::Map(map) => {
            let obj: serde_json::Map<String, serde_json::Value> = map
                .iter()
                .map(|(k, v)| (k.to_string(), loro_value_to_json(v.clone())))
                .collect();
            serde_json::Value::Object(obj)
        }
        loro::LoroValue::Container(_) => {
            // Containers are resolved in get_deep_value, shouldn't appear here
            serde_json::Value::Null
        }
    }
}

/// Convert serde_json::Value to LoroValue (for set_from_json)
fn json_to_loro_value(value: &serde_json::Value) -> loro::LoroValue {
    match value {
        serde_json::Value::Null => loro::LoroValue::Null,
        serde_json::Value::Bool(b) => loro::LoroValue::Bool(*b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                loro::LoroValue::I64(i)
            } else if let Some(f) = n.as_f64() {
                loro::LoroValue::Double(f)
            } else {
                loro::LoroValue::Null
            }
        }
        serde_json::Value::String(s) => loro::LoroValue::String(s.clone().into()),
        serde_json::Value::Array(arr) => {
            let list: Vec<loro::LoroValue> = arr.iter().map(json_to_loro_value).collect();
            loro::LoroValue::List(list.into())
        }
        serde_json::Value::Object(obj) => {
            let map: std::collections::HashMap<String, loro::LoroValue> = obj
                .iter()
                .map(|(k, v)| (k.clone(), json_to_loro_value(v)))
                .collect();
            loro::LoroValue::Map(map.into())
        }
    }
}

impl Default for Layer {
    fn default() -> Self {
        Self::new()
    }
}

/// Layer operation errors
#[derive(Debug, Clone)]
pub enum LayerError {
    Import(String),
    Export(String),
    Decode(String),
}

impl std::fmt::Display for LayerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LayerError::Import(e) => write!(f, "Import error: {}", e),
            LayerError::Export(e) => write!(f, "Export error: {}", e),
            LayerError::Decode(e) => write!(f, "Decode error: {}", e),
        }
    }
}

impl std::error::Error for LayerError {}
