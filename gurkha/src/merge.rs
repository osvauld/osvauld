//! Pure CRDT Merge Logic Layer
//!
//! Provides stateless Loro document merge operations without infrastructure dependencies.
//! This layer is pure business logic and can be FFI'd to other languages.

use loro::{ExportMode, LoroDoc, LoroError, ToJson, VersionVector};
use tracing::{debug, error, info};

/// Pure CRDT merge service for Loro documents
///
/// Provides stateless operations for merging, exporting, and managing CRDT documents.
/// No database, no network, no state - just pure logic.
pub struct MergeService;

impl MergeService {
    /// Create a new empty Loro document
    ///
    /// # Returns
    /// * `LoroDoc` - A new empty Loro document
    pub fn create_doc() -> LoroDoc {
        debug!("📄 Creating new Loro document");
        LoroDoc::new()
    }

    /// Import snapshot into a new document
    ///
    /// # Arguments
    /// * `snapshot_bytes` - Full or shallow snapshot bytes
    ///
    /// # Returns
    /// * `Ok(LoroDoc)` - Document with imported state
    /// * `Err(LoroError)` - If import fails
    pub fn import_snapshot(snapshot_bytes: &[u8]) -> Result<LoroDoc, LoroError> {
        debug!("📥 Importing snapshot ({} bytes)", snapshot_bytes.len());
        let doc = LoroDoc::new();
        doc.import(snapshot_bytes)?;
        info!("✓ Snapshot imported successfully");
        Ok(doc)
    }

    /// Merge updates into an existing document
    ///
    /// This is the core merge operation - applies updates from another document
    /// to the current document using CRDT conflict resolution.
    ///
    /// # Arguments
    /// * `doc` - Target document to merge into
    /// * `updates_bytes` - Updates to merge (from export_updates or export_snapshot)
    ///
    /// # Returns
    /// * `Ok(())` - If merge succeeds
    /// * `Err(LoroError)` - If merge fails
    pub fn merge_updates(doc: &LoroDoc, updates_bytes: &[u8]) -> Result<(), LoroError> {
        debug!("🔀 Merging updates ({} bytes)", updates_bytes.len());
        doc.import(updates_bytes)?;
        info!("✓ Updates merged successfully");
        Ok(())
    }

    /// Export full snapshot with complete history
    ///
    /// Use this for owner/node storage where full history is needed.
    ///
    /// # Arguments
    /// * `doc` - Document to export
    ///
    /// # Returns
    /// * `Vec<u8>` - Encoded snapshot with all history
    pub fn export_snapshot(doc: &LoroDoc) -> Vec<u8> {
        debug!("📤 Exporting full snapshot");
        let snapshot = doc
            .export(ExportMode::Snapshot)
            .expect("Failed to export Loro snapshot");
        info!("✓ Full snapshot exported ({} bytes)", snapshot.len());
        snapshot
    }

    /// Export shallow snapshot without full history
    ///
    /// Use this for viewers who only need current state, not full history.
    ///
    /// # Arguments
    /// * `doc` - Document to export
    ///
    /// # Returns
    /// * `Vec<u8>` - Encoded shallow snapshot
    pub fn export_shallow_snapshot(doc: &LoroDoc) -> Vec<u8> {
        debug!("📤 Exporting shallow snapshot");
        let frontiers = doc.state_frontiers();
        let snapshot = doc
            .export(ExportMode::shallow_snapshot(&frontiers))
            .expect("Failed to export Loro shallow snapshot");
        info!("✓ Shallow snapshot exported ({} bytes)", snapshot.len());
        snapshot
    }

    /// Export updates from a specific version
    ///
    /// Use this for incremental sync - only export what's changed since a version.
    ///
    /// # Arguments
    /// * `doc` - Document to export from
    /// * `from_version_bytes` - Encoded version vector to export from
    ///
    /// # Returns
    /// * `Ok(Vec<u8>)` - Encoded updates
    /// * `Err(LoroError)` - If version parsing fails
    pub fn export_updates(doc: &LoroDoc, from_version_bytes: &[u8]) -> Result<Vec<u8>, LoroError> {
        debug!(
            "📤 Exporting updates from version ({} bytes)",
            from_version_bytes.len()
        );
        let vv = VersionVector::decode(from_version_bytes)?;
        let updates = doc
            .export(ExportMode::updates(&vv))
            .expect("Failed to export Loro updates");
        info!("✓ Updates exported ({} bytes)", updates.len());
        Ok(updates)
    }

