//! Pure Sync Logic Layer
//!
//! Provides permit-driven document synchronization operations.

use std::collections::HashMap;
use loro::LoroDoc;
use gurkha::decision::{SyncContext, should_send_updates, can_receive_updates, should_request_updates};
use gurkha::types::SyncDecision;
use tracing::instrument;
use crate::merge::MergeService;

/// Data to send in sync request
#[derive(Debug)]
pub struct SyncRequestData {
    pub state_vectors: HashMap<String, Vec<u8>>,
    pub full_docs: HashMap<String, Vec<u8>>,
}

/// Data to send in sync response
#[derive(Debug)]
pub struct SyncResponseData {
    pub updates: HashMap<String, Vec<u8>>,
    pub state_vectors: HashMap<String, Vec<u8>>,
}

/// Prepare sync request: determine what to send based on permits
///
/// Takes documents and both permits, returns what to send (state vectors vs full docs)
///
/// # Arguments
/// * `documents` - Resource documents (CRDT LoroDoc instances)
/// * `our_permit` - Current user's permit token
/// * `peer_permit` - Peer's permit token
///
/// # Returns
/// SyncRequestData containing state_vectors for incremental docs and full_docs for submitter docs
#[instrument(skip_all)]
pub fn prepare_sync_request(
    documents: &HashMap<String, LoroDoc>,
    our_permit: &str,
    peer_permit: &str,
) -> Result<SyncRequestData, Box<dyn std::error::Error>> {
    // Create SyncContext for dual-permit validation (takes token strings)
    let context = SyncContext::new(our_permit, peer_permit)?;

    let mut state_vectors = HashMap::new();
    let mut full_docs = HashMap::new();

    // For each document, determine what to send/request based on permits
    for (doc_name, doc) in documents {
        // First check if we should SEND updates (for submitter docs)
        match should_send_updates(&context, doc_name) {
            SyncDecision::SendFullSnapshot => {
                // Submitter docs: send full snapshot for proper isolated namespace merge
                full_docs.insert(doc_name.clone(), MergeService::export_snapshot(doc));
                continue; // Skip request_updates check since we're sending full doc
            }
            _ => {
                // For non-submitter docs, check if we should REQUEST updates
                match should_request_updates(&context, doc_name) {
                    SyncDecision::SendIncrementalUpdates => {
                        // Send state vector to request incremental updates
                        state_vectors.insert(doc_name.clone(), MergeService::state_frontiers(doc));
                    }
                    _ => {
                        // Don't request (local_only, no permission, etc.)
                    }
                }
            }
        }
    }

    Ok(SyncRequestData { state_vectors, full_docs })
}

/// Generate sync response: create updates based on peer's state vectors
///
/// # Arguments
/// * `documents` - Resource documents
/// * `our_permit` - Current user's permit token
/// * `peer_permit` - Peer's permit token (from message)
/// * `peer_state_vectors` - Peer's state vectors to compute diffs
///
/// # Returns
/// SyncResponseData containing updates and our current state vectors
#[instrument(skip_all)]
pub fn generate_sync_response(
    documents: &HashMap<String, LoroDoc>,
    our_permit: &str,
    peer_permit: &str,
    peer_state_vectors: &HashMap<String, Vec<u8>>,
) -> Result<SyncResponseData, Box<dyn std::error::Error>> {
    let context = SyncContext::new(our_permit, peer_permit)?;

    let mut updates = HashMap::new();
    let mut our_state_vectors = HashMap::new();

    for (doc_name, doc) in documents {
        match should_send_updates(&context, doc_name) {
            SyncDecision::SendFullSnapshot => {
                // Send full snapshot
                updates.insert(doc_name.clone(), MergeService::export_snapshot(doc));
            }
            SyncDecision::SendIncrementalUpdates => {
                // Generate diff from peer's state to our current state
                if let Some(peer_vec) = peer_state_vectors.get(doc_name) {
                    let diff = MergeService::export_updates(doc, peer_vec)?;
                    if !diff.is_empty() {
                        updates.insert(doc_name.clone(), diff);
                    }
                } else {
                    // Peer doesn't have state vector for this document
                    // Send full snapshot so they can receive it
                    tracing::warn!("⚠️ [generate_sync_response] Peer has no state vector for '{}' - sending full snapshot", doc_name);
                    updates.insert(doc_name.clone(), MergeService::export_snapshot(doc));
                }
            }
            SyncDecision::DontSend => {}
        }

        // Generate our state vectors for all docs
        our_state_vectors.insert(doc_name.clone(), MergeService::state_frontiers(doc));
    }

    Ok(SyncResponseData { updates, state_vectors: our_state_vectors })
}

