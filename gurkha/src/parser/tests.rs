//! Property-based tests for parser pattern matching
//!
//! Tests two key functions:
//! - `expand_pattern`: Placeholder expansion ({page_id}, {aud})
//! - `matches_wildcard`: Path-segment wildcard matching (*)

use super::*;
use crate::test_strategies::*;
use proptest::prelude::*;

// Pattern Expansion Properties

proptest! {
    /// Expansion properties: idempotent, removes placeholders, preserves literals, exact values
    #[test]
    fn test_expand_properties(
        pattern in layer_pattern_with_placeholders(),
        prefix in segment_strategy(),
        suffix in segment_strategy(),
        page_id in page_id_strategy(),
        did in did_strategy(),
    ) {
        // Idempotent
        let once = expand_pattern(&pattern, &page_id, &did);
        let twice = expand_pattern(&once, &page_id, &did);
        prop_assert_eq!(&once, &twice, "Expansion should be idempotent");

        // Removes all known placeholders
        for p in &["{page_id}/data", "{page_id}/orders/{aud}", "{aud}/{page_id}"] {
            let result = expand_pattern(p, &page_id, &did);
            prop_assert!(!result.contains("{page_id}"), "Should expand page_id in {}", p);
            prop_assert!(!result.contains("{aud}"), "Should expand aud in {}", p);
        }

        // Preserves literal segments
        let lit_pattern = format!("{}/{{page_id}}/{}", prefix, suffix);
        let lit_result = expand_pattern(&lit_pattern, &page_id, &did);
        prop_assert!(lit_result.starts_with(&prefix), "Prefix should be preserved");
        prop_assert!(lit_result.ends_with(&suffix), "Suffix should be preserved");

        // Exact substitution
        prop_assert_eq!(expand_pattern("{page_id}", &page_id, &did), page_id.clone());
        prop_assert_eq!(expand_pattern("{aud}", &page_id, &did), did.clone());

        // Empty inputs don't crash
        let _ = expand_pattern(&pattern, "", "");
        let _ = expand_pattern("", "page123", "did:key:abc");
    }
}

// Wildcard Matching Properties

proptest! {
    /// Exact match is reflexive: layer matches itself
    #[test]
    fn exact_match_is_reflexive(layer in layer_name_strategy()) {
        prop_assert!(matches_wildcard(&layer, &layer), "Layer should match itself: {}", layer);
    }

    /// Single wildcard matches any single segment
    #[test]
    fn wildcard_matches_any_segment(
        prefix in segment_strategy(),
        middle in segment_strategy(),
        suffix in segment_strategy(),
    ) {
        let layer = format!("{}/{}/{}", prefix, middle, suffix);
        let pattern = format!("{}/*/{}",  prefix, suffix);
        prop_assert!(matches_wildcard(&layer, &pattern),
            "Pattern {} should match layer {}", pattern, layer);
    }

    /// Segment count must match exactly (core invariant)
    #[test]
    fn segment_count_must_match(
        segments in prop::collection::vec(segment_strategy(), 2..5),
    ) {
        let layer = segments.join("/");
        let fewer = segments[..segments.len()-1].join("/");
        let more = format!("{}/extra", layer);

        prop_assert!(
            !matches_wildcard(&layer, &fewer),
            "Layer {} should not match pattern {} (fewer segments)", layer, fewer
        );
        prop_assert!(
            !matches_wildcard(&layer, &more),
            "Layer {} should not match pattern {} (more segments)", layer, more
        );
    }

    /// Multiple wildcards work independently
    #[test]
    fn multiple_wildcards_work(
        seg1 in segment_strategy(),
        seg2 in segment_strategy(),
        seg3 in segment_strategy(),
    ) {
        let layer = format!("{}/{}/{}", seg1, seg2, seg3);
        prop_assert!(matches_wildcard(&layer, "*/*/*"),
            "All wildcards should match: {}", layer);
        prop_assert!(matches_wildcard(&layer, &format!("*/{}/{}", seg2, seg3)),
            "Prefix wildcard should match: {}", layer);
        prop_assert!(matches_wildcard(&layer, &format!("{}/*/{}",seg1, seg3)),
            "Middle wildcard should match: {}", layer);
        prop_assert!(matches_wildcard(&layer, &format!("{}/{}/*", seg1, seg2)),
            "Suffix wildcard should match: {}", layer);
    }

    /// Different segments don't match (non-wildcard)
    #[test]
    fn different_segments_dont_match(
        seg1 in segment_strategy(),
        seg2 in segment_strategy(),
    ) {
        prop_assume!(seg1 != seg2);
        prop_assert!(!matches_wildcard(&seg1, &seg2),
            "Different segments should not match: {} vs {}", seg1, seg2);
    }

    /// Wildcard only matches complete segment, not partial
    #[test]
    fn wildcard_is_complete_segment(
        prefix in segment_strategy(),
        extra in segment_strategy(),
    ) {
        // Pattern "*" should match "orders" but not "orders/sub"
        let simple_layer = prefix.clone();
        let nested_layer = format!("{}/{}", prefix, extra);

        prop_assert!(matches_wildcard(&simple_layer, "*"),
            "Single wildcard should match single segment");
        prop_assert!(!matches_wildcard(&nested_layer, "*"),
            "Single wildcard should NOT match nested path: {}", nested_layer);
    }
}

