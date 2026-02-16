//! Layer Pattern Authorization
//!
//! Functions for checking if a permit authorizes access to a layer.
//! Supports two-tier access: page permit (static layers) + layer permits (dynamic layers).

/// Check if permit authorizes access to a layer
///
/// With fully-resolved permits, this is a simple map lookup.
/// Layer keys in the permit are already expanded (e.g., "abc123/products").
///
/// # Arguments
/// * `permit` - The viewer's permit
/// * `layer_name` - Name of the layer to access (bare or full)
/// * `operation` - Operation to check: "read", "write", "sync"
///
/// # Returns
/// `true` if the permit authorizes this layer access
pub fn can_access_layer(permit: &crate::parser::Permit, layer_name: &str, operation: &str) -> bool {
    let page_id = permit
        .get_fact("page_id")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    // Try direct lookup first
    if let Some(config) = permit.layers().get(layer_name) {
        return check_operation(config, operation);
    }

    // Try with page_id prefix (for bare names like "products")
    let full_name = format!("{}/{}", page_id, layer_name);
    if let Some(config) = permit.layers().get(&full_name) {
        return check_operation(config, operation);
    }

    // Try stripping page_id prefix (for full names when permit has bare)
    let page_prefix = format!("{}/", page_id);
    if let Some(bare) = layer_name.strip_prefix(&page_prefix) {
        if let Some(config) = permit.layers().get(bare) {
            return check_operation(config, operation);
        }
    }

    tracing::debug!("[can_access_layer] No layer match for '{}'", layer_name);
    false
}

/// Check access across a page permit + set of layer permits (two-tier model)
///
/// **Context**: Dynamic layers are authorized via separate layer permits.
/// This function checks if any permit in the set grants access to the layer.
///
/// Access check = page permit layers ∪ all layer permit layers
pub fn can_access_with_layer_permits(
    page_permit: &crate::parser::Permit,
    layer_permits: &[crate::parser::Permit],
    layer_name: &str,
    page_id: &str,
    did: &str,
    operation: &str,
) -> bool {
    // Check page permit first
    if can_access_layer(page_permit, layer_name, operation) {
        return true;
    }

    // Check each layer permit
    for lp in layer_permits {
        if can_access_layer(lp, layer_name, operation) {
            return true;
        }
    }

    // Schema fallback: creator can access dynamic layers matching their own DID in path
    if matches_creator_schema(page_permit, layer_name, page_id, did, operation) {
        return true;
    }

    false
}

/// Check if a layer matches a dynamic schema (node-side auth)
///
/// **Context**: Node needs to authorize writes from peers to dynamic layers.
/// The peer's page permit doesn't cover the dynamic layer (it has another user's DID in the path).
/// Instead, the node checks its own permit's `dynamic_layer_schemas` permissions.
///
/// **Example**: Bob writes to `channels/did:key:alice/project-x/messages`.
/// Node's permit has schema `channels/{id}/messages` with `permissions.write = true`.
/// The path structure matches the schema → write is allowed.
pub fn matches_dynamic_schema(
    permit: &crate::parser::Permit,
    layer_name: &str,
    page_id: &str,
    operation: &str,
) -> bool {
    let schemas = permit.dynamic_layer_schemas();
    if schemas.is_empty() {
        return false;
    }

    // Strip page_id prefix to get bare layer path
    let page_prefix = format!("{}/", page_id);
    let bare_path = layer_name.strip_prefix(&page_prefix).unwrap_or(layer_name);

    for (schema_pattern, schema) in schemas {
        if matches_dynamic_path_any_did(bare_path, schema_pattern) {
            if check_operation(&schema.permissions, operation) {
                return true;
            }
        }
    }

    false
}

