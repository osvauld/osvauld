use loro::{LoroDoc, LoroError, ExportMode};

/// Document - Wrapper around LoroDoc that hides loro internals
///
/// Consumers (like Butler) use this type, never LoroDoc directly.
/// All loro operations go through this wrapper.
pub struct Document {
    inner: LoroDoc,
}

impl Document {
    /// Create a new empty document
    pub fn new() -> Self {
        Self {
            inner: LoroDoc::new(),
        }
    }

    /// Create a document from snapshot bytes
    pub fn from_snapshot(snapshot_bytes: &[u8]) -> Result<Self, DocumentError> {
        let doc = LoroDoc::new();
        doc.import(snapshot_bytes)
            .map_err(|e| DocumentError::Import(e.to_string()))?;
        Ok(Self { inner: doc })
    }

    /// Apply updates/snapshot bytes to this document (merge)
    pub fn apply(&self, bytes: &[u8]) -> Result<(), DocumentError> {
        self.inner
            .import(bytes)
            .map(|_| ())
            .map_err(|e| DocumentError::Import(e.to_string()))
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
    pub fn export_updates(&self, from_version: &[u8]) -> Result<Vec<u8>, DocumentError> {
        let vv = loro::VersionVector::decode(from_version)
            .map_err(|e| DocumentError::Decode(e.to_string()))?;
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
    /// Use this to get/create containers and perform actual document edits.
    /// Example: `doc.loro().get_text("content")`
    pub fn loro(&self) -> &LoroDoc {
        &self.inner
    }
}

impl Default for Document {
    fn default() -> Self {
        Self::new()
    }
}

/// Document operation errors
#[derive(Debug, Clone)]
pub enum DocumentError {
    Import(String),
    Export(String),
    Decode(String),
}

impl std::fmt::Display for DocumentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DocumentError::Import(e) => write!(f, "Import error: {}", e),
            DocumentError::Export(e) => write!(f, "Export error: {}", e),
            DocumentError::Decode(e) => write!(f, "Decode error: {}", e),
        }
    }
}

impl std::error::Error for DocumentError {}

// =============================================================================
// Legacy functions (for existing code that uses these directly)
// TODO: Migrate existing code to use Document wrapper, then remove these
// =============================================================================

/// Create a new Loro document
#[deprecated(note = "Use Document::new() instead")]
pub fn create_doc() -> LoroDoc {
    LoroDoc::new()
}

/// Import a snapshot into a new Loro document
#[deprecated(note = "Use Document::from_snapshot() instead")]
pub fn import_snapshot(snapshot_bytes: &[u8]) -> Result<LoroDoc, LoroError> {
    let doc = LoroDoc::new();
    doc.import(snapshot_bytes)?;
    Ok(doc)
}

/// Import snapshot or updates into an existing Loro document
#[deprecated(note = "Use Document::apply() instead")]
pub fn import_into(doc: &LoroDoc, bytes: &[u8]) -> Result<(), LoroError> {
    doc.import(bytes).map(|_| ())
}

/// Export full snapshot with complete history
#[deprecated(note = "Use Document::export_snapshot() instead")]
pub fn export_snapshot(doc: &LoroDoc) -> Vec<u8> {
    doc.export(ExportMode::Snapshot)
        .expect("Failed to export Loro snapshot")
}

/// Export shallow snapshot without full history
#[deprecated(note = "Use Document::export_shallow_snapshot() instead")]
pub fn export_shallow_snapshot(doc: &LoroDoc) -> Vec<u8> {
    let frontiers = doc.state_frontiers();
    doc.export(ExportMode::shallow_snapshot(&frontiers))
        .expect("Failed to export Loro shallow snapshot")
}

/// Export updates from a specific version vector
#[deprecated(note = "Use Document::export_updates() instead")]
pub fn export_updates(doc: &LoroDoc, from_version: &[u8]) -> Result<Vec<u8>, LoroError> {
    let vv = loro::VersionVector::decode(from_version)?;
    Ok(doc.export(ExportMode::updates(&vv))
        .expect("Failed to export Loro updates"))
}

/// Apply updates to a document
#[deprecated(note = "Use Document::apply() instead")]
pub fn apply_updates(doc: &LoroDoc, updates: &[u8]) -> Result<(), LoroError> {
    doc.import(updates).map(|_| ())
}

/// Get the operation log version vector
#[deprecated(note = "Use Document::version_vector() instead")]
pub fn oplog_vv(doc: &LoroDoc) -> Vec<u8> {
    doc.oplog_vv().encode()
}

/// Get the state frontiers
#[deprecated(note = "Use Document::frontiers() instead")]
pub fn state_frontiers(doc: &LoroDoc) -> Vec<u8> {
    doc.state_frontiers().encode()
}
