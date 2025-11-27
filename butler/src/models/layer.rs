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