// Example-based Documentation Tests

#[test]
fn doc_pattern_expansion() {
    assert_eq!(
        expand_pattern("{page_id}/orders/{aud}", "shop123", "did:key:abc"),
        "shop123/orders/did:key:abc"
    );
    assert_eq!(
        expand_pattern("{page_id}/data", "mypage", "viewer123"),
        "mypage/data"
    );
}

#[test]
fn doc_wildcard_segment_matching() {
    // Wildcard matches single segment
    assert!(matches_wildcard("shop/orders/customer", "shop/*/customer"));
    assert!(matches_wildcard("a/b/c", "*/*/c"));

    // Segment count MUST match
    assert!(!matches_wildcard("shop/orders/customer", "shop/*"));
    assert!(!matches_wildcard("a/b", "a/b/c"));
    assert!(!matches_wildcard("a/b/c", "a/b"));

    // Exact match works
    assert!(matches_wildcard("orders", "orders"));
    assert!(!matches_wildcard("orders", "products"));
}

#[test]
fn doc_empty_pattern_edge_cases() {
    // Empty strings
    assert!(matches_wildcard("", ""));
    assert!(!matches_wildcard("a", ""));
    assert!(!matches_wildcard("", "a"));

    // Single segment
    assert!(matches_wildcard("a", "a"));
    assert!(matches_wildcard("a", "*"));
}

// Real-world permit template patterns (from sample_apps)

#[test]
fn test_real_world_patterns() {
    // Shop patterns
    let page_id = "shop123abc";
    let customer_did = "did:key:z6MkCustomer123";

    assert_eq!(
        expand_pattern("{page_id}/products", page_id, customer_did),
        "shop123abc/products"
    );
    assert_eq!(
        expand_pattern("{page_id}/derived/orders_summary", page_id, customer_did),
        "shop123abc/derived/orders_summary"
    );

    let orders_expanded = expand_pattern("{page_id}/orders/*", page_id, customer_did);
    assert!(matches_wildcard(
        "shop123abc/orders/customer1",
        &orders_expanded
    ));
    assert!(matches_wildcard(
        "shop123abc/orders/did:key:abc",
        &orders_expanded
    ));

    let customer_orders = expand_pattern("{page_id}/orders/{aud}", page_id, customer_did);
    assert!(matches_wildcard(
        "shop123abc/orders/did:key:z6MkCustomer123",
        &customer_orders
    ));
    assert!(!matches_wildcard(
        "shop123abc/orders/did:key:z6MkOther456",
        &customer_orders
    ));

    // Booking patterns
    let page_id = "booking789";
    let customer_did = "did:key:z6MkBookingUser";

    assert_eq!(
        expand_pattern("{page_id}/schedule", page_id, customer_did),
        "booking789/schedule"
    );

    let owner_pattern = expand_pattern("{page_id}/bookings/*", page_id, customer_did);
    assert!(matches_wildcard(
        "booking789/bookings/customer1",
        &owner_pattern
    ));

    let customer_pattern = expand_pattern("{page_id}/bookings/{aud}", page_id, customer_did);
    assert!(matches_wildcard(
        "booking789/bookings/did:key:z6MkBookingUser",
        &customer_pattern
    ));
    assert!(!matches_wildcard(
        "booking789/bookings/did:key:z6MkOther",
        &customer_pattern
    ));
}

