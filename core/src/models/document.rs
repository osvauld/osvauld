use loro::{LoroDoc, LoroError, ExportMode};

/// Create a new Loro document
///
/// # Returns
/// * `LoroDoc` - A new Loro document ready for use
pub fn create_doc() -> LoroDoc {
    LoroDoc::new()
}

/// Import a snapshot (full or shallow) into a new Loro document
///
/// # Arguments
/// * `snapshot_bytes` - The snapshot bytes (from export_snapshot or export_shallow_snapshot)
///
/// # Returns
/// * `Result<LoroDoc, LoroError>` - A new document with imported state
pub fn import_snapshot(snapshot_bytes: &[u8]) -> Result<LoroDoc, LoroError> {
    let doc = LoroDoc::new();
    doc.import(snapshot_bytes)?;
    Ok(doc)
}

/// Import snapshot or updates into an existing Loro document
///
/// # Arguments
/// * `doc` - The document to import into
/// * `bytes` - The snapshot or update bytes to import
///
/// # Returns
/// * `Result<(), LoroError>` - Success or error
pub fn import_into(doc: &LoroDoc, bytes: &[u8]) -> Result<(), LoroError> {
    doc.import(bytes).map(|_| ())
}

/// Export full snapshot with complete history (for owner/node storage)
///
/// # Arguments
/// * `doc` - The document to export
///
/// # Returns
/// * `Vec<u8>` - Encoded snapshot with all history
pub fn export_snapshot(doc: &LoroDoc) -> Vec<u8> {
    doc.export(ExportMode::Snapshot)
        .expect("Failed to export Loro snapshot")
}

/// Export shallow snapshot without full history (for viewers)
///
/// # Arguments
/// * `doc` - The document to export
///
/// # Returns
/// * `Vec<u8>` - Encoded shallow snapshot
pub fn export_shallow_snapshot(doc: &LoroDoc) -> Vec<u8> {
    let frontiers = doc.state_frontiers();
    doc.export(ExportMode::shallow_snapshot(&frontiers))
        .expect("Failed to export Loro shallow snapshot")
}

/// Export updates from a specific version vector (for incremental sync)
///
/// # Arguments
/// * `doc` - The document to export from
/// * `from_version` - The version vector to export updates from
///
/// # Returns
/// * `Vec<u8>` - Encoded updates from the specified version
pub fn export_updates(doc: &LoroDoc, from_version: &[u8]) -> Result<Vec<u8>, LoroError> {
    // Parse the version vector
    let vv = loro::VersionVector::decode(from_version)?;
    Ok(doc.export(ExportMode::updates(&vv))
        .expect("Failed to export Loro updates"))
}

/// Apply updates to a document
///
/// # Arguments
/// * `doc` - The document to apply updates to
/// * `updates` - The update bytes to apply
///
/// # Returns
/// * `Result<(), LoroError>` - Success or error
pub fn apply_updates(doc: &LoroDoc, updates: &[u8]) -> Result<(), LoroError> {
    doc.import(updates).map(|_| ())
}

/// Get the operation log version vector (for owner/node - includes full history)
///
/// # Arguments
/// * `doc` - The document
///
/// # Returns
/// * `Vec<u8>` - Encoded version vector of the operation log
pub fn oplog_vv(doc: &LoroDoc) -> Vec<u8> {
    doc.oplog_vv().encode()
}

/// Get the state frontiers (for viewers - current state without history)
///
/// # Arguments
/// * `doc` - The document
///
/// # Returns
/// * `Vec<u8>` - Encoded frontiers representing current state
pub fn state_frontiers(doc: &LoroDoc) -> Vec<u8> {
    doc.state_frontiers().encode()
}
