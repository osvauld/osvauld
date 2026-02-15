//! Permit parsing and permission checking for Scribe actor
//!
//! Handles:
//! - Layer access control (read/write permissions via gurkha::Permit)
//! - Sync policy extraction and enforcement
//! - Pattern-based permission rules
//! - Glob matching for layer names

use tracing::{debug, trace};

use crate::state::ScribeState;

/// Simple glob pattern matching for layer names
///
/// **Supported patterns:**
/// - `*` at end: prefix match (e.g., "*:order" matches anything ending with ":order")
/// - `*` at start: suffix match (e.g., "did:*" matches anything starting with "did:")
/// - Exact match otherwise
pub fn glob_match(pattern: &str, name: &str) -> bool {
    if pattern == "*" {
        true
    } else if pattern.starts_with('*') {
        // Suffix match: "*:order" matches "did:key:abc:order"
        name.ends_with(&pattern[1..])
    } else if pattern.ends_with('*') {
        // Prefix match: "app:*" matches "app:shop"
        name.starts_with(&pattern[..pattern.len() - 1])
    } else {
        // Exact match
        pattern == name
    }
}

// Permission Checking

/// Scribe permission checking methods
pub struct Permissions;

impl Permissions {
    /// Check if layer is local_only (never synced)
    ///
    /// **Context**: Uses the LayerUnit's permit-based local_only flag.
    /// Falls back to false if layer doesn't exist yet (will be set on creation).
    pub fn is_local_only(state: &ScribeState, layer_name: &str) -> bool {
        state
            .units
            .get(layer_name)
            .map(|unit| unit.is_local_only())
            .unwrap_or(false)
    }

    /// Check if peer can write to layer
    ///
    /// **Context**: Remote peer is trying to write to this layer
    /// **We check**:
    /// 1. Peer is a subscriber with write permission via permit.can_write_layer()
    /// 2. OR peer is our sync_target (owner/viewer mode - trust sync source)
    /// 3. OR peer has a stored permit with write access (node mode - permit-based sync auth)
    /// 4. OR our permit's dynamic_layer_schemas grant write for peer's role (dynamic layers)
    ///
    /// **Security**: For pattern-based layers, gurkha::Permit validates that:
    /// - The layer name matches the expanded pattern
    /// - Patterns with {aud} expand to the peer's actual DID (namespace enforcement)
    pub fn can_write(state: &ScribeState, peer: &(String, String), layer_name: &str) -> bool {
        let peer_did = &peer.0;

        // 0. Protocol layers: __sync_meta:{peer_did} allows writes from the named peer
        //
        // **Context**: Each peer owns their __sync_meta layer and writes discovery entries.
        // The node also writes status entries (locally, no remote check needed).
        // When the peer sends SyncOffer for their own __sync_meta, we allow it.
        if crate::sync::sync_meta::is_sync_meta_layer(layer_name) {
            if let Some(meta_did) = crate::sync::sync_meta::extract_peer_did(layer_name) {
                if meta_did == peer_did {
                    debug!(user_did = %peer_did, layer = %layer_name, "Write allowed: peer owns this __sync_meta layer");
                    state.emit_permission_check_capture(
                        layer_name,
                        peer_did,
                        "allowed",
                        "sync_meta_owner",
                    );
                    return true;
                }
            }
        }

        // 1. Check if peer is a subscriber with write permission
        if let Ok(subs) = state.subscribers.read() {
            if let Some(info) = subs.get(peer) {
                // Use permit's can_write_layer which handles both fixed and pattern-based
                if info.can_write_layer(layer_name, &state.page_id) {
                    trace!(
                        user_did = %peer_did,
                        layer = %layer_name,
                        "Write allowed via subscriber permit"
                    );
                    state.emit_permission_check_capture(
                        layer_name,
                        peer_did,
                        "allowed",
                        "subscriber_permit",
                    );
                    return true;
                }
            }
        }

        // 2. Owner/Viewer mode: trust data from our sync_target (the node we sync with)
        // This allows receiving sync updates from the node without explicit subscription
        if let Some(ref config) = state.sync_config {
            if let Some(ref sync_target) = config.sync_target {
                if peer_did == sync_target {
                    debug!(user_did = %peer_did, layer = %layer_name, "Write allowed from sync_target");
                    state.emit_permission_check_capture(
                        layer_name,
                        peer_did,
                        "allowed",
                        "sync_target",
                    );
                    return true;
                }
            }
        }

        // 3. Node mode: check stored permit for write access (permit-based auth)
        if let Some(ref resolver) = state.peer_resolver {
            if let Some(permit_token) = resolver.load_user_permit(peer_did) {
                // Parse permit and check write permissions
                if let Ok(permit) = gurkha::Permit::from_token(&permit_token) {
                    if permit.can_write_layer(layer_name, &state.page_id, peer_did) {
                        debug!(user_did = %peer_did, layer = %layer_name, "Write allowed via stored permit");
                        state.emit_permission_check_capture(
                            layer_name,
                            peer_did,
                            "allowed",
                            "stored_permit",
                        );
                        return true;
                    }
                }
            }
        }

        // 4. Node mode: check our permit's dynamic_layer_schemas permissions
        //
        // **Context**: Dynamic layers (e.g. channels/did:key:alice/project-x/messages)
        // aren't in the peer's page permit (they contain another user's DID in the path).
        // The node's own permit has dynamic_layer_schemas with permissions that grant
        // write access to all peers. We check the schema permissions directly.
        if let Some(ref our_permit) = state.our_permit {
            if gurkha::matches_dynamic_schema(our_permit, layer_name, &state.page_id, "write") {
                debug!(
                    user_did = %peer_did,
                    layer = %layer_name,
                    "Write allowed via dynamic_layer_schema check"
                );
                state.emit_permission_check_capture(
                    layer_name,
                    peer_did,
                    "allowed",
                    "dynamic_schema",
                );
                return true;
            }
        }

        state.emit_permission_check_capture(layer_name, peer_did, "denied", "all_paths_failed");
        false
    }