    /// Get operation log version vector (includes full history)
    ///
    /// Use this for owner/node to track version with full history.
    ///
    /// # Arguments
    /// * `doc` - Document to get version from
    ///
    /// # Returns
    /// * `Vec<u8>` - Encoded version vector
    pub fn oplog_vv(doc: &LoroDoc) -> Vec<u8> {
        debug!("📊 Getting operation log version vector");
        doc.oplog_vv().encode()
    }

    /// Get state frontiers (current state without history)
    ///
    /// Use this for viewers to track version without full history.
    ///
    /// # Arguments
    /// * `doc` - Document to get frontiers from
    ///
    /// # Returns
    /// * `Vec<u8>` - Encoded frontiers
    pub fn state_frontiers(doc: &LoroDoc) -> Vec<u8> {
        debug!("📊 Getting state frontiers");
        doc.state_frontiers().encode()
    }

    /// Log document content for debugging CRDT sync issues
    ///
    /// Exports document to JSON and logs relevant summary info
    /// without overwhelming logs with full content.
    ///
    /// # Arguments
    /// * `doc` - Document to log
    /// * `doc_name` - Name of the document (for logging context)
    /// * `context` - Context string (e.g., "before merge", "after merge")
    pub fn log_doc_content(doc: &LoroDoc, doc_name: &str, context: &str) {
        // Export to JSON
        let json_value = doc.get_deep_value().to_json_value();

        // Try to pretty-print for readability (truncate if too long)
        let json_str = serde_json::to_string_pretty(&json_value)
            .unwrap_or_else(|_| format!("{:?}", json_value));

        // Log summary
        let char_count = json_str.len();

        // If it's a map, try to count keys
        if let Some(obj) = json_value.as_object() {
            let key_count = obj.len();
            info!("📄 [{}] Document '{}' content: {} keys, {} chars total",
                  context, doc_name, key_count, char_count);

            // Log first-level keys
            let keys: Vec<&String> = obj.keys().collect();
            info!("📄 [{}] Document '{}' keys: {:?}", context, doc_name, keys);

            // Special handling for comments - count them
            if let Some(comments_val) = obj.get("comments") {
                if let Some(comments_arr) = comments_val.as_array() {
                    info!("📄 [{}] Document '{}' has {} comments",
                          context, doc_name, comments_arr.len());
                } else if let Some(comments_map) = comments_val.as_object() {
                    info!("📄 [{}] Document '{}' has comments map with {} keys",
                          context, doc_name, comments_map.len());
                }
            }
        } else if let Some(arr) = json_value.as_array() {
            info!("📄 [{}] Document '{}' content: array with {} items",
                  context, doc_name, arr.len());
        }

        // Log truncated content (first 500 chars) for detailed debugging
        let preview = if char_count > 500 {
            format!("{}... [truncated {} more chars]",
                    &json_str[..500], char_count - 500)
        } else {
            json_str
        };

        debug!("📄 [{}] Document '{}' JSON preview:\n{}", context, doc_name, preview);
    }

    /// Merge multiple updates in sequence
    ///
    /// Convenience method to merge multiple update batches into a single document.
    ///
    /// # Arguments
    /// * `doc` - Target document
    /// * `updates_list` - List of update byte arrays to merge in order
    ///
    /// # Returns
    /// * `Ok(())` - If all merges succeed
    /// * `Err(LoroError)` - If any merge fails
    pub fn merge_batch(doc: &LoroDoc, updates_list: &[Vec<u8>]) -> Result<(), LoroError> {
        debug!("🔀 Merging batch of {} updates", updates_list.len());
        for (i, updates) in updates_list.iter().enumerate() {
            debug!("  Merging update {}/{}", i + 1, updates_list.len());
            Self::merge_updates(doc, updates)?;
        }
        info!("✓ Batch merge completed");
        Ok(())
    }

