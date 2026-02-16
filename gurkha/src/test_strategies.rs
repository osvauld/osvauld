//! Composable test strategies for property-based testing
//!
//! Layered design:
//!   Level 1 (Primitives): did, page_id, segment
//!   Level 2 (Structures): layer_name, pattern, layer_config
//!   Level 3 (Complex): permit_facts, sync_context builders
//!
//! Some strategies are prepared for future tests and may appear unused.

#![allow(dead_code)]

use crate::parser::{LayerConfig, LayerPatternConfig};
use proptest::prelude::*;
use std::collections::HashMap;

/// Base58 multibase string (like real DIDs)
pub fn did_strategy() -> impl Strategy<Value = String> {
    "[a-km-zA-HJ-NP-Z2-9]{43}" // Base58 charset (no 0, O, I, l)
        .prop_map(|s| format!("did:key:z6Mk{}", s))
}

/// Valid page ID (hex string, like real page IDs)
pub fn page_id_strategy() -> impl Strategy<Value = String> {
    "[a-f0-9]{32}"
}

/// Single path segment (no slashes, no special chars)
/// Starts with letter, alphanumeric with underscores
pub fn segment_strategy() -> impl Strategy<Value = String> {
    "[a-z][a-z0-9_]{0,19}"
}

/// Segment that could be a placeholder token
pub fn placeholder_segment_strategy() -> impl Strategy<Value = String> {
    prop_oneof![
        Just("{page_id}".to_string()),
        Just("{aud}".to_string()),
        Just("{iss}".to_string()),
        segment_strategy(),
    ]
}

/// Layer name with 1-4 segments (e.g., "orders", "shop/orders", "shop/orders/customer")
pub fn layer_name_strategy() -> impl Strategy<Value = String> {
    prop_oneof![
        // 1 segment
        segment_strategy(),
        // 2 segments
        (segment_strategy(), segment_strategy()).prop_map(|(a, b)| format!("{}/{}", a, b)),
        // 3 segments
        (segment_strategy(), segment_strategy(), segment_strategy())
            .prop_map(|(a, b, c)| format!("{}/{}/{}", a, b, c)),
        // 4 segments
        (
            segment_strategy(),
            segment_strategy(),
            segment_strategy(),
            segment_strategy()
        )
            .prop_map(|(a, b, c, d)| format!("{}/{}/{}/{}", a, b, c, d)),
    ]
}

/// Layer pattern with placeholders (unexpanded, for testing expansion)
pub fn layer_pattern_with_placeholders() -> impl Strategy<Value = String> {
    prop_oneof![
        Just("{page_id}/data".to_string()),
        Just("{page_id}/orders/{aud}".to_string()),
        Just("{page_id}/{aud}/private".to_string()),
        Just("{aud}/inbox".to_string()),
        segment_strategy().prop_map(|s| format!("{{page_id}}/{}", s)),
        segment_strategy().prop_map(|s| format!("{{page_id}}/{}/{{aud}}", s)),
    ]
}

/// Pattern with path-segment wildcards (for parser::matches_wildcard)
pub fn wildcard_pattern_strategy() -> impl Strategy<Value = String> {
    prop_oneof![
        // Exact match (no wildcard)
        layer_name_strategy(),
        // Trailing wildcard: "prefix/*"
        segment_strategy().prop_map(|s| format!("{}/*", s)),
        // Leading wildcard: "*/suffix"
        segment_strategy().prop_map(|s| format!("*/{}", s)),
        // Middle wildcard: "prefix/*/suffix"
        (segment_strategy(), segment_strategy()).prop_map(|(a, b)| format!("{}/*/{}", a, b)),
        // Multiple wildcards
        segment_strategy().prop_map(|s| format!("{}/*/*", s)),
        Just("*/*/*".to_string()),
    ]
}

