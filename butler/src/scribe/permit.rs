//! Permit parsing and permission checking for Scribe actor
//!
//! Handles:
//! - Layer access control (read/write permissions)
//! - Sync policy extraction and enforcement
//! - Pattern-based permission rules
//! - Static layer extraction for pre-creation

use std::collections::HashMap;

use tracing::{debug, trace, warn};

use crate::error::{ButlerError, Result};

use super::state::{LayerWritePermission, PatternRule, ScribeState, SyncPolicy};

// =============================================================================
// Permit Parsing
// =============================================================================

/// Parse permit to extract layer write permissions and sync policies
///
/// **Context**: PeerActor has already validated the permit. We just extract facts.
pub fn parse_permit_for_layers(permit: &str) -> Result<(HashMap<String, LayerWritePermission>, SyncPolicy)> {
    let parsed = gurkha::Permit::from_token(permit)
        .map_err(|e| ButlerError::PermitError(format!("Failed to parse permit: {:?}", e)))?;

    let mut layer_permissions = HashMap::new();
    let mut sync_policy = SyncPolicy::default();

    // Extract layers fact
    if let Some(layers_value) = parsed.get_fact("layers") {
        if let Some(layers_obj) = layers_value.as_object() {
            for (layer_name, config) in layers_obj {
                // Check if layer has sync enabled
                let has_sync = config.get("sync").and_then(|v| v.as_bool()).unwrap_or(false);
                if !has_sync {
                    continue;
                }

                // Get write permission (defaults to false if not specified)
                let can_write = config.get("write").and_then(|v| v.as_bool()).unwrap_or(false);
                layer_permissions.insert(layer_name.clone(), can_write);
            }
        }
    }

    // Extract sync fact
    if let Some(sync_value) = parsed.get_fact("sync") {
        if let Some(sync_obj) = sync_value.as_object() {
            if let Some(local_only) = sync_obj.get("local_only").and_then(|v| v.as_array()) {
                for item in local_only {
                    if let Some(s) = item.as_str() {
                        sync_policy.local_only.insert(s.to_string());
                    }
                }
            }
            if let Some(no_incoming) = sync_obj.get("no_incoming_updates").and_then(|v| v.as_array()) {
                for item in no_incoming {
                    if let Some(s) = item.as_str() {
                        sync_policy.no_incoming_updates.insert(s.to_string());
                    }
                }
            }
            if let Some(full_snap) = sync_obj.get("send_full_snapshot").and_then(|v| v.as_array()) {
                for item in full_snap {
                    if let Some(s) = item.as_str() {
                        sync_policy.send_full_snapshot.insert(s.to_string());
                    }
                }
            }
        }
    }

    Ok((layer_permissions, sync_policy))
}

/// Extract pattern rules from permit for role-agnostic layer access
///
/// **Context**: Parsing permit's `layer_patterns` for viewer-created layers
/// **Returns**: (readable_patterns, writable_patterns)
pub fn extract_patterns_from_permit(permit: &str, _page_id: &str) -> (Vec<PatternRule>, Vec<PatternRule>) {
    let mut readable = Vec::new();
    let mut writable = Vec::new();

    let parsed = match gurkha::Permit::from_token(permit) {
        Ok(p) => p,
        Err(e) => {
            warn!(error = ?e, "Failed to parse permit for patterns");
            return (readable, writable);
        }
    };

    // Extract layer_patterns from permit
    for (pattern, config) in parsed.layer_patterns() {
        let rule = PatternRule {
            pattern: pattern.clone(),
            sync: config.sync,
            create: config.create,
            write: config.write,
        };

        if config.sync {
            readable.push(rule.clone());
        }
        // write: can write to existing layers, create: can create new layers
        // Both grant write permission
        if config.write || config.create {
            writable.push(rule);
        }
    }

    debug!(
        readable_count = readable.len(),
        writable_count = writable.len(),
        "Extracted patterns from permit"
    );

    (readable, writable)
}

/// Extract role/relationship from permit
///
/// **Context**: Parsing permit to determine role for validation
/// **Returns**: Role string like "owner", "viewer", "node", "customer"
pub fn extract_role_from_permit(permit: &str) -> String {
    let parsed = match gurkha::Permit::from_token(permit) {
        Ok(p) => p,
        Err(e) => {
            warn!(error = ?e, "Failed to parse permit for role extraction");
            return "unknown".to_string();
        }
    };

    // First check relationship fact (common in sync permits)
    if let Some(rel_value) = parsed.get_fact("relationship") {
        if let Some(rel) = rel_value.as_str() {
            return rel.to_string();
        }
    }

    // Fall back to role fact
    if let Some(role_value) = parsed.get_fact("role") {
        if let Some(role) = role_value.as_str() {
            return role.to_string();
        }
    }

    // Default based on permit type
    "peer".to_string()
}

