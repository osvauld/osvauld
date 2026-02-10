//! Integration tests for Permit methods using test templates
//!
//! Tests: template file -> signed token -> parsed Permit -> method calls
//!
//! These tests verify real permit behavior with actual signed tokens,
//! complementing the property-based tests in test_strategies.rs.

use crate::test_fixtures::*;

// SHOP: Owner / Customer / Admin isolation

#[test]
fn test_shop_permissions() {
    let owner = shop_owner("shop123", "did:key:owner");
    let customer = shop_customer("shop123", "did:key:customer1");
    let admin = shop_admin("shop123", "did:key:admin");

    // (permit, layer, page_id, did, can_write, can_read, description)
    let cases: Vec<(&crate::parser::Permit, &str, &str, &str, bool, bool, &str)> = vec![
        (&owner, "shop123/products", "shop123", "did:key:owner", true, true, "owner writes products"),
        (&owner, "shop123/orders/customer1", "shop123", "did:key:owner", true, true, "owner writes any orders"),
        (&owner, "shop123/orders/customer2", "shop123", "did:key:owner", true, true, "owner writes any orders 2"),
        (&customer, "shop123/products", "shop123", "did:key:customer1", false, true, "customer reads products"),
        (&customer, "shop123/orders/did:key:customer1", "shop123", "did:key:customer1", true, true, "customer writes own orders"),
        (&customer, "shop123/orders/did:key:customer2", "shop123", "did:key:customer1", false, false, "customer cannot write other orders"),
        (&admin, "shop123/products", "shop123", "did:key:admin", true, true, "admin writes products"),
        (&admin, "shop123/orders/customer1", "shop123", "did:key:admin", false, true, "admin reads orders no write"),
    ];

    for (permit, layer, page_id, did, expect_write, expect_read, desc) in cases {
        assert_eq!(permit.can_write_layer(layer, page_id, did), expect_write, "{}", desc);
        assert_eq!(permit.can_read_layer(layer, page_id, did), expect_read, "{}", desc);
    }

    // Sync-specific checks
    assert!(!owner.should_sync_layer("shop123/drafts", "shop123", "did:key:owner"), "owner drafts local only");

    // Customer isolation
    let c2 = shop_customer("myshop", "did:key:z6MkC2");
    let c1 = shop_customer("myshop", "did:key:z6MkC1");
    assert!(c1.can_write_layer("myshop/orders/did:key:z6MkC1", "myshop", "did:key:z6MkC1"));
    assert!(!c1.can_write_layer("myshop/orders/did:key:z6MkC2", "myshop", "did:key:z6MkC1"));
    assert!(c2.can_write_layer("myshop/orders/did:key:z6MkC2", "myshop", "did:key:z6MkC2"));
    assert!(!c2.can_write_layer("myshop/orders/did:key:z6MkC1", "myshop", "did:key:z6MkC2"));
}

// BOOKING: Provider / Customer

#[test]
fn test_booking_permissions() {
    let provider = booking_provider("biz123", "did:key:provider");
    let customer = booking_customer("biz123", "did:key:customer1");

    let cases: Vec<(&crate::parser::Permit, &str, &str, &str, bool, bool, &str)> = vec![
        (&provider, "biz123/schedule", "biz123", "did:key:provider", true, true, "provider writes schedule"),
        (&provider, "biz123/bookings/customer1", "biz123", "did:key:provider", true, true, "provider writes any booking"),
        (&customer, "biz123/schedule", "biz123", "did:key:customer1", false, true, "customer reads schedule"),
        (&customer, "biz123/bookings/did:key:customer1", "biz123", "did:key:customer1", true, true, "customer writes own booking"),
        (&customer, "biz123/bookings/did:key:customer2", "biz123", "did:key:customer1", false, false, "customer cannot write other booking"),
    ];

    for (permit, layer, page_id, did, expect_write, expect_read, desc) in cases {
        assert_eq!(permit.can_write_layer(layer, page_id, did), expect_write, "{}", desc);
        assert_eq!(permit.can_read_layer(layer, page_id, did), expect_read, "{}", desc);
    }

    // Sync-specific
    assert!(!customer.should_sync_layer("biz123/drafts", "biz123", "did:key:customer"), "customer drafts local only");

    // Derived calendar is read-only for both
    assert!(provider.can_read_layer("biz123/derived/calendar", "biz123", "did:key:provider"));
    assert!(customer.can_read_layer("biz123/derived/calendar", "biz123", "did:key:customer1"));
    assert!(!provider.can_write_layer("biz123/derived/calendar", "biz123", "did:key:provider"));
    assert!(!customer.can_write_layer("biz123/derived/calendar", "biz123", "did:key:customer1"));
}

// COLLABORATIVE: Equal write access