/// Pattern for colon-based matching (for decision/layer.rs::matches_layer_pattern)
pub fn colon_pattern_strategy() -> impl Strategy<Value = String> {
    prop_oneof![
        // Exact match
        "[a-z0-9]+:[a-z0-9]+".prop_map(|s| s),
        // Prefix wildcard: "prefix:*"
        segment_strategy().prop_map(|s| format!("{}:*", s)),
        // Suffix wildcard: "*:suffix"
        segment_strategy().prop_map(|s| format!("*:{}", s)),
        // DID-style patterns
        Just("did:key:*".to_string()),
    ]
}

/// Layer pattern config (sync, create, write permissions)
pub fn layer_pattern_config_strategy() -> impl Strategy<Value = LayerPatternConfig> {
    (any::<bool>(), any::<bool>(), any::<bool>()).prop_map(|(sync, create, write)| {
        LayerPatternConfig {
            sync,
            create,
            write,
        }
    })
}

/// Layer config (sync, write, optional type)
pub fn layer_config_strategy() -> impl Strategy<Value = LayerConfig> {
    (
        any::<bool>(),
        any::<bool>(),
        prop::option::of(Just("list".to_string())),
    )
        .prop_map(|(sync, write, layer_type)| LayerConfig {
            sync,
            write,
            layer_type,
        })
}

/// Generate a set of layer patterns with configs
pub fn layer_patterns_map_strategy() -> impl Strategy<Value = HashMap<String, LayerPatternConfig>> {
    prop::collection::hash_map(
        wildcard_pattern_strategy(),
        layer_pattern_config_strategy(),
        0..5, // 0-5 patterns
    )
}

/// Generate fixed layers (exact names with placeholders)
pub fn fixed_layers_map_strategy() -> impl Strategy<Value = HashMap<String, LayerConfig>> {
    prop::collection::hash_map(
        layer_pattern_with_placeholders(),
        layer_config_strategy(),
        0..4, // 0-4 fixed layers
    )
}

/// Create a layer name that SHOULD match a given wildcard pattern
/// Replaces * with random segment
pub fn layer_matching_wildcard_pattern(pattern: String) -> impl Strategy<Value = String> {
    let parts: Vec<String> = pattern.split('/').map(String::from).collect();

    // Build a strategy that generates matching layers
    parts
        .into_iter()
        .map(|p| {
            if p == "*" {
                segment_strategy().boxed()
            } else {
                Just(p).boxed()
            }
        })
        .collect::<Vec<_>>()
        .into_iter()
        .fold(Just(String::new()).boxed(), |acc, s| {
            (acc, s)
                .prop_map(|(a, b)| {
                    if a.is_empty() {
                        b
                    } else {
                        format!("{}/{}", a, b)
                    }
                })
                .boxed()
        })
}

/// Create a layer name that should NOT match a pattern (different segment count)
pub fn layer_not_matching_pattern(pattern: String) -> impl Strategy<Value = String> {
    let segment_count = pattern.split('/').count();

    // Generate a layer with different segment count
    let target_segments = if segment_count > 1 {
        segment_count - 1
    } else {
        segment_count + 1
    };

    prop::collection::vec(segment_strategy(), target_segments..=target_segments)
        .prop_map(|segments| segments.join("/"))
}

/// Expand a pattern with concrete values (test helper)
pub fn expand_pattern_concrete(pattern: &str, page_id: &str, did: &str) -> String {
    pattern
        .replace("{page_id}", page_id)
        .replace("{aud}", did)
        .replace("{iss}", did)
}

/// Create a layer name that matches a colon prefix pattern
pub fn layer_matching_colon_prefix(prefix: String) -> impl Strategy<Value = String> {
    segment_strategy().prop_map(move |suffix| format!("{}:{}", prefix, suffix))
}

/// Create a layer name that matches a colon suffix pattern
pub fn layer_matching_colon_suffix(suffix: String) -> impl Strategy<Value = String> {
    segment_strategy().prop_map(move |prefix| format!("{}:{}", prefix, suffix))
}
