use super::capability::SyncDecision;
use super::ucan_domain::ResourceUcan;

/// Context for dual-UCAN sync decisions
#[derive(Debug, Clone)]
pub struct SyncContext {
    our_ucan: ResourceUcan,
    peer_ucan: ResourceUcan,
}

impl SyncContext {
    /// Create sync context from two raw tokens
    pub fn new(our_token: &str, peer_token: &str) -> Result<Self, String> {
        let our_ucan = ResourceUcan::from_token(our_token)
            .map_err(|e| format!("Failed to parse our token: {}", e))?;
        let peer_ucan = ResourceUcan::from_token(peer_token)
            .map_err(|e| format!("Failed to parse peer token: {}", e))?;

        Ok(Self { our_ucan, peer_ucan })
    }

    /// Create sync context from parsed UCANs
    pub fn from_ucans(our_ucan: ResourceUcan, peer_ucan: ResourceUcan) -> Self {
        Self { our_ucan, peer_ucan }
    }

    pub fn our_ucan(&self) -> &ResourceUcan {
        &self.our_ucan
    }

    pub fn peer_ucan(&self) -> &ResourceUcan {
        &self.peer_ucan
    }
}

/// Determine if we should send updates for a document
///
/// Algorithm:
/// 1. Check our local_only facts → DontSend
/// 2. Check our capability (can WE write?) → Viewer: DontSend, Collaborator: Continue, Submitter: SendFullSnapshot
/// 3. Check peer's no_incoming_updates → DontSend
/// 4. Check peer capability (can THEY receive?) → Collaborator/Viewer: SendIncrementalUpdates
pub fn should_send_updates(context: &SyncContext, doc_name: &str) -> SyncDecision {
    // 1. Check our local_only facts
    if context.our_ucan.is_local_only(doc_name) {
        return SyncDecision::DontSend;
    }

    // 2. Check our capability (can WE write?)
    match context.our_ucan.get_capability(doc_name) {
        None => return SyncDecision::DontSend,
        Some(our_cap) => {
            if !our_cap.can_write() {
                // We have Viewer capability → can't send
                return SyncDecision::DontSend;
            }

            // Check if we should send full snapshot (Submitter)
            if context.our_ucan.should_send_full_snapshot(doc_name) {
                return SyncDecision::SendFullSnapshot;
            }
        }
    }

    // 3. Check peer's no_incoming_updates
    if context.peer_ucan.has_no_incoming_updates(doc_name) {
        return SyncDecision::DontSend;
    }

    // 4. Check peer capability (can THEY receive?)
    match context.peer_ucan.get_capability(doc_name) {
        None => SyncDecision::DontSend,
        Some(peer_cap) => {
            // Peer can receive if they have Collaborator or Viewer capability
            if peer_cap.can_sync_bidirectional() || !peer_cap.can_write() {
                SyncDecision::SendIncrementalUpdates
            } else {
                SyncDecision::DontSend
            }
        }
    }
}

/// Check if we can receive updates for a document
///
/// Algorithm:
/// 1. Check our no_incoming_updates facts → false
/// 2. Check our capability (can WE receive?) → Collaborator/Viewer: true
pub fn can_receive_updates(context: &SyncContext, doc_name: &str) -> bool {
    // 1. Check our no_incoming_updates facts
    if context.our_ucan.has_no_incoming_updates(doc_name) {
        return false;
    }

    // 2. Check our capability (can WE receive?)
    match context.our_ucan.get_capability(doc_name) {
        None => false,
        Some(our_cap) => {
            // We can receive if we have Collaborator or Viewer capability
            our_cap.can_sync_bidirectional() || !our_cap.can_write()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::capability::Capability;

    // TODO: Add comprehensive tests once we have test UCAN token generation
    // Test cases:
    // 1. Owner ↔ Node: bidirectional sync for all docs
    // 2. Node ↔ Viewer: one-way for viewer docs (content, template, static_assets)
    // 3. Viewer → Node: blocked for viewer docs (readonly)
    // 4. Viewer → Node: allowed for collaborative_doc, shared_assets
    // 5. Node → Viewer: blocked for submissions_doc (isolated namespace)
    // 6. user_content_doc: never syncs (local_only)
}