    /// Check if two documents are at the same version
    ///
    /// Compares the operation log version vectors to determine if documents
    /// have the same state.
    ///
    /// # Arguments
    /// * `doc1` - First document
    /// * `doc2` - Second document
    ///
    /// # Returns
    /// * `bool` - True if both documents are at the same version
    pub fn are_docs_equal(doc1: &LoroDoc, doc2: &LoroDoc) -> bool {
        let vv1 = doc1.oplog_vv();
        let vv2 = doc2.oplog_vv();
        vv1 == vv2
    }

    /// Clone a document by exporting and importing
    ///
    /// Creates a deep copy of a document through export/import cycle.
    ///
    /// # Arguments
    /// * `doc` - Document to clone
    ///
    /// # Returns
    /// * `Result<LoroDoc, LoroError>` - Cloned document
    pub fn clone_doc(doc: &LoroDoc) -> Result<LoroDoc, LoroError> {
        debug!("📋 Cloning document");
        let snapshot = Self::export_snapshot(doc);
        Self::import_snapshot(&snapshot)
    }

    // ==================== PERMIT-AWARE MERGE OPERATIONS ====================
    // These methods combine permit authorization with CRDT merge logic.
    // All decision-making happens here - services layer just provides data.

    /// Filter documents to send to peer based on permit permissions
    ///
    /// Takes our permit and peer's permit, determines which documents should be sent.
    /// Checks:
    /// - Our sync facts (local_only, no_outgoing_updates)
    /// - Peer's capabilities (which documents they can access)
    ///
    /// # Arguments
    /// * `documents` - Map of doc_name -> LoroDoc reference
    /// * `our_permit_token` - Our permit token (for sync facts)
    /// * `peer_permit_token` - Peer's permit token (for capabilities)
    ///
    /// # Returns
    /// * `Ok(HashMap<doc_name, snapshot_bytes>)` - Filtered documents as shallow snapshots
    /// * `Err(...)` - If permit parsing or export fails
    pub fn filter_documents_for_peer(
        documents: std::collections::HashMap<String, &LoroDoc>,
        our_permit_token: &str,
        peer_permit_token: &str,
    ) -> Result<std::collections::HashMap<String, Vec<u8>>, Box<dyn std::error::Error>> {
        use crate::decision::{should_send_updates, SyncContext};
        use crate::parser::Permit;

        debug!("🔍 Filtering documents for peer based on permit permissions");

        // Parse permits
        let our_permit = Permit::from_token(our_permit_token)?;
        let peer_permit = Permit::from_token(peer_permit_token)?;

        // Create sync context for dual-permit decisions
        let sync_context = SyncContext::from_permits(our_permit, peer_permit);

        let mut filtered_docs = std::collections::HashMap::new();

        for (doc_name, doc) in documents {
            // Check if we should send this document based on sync facts and capabilities
            use crate::types::SyncDecision;
            let decision = should_send_updates(&sync_context, &doc_name);

            match decision {
                SyncDecision::DontSend => {
                    debug!("  ⊘ Skipping '{}' - should not send", doc_name);
                    continue;
                }
                SyncDecision::SendFullSnapshot | SyncDecision::SendIncrementalUpdates => {
                    // Export as shallow snapshot for peer
                    let snapshot = Self::export_shallow_snapshot(doc);
                    debug!("  ✓ Including '{}' ({} bytes)", doc_name, snapshot.len());
                    filtered_docs.insert(doc_name.clone(), snapshot);
                }
            }
        }

        info!("✓ Filtered {} documents for peer", filtered_docs.len());
        Ok(filtered_docs)
    }