/// Match a layer path against a dynamic schema pattern (any DID accepted)
///
/// **Context**: Unlike `matches_dynamic_path`, this doesn't require a specific DID.
/// Used for node-side authorization where the peer writing isn't the creator.
///
/// **Pattern**: `channels/{id}/messages`
/// **Path**:    `channels/did:key:alice/project-x/messages`
///
/// Accepts any value in the DID position (path_parts[1]).
fn matches_dynamic_path_any_did(bare_path: &str, schema_pattern: &str) -> bool {
    let path_parts: Vec<&str> = bare_path.split('/').collect();
    let pattern_parts: Vec<&str> = schema_pattern.split('/').collect();

    // Dynamic path has one extra segment (the DID) compared to schema pattern
    if path_parts.len() != pattern_parts.len() + 1 {
        return false;
    }

    // First segment must match literally
    if path_parts.is_empty() || pattern_parts.is_empty() || path_parts[0] != pattern_parts[0] {
        return false;
    }

    // path_parts[1] is the DID segment — accept any value (no specific DID check)

    // Remaining segments: match path[2..] against pattern[1..]
    for (path_seg, pat_seg) in path_parts[2..].iter().zip(pattern_parts[1..].iter()) {
        if *pat_seg != "{id}" && path_seg != pat_seg {
            return false;
        }
    }

    true
}

/// Schema fallback for creator access to dynamic layers
///
/// **Context**: When a creator creates a dynamic layer, they can access it immediately
/// via schema matching (before the node issues a layer permit). The layer path must
/// contain the creator's DID to prove ownership.
///
/// **Example**: Creator with DID `did:key:alice` and schema `channels/{id}/messages`
/// can access `page1/channels/did:key:alice/general/messages` because:
/// 1. Schema `channels/{id}/messages` exists in their permit
/// 2. The path contains their DID after stripping page_id prefix
pub fn matches_creator_schema(
    permit: &crate::parser::Permit,
    layer_name: &str,
    page_id: &str,
    our_did: &str,
    operation: &str,
) -> bool {
    let schemas = permit.dynamic_layer_schemas();
    if schemas.is_empty() {
        return false;
    }

    // Strip page_id prefix to get bare layer path
    let page_prefix = format!("{}/", page_id);
    let bare_path = layer_name.strip_prefix(&page_prefix).unwrap_or(layer_name);

    // For creator access, the path must contain the creator's DID
    // Path format: {schema_prefix}/{creator_did}/{id_segments}/{schema_suffix}
    if !bare_path.contains(our_did) {
        return false;
    }

    // Try to match against each schema pattern
    // Schema pattern: "channels/{id}/messages"
    // Layer path:     "channels/did:key:alice/general/messages"
    // We need to check if the path matches the schema with DID inserted
    for (schema_pattern, schema) in schemas {
        if matches_dynamic_path(bare_path, schema_pattern, our_did) {
            if check_operation(&schema.permissions, operation) {
                return true;
            }
        }
    }

    false
}

/// Check if a layer path matches a dynamic schema pattern with a DID inserted
///
/// Schema patterns use `{id}` as wildcard. For creator access, the DID is
/// inserted as an additional path segment after the first literal segment.
///
/// **Pattern**: `channels/{id}/messages`
/// **Path**:    `channels/did:key:alice/general/messages`
///
/// The DID segment (`did:key:alice`) is the creator's namespace.
/// The `{id}` matches the user-chosen ID (`general`).
fn matches_dynamic_path(bare_path: &str, schema_pattern: &str, creator_did: &str) -> bool {
    let path_parts: Vec<&str> = bare_path.split('/').collect();
    let pattern_parts: Vec<&str> = schema_pattern.split('/').collect();

    // Dynamic path has one extra segment (the DID) compared to schema pattern
    if path_parts.len() != pattern_parts.len() + 1 {
        return false;
    }

    // First segment must match literally
    if path_parts.is_empty() || pattern_parts.is_empty() || path_parts[0] != pattern_parts[0] {
        return false;
    }

    // Second segment in path must be the creator's DID
    if path_parts.len() < 2 || path_parts[1] != creator_did {
        return false;
    }

    // Remaining segments: match path[2..] against pattern[1..]
    for (path_seg, pat_seg) in path_parts[2..].iter().zip(pattern_parts[1..].iter()) {
        if *pat_seg != "{id}" && path_seg != pat_seg {
            return false;
        }
    }

    true
}