    /// Check if we (local user) can write to a layer
    ///
    /// **Context**: Local Lua/UI is trying to modify a layer
    /// **We check**: Delegates to ScribeState::can_write_layer()
    ///
    /// **Security**: Ensures local writes respect our permit constraints
    /// (e.g., viewer can only write to their namespaced layers)
    pub fn can_local_write(state: &ScribeState, layer_name: &str) -> bool {
        state.can_write_layer(layer_name)
    }
}

// Tests

#[cfg(test)]
mod tests {
    /// Helper to test pattern matching with expansion
    fn matches_with_expansion(layer_name: &str, pattern: &str, page_id: &str, did: &str) -> bool {
        let expanded = gurkha::expand_pattern(pattern, page_id, did);
        gurkha::matches_wildcard(layer_name, &expanded)
    }

    #[test]
    fn test_wildcard_pattern_matching() {
        // Exact segments with single wildcard
        assert!(gurkha::matches_wildcard(
            "shop123/orders/did:key:customer_a",
            "shop123/*/did:key:customer_a"
        ));
        assert!(gurkha::matches_wildcard(
            "shop123/cart/did:key:customer_a",
            "shop123/*/did:key:customer_a"
        ));

        // Double wildcard (owner/admin pattern)
        assert!(gurkha::matches_wildcard(
            "shop123/orders/did:key:customer_a",
            "shop123/*/*"
        ));
        assert!(gurkha::matches_wildcard(
            "shop123/anything/anything_else",
            "shop123/*/*"
        ));

        // Wrong segment count
        assert!(!gurkha::matches_wildcard(
            "shop123/orders",
            "shop123/*/did:key:customer_a"
        ));
        assert!(!gurkha::matches_wildcard(
            "shop123/orders/sub/did:key:customer_a",
            "shop123/*/did:key:customer_a"
        ));

        // Wrong DID
        assert!(!gurkha::matches_wildcard(
            "shop123/orders/did:key:customer_b",
            "shop123/*/did:key:customer_a"
        ));
    }

    #[test]
    fn test_pattern_expansion_and_privacy() {
        let pattern = "{page_id}/*/{aud}";

        // Expansion matches own DID
        assert!(matches_with_expansion(
            "shop123/orders/did:key:customer_a",
            pattern,
            "shop123",
            "did:key:customer_a"
        ));
        // Expansion rejects other DID
        assert!(!matches_with_expansion(
            "shop123/orders/did:key:customer_b",
            pattern,
            "shop123",
            "did:key:customer_a"
        ));

        // Owner wildcard pattern matches all
        let owner_pattern = "{page_id}/*/*";
        assert!(matches_with_expansion(
            "shop123/orders/did:key:customer_a",
            owner_pattern,
            "shop123",
            "did:key:owner"
        ));
        assert!(matches_with_expansion(
            "shop123/orders/did:key:customer_b",
            owner_pattern,
            "shop123",
            "did:key:owner"
        ));
    }

    #[test]
    fn test_privacy_isolation() {
        let pattern = "{page_id}/*/{aud}";
        let layer_a = "shop123/orders/did:key:customer_a";
        let layer_b = "shop123/orders/did:key:customer_b";

        // Customer A can access their own layer
        assert!(matches_with_expansion(
            layer_a,
            pattern,
            "shop123",
            "did:key:customer_a"
        ));
        // Customer A cannot access Customer B's layer
        assert!(!matches_with_expansion(
            layer_b,
            pattern,
            "shop123",
            "did:key:customer_a"
        ));

        // Customer B can access their own layer
        assert!(matches_with_expansion(
            layer_b,
            pattern,
            "shop123",
            "did:key:customer_b"
        ));
        // Customer B cannot access Customer A's layer
        assert!(!matches_with_expansion(
            layer_a,
            pattern,
            "shop123",
            "did:key:customer_b"
        ));
    }
}