/// Apply peer's full documents (submissions)
///
/// Used when receiving full snapshots from peer (e.g., viewer submissions)
///
/// # Arguments
/// * `documents` - Mutable reference to resource documents
/// * `our_permit` - Current user's permit token
/// * `peer_permit` - Peer's permit token
/// * `peer_full_docs` - Peer's full document snapshots
#[instrument(skip_all)]
pub fn apply_peer_docs(
    documents: &mut HashMap<String, LoroDoc>,
    our_permit: &str,
    peer_permit: &str,
    peer_full_docs: HashMap<String, Vec<u8>>,
) -> Result<(), Box<dyn std::error::Error>> {
    let context = SyncContext::new(our_permit, peer_permit)?;

    tracing::info!("📥 [apply_peer_docs] Starting to apply {} full document snapshots", peer_full_docs.len());

    for (doc_name, snapshot) in peer_full_docs {
        let snapshot_size = snapshot.len();
        tracing::info!("📥 [apply_peer_docs] Processing full snapshot for '{}' ({} bytes)", doc_name, snapshot_size);

        let can_receive = can_receive_updates(&context, &doc_name);
        tracing::info!("🔒 [apply_peer_docs] can_receive_updates('{}') = {}", doc_name, can_receive);

        if can_receive {
            if let Some(existing_doc) = documents.get_mut(&doc_name) {
                // Document exists - merge the snapshot into it (CRDT merge)
                tracing::info!("🔀 [apply_peer_docs] Document '{}' exists - merging snapshot", doc_name);

                // Log state before merge
                let state_before = MergeService::state_frontiers(existing_doc);
                tracing::info!("📊 [apply_peer_docs] Document '{}' state before merge: {} bytes", doc_name, state_before.len());

                // Merge peer's snapshot into existing document
                MergeService::merge_updates(existing_doc, &snapshot)?;

                // Log state after merge
                let state_after = MergeService::state_frontiers(existing_doc);
                tracing::info!("✅ [apply_peer_docs] Document '{}' snapshot merged! State after: {} bytes", doc_name, state_after.len());
            } else {
                // Document doesn't exist - import as new document
                tracing::info!("📝 [apply_peer_docs] Document '{}' doesn't exist - importing snapshot as new document", doc_name);
                let doc = MergeService::import_snapshot(&snapshot)?;
                documents.insert(doc_name.clone(), doc);
                tracing::info!("✅ [apply_peer_docs] Document '{}' snapshot imported and inserted", doc_name);
            }
        } else {
            tracing::warn!("🚫 [apply_peer_docs] Skipping document '{}' - can_receive_updates returned false", doc_name);
        }
    }

    tracing::info!("✅ [apply_peer_docs] Finished applying full document snapshots");
    Ok(())
}

/// Apply peer's updates
///
/// Merges peer's incremental updates into our documents
///
/// # Arguments
/// * `documents` - Mutable reference to resource documents
/// * `our_permit` - Current user's permit token
/// * `peer_permit` - Peer's permit token
/// * `peer_updates` - Peer's update data per document
#[instrument(skip_all)]
pub fn apply_peer_updates(
    documents: &mut HashMap<String, LoroDoc>,
    our_permit: &str,
    peer_permit: &str,
    peer_updates: HashMap<String, Vec<u8>>,
) -> Result<(), Box<dyn std::error::Error>> {
    let context = SyncContext::new(our_permit, peer_permit)?;

    tracing::info!("📥 [apply_peer_updates] Starting to apply {} peer updates", peer_updates.len());

    for (doc_name, update) in peer_updates {
        let update_size = update.len();
        tracing::info!("📥 [apply_peer_updates] Processing document '{}' ({} bytes)", doc_name, update_size);

        let can_receive = can_receive_updates(&context, &doc_name);
        tracing::info!("🔒 [apply_peer_updates] can_receive_updates('{}') = {}", doc_name, can_receive);

        if can_receive {
            if let Some(doc) = documents.get_mut(&doc_name) {
                // Log state before merge
                let state_before = MergeService::state_frontiers(doc);
                tracing::info!("📊 [apply_peer_updates] Document '{}' state before merge: {} bytes", doc_name, state_before.len());

                // NEW: Log document content BEFORE merge
                MergeService::log_doc_content(doc, &doc_name, "apply_peer_updates BEFORE");

                MergeService::merge_updates(doc, &update)?;

                // Log state after merge
                let state_after = MergeService::state_frontiers(doc);
                tracing::info!("✅ [apply_peer_updates] Document '{}' merged successfully! State after: {} bytes", doc_name, state_after.len());

                // NEW: Log document content AFTER merge
                MergeService::log_doc_content(doc, &doc_name, "apply_peer_updates AFTER");

                // NEW: Log if state vector changed but content looks the same
                if state_before.len() != state_after.len() {
                    tracing::info!("🔍 [apply_peer_updates] Document '{}' state vector SIZE CHANGED: {} → {} bytes",
                                   doc_name, state_before.len(), state_after.len());
                } else {
                    tracing::warn!("⚠️ [apply_peer_updates] Document '{}' state vector size UNCHANGED after merge (possible no-op?)", doc_name);
                }
            } else {
                tracing::warn!("⚠️ [apply_peer_updates] Document '{}' not found in documents map - skipping", doc_name);
            }
        } else {
            tracing::warn!("🚫 [apply_peer_updates] Skipping document '{}' - can_receive_updates returned false", doc_name);
        }
    }

    tracing::info!("✅ [apply_peer_updates] Finished applying peer updates");
    Ok(())
}