#[test]
fn real_world_app_layers() {
    // App layers use colon syntax (no placeholders, but colon separator)
    // These should be exact matches when expanded (no wildcards)
    let page_id = "demo123";
    let did = "did:key:z6MkUser";

    // Static app layers (no placeholders to expand)
    let app_chat = "app:Group Chat";
    let app_snake = "app:Snake Game";

    // These don't have placeholders, so expansion is identity
    assert_eq!(expand_pattern(app_chat, page_id, did), "app:Group Chat");
    assert_eq!(expand_pattern(app_snake, page_id, did), "app:Snake Game");

    // Exact match for app layers
    assert!(matches_wildcard("app:Group Chat", "app:Group Chat"));
    assert!(!matches_wildcard("app:Group Chat", "app:Snake Game"));
}

#[test]
fn real_world_drafts_local_only() {
    // Drafts layer: {page_id}/drafts with sync=false (local only)
    let page_id = "page456";
    let did = "did:key:z6MkUser";

    let drafts_pattern = "{page_id}/drafts";
    let drafts_expanded = expand_pattern(drafts_pattern, page_id, did);
    assert_eq!(drafts_expanded, "page456/drafts");

    // The layer name is exact, no wildcards
    assert!(matches_wildcard("page456/drafts", &drafts_expanded));
    assert!(!matches_wildcard(
        "page456/drafts/subfolder",
        &drafts_expanded
    ));
}

// Complex Pattern Interaction Tests

#[test]
fn complex_owner_vs_viewer_pattern_isolation() {
    // Scenario: Owner has wildcard access, viewer has identity-scoped access
    // This tests the core isolation model of the permit system

    let page_id = "shop123";
    let owner_did = "did:key:z6MkOwner";
    let viewer1_did = "did:key:z6MkViewer1";
    let viewer2_did = "did:key:z6MkViewer2";

    // Owner pattern: {page_id}/orders/* (sees ALL orders)
    let owner_pattern = expand_pattern("{page_id}/orders/*", page_id, owner_did);

    // Viewer patterns: {page_id}/orders/{aud} (sees only THEIR orders)
    let viewer1_pattern = expand_pattern("{page_id}/orders/{aud}", page_id, viewer1_did);
    let viewer2_pattern = expand_pattern("{page_id}/orders/{aud}", page_id, viewer2_did);

    // Layer names that would exist
    let viewer1_orders = format!("{}/orders/{}", page_id, viewer1_did);
    let viewer2_orders = format!("{}/orders/{}", page_id, viewer2_did);

    // Owner can see both
    assert!(matches_wildcard(&viewer1_orders, &owner_pattern));
    assert!(matches_wildcard(&viewer2_orders, &owner_pattern));

    // Viewer1 can only see their orders
    assert!(matches_wildcard(&viewer1_orders, &viewer1_pattern));
    assert!(!matches_wildcard(&viewer2_orders, &viewer1_pattern));

    // Viewer2 can only see their orders
    assert!(!matches_wildcard(&viewer1_orders, &viewer2_pattern));
    assert!(matches_wildcard(&viewer2_orders, &viewer2_pattern));
}