/// Check if a layer should sync based on permit configuration
///
/// **Context**: Used to filter out local-only layers (sync: false)
/// **Checks**:
/// 1. Named layers in `layers` section - check `sync` flag
/// 2. Pattern matches in `layer_patterns` section - check `sync` flag
/// **Returns**: true if layer should sync, false if local-only
///
/// **Pattern expansion**: {page_id} and {aud} are expanded before matching
pub fn should_sync_layer(permit: &str, layer_name: &str, page_id: &str, our_did: &str) -> bool {
    let parsed = match gurkha::Permit::from_token(permit) {
        Ok(p) => p,
        Err(e) => {
            warn!(error = ?e, "Failed to parse permit for sync check");
            return true; // Default to sync on parse error
        }
    };

    // 1. Check named layers section
    if let Some(layers_value) = parsed.get_fact("layers") {
        if let Some(layers_obj) = layers_value.as_object() {
            for (pattern, config) in layers_obj {
                // Expand {page_id} in layer name pattern
                let expanded_pattern = pattern
                    .replace("{page_id}", page_id)
                    .replace("{aud}", our_did);

                if expanded_pattern == layer_name {
                    // Found matching layer, check sync flag
                    let should_sync = config.get("sync").and_then(|v| v.as_bool()).unwrap_or(true);
                    debug!(
                        layer = layer_name,
                        pattern = pattern,
                        sync = should_sync,
                        "Named layer sync check"
                    );
                    return should_sync;
                }
            }
        }
    }

    // 2. Check layer_patterns section
    for (pattern, config) in parsed.layer_patterns() {
        // Expand {page_id} and {aud} in pattern
        let expanded = pattern
            .replace("{page_id}", page_id)
            .replace("{aud}", our_did);

        // Check if layer matches pattern (with wildcard support)
        if super::state::matches_wildcard_pattern(layer_name, &expanded) {
            debug!(
                layer = layer_name,
                pattern = pattern,
                sync = config.sync,
                "Pattern match sync check"
            );
            return config.sync;
        }
    }

    // Default: sync
    debug!(layer = layer_name, "No match found, defaulting to sync=true");
    true
}

/// Extract all static layer names from permit
///
/// **Context**: Used to pre-create layers at page open time
/// **Static layers**: Layer names that are fully known after expanding {page_id} and {aud}
/// **Dynamic layers**: Pattern-based with wildcards (e.g., {page_id}/orders/*) - NOT included
///
/// **Returns**: List of expanded layer names that should exist at page load
pub fn extract_static_layers(permit: &str, page_id: &str, our_did: &str) -> Vec<String> {
    let parsed = match gurkha::Permit::from_token(permit) {
        Ok(p) => p,
        Err(e) => {
            warn!(error = ?e, "Failed to parse permit for static layers");
            return Vec::new();
        }
    };

    let mut layers = Vec::new();

    // Extract from `layers` section
    if let Some(layers_value) = parsed.get_fact("layers") {
        if let Some(layers_obj) = layers_value.as_object() {
            for pattern in layers_obj.keys() {
                let expanded = pattern
                    .replace("{page_id}", page_id)
                    .replace("{aud}", our_did);

                // Only static layers (no wildcards, no remaining placeholders)
                if !expanded.contains('*') && !expanded.contains('{') {
                    layers.push(expanded);
                }
            }
        }
    }

    // Note: layer_patterns section contains dynamic patterns (with *) - not included
    // Those are created on-demand when peers sync

    debug!(
        static_layers = ?layers,
        permit_page_id = page_id,
        "Extracted static layers from permit"
    );

    layers
}

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

// =============================================================================
// Permission Checking
// =============================================================================

/// Scribe permission checking methods
pub struct Permissions;

impl Permissions {
    /// Check if layer is local_only (never synced)
    pub fn is_local_only(layer_name: &str) -> bool {
        // Check all subscribers' policies - if ANY marks it local_only, treat as such
        // Actually, local_only is typically a page-level setting, not per-peer
        // For simplicity, we'll check the layer name
        layer_name == "user_content_doc"
    }