/// Check if a layer config allows the given operation
fn check_operation(config: &crate::parser::LayerConfig, operation: &str) -> bool {
    match operation {
        "read" => config.sync,
        "write" => config.write,
        "sync" => config.sync,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_fixtures;

    #[test]
    fn test_can_access_layer_direct_match() {
        let permit = test_fixtures::shop_owner("shop123", "did:key:owner");
        // Owner has resolved layer "shop123/products"
        assert!(can_access_layer(&permit, "shop123/products", "read"));
        assert!(can_access_layer(&permit, "shop123/products", "write"));
        assert!(can_access_layer(&permit, "shop123/products", "sync"));
    }

    #[test]
    fn test_can_access_layer_bare_name() {
        let permit = test_fixtures::shop_owner("shop123", "did:key:owner");
        // Bare name "products" should resolve via page_id prefix
        assert!(can_access_layer(&permit, "products", "read"));
        assert!(can_access_layer(&permit, "products", "write"));
    }

    #[test]
    fn test_can_access_layer_app_layer() {
        let permit = test_fixtures::shop_owner("shop123", "did:key:owner");
        // App layers don't have page_id prefix
        assert!(can_access_layer(&permit, "app:Shop", "read"));
        assert!(can_access_layer(&permit, "app:Shop", "write"));
    }

    #[test]
    fn test_can_access_layer_no_match() {
        let permit = test_fixtures::shop_owner("shop123", "did:key:owner");
        assert!(!can_access_layer(&permit, "nonexistent", "read"));
        assert!(!can_access_layer(&permit, "shop123/nonexistent", "read"));
    }

    #[test]
    fn test_can_access_layer_read_only() {
        let permit = test_fixtures::shop_customer("shop123", "did:key:customer");
        // Customer has products with write=false, sync=true
        assert!(can_access_layer(&permit, "shop123/products", "read"));
        assert!(can_access_layer(&permit, "shop123/products", "sync"));
        assert!(!can_access_layer(&permit, "shop123/products", "write"));
    }

    #[test]
    fn test_check_operation() {
        let config = crate::parser::LayerConfig {
            sync: true,
            write: false,
            layer_type: None,
        };
        assert!(check_operation(&config, "read"));
        assert!(check_operation(&config, "sync"));
        assert!(!check_operation(&config, "write"));
        assert!(!check_operation(&config, "unknown"));
    }

    #[test]
    fn test_two_tier_access_check() {
        let page_permit = test_fixtures::shop_owner("shop1", "did:key:owner");
        // Page permit has static layers (products, etc.)
        assert!(can_access_with_layer_permits(
            &page_permit,
            &[],
            "shop1/products",
            "shop1",
            "did:key:owner",
            "read"
        ));
        // Dynamic layer not in page permit → no access without layer permit
        assert!(!can_access_with_layer_permits(
            &page_permit,
            &[],
            "shop1/channels/did:key:bob/general/messages",
            "shop1",
            "did:key:owner",
            "read"
        ));
    }

    #[test]
    fn test_schema_fallback_creator_access() {
        // Shop owner has dynamic_layer_schemas: { "orders/{id}": { grant: "explicit", ... } }
        let owner = test_fixtures::shop_owner("shop1", "did:key:owner");

        // Creator can access dynamic layer matching their DID via schema fallback
        assert!(can_access_with_layer_permits(
            &owner,
            &[],
            "shop1/orders/did:key:owner/uuid-123",
            "shop1",
            "did:key:owner",
            "read"
        ));
        assert!(can_access_with_layer_permits(
            &owner,
            &[],
            "shop1/orders/did:key:owner/uuid-123",
            "shop1",
            "did:key:owner",
            "write"
        ));

        // Non-matching DID in path → denied
        assert!(!can_access_with_layer_permits(
            &owner,
            &[],
            "shop1/orders/did:key:other/uuid-123",
            "shop1",
            "did:key:owner",
            "read"
        ));

        // Non-matching schema path → denied
        assert!(!can_access_with_layer_permits(
            &owner,
            &[],
            "shop1/invalid_schema/did:key:owner/foo",
            "shop1",
            "did:key:owner",
            "read"
        ));
    }

    #[test]
    fn test_matches_dynamic_path() {
        // orders/{id} with DID inserted
        assert!(matches_dynamic_path(
            "orders/did:key:alice/uuid-123",
            "orders/{id}",
            "did:key:alice"
        ));
        // channels/{id}/messages with DID inserted
        assert!(matches_dynamic_path(
            "channels/did:key:alice/general/messages",
            "channels/{id}/messages",
            "did:key:alice"
        ));
        // Wrong DID
        assert!(!matches_dynamic_path(
            "channels/did:key:bob/general/messages",
            "channels/{id}/messages",
            "did:key:alice"
        ));
        // Wrong prefix
        assert!(!matches_dynamic_path(
            "wrong/did:key:alice/general/messages",
            "channels/{id}/messages",
            "did:key:alice"
        ));
        // Different segment count
        assert!(!matches_dynamic_path(
            "channels/did:key:alice/messages",
            "channels/{id}/messages",
            "did:key:alice"
        ));
    }

    #[test]
    fn test_matches_dynamic_path_any_did() {
        // Matches with any DID in position 1
        assert!(matches_dynamic_path_any_did(
            "channels/did:key:alice/general/messages",
            "channels/{id}/messages"
        ));
        assert!(matches_dynamic_path_any_did(
            "channels/did:key:bob/project-x/messages",
            "channels/{id}/messages"
        ));
        // Wrong prefix
        assert!(!matches_dynamic_path_any_did(
            "wrong/did:key:alice/general/messages",
            "channels/{id}/messages"
        ));
        // Wrong suffix
        assert!(!matches_dynamic_path_any_did(
            "channels/did:key:alice/general/wrong",
            "channels/{id}/messages"
        ));
        // Different segment count
        assert!(!matches_dynamic_path_any_did(
            "channels/did:key:alice/messages",
            "channels/{id}/messages"
        ));
    }

    #[test]
    fn test_matches_dynamic_schema_explicit_grant() {
        // Shop owner has explicit grant: permissions apply to all peers
        let owner = test_fixtures::shop_owner("shop1", "did:key:owner");

        // Any peer can access via permissions { sync: true, write: true }
        assert!(matches_dynamic_schema(
            &owner,
            "orders/did:key:customer/uuid-123",
            "shop1",
            "write"
        ));
        assert!(matches_dynamic_schema(
            &owner,
            "orders/did:key:customer/uuid-123",
            "shop1",
            "read"
        ));

        // Non-matching schema path → denied
        assert!(!matches_dynamic_schema(
            &owner,
            "invalid/did:key:customer/uuid-123",
            "shop1",
            "write"
        ));

        // Works with page_id prefix stripped
        assert!(matches_dynamic_schema(
            &owner,
            "shop1/orders/did:key:customer/uuid-123",
            "shop1",
            "write"
        ));
    }

    #[test]
    fn test_matches_dynamic_schema_open_grant() {
        // Admin has open grant: permissions { sync: true, write: false }
        let admin = test_fixtures::shop_admin("shop1", "did:key:admin");

        // All peers can read but not write (same permissions for everyone)
        assert!(matches_dynamic_schema(
            &admin,
            "orders/did:key:customer/uuid-123",
            "shop1",
            "read"
        ));
        assert!(!matches_dynamic_schema(
            &admin,
            "orders/did:key:customer/uuid-123",
            "shop1",
            "write"
        ));
    }
}