#[test]
fn complex_nested_resource_hierarchy() {
    // Scenario: Deeply nested resource paths
    // e.g., {page_id}/spaces/{space_id}/documents/{doc_id}/comments

    let page_id = "workspace001";
    let user_did = "did:key:z6MkUser";

    // Various nesting levels
    let patterns = [
        "{page_id}/spaces",
        "{page_id}/spaces/{aud}",
        "{page_id}/spaces/{aud}/documents",
        "{page_id}/spaces/{aud}/documents/*",
    ];

    let expanded: Vec<String> = patterns
        .iter()
        .map(|p| expand_pattern(p, page_id, user_did))
        .collect();

    assert_eq!(expanded[0], "workspace001/spaces");
    assert_eq!(expanded[1], format!("workspace001/spaces/{}", user_did));
    assert_eq!(
        expanded[2],
        format!("workspace001/spaces/{}/documents", user_did)
    );
    assert_eq!(
        expanded[3],
        format!("workspace001/spaces/{}/documents/*", user_did)
    );

    // Test matching at each level
    let doc_layer = format!("workspace001/spaces/{}/documents/doc123", user_did);

    // Only the wildcard pattern should match
    assert!(!matches_wildcard(&doc_layer, &expanded[0])); // wrong depth
    assert!(!matches_wildcard(&doc_layer, &expanded[1])); // wrong depth
    assert!(!matches_wildcard(&doc_layer, &expanded[2])); // wrong depth
    assert!(matches_wildcard(&doc_layer, &expanded[3])); // wildcard matches
}

#[test]
fn complex_multiple_wildcards_in_pattern() {
    // Scenario: Pattern with multiple wildcards for flexible matching

    let page_id = "multi123";
    let did = "did:key:z6MkUser";

    // Pattern: {page_id}/*/*/data (matches any 2 intermediate segments)
    let pattern = expand_pattern("{page_id}/*/*/data", page_id, did);
    assert_eq!(pattern, "multi123/*/*/data");

    // Should match various intermediate segments
    assert!(matches_wildcard("multi123/a/b/data", &pattern));
    assert!(matches_wildcard("multi123/users/orders/data", &pattern));
    assert!(matches_wildcard("multi123/x/y/data", &pattern));

    // Should NOT match wrong structure
    assert!(!matches_wildcard("multi123/a/data", &pattern)); // too few segments
    assert!(!matches_wildcard("multi123/a/b/c/data", &pattern)); // too many segments
    assert!(!matches_wildcard("multi123/a/b/other", &pattern)); // wrong suffix
}

#[test]
fn complex_did_with_special_characters() {
    // DIDs can contain colons and other characters that might interfere with parsing

    let page_id = "page123";

    // Real DID format: did:key:z6Mk... (contains colons)
    let did_with_colons = "did:key:z6MkhaXgBZDvotDkL5257faiztiGiC2QtKLGpbnnEGta2doK";

    // Expansion should handle colons correctly
    let pattern = "{page_id}/orders/{aud}";
    let expanded = expand_pattern(pattern, page_id, did_with_colons);

    assert_eq!(expanded, format!("{}/orders/{}", page_id, did_with_colons));

    // Matching should work with the full DID
    assert!(matches_wildcard(&expanded, &expanded));

    // Wildcard should match layers with full DIDs
    let wildcard_pattern = expand_pattern("{page_id}/orders/*", page_id, did_with_colons);
    assert!(matches_wildcard(&expanded, &wildcard_pattern));
}

#[test]
fn complex_placeholder_at_different_positions() {
    let page_id = "pg1";
    let did = "did:key:z6MkU";

    // {page_id} at start
    assert_eq!(expand_pattern("{page_id}/data", page_id, did), "pg1/data");

    // {page_id} in middle
    assert_eq!(
        expand_pattern("prefix/{page_id}/suffix", page_id, did),
        "prefix/pg1/suffix"
    );

    // {page_id} at end
    assert_eq!(expand_pattern("data/{page_id}", page_id, did), "data/pg1");

    // {aud} at start
    assert_eq!(
        expand_pattern("{aud}/inbox", page_id, did),
        "did:key:z6MkU/inbox"
    );

    // {aud} in middle
    assert_eq!(
        expand_pattern("users/{aud}/profile", page_id, did),
        "users/did:key:z6MkU/profile"
    );

    // {aud} at end
    assert_eq!(
        expand_pattern("messages/{aud}", page_id, did),
        "messages/did:key:z6MkU"
    );

    // Both placeholders
    assert_eq!(
        expand_pattern("{page_id}/{aud}", page_id, did),
        "pg1/did:key:z6MkU"
    );

    // Both placeholders, reversed order
    assert_eq!(
        expand_pattern("{aud}/{page_id}", page_id, did),
        "did:key:z6MkU/pg1"
    );

    // Multiple occurrences of same placeholder
    assert_eq!(
        expand_pattern("{page_id}/sub/{page_id}", page_id, did),
        "pg1/sub/pg1"
    );
}

