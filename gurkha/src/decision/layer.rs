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
    let aud_b64 = permit.audience().unwrap_or("");
    let iss = permit.issuer().unwrap_or("");

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
        let expanded_key = expand_pattern_with_iss(layer_key, page_id, &aud_did, iss);

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
        let expanded = expand_pattern_with_iss(pattern, page_id, &aud_did, iss);

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

/// Expand pattern with {page_id}, {aud}, and {iss} placeholders
///
/// Extends parser::expand_pattern with additional {iss} support for layer access checks.
fn expand_pattern_with_iss(pattern: &str, page_id: &str, aud: &str, iss: &str) -> String {
    crate::parser::expand_pattern(pattern, page_id, aud)
        .replace("{iss}", iss)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_strategies::*;
    use proptest::prelude::*;

    // expand_pattern_with_iss: Placeholder expansion with {iss}

    proptest! {
        /// expand_pattern_with_iss handles all three placeholders
        #[test]
        fn expand_with_iss_handles_all_placeholders(
            page_id in page_id_strategy(),
            aud in did_strategy(),
            iss in did_strategy(),
        ) {
            let pattern = "{page_id}/{aud}/{iss}";
            let result = expand_pattern_with_iss(pattern, &page_id, &aud, &iss);

            prop_assert!(!result.contains("{page_id}"), "Should expand page_id");
            prop_assert!(!result.contains("{aud}"), "Should expand aud");
            prop_assert!(!result.contains("{iss}"), "Should expand iss");
            prop_assert!(result.contains(&page_id), "Should contain page_id value");
            prop_assert!(result.contains(&aud), "Should contain aud value");
            prop_assert!(result.contains(&iss), "Should contain iss value");
        }
    }

    #[test]
    fn doc_expand_pattern_with_iss() {
        let result = expand_pattern_with_iss(
            "{page_id}/orders/{aud}/from/{iss}",
            "shop123",
            "did:key:viewer",
            "did:key:issuer"
        );
        assert_eq!(result, "shop123/orders/did:key:viewer/from/did:key:issuer");
    }

    // Complex Colon Pattern Tests

    #[test]
    fn complex_did_colon_nested_patterns() {
        let did = "did:key:z6MkhaXgBZDvotDkL5257faiztiGiC2QtKLGpbnnEGta2doK";

        let layer = format!("{}:data", did);
        assert!(matches_layer_pattern(&layer, &layer));

        let prefix_pattern = format!("{}:*", did);
        assert!(matches_layer_pattern(&layer, &prefix_pattern));
        assert!(matches_layer_pattern(&format!("{}:orders", did), &prefix_pattern));

        assert!(matches_layer_pattern(&layer, "*:data"));
    }

    #[test]
    fn complex_prefix_vs_suffix_wildcard_semantics() {
        // Prefix matching
        assert!(matches_layer_pattern("app:feature1", "app:*"));
        assert!(matches_layer_pattern("app:feature2", "app:*"));
        assert!(matches_layer_pattern("app:", "app:*"));
        assert!(!matches_layer_pattern("other:feature1", "app:*"));

        // Suffix matching
        assert!(matches_layer_pattern("ns1:data", "*:data"));
        assert!(matches_layer_pattern("ns2:data", "*:data"));
        assert!(matches_layer_pattern(":data", "*:data"));
        assert!(!matches_layer_pattern("ns1:other", "*:data"));
    }

    #[test]
    fn complex_general_prefix_wildcard() {
        assert!(matches_layer_pattern("appdata", "app*"));
        assert!(matches_layer_pattern("application", "app*"));
        assert!(matches_layer_pattern("app", "app*"));
        assert!(!matches_layer_pattern("myapp", "app*"));

        // Compare with colon pattern
        assert!(matches_layer_pattern("app:data", "app:*"));
        assert!(!matches_layer_pattern("appdata", "app:*"));
    }

    #[test]
    fn complex_no_wildcard_exact_match() {
        assert!(matches_layer_pattern("exact:match", "exact:match"));
        assert!(!matches_layer_pattern("exact:match", "exact:other"));
        assert!(!matches_layer_pattern("exact:match", "other:match"));
        assert!(!matches_layer_pattern("exact:match:extra", "exact:match"));
        assert!(!matches_layer_pattern("exact:matc", "exact:match"));
    }

    #[test]
    fn complex_colon_in_middle_of_pattern() {
        let layer = "namespace:category:item";

        assert!(matches_layer_pattern(layer, "namespace:category:item"));
        assert!(matches_layer_pattern(layer, "namespace:category:*"));
        assert!(matches_layer_pattern(layer, "*:item"));
        assert!(matches_layer_pattern(layer, "namespace:*"));
        assert!(matches_layer_pattern("namespace:anything:else:here", "namespace:*"));
        assert!(!matches_layer_pattern("namespace:other:item", "namespace:category:*"));
    }

    #[test]
    fn edge_case_empty_segments_colon() {
        assert!(matches_layer_pattern(":data", ":data"));
        assert!(matches_layer_pattern(":data", "*:data"));
        assert!(matches_layer_pattern("prefix:", "prefix:"));
        assert!(matches_layer_pattern("prefix:", "prefix:*"));
        assert!(matches_layer_pattern(":", ":"));
    }

    #[test]
    fn edge_case_no_colon_in_layer() {
        assert!(matches_layer_pattern("simple", "simple"));
        assert!(!matches_layer_pattern("simple", "other"));
        assert!(matches_layer_pattern("simple", "sim*"));
        assert!(matches_layer_pattern("simulation", "sim*"));
    }

    proptest! {
        /// Prefix wildcard matches all suffixes
        #[test]
        fn colon_prefix_wild_matches_all_suffixes(
            prefix in "[a-z][a-z0-9]{0,10}",
            suffix1 in "[a-z][a-z0-9]{0,10}",
            suffix2 in "[a-z][a-z0-9]{0,10}",
        ) {
            let pattern = format!("{}:*", prefix);
            let layer1 = format!("{}:{}", prefix, suffix1);
            let layer2 = format!("{}:{}", prefix, suffix2);

            prop_assert!(matches_layer_pattern(&layer1, &pattern),
                "Prefix wildcard should match: {} vs {}", layer1, pattern);
            prop_assert!(matches_layer_pattern(&layer2, &pattern),
                "Prefix wildcard should match: {} vs {}", layer2, pattern);
        }

        /// Suffix wildcard matches all prefixes
        #[test]
        fn colon_suffix_wild_matches_all_prefixes(
            prefix1 in "[a-z][a-z0-9]{0,10}",
            prefix2 in "[a-z][a-z0-9]{0,10}",
            suffix in "[a-z][a-z0-9]{0,10}",
        ) {
            let pattern = format!("*:{}", suffix);
            let layer1 = format!("{}:{}", prefix1, suffix);
            let layer2 = format!("{}:{}", prefix2, suffix);

            prop_assert!(matches_layer_pattern(&layer1, &pattern),
                "Suffix wildcard should match: {} vs {}", layer1, pattern);
            prop_assert!(matches_layer_pattern(&layer2, &pattern),
                "Suffix wildcard should match: {} vs {}", layer2, pattern);
        }

        /// Different prefixes don't match without wildcard
        #[test]
        fn colon_different_prefix_no_match(
            prefix1 in "[a-z][a-z0-9]{0,10}",
            prefix2 in "[a-z][a-z0-9]{0,10}",
            suffix in "[a-z][a-z0-9]{0,10}",
        ) {
            prop_assume!(prefix1 != prefix2);

            let layer = format!("{}:{}", prefix1, suffix);
            let pattern = format!("{}:{}", prefix2, suffix);

            prop_assert!(!matches_layer_pattern(&layer, &pattern),
                "Different prefixes should not match: {} vs {}", layer, pattern);
        }
    }
}