#[test]
fn test_collaborative_permissions() {
    let owner = collab_owner("doc1", "did:key:owner");
    let collab = collab_collaborator("doc1", "did:key:collab");

    // Both can write canvas and shapes
    for layer in &["doc1/canvas", "doc1/shapes"] {
        assert!(owner.can_write_layer(layer, "doc1", "did:key:owner"), "owner writes {}", layer);
        assert!(collab.can_write_layer(layer, "doc1", "did:key:collab"), "collab writes {}", layer);
    }

    // App layer: owner can write, collaborator can read only
    assert!(owner.can_write_layer("app:Canvas", "doc1", "did:key:owner"));
    assert!(!collab.can_write_layer("app:Canvas", "doc1", "did:key:collab"));
    assert!(collab.can_read_layer("app:Canvas", "doc1", "did:key:collab"));
}

// GAME: Host / Player with local state

#[test]
fn test_game_permissions() {
    let host = game_host("game1", "did:key:host");
    let player = game_player("game1", "did:key:player");

    // Config: host writes, player reads
    assert!(host.can_write_layer("game1/config", "game1", "did:key:host"));
    assert!(!player.can_write_layer("game1/config", "game1", "did:key:player"));
    assert!(player.can_read_layer("game1/config", "game1", "did:key:player"));

    // Scores: both can write, synced
    assert!(player.can_write_layer("game1/scores", "game1", "did:key:player"));
    assert!(host.should_sync_layer("game1/scores", "game1", "did:key:host"));

    // Game state: local only for both
    assert!(player.can_write_layer("game1/game_state", "game1", "did:key:player"));
    assert!(!host.should_sync_layer("game1/game_state", "game1", "did:key:host"));
    assert!(!player.should_sync_layer("game1/game_state", "game1", "did:key:player"));
}

// EDGE CASES

#[test]
fn test_permit_edge_cases() {
    // No layers permit
    let no_layers = edge_case("no_layers", "page1", "did:key:user");
    assert!(!no_layers.can_write_layer("page1/anything", "page1", "did:key:user"));
    assert!(!no_layers.can_read_layer("page1/anything", "page1", "did:key:user"));

    // Only patterns permit
    let only_patterns = edge_case("only_patterns", "page1", "did:key:user");
    assert!(only_patterns.can_read_layer("page1/anything", "page1", "did:key:user"));
    assert!(!only_patterns.can_write_layer("page1/anything", "page1", "did:key:user"));
    assert!(only_patterns.can_read_layer("page1/foo", "page1", "did:key:user"));
    assert!(!only_patterns.can_read_layer("page1/foo/bar", "page1", "did:key:user"));

    // Deep nesting
    let deep = edge_case("deep_nesting", "page1", "did:key:user");
    assert!(deep.can_write_layer("page1/a/b/c/d", "page1", "did:key:user"));
    assert!(deep.can_write_layer("page1/x/y/z", "page1", "did:key:user"));
    assert!(deep.can_write_layer("page1/x/y/anything", "page1", "did:key:user"));

    // Colon layers
    let colon = edge_case("colon_layers", "page1", "did:key:user");
    assert!(colon.can_write_layer("app:MyApp", "page1", "did:key:user"));

    // Write no sync
    let wns = edge_case("write_no_sync", "page1", "did:key:user");
    assert!(wns.can_write_layer("page1/local", "page1", "did:key:user"));
    assert!(!wns.should_sync_layer("page1/local", "page1", "did:key:user"));

    // Sync no write
    let snw = edge_case("sync_no_write", "page1", "did:key:user");
    assert!(!snw.can_write_layer("page1/readonly", "page1", "did:key:user"));
    assert!(snw.can_read_layer("page1/readonly", "page1", "did:key:user"));
    assert!(snw.should_sync_layer("page1/readonly", "page1", "did:key:user"));

    // Unknown layer defaults
    assert!(!snw.can_read_layer("page1/unknown", "page1", "did:key:user"));
    assert!(!snw.can_write_layer("page1/unknown", "page1", "did:key:user"));
    assert!(snw.should_sync_layer("page1/unknown", "page1", "did:key:user"));
}

// STATIC LAYER EXPANSION

#[test]
fn test_static_layers() {
    // Shop owner — static_layers returns bare names (no page_id/ prefix)
    let owner = shop_owner("shop123", "did:key:owner");
    let owner_layers = owner.static_layers("shop123", "did:key:owner");
    assert!(owner_layers.contains(&"products".to_string()));
    assert!(owner_layers.contains(&"drafts".to_string()));
    assert!(owner_layers.contains(&"app:Shop".to_string()));
    assert!(!owner_layers.iter().any(|l| l.contains("orders")));

    // Booking customer
    let customer = booking_customer("biz123", "did:key:customer");
    let customer_layers = customer.static_layers("biz123", "did:key:customer");
    assert!(customer_layers.contains(&"schedule".to_string()));
    assert!(customer_layers.contains(&"blocked".to_string()));
    assert!(customer_layers.contains(&"drafts".to_string()));
    assert!(customer_layers.contains(&"derived/calendar".to_string()));

    // Shop customer: {aud} expands to static (bare name)
    let shop_cust = shop_customer("shop123", "did:key:customer1");
    let shop_cust_layers = shop_cust.static_layers("shop123", "did:key:customer1");
    assert!(shop_cust_layers.contains(&"orders/did:key:customer1".to_string()));

    // Collaborative
    let collab = collab_collaborator("doc1", "did:key:collab");
    let collab_layers = collab.static_layers("doc1", "did:key:collab");
    assert!(collab_layers.contains(&"canvas".to_string()));
    assert!(collab_layers.contains(&"shapes".to_string()));
    assert!(collab_layers.contains(&"app:Canvas".to_string()));
}