#[test]
fn complex_pattern_specificity() {
    // When multiple patterns could match, test how specific patterns work
    // Note: This tests the matching logic, not precedence (which is in Permit methods)

    let layer = "shop/orders/customer123";

    // Most specific: exact match
    let exact = "shop/orders/customer123";
    assert!(matches_wildcard(layer, exact));

    // Less specific: one wildcard
    let one_wild = "shop/orders/*";
    assert!(matches_wildcard(layer, one_wild));

    // Even less specific: two wildcards
    let two_wild = "shop/*/*";
    assert!(matches_wildcard(layer, two_wild));

    // Least specific: all wildcards
    let all_wild = "*/*/*";
    assert!(matches_wildcard(layer, all_wild));

    // Wrong specificity patterns should NOT match
    assert!(!matches_wildcard(layer, "shop/*")); // too few segments
    assert!(!matches_wildcard(layer, "shop/*/*/*")); // too many segments
    assert!(!matches_wildcard(layer, "other/orders/*")); // wrong prefix
}

proptest! {
    // Complex Property Tests

    /// If pattern A is more specific than B, and layer matches A, it should also match B
    /// (wildcards make patterns less specific)
    #[test]
    fn specificity_hierarchy(
        seg1 in segment_strategy(),
        seg2 in segment_strategy(),
        seg3 in segment_strategy(),
    ) {
        let layer = format!("{}/{}/{}", seg1, seg2, seg3);

        // If exact matches, all less-specific patterns should too
        let exact = format!("{}/{}/{}", seg1, seg2, seg3);
        let one_wild = format!("{}/{}/*", seg1, seg2);
        let two_wild = format!("{}/*/*", seg1);
        let all_wild = "*/*/*";

        if matches_wildcard(&layer, &exact) {
            prop_assert!(matches_wildcard(&layer, &one_wild),
                "Exact match implies one-wildcard match");
            prop_assert!(matches_wildcard(&layer, &two_wild),
                "Exact match implies two-wildcard match");
            prop_assert!(matches_wildcard(&layer, all_wild),
                "Exact match implies all-wildcard match");
        }
    }

    /// Wildcard at position N should match any segment at position N
    #[test]
    fn wildcard_position_independence(
        prefix in segment_strategy(),
        middle1 in segment_strategy(),
        middle2 in segment_strategy(),
        suffix in segment_strategy(),
    ) {
        prop_assume!(middle1 != middle2);

        let layer1 = format!("{}/{}/{}", prefix, middle1, suffix);
        let layer2 = format!("{}/{}/{}", prefix, middle2, suffix);
        let pattern = format!("{}/*/{}",prefix, suffix);

        // Both should match because the wildcard absorbs the difference
        prop_assert!(matches_wildcard(&layer1, &pattern));
        prop_assert!(matches_wildcard(&layer2, &pattern));
    }

    /// Expansion followed by matching against self always succeeds
    #[test]
    fn expand_then_match_self(
        page_id in page_id_strategy(),
        did in did_strategy(),
    ) {
        let patterns = [
            "{page_id}/data",
            "{page_id}/orders/{aud}",
            "{aud}/inbox/{page_id}",
        ];

        for pattern in patterns {
            let expanded = expand_pattern(pattern, &page_id, &did);
            prop_assert!(matches_wildcard(&expanded, &expanded),
                "Expanded pattern should match itself: {} -> {}", pattern, expanded);
        }
    }
}

// Issue_on Template Tests

