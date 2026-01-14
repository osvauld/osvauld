//! Layer Pattern Authorization
//!
//! Functions for checking if a permit authorizes access to a layer.

/// Check if permit authorizes access to a layer (IDENTITY-based)
///
/// **Note**: This only checks if the viewer CAN access the layer based on identity.
/// State-based checks (can_write, can_delete) happen via auth_lua in AuthSandbox.
///
/// # Arguments
/// * `permit` - The viewer's permit
/// * `layer_name` - Name of the layer to access
/// * `operation` - Operation to check: "create", "sync", "read", "write"
///
/// # Returns
/// `true` if the permit authorizes this layer access
pub fn can_access_layer(permit: &crate::parser::Permit, layer_name: &str, operation: &str) -> bool {
    let aud_b64 = permit.core().audience().unwrap_or("");
    let iss = permit.core().issuer().unwrap_or("");

    // Convert base64 audience to DID format for pattern matching
    // Layer names use DID format (e.g., "shop123/orders/did:key:z6Mk...")
    let aud_did = crate::crypto::did_from_base64_pubkey(aud_b64)
        .unwrap_or_else(|_| aud_b64.to_string());

    // Get page_id from permit facts for placeholder expansion
    let page_id = permit.get_fact("page_id")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    // Check fixed layers with {page_id} expansion
    // Fixed layers in permit have keys like "{page_id}/products" that need expansion
    for (layer_key, config) in permit.layers() {
        let expanded_key = layer_key
            .replace("{page_id}", page_id)
            .replace("{aud}", &aud_did)
            .replace("{iss}", iss);

        if expanded_key == layer_name {
            let allowed = config.sync || operation == "read" || operation == "write";
            tracing::debug!(
                "✅ [can_access_layer] Fixed layer '{}' → '{}' matches '{}': {} = {}",
                layer_key, expanded_key, layer_name, operation, allowed
            );
            return allowed;
        }
    }

    // Check layer_patterns with placeholder expansion
    // Use DID format for {aud} so it matches DID-based layer names
    for (pattern, config) in permit.layer_patterns() {
        let expanded = pattern
            .replace("{page_id}", page_id)
            .replace("{aud}", &aud_did)
            .replace("{iss}", iss);

        if matches_layer_pattern(layer_name, &expanded) {
            let allowed = match operation {
                "create" => config.create,
                "sync" => config.sync,
                "read" | "write" => true, // State-based checks in AuthSandbox
                _ => false,
            };
            tracing::debug!(
                "🔍 [can_access_layer] Pattern '{}' → '{}' matches '{}': {} = {}",
                pattern, expanded, layer_name, operation, allowed
            );
            return allowed;
        }
    }

    tracing::debug!("🚫 [can_access_layer] No pattern matches '{}'", layer_name);
    false
}

/// Match a layer name against a pattern
///
/// Supports:
/// - Exact match: "layer_name" matches "layer_name"
/// - Prefix wildcard: "did:key:abc:*" matches "did:key:abc:orders"
/// - Suffix wildcard: "*:orders" matches "did:key:abc:orders"
fn matches_layer_pattern(layer_name: &str, pattern: &str) -> bool {
    if pattern.ends_with(":*") {
        // Prefix match: "did:key:abc:*" matches "did:key:abc:anything"
        layer_name.starts_with(&pattern[..pattern.len() - 1])
    } else if pattern.ends_with("*") {
        // General prefix: "prefix*" matches "prefixanything"
        layer_name.starts_with(&pattern[..pattern.len() - 1])
    } else if pattern.starts_with("*:") {
        // Suffix match: "*:orders" matches "anything:orders"
        layer_name.ends_with(&pattern[1..])
    } else {
        // Exact match
        layer_name == pattern
    }
}
