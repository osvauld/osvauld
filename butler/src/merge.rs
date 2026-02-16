//! Pure CRDT Merge Logic Layer
//!
//! Provides stateless Loro document merge operations.

use loro::{ExportMode, LoroDoc, LoroError, ToJson, VersionVector};
use tracing::{debug, info, instrument};

/// Pure CRDT merge service for Loro documents
///
/// Provides stateless operations for merging, exporting, and managing CRDT documents.
/// No database, no network, no state - just pure logic.
pub struct MergeService;

impl MergeService {
    /// Create a new empty Loro document
    #[instrument(skip_all)]
    pub fn create_doc() -> LoroDoc {
        debug!("📄 Creating new Loro document");
        LoroDoc::new()
    }

    /// Import snapshot into a new document
    #[instrument(skip_all)]
    pub fn import_snapshot(snapshot_bytes: &[u8]) -> Result<LoroDoc, LoroError> {
        debug!("📥 Importing snapshot ({} bytes)", snapshot_bytes.len());
        let doc = LoroDoc::new();
        doc.import(snapshot_bytes)?;
        info!("✓ Snapshot imported successfully");
        Ok(doc)
    }

    /// Merge updates into an existing document
    #[instrument(skip_all)]
    pub fn merge_updates(doc: &LoroDoc, updates_bytes: &[u8]) -> Result<(), LoroError> {
        debug!("🔀 Merging updates ({} bytes)", updates_bytes.len());
        doc.import(updates_bytes)?;
        info!("✓ Updates merged successfully");
        Ok(())
    }

    /// Export full snapshot with complete history
    #[instrument(skip_all)]
    pub fn export_snapshot(doc: &LoroDoc) -> Vec<u8> {
        debug!("📤 Exporting full snapshot");
        let snapshot = doc
            .export(ExportMode::Snapshot)
            .expect("Failed to export Loro snapshot");
        info!("✓ Full snapshot exported ({} bytes)", snapshot.len());
        snapshot
    }

    /// Export shallow snapshot without full history
    #[instrument(skip_all)]
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
    #[instrument(skip_all)]
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
    pub fn oplog_vv(doc: &LoroDoc) -> Vec<u8> {
        debug!("📊 Getting operation log version vector");
        doc.oplog_vv().encode()
    }

    /// Get state frontiers (current state without history)
    pub fn state_frontiers(doc: &LoroDoc) -> Vec<u8> {
        debug!("📊 Getting state frontiers");
        doc.state_frontiers().encode()
    }

    /// Log document content for debugging CRDT sync issues
    pub fn log_doc_content(doc: &LoroDoc, doc_name: &str, context: &str) {
        let json_value = doc.get_deep_value().to_json_value();
        let json_str = serde_json::to_string_pretty(&json_value)
            .unwrap_or_else(|_| format!("{:?}", json_value));
        let char_count = json_str.len();

        if let Some(obj) = json_value.as_object() {
            let key_count = obj.len();
            info!(
                "📄 [{}] Document '{}' content: {} keys, {} chars total",
                context, doc_name, key_count, char_count
            );
            let keys: Vec<&String> = obj.keys().collect();
            info!("📄 [{}] Document '{}' keys: {:?}", context, doc_name, keys);

            if let Some(comments_val) = obj.get("comments") {
                if let Some(comments_arr) = comments_val.as_array() {
                    info!(
                        "📄 [{}] Document '{}' has {} comments",
                        context,
                        doc_name,
                        comments_arr.len()
                    );
                } else if let Some(comments_map) = comments_val.as_object() {
                    info!(
                        "📄 [{}] Document '{}' has comments map with {} keys",
                        context,
                        doc_name,
                        comments_map.len()
                    );
                }
            }
        } else if let Some(arr) = json_value.as_array() {
            info!(
                "📄 [{}] Document '{}' content: array with {} items",
                context,
                doc_name,
                arr.len()
            );
        }

        let preview = if char_count > 500 {
            format!(
                "{}... [truncated {} more chars]",
                &json_str[..500],
                char_count - 500
            )
        } else {
            json_str
        };
        debug!(
            "📄 [{}] Document '{}' JSON preview:\n{}",
            context, doc_name, preview
        );
    }