    /// Check if peer can write to layer
    ///
    /// **Context**: Remote peer is trying to write to this layer
    /// **We check**:
    /// 1. Peer is a subscriber with collaborator/submitter permission (fixed layers)
    /// 2. Peer is a subscriber with matching writable_pattern (pattern-based layers)
    /// 3. OR peer is our sync_target (owner/viewer mode - trust sync source)
    /// 4. OR peer has a stored permit with write access (node mode - permit-based sync auth)
    ///
    /// **Security**: For pattern-based layers, we validate that:
    /// - The layer name matches the expanded pattern
    /// - Patterns with {aud} expand to the peer's actual DID (namespace enforcement)
    pub fn can_write(state: &ScribeState, peer: &(String, String), layer_name: &str) -> bool {
        let peer_did = &peer.0;

        // Check if peer is a subscriber
        if let Ok(subs) = state.subscribers.read() {
            if let Some(info) = subs.get(peer) {
                // 1. Check fixed layer write permission
                if let Some(&can_write) = info.layer_permissions.get(layer_name) {
                    if can_write {
                        return true;
                    }
                }

                // 2. Check pattern-based write permission (role-agnostic)
                // This enforces namespace: patterns like {page_id}/*/{aud} only allow
                // writing to layers that contain the peer's own DID
                // write: can write to existing layers, create: can create new layers
                for pattern in &info.writable_patterns {
                    if (pattern.write || pattern.create) && pattern.matches(layer_name, &state.page_id, peer_did) {
                        trace!(
                            user_did = %peer_did,
                            layer = %layer_name,
                            pattern = %pattern.pattern,
                            "Write allowed via pattern match"
                        );
                        return true;
                    }
                }
            }
        }

        // 3. Owner/Viewer mode: trust data from our sync_target (the node we sync with)
        // This allows receiving sync updates from the node without explicit subscription
        if let Some(ref config) = state.sync_config {
            if let Some(ref sync_target) = config.sync_target {
                if peer_did == sync_target {
                    debug!(user_did = %peer_did, layer = %layer_name, "Write allowed from sync_target");
                    return true;
                }
            }
        }

        // 4. Node mode: check stored permit for write access (permit-based auth)
        if let Some(ref load_fn) = state.load_user_permit {
            if let Some(permit_token) = load_fn(&state.page_id, peer_did) {
                // Check fixed layer write permissions from permit
                if let Ok((layer_permissions, _)) = parse_permit_for_layers(&permit_token) {
                    if let Some(&can_write) = layer_permissions.get(layer_name) {
                        if can_write {
                            debug!(user_did = %peer_did, layer = %layer_name, "Write allowed via stored permit");
                            return true;
                        }
                    }
                }

                // Check pattern-based permissions from permit
                // write: can write to existing layers, create: can create new layers
                let (_, writable_patterns) = extract_patterns_from_permit(&permit_token, &state.page_id);
                for pattern in &writable_patterns {
                    if (pattern.write || pattern.create) && pattern.matches(layer_name, &state.page_id, peer_did) {
                        trace!(
                            user_did = %peer_did,
                            layer = %layer_name,
                            pattern = %pattern.pattern,
                            "Write allowed via stored permit pattern"
                        );
                        return true;
                    }
                }
            }
        }

        false
    }

    /// Check if peer can receive updates for layer
    ///
    /// **Context**: Deciding whether to broadcast layer update to peer
    /// **We check**:
    /// 1. Fixed layer permission exists AND not in no_incoming_updates
    /// 2. OR pattern matches peer's readable_patterns
    ///
    /// **Privacy**: Patterns like {page_id}/*/{aud} ensure:
    /// - Customer A only receives their own layers (shop123/orders/did:key:customer_a)
    /// - Customer B doesn't receive Customer A's layers
    /// - Owner/Admin with {page_id}/*/* pattern receives all layers
    #[allow(dead_code)]
    pub fn can_receive(state: &ScribeState, peer: &(String, String), layer_name: &str) -> bool {
        if let Ok(subs) = state.subscribers.read() {
            if let Some(info) = subs.get(peer) {
                return info.can_receive_layer(layer_name, &state.page_id);
            }
        }
        false
    }