/// Generate updates to send back after receiving peer's updates
///
/// After applying peer's updates, check if we have any collaborative updates to send back
///
/// # Arguments
/// * `documents` - Resource documents
/// * `our_permit` - Current user's permit token
/// * `peer_permit` - Peer's permit token
/// * `peer_state_vectors` - Peer's current state vectors (from their response)
///
/// # Returns
/// Option<SyncResponseData> - Some if we have updates to send, None otherwise
#[instrument(skip_all)]
pub fn generate_collaborative_updates(
    documents: &HashMap<String, LoroDoc>,
    our_permit: &str,
    peer_permit: &str,
    peer_state_vectors: &HashMap<String, Vec<u8>>,
) -> Result<Option<SyncResponseData>, Box<dyn std::error::Error>> {
    tracing::info!("🔄 [generate_collaborative_updates] Starting collaborative update generation for {} documents", documents.len());
    let context = SyncContext::new(our_permit, peer_permit)?;

    let mut updates = HashMap::new();

    for (doc_name, doc) in documents {
        tracing::debug!("📝 [generate_collaborative_updates] Checking document '{}'", doc_name);

        let decision = should_send_updates(&context, doc_name);
        tracing::debug!("📊 [generate_collaborative_updates] '{}' decision: {:?}", doc_name, decision);

        if let SyncDecision::SendIncrementalUpdates = decision {
            if let Some(peer_vec) = peer_state_vectors.get(doc_name) {
                tracing::debug!("📊 [generate_collaborative_updates] '{}' has peer state vector ({} bytes)", doc_name, peer_vec.len());
                let diff = MergeService::export_updates(doc, peer_vec)?;
                tracing::debug!("📊 [generate_collaborative_updates] '{}' generated diff: {} bytes", doc_name, diff.len());
                if !diff.is_empty() {
                    tracing::info!("✅ [generate_collaborative_updates] Including '{}' in updates ({} bytes)", doc_name, diff.len());
                    updates.insert(doc_name.clone(), diff);
                } else {
                    tracing::debug!("⚠️ [generate_collaborative_updates] '{}' diff is empty (no changes)", doc_name);
                }
            } else {
                tracing::warn!("⚠️ [generate_collaborative_updates] '{}' has no peer state vector", doc_name);
            }
        } else {
            tracing::warn!("🚫 [generate_collaborative_updates] '{}' skipped (decision: {:?})", doc_name, decision);
        }
    }

    tracing::info!("📦 [generate_collaborative_updates] Final update count: {} documents", updates.len());
    for (doc_name, data) in &updates {
        tracing::info!("  📄 '{}': {} bytes", doc_name, data.len());
    }

    if updates.is_empty() {
        tracing::info!("⚠️ [generate_collaborative_updates] No updates to send");
        Ok(None)
    } else {
        let mut our_state_vectors = HashMap::new();
        for (doc_name, doc) in documents {
            our_state_vectors.insert(doc_name.clone(), MergeService::state_frontiers(doc));
        }
        Ok(Some(SyncResponseData { updates, state_vectors: our_state_vectors }))
    }
}