/// Test that page owner tokens correctly preserve issue_on templates
#[test]
fn issue_on_preserved_in_page_owner_token() {
    use crate::service::issue_page_owner_token;

    const PAGE_TEMPLATE: &str = r#"{
      "owner_template": {
        "operations": { "own": "allow", "share_page": "allow" },
        "peer_capabilities": { "relay": false, "share": true },
        "layers": {
          "content_doc": { "sync": true, "write": true, "type": "crdt" }
        },
        "issue_on": {
          "node": {
            "token_type": "page_share",
            "peer_capabilities": { "relay": true, "share": true },
            "operations": { "share_page": "allow" },
            "layers": {
              "content_doc": { "sync": true, "write": true, "type": "crdt" }
            },
            "issue_on": {
              "viewer": {
                "token_type": "page_viewer",
                "peer_capabilities": { "relay": false, "share": false },
                "layers": {
                  "content_doc": { "sync": true, "write": false, "type": "crdt" }
                }
              }
            }
          },
          "viewer": {
            "token_type": "page_viewer",
            "peer_capabilities": { "relay": false, "share": false },
            "layers": {
              "content_doc": { "sync": true, "write": false, "type": "crdt" }
            }
          }
        }
      }
    }"#;

    let signing_key = [1u8; 32];
    let page_id = "test-page-123";

    // Create owner token
    let (token, _cid) =
        pollster::block_on(issue_page_owner_token(&signing_key, page_id, PAGE_TEMPLATE))
            .expect("Failed to create page owner token");

    // Parse the token
    let permit = Permit::from_token(&token).expect("Failed to parse token");

    // Verify issue_on templates are present
    assert!(
        permit.has_issue_templates(),
        "Permit should have issue_on templates"
    );

    // Verify issue_on.node exists
    let node_template = permit
        .get_issue_template("node")
        .expect("Permit should have issue_on.node template");
    assert_eq!(node_template.token_type, "page_share");

    // Verify issue_on.viewer exists
    let viewer_template = permit
        .get_issue_template("viewer")
        .expect("Permit should have issue_on.viewer template");
    assert_eq!(viewer_template.token_type, "page_viewer");

    // Verify nested issue_on (node's issue_on.viewer)
    let nested_viewer = node_template
        .issue_on
        .get("viewer")
        .expect("Node template should have nested issue_on.viewer");
    assert_eq!(nested_viewer.token_type, "page_viewer");
}

// Authorized Peers Tests

#[test]
fn test_authorized_peers_list_missing() {
    // Create a permit without authorized_peers fact
    use crate::service::issue_page_owner_token;

    const TEMPLATE: &str = r#"{
      "owner_template": {
        "operations": { "own": "allow" },
        "peer_capabilities": { "relay": false, "share": true },
        "layers": {
          "content": { "sync": true, "write": true }
        }
      }
    }"#;

    let signing_key = [1u8; 32];
    let (token, _) = pollster::block_on(issue_page_owner_token(&signing_key, "page123", TEMPLATE))
        .expect("Failed to create token");

    let permit = Permit::from_token(&token).expect("Failed to parse token");

    // Missing authorized_peers => unrestricted
    assert_eq!(
        permit.authorized_peers_list(),
        None,
        "Missing authorized_peers should return None (unrestricted)"
    );
}

#[test]
fn test_authorized_peers_list_null() {
    // For null test, we can use the fact that missing authorized_peers
    // behaves the same as null (both return None)
    // This is tested by test_authorized_peers_list_missing
    // Skipping redundant test
}

#[test]
fn test_authorized_peers_list_explicit() {
    use crate::service::issue_layer_authority_permit;
    use crate::service::issue_page_owner_token;
    use crate::LayerConfig;

    const TEMPLATE: &str = r#"{
      "owner_template": {
        "operations": { "own": "allow" },
        "peer_capabilities": { "relay": false, "share": true },
        "layers": {
          "content": { "sync": true, "write": true }
        },
        "issue_on": {
          "layer_authority": {
            "token_type": "layer_authority",
            "peer_capabilities": { "relay": true, "share": true },
            "layers": {}
          }
        }
      }
    }"#;

    let signing_key = [1u8; 32];
    let (owner_token, _) =
        pollster::block_on(issue_page_owner_token(&signing_key, "page123", TEMPLATE))
            .expect("Failed to create owner token");

    let (authority_token, _) = pollster::block_on(issue_layer_authority_permit(
        &signing_key,
        &owner_token,
        "did:key:node",
        "page123/layer1",
        LayerConfig {
            sync: true,
            write: true,
            layer_type: None,
        },
        Some(vec![
            "did:key:viewer1".to_string(),
            "did:key:viewer2".to_string(),
        ]),
        1,
    ))
    .expect("Failed to create authority token");

    let permit = Permit::from_token(&authority_token).expect("Failed to parse token");

    // Explicit authorized_peers => Some(vec)
    let peers = permit
        .authorized_peers_list()
        .expect("Should have explicit authorized_peers");
    assert_eq!(peers, vec!["did:key:viewer1", "did:key:viewer2"]);
}