// TOKEN METADATA

#[test]
fn test_token_metadata() {
    let owner = shop_owner("my-unique-page", "did:key:z6MkOwner");
    assert_eq!(owner.token_type(), Some("page_owner"));
    assert_eq!(owner.page_id(), Some("my-unique-page".to_string()));
    assert_eq!(owner.audience(), Some("did:key:z6MkOwner"));

    let customer = shop_customer("shop123", "did:key:customer");
    assert_eq!(customer.token_type(), Some("page_viewer"));
}

// CHAT: Presence and Ephemeral Tests

#[test]
fn test_chat_permissions() {
    let owner = chat_owner("chat123", "did:key:owner");
    let participant = chat_participant("chat123", "did:key:p1");
    let lurker = chat_lurker("chat123", "did:key:lurk");

    // Owner: visible, display name, all ephemeral
    assert!(owner.is_visible());
    assert_eq!(owner.display_name(), Some("Owner"));
    assert!(owner.can_send_ephemeral("typing"));
    assert!(owner.can_send_ephemeral("cursor"));
    assert!(owner.can_send_ephemeral("presence"));

    // Participant: visible, display name, limited ephemeral
    assert!(participant.is_visible());
    assert_eq!(participant.display_name(), Some("Participant"));
    assert!(participant.can_send_ephemeral("typing"));
    assert!(!participant.can_send_ephemeral("presence"));

    // Lurker: not visible, no display name
    assert!(!lurker.is_visible());
    assert_eq!(lurker.display_name(), None);
}

// DELEGATION CHAIN: Owner -> Node -> Viewer (presence propagation)

mod delegation_chain {
    use crate::parser::Permit;
    use crate::service::{issue_page_owner_token, delegate_page};

    const TEST_KEY: [u8; 32] = [1u8; 32];

    fn load_production_template() -> String {
        std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .unwrap()
                .join("sample_apps/osvauld-demos/permit_template.json"),
        )
        .expect("Failed to load permit template")
    }

    #[test]
    fn test_delegation_chain_full_flow() {
        let template = load_production_template();

        // 1. Owner permit has presence
        let (owner_token, _) = pollster::block_on(
            issue_page_owner_token(&TEST_KEY, "page123", &template),
        ).unwrap();
        let owner_permit = Permit::from_token(&owner_token).unwrap();
        assert!(owner_permit.is_visible(), "Owner should be visible");
        assert!(owner_permit.can_send_ephemeral("typing"), "Owner should send typing");

        // 2. Owner delegates to node — node carries issue_on.viewer
        let (node_token, _) = pollster::block_on(
            delegate_page(&TEST_KEY, &owner_token, "node", "did:key:node123"),
        ).unwrap();
        let node_permit = Permit::from_token(&node_token).unwrap();
        assert!(
            node_permit.get_issue_template("viewer").is_some(),
            "Node permit must carry issue_on.viewer"
        );
        assert!(!node_permit.is_visible(), "Node should NOT be visible");

        // 3. Node delegates to viewer
        let (viewer_token, _) = pollster::block_on(
            delegate_page(&TEST_KEY, &node_token, "viewer", "did:key:viewer456"),
        ).unwrap();
        let viewer_permit = Permit::from_token(&viewer_token).unwrap();

        // 4. Viewer has presence after full chain
        assert_eq!(viewer_permit.token_type(), Some("page_viewer"));
        assert!(viewer_permit.is_visible(), "Viewer should be visible");
        assert!(viewer_permit.can_send_ephemeral("typing"), "Viewer should send typing");
    }

    #[test]
    fn test_delegation_preserves_templates() {
        let template = load_production_template();
        let (owner_token, _) = pollster::block_on(
            issue_page_owner_token(&TEST_KEY, "page123", &template),
        ).unwrap();

        let (node_token, _) = pollster::block_on(
            delegate_page(&TEST_KEY, &owner_token, "node", "did:key:node123"),
        ).unwrap();
        let node_permit = Permit::from_token(&node_token).unwrap();

        // Node should carry issue_on.viewer for further delegation
        assert!(
            node_permit.get_issue_template("viewer").is_some(),
            "Node permit must carry issue_on.viewer for further delegation"
        );
    }
}
