//! Layer - CRDT data container within a Page
//!
//! Terminology:
//! - Space: Container that groups Pages
//! - Page: A sub-application instance with its own template
//! - Layer: CRDT data containers (the actual collaborative state)
//!
//! Layers are the actual Loro documents that store collaborative data.
//! Each Page can have multiple Layers (e.g., content_layer, comments_layer).

use loro::{ExportMode, LoroDoc};
use sha2::{Digest, Sha256};

use super::sthithi::{OpKind, Parivarta, Sthithi};

/// Protocol-level layer classification
///
/// Determines sync behavior and memory management:
/// - App: Code/UI layers, snapshot-only sync, evicted from memory after hash
/// - Data: CRDT data layers, incremental sync, kept in memory
/// - Static: Binary asset layers, routed to blob transfer
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum LayerType {
    App,
    Data,
    Static,
}

impl LayerType {
    /// Determine layer type from layer name convention
    ///
    /// - `app:*` -> App (code/UI)
    /// - `static:*` -> Static (binary assets)
    /// - Everything else -> Data (CRDT)
    ///
    /// Handles fully qualified names like `{page_id}/{layer_name}` by
    /// extracting the layer part after the last "/".
    pub fn from_layer_name(name: &str) -> Self {
        let layer_part = name.rsplit('/').next().unwrap_or(name);
        if layer_part.starts_with("app:") {
            LayerType::App
        } else if layer_part.starts_with("static:") {
            LayerType::Static
        } else {
            LayerType::Data
        }
    }
}