    /// Check if we (local user) can write to a layer
    ///
    /// **Context**: Local Lua/UI is trying to modify a layer
    /// **We check**:
    /// 1. No permit = owner mode = full access
    /// 2. Fixed layer permission from our permit
    /// 3. Pattern matches our writable_patterns
    ///
    /// **Security**: Ensures local writes respect our permit constraints
    /// (e.g., viewer can only write to their namespaced layers)
    pub fn can_local_write(state: &ScribeState, layer_name: &str) -> bool {
        // No permit = owner mode = full access to all layers
        if state.our_permit.is_none() {
            return true;
        }

        // Check fixed layer permission from our permit
        if let Some(&can_write) = state.our_layer_permissions.get(layer_name) {
            if can_write {
                debug!(layer = %layer_name, "Local write allowed via fixed permission");
                return true;
            }
        }

        // Check pattern-based write permission
        // write: can write to existing layers, create: can create new layers
        for pattern in &state.our_writable_patterns {
            if (pattern.write || pattern.create) && pattern.matches(layer_name, &state.page_id, &state.our_did) {
                trace!(
                    layer = %layer_name,
                    pattern = %pattern.pattern,
                    "Local write allowed via pattern match"
                );
                return true;
            }
        }

        false
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use crate::scribe::state::{matches_wildcard_pattern, PatternRule};

    // ==================== Pattern Matching Tests ====================

    #[test]
    fn test_wildcard_pattern_exact_segments() {
        // Pattern: "shop123/*/did:key:customer_a"
        // Layer:   "shop123/orders/did:key:customer_a"
        assert!(matches_wildcard_pattern(
            "shop123/orders/did:key:customer_a",
            "shop123/*/did:key:customer_a"
        ));

        // Same pattern, different middle segment still matches
        assert!(matches_wildcard_pattern(
            "shop123/cart/did:key:customer_a",
            "shop123/*/did:key:customer_a"
        ));
    }

    #[test]
    fn test_wildcard_pattern_double_wildcard() {
        // Pattern: "shop123/*/*" (owner/admin pattern)
        // Matches any 3-segment layer starting with shop123/
        assert!(matches_wildcard_pattern(
            "shop123/orders/did:key:customer_a",
            "shop123/*/*"
        ));

        assert!(matches_wildcard_pattern(
            "shop123/cart/did:key:customer_b",
            "shop123/*/*"
        ));

        assert!(matches_wildcard_pattern(
            "shop123/anything/anything_else",
            "shop123/*/*"
        ));
    }

    #[test]
    fn test_wildcard_pattern_no_match_wrong_segments() {
        // Pattern expects 3 segments, layer has 2
        assert!(!matches_wildcard_pattern(
            "shop123/orders",
            "shop123/*/did:key:customer_a"
        ));

        // Pattern expects 3 segments, layer has 4
        assert!(!matches_wildcard_pattern(
            "shop123/orders/sub/did:key:customer_a",
            "shop123/*/did:key:customer_a"
        ));
    }

    #[test]
    fn test_wildcard_pattern_no_match_wrong_did() {
        // Customer A's pattern should NOT match Customer B's layer
        assert!(!matches_wildcard_pattern(
            "shop123/orders/did:key:customer_b",
            "shop123/*/did:key:customer_a"
        ));
    }

    #[test]
    fn test_pattern_rule_matches_with_expansion() {
        // Pattern with placeholders
        let rule = PatternRule {
            pattern: "{page_id}/*/{aud}".to_string(),
            sync: true,
            create: true,
            write: false,
        };

        // Customer A's layer matches when expanded with their DID
        assert!(rule.matches(
            "shop123/orders/did:key:customer_a",
            "shop123",
            "did:key:customer_a"
        ));

        // Customer A's pattern does NOT match Customer B's layer
        assert!(!rule.matches(
            "shop123/orders/did:key:customer_b",
            "shop123",
            "did:key:customer_a"
        ));
    }

    #[test]
    fn test_pattern_rule_owner_wildcard() {
        // Owner pattern matches all viewer layers
        let owner_rule = PatternRule {
            pattern: "{page_id}/*/*".to_string(),
            sync: true,
            create: true,
            write: false,
        };

        // Owner can receive Customer A's layer
        assert!(owner_rule.matches(
            "shop123/orders/did:key:customer_a",
            "shop123",
            "did:key:owner"  // Owner's own DID doesn't matter for this pattern
        ));

        // Owner can receive Customer B's layer
        assert!(owner_rule.matches(
            "shop123/orders/did:key:customer_b",
            "shop123",
            "did:key:owner"
        ));
    }

    #[test]
    fn test_privacy_isolation() {
        // Customer A pattern
        let customer_a_pattern = PatternRule {
            pattern: "{page_id}/*/{aud}".to_string(),
            sync: true,
            create: true,
            write: false,
        };

        // Customer B pattern
        let customer_b_pattern = PatternRule {
            pattern: "{page_id}/*/{aud}".to_string(),
            sync: true,
            create: true,
            write: false,
        };

        let layer_a = "shop123/orders/did:key:customer_a";
        let layer_b = "shop123/orders/did:key:customer_b";

        // Customer A can access their own layer
        assert!(customer_a_pattern.matches(layer_a, "shop123", "did:key:customer_a"));
        // Customer A cannot access Customer B's layer
        assert!(!customer_a_pattern.matches(layer_b, "shop123", "did:key:customer_a"));

        // Customer B can access their own layer
        assert!(customer_b_pattern.matches(layer_b, "shop123", "did:key:customer_b"));
        // Customer B cannot access Customer A's layer
        assert!(!customer_b_pattern.matches(layer_a, "shop123", "did:key:customer_b"));
    }
}