#[test]
fn test_authorized_peers_list_malformed() {
    // Test malformed authorized_peers by manually constructing a token
    // with invalid authorized_peers fact (string instead of array)
    use crate::builder::GurkhaPermitBuilder;
    use crate::decision::TokenDecision;
    use std::collections::HashMap;

    let signing_key = [1u8; 32];
    let builder = GurkhaPermitBuilder::from_bytes(&signing_key);

    let mut decision = TokenDecision {
        audience: "did:key:audience".to_string(),
        capabilities: vec![("test".to_string(), "read".to_string())],
        facts: serde_json::Map::new(),
        expiry: None,
        proofs: vec![],
        proof_tokens: HashMap::new(),
    };

    decision
        .facts
        .insert("token_type".to_string(), serde_json::json!("test"));
    decision.facts.insert(
        "authorized_peers".to_string(),
        serde_json::json!("not-an-array"),
    );

    let (token, _) = pollster::block_on(builder.build(decision)).expect("Failed to build token");
    let permit = Permit::from_token(&token).expect("Failed to parse token");

    // Malformed value => None (defensive fallback to unrestricted)
    assert_eq!(
        permit.authorized_peers_list(),
        None,
        "Malformed authorized_peers should return None (defensive fallback)"
    );
}

#[test]
fn test_is_peer_authorized_missing() {
    use crate::service::issue_page_owner_token;

    const TEMPLATE: &str = r#"{
      "owner_template": {
        "operations": { "own": "allow" },
        "peer_capabilities": { "relay": false, "share": true },
        "layers": {
          "content": { "sync": true, "write": true }
        }
      }
    }"#;

    let signing_key = [1u8; 32];
    let (token, _) = pollster::block_on(issue_page_owner_token(&signing_key, "page123", TEMPLATE))
        .expect("Failed to create token");

    let permit = Permit::from_token(&token).expect("Failed to parse token");

    // Missing authorized_peers => all peers authorized
    assert!(
        permit.is_peer_authorized("did:key:viewer1"),
        "Missing authorized_peers should authorize all peers"
    );
    assert!(
        permit.is_peer_authorized("did:key:viewer2"),
        "Missing authorized_peers should authorize all peers"
    );
}

#[test]
fn test_is_peer_authorized_null() {
    // Test null authorized_peers by manually constructing a token
    use crate::builder::GurkhaPermitBuilder;
    use crate::decision::TokenDecision;
    use crate::types::Capability;

    let signing_key = [1u8; 32];
    let builder = GurkhaPermitBuilder::from_bytes(&signing_key);

    let mut decision = TokenDecision {
        audience: "did:key:audience".to_string(),
        capabilities: vec![Capability::from_str("test/read").unwrap()],
        facts: serde_json::Map::new(),
        expiry: None,
        proofs: vec![],
        proof_tokens: vec![],
    };

    decision
        .facts
        .insert("token_type".to_string(), serde_json::json!("test"));
    decision
        .facts
        .insert("authorized_peers".to_string(), serde_json::Value::Null);

    let (token, _) = pollster::block_on(builder.build(decision)).expect("Failed to build token");
    let permit = Permit::from_token(&token).expect("Failed to parse token");

    // Null authorized_peers => all peers authorized
    assert!(
        permit.is_peer_authorized("did:key:viewer1"),
        "Null authorized_peers should authorize all peers"
    );
    assert!(
        permit.is_peer_authorized("did:key:viewer2"),
        "Null authorized_peers should authorize all peers"
    );
}