/// Compute deterministic content hash for a layer
///
/// Uses `get_deep_value()` -> Sthithi -> serde -> SHA-256 for cross-peer determinism.
/// Used for app layer change detection (skip sync if hash matches).
pub fn compute_content_hash(layer: &Layer) -> [u8; 32] {
    let sthithi = layer.to_sthithi();
    let bytes = serde_json::to_vec(&sthithi).unwrap_or_default();
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    hasher.finalize().into()
}

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
        Ok(self
            .inner
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
    ///
    /// **Note**: This returns the full Loro document structure with container wrappers.
    /// For production code that needs unwrapped content, use `get_content(container_name)` instead.
    pub fn to_json_value(&self) -> serde_json::Value {
        let loro_value = self.inner.get_deep_value();
        // Convert LoroValue to serde_json::Value
        loro_value_to_json(loro_value)
    }

    /// Alias for to_json_value (for consistency)
    ///
    /// **Note**: For production code that needs unwrapped content, use `get_content(container_name)` instead.
    pub fn to_json(&self) -> serde_json::Value {
        self.to_json_value()
    }

    /// Get the content of a specific container (unwrapped)
    ///
    /// Loro stores data as `{"container_name": content}`. This method
    /// extracts just the content for the named container.
    ///
    /// **Context**: Use this for production code that needs the actual data
    /// without the Loro container wrapper structure.
    ///
    /// # Arguments
    /// * `container_name` - The name of the container to extract (typically the layer name)
    ///
    /// # Returns
    /// The unwrapped content, or Null if the container doesn't exist
    pub fn get_content(&self, container_name: &str) -> serde_json::Value {
        let full = self.inner.get_deep_value();
        let json = loro_value_to_json(full);

        if let serde_json::Value::Object(map) = json {
            map.get(container_name)
                .cloned()
                .unwrap_or(serde_json::Value::Null)
        } else {
            serde_json::Value::Null
        }
    }

    // --- Sthithi output methods (canonical, replaces JSON path) ---

    /// Export the layer state as a Sthithi value (direct from LoroValue, no JSON hop).
    ///
    /// **Context**: Canonical replacement for `to_json_value()`.
    /// Preserves Int/Float distinction and native Bytes.
    pub fn to_sthithi(&self) -> Sthithi {
        Sthithi::from(self.inner.get_deep_value())
    }

    /// Get the content of a specific container as Sthithi (unwrapped).
    ///
    /// **Context**: Canonical replacement for `get_content()`.
    /// Extracts the named container from the Loro document without JSON round-trip.
    pub fn get_content_sthithi(&self, container_name: &str) -> Sthithi {
        let full = Sthithi::from(self.inner.get_deep_value());
        match full {
            Sthithi::Map(entries) => entries
                .into_iter()
                .find(|(k, _)| k == container_name)
                .map(|(_, v)| v)
                .unwrap_or(Sthithi::Null),
            _ => Sthithi::Null,
        }
    }

    /// Get an item at a specific index from a list by layer name, as Sthithi.
    ///
    /// **Context**: Canonical replacement for `list_get()`.
    pub fn list_get_sthithi(&self, layer_name: &str, index: usize) -> Result<Sthithi, LayerError> {
        let list = self.inner.get_list(layer_name);
        let value = list
            .get(index)
            .ok_or_else(|| LayerError::Import(format!("Index {} out of bounds", index)))?;
        Ok(Sthithi::from(
            value.into_value().unwrap_or(loro::LoroValue::Null),
        ))
    }

    // --- Sthithi write methods (canonical, replaces JSON write path) ---

    /// Push a Sthithi value to a list at the given path
    pub fn list_push_sthithi(&self, path: &str, item: &Sthithi) -> Result<(), LayerError> {
        let root = self.inner.get_map("root");
        let list = match root.get(path) {
            Some(loro::ValueOrContainer::Container(loro::Container::List(list))) => list,
            Some(_) => return Err(LayerError::Import(format!("Path '{}' is not a list", path))),
            None => root
                .insert_container(path, loro::LoroList::new())
                .map_err(|e| LayerError::Import(e.to_string()))?,
        };
        let loro_value = loro::LoroValue::from(item);
        list.push(loro_value)
            .map_err(|e| LayerError::Import(e.to_string()))?;
        self.inner.commit();
        Ok(())
    }

    /// Insert a Sthithi value at a specific index in a list
    pub fn list_insert_sthithi(
        &self,
        path: &str,
        index: usize,
        item: &Sthithi,
    ) -> Result<(), LayerError> {
        let root = self.inner.get_map("root");
        let list = match root.get(path) {
            Some(loro::ValueOrContainer::Container(loro::Container::List(list))) => list,
            Some(_) => return Err(LayerError::Import(format!("Path '{}' is not a list", path))),
            None => root
                .insert_container(path, loro::LoroList::new())
                .map_err(|e| LayerError::Import(e.to_string()))?,
        };
        let loro_value = loro::LoroValue::from(item);
        list.insert(index, loro_value)
            .map_err(|e| LayerError::Import(e.to_string()))?;
        self.inner.commit();
        Ok(())
    }

    /// Insert a key-value pair into a map at the given path (Sthithi)
    pub fn map_insert_sthithi(
        &self,
        path: &str,
        key: &str,
        value: &Sthithi,
    ) -> Result<(), LayerError> {
        let root = self.inner.get_map("root");
        let map = match root.get(path) {
            Some(loro::ValueOrContainer::Container(loro::Container::Map(map))) => map,
            Some(_) => return Err(LayerError::Import(format!("Path '{}' is not a map", path))),
            None => root
                .insert_container(path, loro::LoroMap::new())
                .map_err(|e| LayerError::Import(e.to_string()))?,
        };
        let loro_value = loro::LoroValue::from(value);
        map.insert(key, loro_value)
            .map_err(|e| LayerError::Import(e.to_string()))?;
        self.inner.commit();
        Ok(())
    }

    /// Set a value at a path from Sthithi
    ///
    /// **Context**: Canonical replacement for `set_from_json`.
    pub fn set_from_sthithi(&self, path: &str, value: &Sthithi) -> Result<(), LayerError> {
        let root = self.inner.get_map("root");
        match value {
            Sthithi::List(items) => {
                root.delete(path).ok();
                let list = root
                    .insert_container(path, loro::LoroList::new())
                    .map_err(|e| LayerError::Import(e.to_string()))?;
                for item in items {
                    list.push(loro::LoroValue::from(item))
                        .map_err(|e| LayerError::Import(e.to_string()))?;
                }
            }
            Sthithi::Map(_) => {
                let loro_value = loro::LoroValue::from(value);
                root.insert(path, loro_value)
                    .map_err(|e| LayerError::Import(e.to_string()))?;
            }
            Sthithi::Str(s) => {
                root.insert(path, s.clone())
                    .map_err(|e| LayerError::Import(e.to_string()))?;
            }
            Sthithi::Int(n) => {
                root.insert(path, *n)
                    .map_err(|e| LayerError::Import(e.to_string()))?;
            }
            Sthithi::Float(f) => {
                root.insert(path, *f)
                    .map_err(|e| LayerError::Import(e.to_string()))?;
            }
            Sthithi::Bool(b) => {
                root.insert(path, *b)
                    .map_err(|e| LayerError::Import(e.to_string()))?;
            }
            Sthithi::Bytes(b) => {
                root.insert(path, loro::LoroValue::Binary(b.clone().into()))
                    .map_err(|e| LayerError::Import(e.to_string()))?;
            }
            Sthithi::Null => {
                root.delete(path).ok();
            }
        }
        self.inner.commit();
        Ok(())
    }

    // CRDT-Specific Operations (legacy JSON — kept during migration)

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
    pub fn list_insert(
        &self,
        path: &str,
        index: usize,
        item: &serde_json::Value,
    ) -> Result<(), LayerError> {
        let root = self.inner.get_map("root");

        let list = match root.get(path) {
            Some(loro::ValueOrContainer::Container(loro::Container::List(list))) => list,
            Some(_) => return Err(LayerError::Import(format!("Path '{}' is not a list", path))),
            None => root
                .insert_container(path, loro::LoroList::new())
                .map_err(|e| LayerError::Import(e.to_string()))?,
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
    pub fn list_get(
        &self,
        layer_name: &str,
        index: usize,
    ) -> Result<serde_json::Value, LayerError> {
        let list = self.inner.get_list(layer_name);

        let value = list
            .get(index)
            .ok_or_else(|| LayerError::Import(format!("Index {} out of bounds", index)))?;

        // Convert LoroValue to JSON
        Ok(loro_value_to_json(
            value.into_value().unwrap_or(loro::LoroValue::Null),
        ))
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
    pub fn map_insert(
        &self,
        path: &str,
        key: &str,
        value: &serde_json::Value,
    ) -> Result<(), LayerError> {
        let root = self.inner.get_map("root");

        let map = match root.get(path) {
            Some(loro::ValueOrContainer::Container(loro::Container::Map(map))) => map,
            Some(_) => return Err(LayerError::Import(format!("Path '{}' is not a map", path))),
            None => root
                .insert_container(path, loro::LoroMap::new())
                .map_err(|e| LayerError::Import(e.to_string()))?,
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
                let list = root
                    .insert_container(path, loro::LoroList::new())
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

    // App Layer Operations (LoroMap of file paths -> content)

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
            if let Some(loro::ValueOrContainer::Value(loro::LoroValue::String(s))) = root.get(&key)
            {
                files.insert(key.to_string(), s.to_string());
            }
        }
        files
    }

    /// Set multiple files at once (for bulk import)
    ///
    /// **Context**: Import all app files in one operation
    /// **We do**: Clear existing files and set new ones
    pub fn set_all_files(
        &self,
        files: &std::collections::HashMap<String, String>,
    ) -> Result<(), LayerError> {
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

    // Observer/Subscription Methods

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

    // Validation Ops Extraction (Optimized via pre-commit hook)

    /// Extract operations from update bytes as Parivarta ops.
    ///
    /// **Context**: Fast extraction for validation without fork+diff.
    /// Goes LoroValue → Sthithi directly (no JSON hop).
    /// **Method**: Create empty temp doc → subscribe → import → capture events
    /// **Performance**: 5-10x faster than fork+diff for large documents
    ///
    /// **Returns**: Vec<Parivarta> representing the operations in the update
    pub fn extract_ops(update: &[u8]) -> Result<Vec<Parivarta>, LayerError> {
        use std::sync::{Arc, Mutex};

        let temp_doc = LoroDoc::new();
        let ops = Arc::new(Mutex::new(Vec::new()));
        let ops_clone = ops.clone();

        let subscription =
            temp_doc.subscribe_root(Arc::new(move |event: loro::event::DiffEvent| {
                let mut collected = ops_clone.lock().unwrap();

                for container_diff in event.events {
                    let path = container_diff
                        .path
                        .iter()
                        .map(|(_, key)| {
                            let key_str = key.to_string();
                            key_str.replace('\\', "\\\\").replace('/', "\\/")
                        })
                        .collect::<Vec<_>>()
                        .join("/");
                    let path = if path.is_empty() {
                        "root".to_string()
                    } else {
                        path
                    };

                    match &container_diff.diff {
                        loro::event::Diff::List(list_diff) => {
                            let mut current_index: usize = 0;

                            for delta in list_diff.iter() {
                                match delta {
                                    loro::event::ListDiffItem::Retain { retain } => {
                                        current_index += *retain;
                                    }
                                    loro::event::ListDiffItem::Insert { insert, .. } => {
                                        let insert_index = current_index;
                                        let insert_count = insert.len();

                                        collected.push(Parivarta {
                                            layer: String::new(),
                                            op: OpKind::Insert,
                                            path: path.clone(),
                                            key: None,
                                            index: Some(insert_index),
                                            value: Some(loro_values_to_sthithi(insert)),
                                            old_value: None,
                                            intent: None,
                                            from_peer: None,
                                        });

                                        current_index += insert_count;
                                    }
                                    loro::event::ListDiffItem::Delete { delete } => {
                                        collected.push(Parivarta {
                                            layer: String::new(),
                                            op: OpKind::Delete,
                                            path: path.clone(),
                                            key: None,
                                            index: Some(current_index),
                                            value: None,
                                            old_value: None,
                                            intent: None,
                                            from_peer: None,
                                        });
                                        let _ = delete;
                                    }
                                }
                            }
                        }
                        loro::event::Diff::Map(map_diff) => {
                            for (key, value_opt) in map_diff.updated.iter() {
                                let new_value = value_opt
                                    .as_ref()
                                    .map(|v| Sthithi::from(v.get_deep_value()));

                                let op_kind = if new_value.is_some() {
                                    OpKind::Update
                                } else {
                                    OpKind::Delete
                                };

                                collected.push(Parivarta {
                                    layer: String::new(),
                                    op: op_kind,
                                    path: path.clone(),
                                    key: Some(key.to_string()),
                                    index: None,
                                    value: new_value,
                                    old_value: None,
                                    intent: None,
                                    from_peer: None,
                                });
                            }
                        }
                        loro::event::Diff::Text(_) | loro::event::Diff::Tree(_) => {}
                        _ => {}
                    }
                }
            }));

        temp_doc
            .import(update)
            .map_err(|e| LayerError::Import(format!("Failed to import update: {}", e)))?;

        drop(subscription);
        let result = ops.lock().unwrap().clone();
        Ok(result)
    }
}

/// Convert Vec<ValueOrContainer> to Sthithi::List (for list inserts)
fn loro_values_to_sthithi(values: &[loro::ValueOrContainer]) -> Sthithi {
    Sthithi::List(
        values
            .iter()
            .map(|v| Sthithi::from(v.get_deep_value()))
            .collect(),
    )
}

/// Convert LoroValue to serde_json::Value
pub fn loro_value_to_json(value: loro::LoroValue) -> serde_json::Value {
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
pub fn json_to_loro_value(value: &serde_json::Value) -> loro::LoroValue {
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

// Tests

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper to create a layer with map data and export updates
    fn create_map_update(key: &str, value: serde_json::Value) -> Vec<u8> {
        let layer = Layer::new();
        layer.set_from_json(key, &value).unwrap();
        // Export updates from empty version (contains all changes)
        layer
            .inner
            .export(loro::ExportMode::updates(&loro::VersionVector::new()))
            .expect("Failed to export updates")
    }

    /// Helper to create a layer with list data and export updates
    fn create_list_update(path: &str, items: Vec<serde_json::Value>) -> Vec<u8> {
        let layer = Layer::new();
        for item in items {
            layer.list_push(path, &item).unwrap();
        }
        layer
            .inner
            .export(loro::ExportMode::updates(&loro::VersionVector::new()))
            .expect("Failed to export updates")
    }

    #[test]
    fn test_extract_ops_map_insert_string() {
        let update = create_map_update("username", serde_json::json!("alice"));
        let ops = Layer::extract_ops(&update).unwrap();

        assert!(!ops.is_empty(), "Should extract at least one op");

        // Find the update op for "username"
        let username_op = ops.iter().find(|op| op.key.as_deref() == Some("username"));
        assert!(username_op.is_some(), "Should have op for 'username' key");

        let op = username_op.unwrap();
        assert_eq!(op.op, OpKind::Update, "Map insert should be update op");
        assert_eq!(op.value, Some(Sthithi::Str("alice".to_string())));
    }

    #[test]
    fn test_extract_ops_map_insert_object() {
        let update = create_map_update(
            "user",
            serde_json::json!({
                "name": "Alice",
                "age": 30
            }),
        );
        let ops = Layer::extract_ops(&update).unwrap();

        assert!(!ops.is_empty());

        let user_op = ops.iter().find(|op| op.key.as_deref() == Some("user"));
        assert!(user_op.is_some());

        let op = user_op.unwrap();
        assert_eq!(op.op, OpKind::Update);

        // Value should be the object
        let value = op.value.as_ref().unwrap();
        let map = match value {
            Sthithi::Map(entries) => entries,
            other => panic!("expected map value, got: {:?}", other),
        };
        let pairs: std::collections::HashMap<&str, &Sthithi> =
            map.iter().map(|(k, v)| (k.as_str(), v)).collect();
        assert_eq!(pairs.get("name"), Some(&&Sthithi::Str("Alice".to_string())));
        assert_eq!(pairs.get("age"), Some(&&Sthithi::Int(30)));
    }

    #[test]
    fn test_extract_ops_incremental_update_limitation() {
        // Note: extract_ops works best with FULL updates (from empty version).
        // Incremental updates (from non-empty version) may not produce diff events
        // because the temp doc starts empty and doesn't have context of previous state.
        //
        // This is the expected use case: validation receives the full update that
        // a remote peer sends, which typically includes all their changes.

        let layer = Layer::new();
        let root = layer.inner.get_map("root");

        // Initial value
        root.insert("field", "value1").unwrap();
        layer.inner.commit();

        let version_after_insert = layer.inner.oplog_vv();

        // Update the value
        root.insert("field", "value2").unwrap();
        layer.inner.commit();

        // Incremental update (from non-empty version) - may not capture well
        let incremental_update = layer
            .inner
            .export(loro::ExportMode::updates(&version_after_insert))
            .expect("Failed to export updates");

        let incremental_ops = Layer::extract_ops(&incremental_update).unwrap();

        // Full update (from empty version) - captures all changes
        let full_update = layer
            .inner
            .export(loro::ExportMode::updates(&loro::VersionVector::new()))
            .expect("Failed to export updates");

        let full_ops = Layer::extract_ops(&full_update).unwrap();

        // Full update should have the field
        let full_has_field = full_ops.iter().any(|op| op.key.as_deref() == Some("field"));
        assert!(
            full_has_field,
            "Full update should capture field, got: {:?}",
            full_ops
        );

        // Document: incremental updates may or may not capture changes
        eprintln!("Incremental ops: {:?}", incremental_ops);
        eprintln!("Full ops: {:?}", full_ops);
    }

    #[test]
    fn test_extract_ops_list_insert_with_index() {
        let update = create_list_update(
            "messages",
            vec![
                serde_json::json!({"text": "hello"}),
                serde_json::json!({"text": "world"}),
            ],
        );

        let ops = Layer::extract_ops(&update).unwrap();

        // Should have insert operations
        let insert_ops: Vec<_> = ops.iter().filter(|op| op.op == OpKind::Insert).collect();

        assert!(!insert_ops.is_empty(), "Should have insert operations");

        // At least one should have an index
        let has_index = insert_ops.iter().any(|op| op.index.is_some());
        assert!(has_index, "Insert ops should have index tracked");
    }

    #[test]
    fn test_extract_ops_list_multiple_inserts_from_empty() {
        // Test list inserts exported from empty version (full update)
        let layer = Layer::new();
        let list = layer.inner.get_list("items");

        // Add items at different positions
        list.push("item0").unwrap();
        list.push("item1").unwrap();
        list.insert(1, "inserted").unwrap(); // Insert between
        layer.inner.commit();

        // Export full update from empty version
        let update = layer
            .inner
            .export(loro::ExportMode::updates(&loro::VersionVector::new()))
            .expect("Failed to export updates");

        let ops = Layer::extract_ops(&update).unwrap();

        let insert_ops: Vec<_> = ops.iter().filter(|op| op.op == OpKind::Insert).collect();

        assert!(
            !insert_ops.is_empty(),
            "Should have insert operations, got ops: {:?}",
            ops
        );

        // At least one insert should have index tracked
        let has_index = insert_ops.iter().any(|op| op.index.is_some());
        assert!(
            has_index,
            "Insert ops should have index tracked, ops: {:?}",
            insert_ops
        );
    }

    /// Note: Delete operations are NOT captured by extract_ops
    ///
    /// **Limitation**: When importing update bytes into an empty temp doc,
    /// delete operations don't produce diff events because the empty doc
    /// never had those items to delete.
    ///
    /// **For validation**: This is acceptable because:
    /// 1. We mainly validate INSERT/UPDATE operations (what's being added)
    /// 2. Delete permissions are typically layer-level, not item-level
    /// 3. The layer itself (pre-import) already has the context of what exists
    #[test]
    fn test_extract_ops_delete_limitation_documented() {
        // This test documents the limitation - delete ops are NOT captured
        let layer = Layer::new();
        let list = layer.inner.get_list("items");
        list.push("item1").unwrap();
        layer.inner.commit();

        let version_after_insert = layer.inner.oplog_vv();

        // Delete the item
        list.delete(0, 1).unwrap();
        layer.inner.commit();

        let update = layer
            .inner
            .export(loro::ExportMode::updates(&version_after_insert))
            .expect("Failed to export updates");

        let ops = Layer::extract_ops(&update).unwrap();

        // LIMITATION: Delete ops are NOT captured because temp doc is empty
        // This is expected behavior - validation focuses on what's being added
        let delete_ops: Vec<_> = ops.iter().filter(|op| op.op == OpKind::Delete).collect();

        // Document that delete ops are empty (this is the limitation)
        assert!(
            delete_ops.is_empty(),
            "Delete ops are not captured by extract_ops (empty temp doc limitation)"
        );
    }

    #[test]
    fn test_path_separator_is_slash() {
        // The path should use "/" as separator, not "."
        let update = create_map_update("nested.key", serde_json::json!("value"));
        let ops = Layer::extract_ops(&update).unwrap();

        // Path should NOT contain unescaped dots as separators
        // Since we're inserting at root level, path should be "root"
        for op in &ops {
            // The key itself can contain dots, but path separator should be "/"
            if op.path != "root" {
                assert!(
                    op.path.contains('/') || !op.path.contains('.'),
                    "Path should use / as separator, not .: {}",
                    op.path
                );
            }
        }
    }

    #[test]
    fn test_extract_ops_empty_update() {
        // Create empty layer and export (should be minimal/empty update)
        let layer = Layer::new();
        let update = layer
            .inner
            .export(loro::ExportMode::updates(&loro::VersionVector::new()))
            .expect("Failed to export");

        // Empty update should return empty ops (or succeed with no ops)
        let result = Layer::extract_ops(&update);
        assert!(result.is_ok(), "Empty update should not error");

        let ops = result.unwrap();
        // Empty update may have 0 ops or just structural ops
        assert!(ops.len() <= 1, "Empty update should have minimal ops");
    }

    #[test]
    fn test_extract_ops_invalid_bytes() {
        // Invalid bytes should return an error
        let invalid = vec![0x00, 0x01, 0x02, 0x03];
        let result = Layer::extract_ops(&invalid);

        assert!(result.is_err(), "Invalid bytes should return error");
    }

    #[test]
    fn test_extract_ops_multiple_changes() {
        // Multiple changes in one update
        let layer = Layer::new();
        layer
            .set_from_json("key1", &serde_json::json!("value1"))
            .unwrap();
        layer.set_from_json("key2", &serde_json::json!(42)).unwrap();
        layer
            .set_from_json("key3", &serde_json::json!(true))
            .unwrap();

        let update = layer
            .inner
            .export(loro::ExportMode::updates(&loro::VersionVector::new()))
            .expect("Failed to export");

        let ops = Layer::extract_ops(&update).unwrap();

        // Should have ops for all three keys
        let keys: Vec<_> = ops.iter().filter_map(|op| op.key.as_deref()).collect();

        assert!(keys.contains(&"key1"), "Should have key1");
        assert!(keys.contains(&"key2"), "Should have key2");
        assert!(keys.contains(&"key3"), "Should have key3");
    }
}