    /// Merge multiple updates in sequence
    #[instrument(skip_all)]
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
    pub fn are_docs_equal(doc1: &LoroDoc, doc2: &LoroDoc) -> bool {
        let vv1 = doc1.oplog_vv();
        let vv2 = doc2.oplog_vv();
        vv1 == vv2
    }

    /// Clone a document by exporting and importing
    #[instrument(skip_all)]
    pub fn clone_doc(doc: &LoroDoc) -> Result<LoroDoc, LoroError> {
        debug!("📋 Cloning document");
        let snapshot = Self::export_snapshot(doc);
        Self::import_snapshot(&snapshot)
    }

    /// Filter documents to send to peer based on permit permissions
    #[instrument(skip_all)]
    pub fn filter_documents_for_peer(
        documents: std::collections::HashMap<String, &LoroDoc>,
        our_permit_token: &str,
        peer_permit_token: &str,
    ) -> Result<std::collections::HashMap<String, Vec<u8>>, Box<dyn std::error::Error>> {
        use gurkha::decision::{should_send_updates, SyncContext};
        use gurkha::parser::Permit;
        use gurkha::types::SyncDecision;

        debug!("🔍 Filtering documents for peer based on permit permissions");

        let our_permit = Permit::from_token(our_permit_token)?;
        let peer_permit = Permit::from_token(peer_permit_token)?;
        let sync_context = SyncContext::from_permits(our_permit, peer_permit);

        let mut filtered_docs = std::collections::HashMap::new();

        for (doc_name, doc) in documents {
            let decision = should_send_updates(&sync_context, &doc_name);

            match decision {
                SyncDecision::DontSend => {
                    debug!("  ⊘ Skipping '{}' - should not send", doc_name);
                    continue;
                }
                SyncDecision::SendFullSnapshot | SyncDecision::SendIncrementalUpdates => {
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
    #[instrument(skip_all)]
    pub fn apply_peer_updates(
        documents: std::collections::HashMap<String, &LoroDoc>,
        updates: std::collections::HashMap<String, Vec<u8>>,
        our_permit_token: &str,
        peer_permit_token: &str,
    ) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        use gurkha::decision::{can_receive_updates, SyncContext};
        use gurkha::parser::Permit;

        debug!("🔍 Applying peer updates based on permit permissions");

        let our_permit = Permit::from_token(our_permit_token)?;
        let peer_permit = Permit::from_token(peer_permit_token)?;
        let sync_context = SyncContext::from_permits(our_permit, peer_permit);

        let mut updated_docs = Vec::new();

        for (doc_name, update_bytes) in updates {
            let can_receive = can_receive_updates(&sync_context, &doc_name);

            if !can_receive {
                debug!("  ⊘ Skipping '{}' - cannot receive updates", doc_name);
                continue;
            }

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
    #[instrument(skip_all)]
    pub fn generate_state_vector(
        doc: &LoroDoc,
        our_permit_token: &str,
    ) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        use gurkha::parser::Permit;

        let our_permit = Permit::from_token(our_permit_token)?;
        let is_viewer = !our_permit.is_owner() && !our_permit.is_host();

        let state_vector = if is_viewer {
            debug!("📊 Generating state_frontiers for viewer relationship");
            Self::state_frontiers(doc)
        } else {
            debug!("📊 Generating oplog_vv for owner/host");
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
        let updates = MergeService::export_snapshot(&doc1);
        MergeService::merge_updates(&doc2, &updates).unwrap();
        assert!(MergeService::are_docs_equal(&doc1, &doc2));
    }

    #[test]
    fn test_clone_doc() {
        let doc = MergeService::create_doc();
        let cloned = MergeService::clone_doc(&doc).unwrap();
        assert!(MergeService::are_docs_equal(&doc, &cloned));
    }
}