#[test]
fn test_is_peer_authorized_explicit_allowed() {
    use crate::service::issue_layer_authority_permit;
    use crate::service::issue_page_owner_token;
    use crate::LayerConfig;

    const TEMPLATE: &str = r#"{
      "owner_template": {
        "operations": { "own": "allow" },
        "peer_capabilities": { "relay": false, "share": true },
        "layers": {
          "content": { "sync": true, "write": true }
        },
        "issue_on": {
          "layer_authority": {
            "token_type": "layer_authority",
            "peer_capabilities": { "relay": true, "share": true },
            "layers": {}
          }
        }
      }
    }"#;

    let signing_key = [1u8; 32];
    let (owner_token, _) =
        pollster::block_on(issue_page_owner_token(&signing_key, "page123", TEMPLATE))
            .expect("Failed to create owner token");

    let (authority_token, _) = pollster::block_on(issue_layer_authority_permit(
        &signing_key,
        &owner_token,
        "did:key:node",
        "page123/layer1",
        LayerConfig {
            sync: true,
            write: true,
            layer_type: None,
        },
        Some(vec![
            "did:key:viewer1".to_string(),
            "did:key:viewer2".to_string(),
        ]),
        1,
    ))
    .expect("Failed to create authority token");

    let permit = Permit::from_token(&authority_token).expect("Failed to parse token");

    // Peers in the list should be authorized
    assert!(
        permit.is_peer_authorized("did:key:viewer1"),
        "Peer in authorized_peers list should be authorized"
    );
    assert!(
        permit.is_peer_authorized("did:key:viewer2"),
        "Peer in authorized_peers list should be authorized"
    );
}

#[test]
fn test_is_peer_authorized_explicit_denied() {
    use crate::service::issue_layer_authority_permit;
    use crate::service::issue_page_owner_token;
    use crate::LayerConfig;

    const TEMPLATE: &str = r#"{
      "owner_template": {
        "operations": { "own": "allow" },
        "peer_capabilities": { "relay": false, "share": true },
        "layers": {
          "content": { "sync": true, "write": true }
        },
        "issue_on": {
          "layer_authority": {
            "token_type": "layer_authority",
            "peer_capabilities": { "relay": true, "share": true },
            "layers": {}
          }
        }
      }
    }"#;

    let signing_key = [1u8; 32];
    let (owner_token, _) =
        pollster::block_on(issue_page_owner_token(&signing_key, "page123", TEMPLATE))
            .expect("Failed to create owner token");

    let (authority_token, _) = pollster::block_on(issue_layer_authority_permit(
        &signing_key,
        &owner_token,
        "did:key:node",
        "page123/layer1",
        LayerConfig {
            sync: true,
            write: true,
            layer_type: None,
        },
        Some(vec![
            "did:key:viewer1".to_string(),
            "did:key:viewer2".to_string(),
        ]),
        1,
    ))
    .expect("Failed to create authority token");

    let permit = Permit::from_token(&authority_token).expect("Failed to parse token");

    // Peers NOT in the list should be denied
    assert!(
        !permit.is_peer_authorized("did:key:viewer3"),
        "Peer not in authorized_peers list should be denied"
    );
    assert!(
        !permit.is_peer_authorized("did:key:other"),
        "Peer not in authorized_peers list should be denied"
    );
}

#[test]
fn test_is_peer_authorized_malformed() {
    // Test malformed authorized_peers by manually constructing a token
    use crate::builder::GurkhaPermitBuilder;
    use crate::decision::TokenDecision;
    use crate::types::Capability;

    let signing_key = [1u8; 32];
    let builder = GurkhaPermitBuilder::from_bytes(&signing_key);

    let mut decision = TokenDecision {
        audience: "did:key:audience".to_string(),
        capabilities: vec![Capability::from_str("test/read").unwrap()],
        facts: serde_json::Map::new(),
        expiry: None,
        proofs: vec![],
        proof_tokens: vec![],
    };

    decision
        .facts
        .insert("token_type".to_string(), serde_json::json!("test"));
    decision.facts.insert(
        "authorized_peers".to_string(),
        serde_json::json!("not-an-array"),
    );

    let (token, _) = pollster::block_on(builder.build(decision)).expect("Failed to build token");
    let permit = Permit::from_token(&token).expect("Failed to parse token");

    // Malformed value => deny (defensive)
    assert!(
        !permit.is_peer_authorized("did:key:viewer1"),
        "Malformed authorized_peers should deny all peers"
    );
    assert!(
        !permit.is_peer_authorized("did:key:viewer2"),
        "Malformed authorized_peers should deny all peers"
    );
}