    /// Apply updates from peer based on permit permissions
    ///
    /// Takes incoming updates and our permit, determines which documents can receive updates.
    /// Checks:
    /// - Our sync facts (no_incoming_updates)
    /// - Our capabilities (which documents we manage)
    ///
    /// # Arguments
    /// * `documents` - Map of doc_name -> mutable LoroDoc reference
    /// * `updates` - Map of doc_name -> update bytes from peer
    /// * `our_permit_token` - Our permit token (for sync facts and capabilities)
    ///
    /// # Returns
    /// * `Ok(Vec<doc_name>)` - List of documents that were updated
    /// * `Err(...)` - If permit parsing or merge fails
    pub fn apply_peer_updates(
        documents: std::collections::HashMap<String, &LoroDoc>,
        updates: std::collections::HashMap<String, Vec<u8>>,
        our_permit_token: &str,
        peer_permit_token: &str,
    ) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        use crate::decision::{can_receive_updates, SyncContext};
        use crate::parser::Permit;

        debug!("🔍 Applying peer updates based on permit permissions");

        // Parse permits
        let our_permit = Permit::from_token(our_permit_token)?;
        let peer_permit = Permit::from_token(peer_permit_token)?;

        // Create sync context for dual-permit decisions
        let sync_context = SyncContext::from_permits(our_permit, peer_permit);

        let mut updated_docs = Vec::new();

        for (doc_name, update_bytes) in updates {
            // Check if we should accept updates for this document
            // can_receive_updates checks our sync facts and capabilities
            let can_receive = can_receive_updates(&sync_context, &doc_name);

            if !can_receive {
                debug!("  ⊘ Skipping '{}' - cannot receive updates", doc_name);
                continue;
            }

            // Get the document and apply updates
            if let Some(doc) = documents.get(&doc_name) {
                Self::merge_updates(doc, &update_bytes)?;
                debug!("  ✓ Applied updates to '{}'", doc_name);
                updated_docs.push(doc_name.clone());
            } else {
                debug!("  ⊘ Skipping '{}' - document not found", doc_name);
            }
        }

        info!("✓ Applied updates to {} documents", updated_docs.len());
        Ok(updated_docs)
    }

    /// Generate state vector based on permit token type
    ///
    /// Returns appropriate version vector based on whether the permit represents
    /// a full history holder (owner/node) or viewer (state only).
    ///
    /// # Arguments
    /// * `doc` - Document to get version from
    /// * `our_permit_token` - Our permit token (determines if we track full history)
    ///
    /// # Returns
    /// * `Vec<u8>` - Encoded version vector (oplog_vv or state_frontiers)
    pub fn generate_state_vector(
        doc: &LoroDoc,
        our_permit_token: &str,
    ) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        use crate::parser::Permit;

        let our_permit = Permit::from_token(our_permit_token)?;

        // Check relationship fact to determine if we have full history
        // Viewers get state frontiers only, others get full oplog
        let relationship = our_permit.relationship();
        let is_viewer = relationship.map(|r| r == "viewer").unwrap_or(false);

        let state_vector = if is_viewer {
            // Viewers use state frontiers (no history)
            debug!("📊 Generating state_frontiers for viewer relationship");
            Self::state_frontiers(doc)
        } else {
            // Full history holders (owners, sharers) use oplog version vector
            debug!("📊 Generating oplog_vv for relationship: {:?}", relationship);
            Self::oplog_vv(doc)
        };

        Ok(state_vector)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_and_export() {
        let doc = MergeService::create_doc();
        let snapshot = MergeService::export_snapshot(&doc);
        assert!(!snapshot.is_empty());
    }

    #[test]
    fn test_import_snapshot() {
        let doc1 = MergeService::create_doc();
        let snapshot = MergeService::export_snapshot(&doc1);

        let doc2 = MergeService::import_snapshot(&snapshot).unwrap();
        assert!(MergeService::are_docs_equal(&doc1, &doc2));
    }

    #[test]
    fn test_merge_updates() {
        let doc1 = MergeService::create_doc();
        let doc2 = MergeService::create_doc();

        // Export from doc1
        let updates = MergeService::export_snapshot(&doc1);

        // Merge into doc2
        MergeService::merge_updates(&doc2, &updates).unwrap();

        // Should be equal
        assert!(MergeService::are_docs_equal(&doc1, &doc2));
    }

    #[test]
    fn test_clone_doc() {
        let doc = MergeService::create_doc();
        let cloned = MergeService::clone_doc(&doc).unwrap();
        assert!(MergeService::are_docs_equal(&doc, &cloned));
    }
}
