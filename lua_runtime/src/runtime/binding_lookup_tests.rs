//! Unit tests for layer name prefix lookup fix in binding system.
//!
//! **Context**: Bindings are registered with expanded pattern including `page_id/` prefix.
//! Observer/LuaCommand LayerChanged delivers bare layer names. These tests verify that
//! the centralized helper correctly normalizes layer names for binding lookup.

use serde_json::json;
use tokio::sync::mpsc;

use crate::{LuaRuntime, LuaRuntimeConfig, MockScribeHandle};

/// Helper to create a test runtime with UI enabled and a binding registered.
fn create_test_runtime_with_binding(
    page_id: &str,
    lua_code: &str,
) -> (
    LuaRuntime,
    mpsc::Sender<crate::commands::LuaCommand>,
    mpsc::Receiver<crate::ui_types::UiMutation>,
) {
    let scribe = MockScribeHandle::new();
    let (ui_tx, ui_rx) = mpsc::channel(10);
    let (query_tx, _query_rx) = mpsc::channel(10);

    let config = LuaRuntimeConfig {
        page_id: page_id.to_string(),
        app_name: "test-app".to_string(),
        scribe,
        user_did: "did:key:test".to_string(),
        user_name: "Test User".to_string(),
        user_role: "owner".to_string(),
        lua_code: lua_code.to_string(),
        ui_enabled: true,
        ui_tx: Some(ui_tx),
        query_tx: Some(query_tx),
        navigate_tx: None,
    };

    let (mut runtime, cmd_tx) = LuaRuntime::new_headless(config).unwrap();
    runtime.call_on_init();

    (runtime, cmd_tx, ui_rx)
}

#[test]
fn test_normalize_layer_name_for_lookup_bare_name() {
    let scribe = MockScribeHandle::new();
    let config = LuaRuntimeConfig {
        page_id: "test-page-123".to_string(),
        app_name: "test-app".to_string(),
        scribe,
        user_did: "did:key:test".to_string(),
        user_name: "Test User".to_string(),
        user_role: "owner".to_string(),
        lua_code: "".to_string(),
        ui_enabled: false,
        ui_tx: None,
        query_tx: None,
        navigate_tx: None,
    };

    let (runtime, _) = LuaRuntime::new_headless(config).unwrap();

    // Bare layer name should get prefixed
    let normalized = runtime.normalize_layer_name_for_lookup("products");
    assert_eq!(normalized, "test-page-123/products");

    // Already prefixed should remain unchanged
    let normalized = runtime.normalize_layer_name_for_lookup("test-page-123/products");
    assert_eq!(normalized, "test-page-123/products");

    // Nested path should get prefixed
    let normalized = runtime.normalize_layer_name_for_lookup("orders/alice");
    assert_eq!(normalized, "test-page-123/orders/alice");

    // Already prefixed nested path should remain unchanged
    let normalized = runtime.normalize_layer_name_for_lookup("test-page-123/orders/alice");
    assert_eq!(normalized, "test-page-123/orders/alice");
}

#[test]
fn test_process_bindings_with_bare_layer_name() {
    let lua_code = r#"
        function on_init()
            scribe:bind("products_ui", "products")
        end
    "#;

    let (runtime, _cmd_tx, mut ui_rx) = create_test_runtime_with_binding("page-abc", lua_code);

    // Verify binding was registered with prefixed pattern
    {
        let manager = runtime.binding_manager.lock();
        let binding = manager.get_binding("products_ui").unwrap();
        assert_eq!(binding.expanded_pattern, "page-abc/products");
    }

    // Create layer and add data
    runtime.scribe.ensure_list("products").unwrap();
    runtime
        .scribe
        .list_push("products", "", json!({"id": "p1", "name": "Product 1"}))
        .unwrap();
    runtime
        .scribe
        .list_push("products", "", json!({"id": "p2", "name": "Product 2"}))
        .unwrap();

    // Get layer data
    let layer_data = runtime.scribe.get_layer_json("products").unwrap().unwrap();

    // Process bindings with BARE layer name (as delivered by LayerChanged)
    let processed = runtime.process_bindings("products", Some(&layer_data), None);

    // Should find the binding and process it
    assert!(
        processed,
        "Binding should be found and processed with bare layer name"
    );

    // Should have emitted a UI mutation
    let mutation = ui_rx.try_recv();
    assert!(
        mutation.is_ok(),
        "Should emit UI mutation when binding is processed"
    );
}

#[test]
fn test_handle_layer_discovered_with_bare_layer_name() {
    let lua_code = r#"
        function on_init()
            scribe:bind("orders_ui", "orders")
        end
        
        function on_layer_discovered(layer_name)
            -- Just a marker that this was called
        end
    "#;

    let (runtime, _cmd_tx, mut ui_rx) = create_test_runtime_with_binding("page-xyz", lua_code);

    while ui_rx.try_recv().is_ok() {}

    // Verify binding was registered with prefixed pattern
    {
        let manager = runtime.binding_manager.lock();
        let binding = manager.get_binding("orders_ui").unwrap();
        assert_eq!(binding.expanded_pattern, "page-xyz/orders");
    }

    // Create layer and add data
    runtime.scribe.ensure_list("orders").unwrap();
    runtime
        .scribe
        .list_push("orders", "", json!({"id": "o1", "status": "pending"}))
        .unwrap();
    runtime
        .scribe
        .list_push("orders", "", json!({"id": "o2", "status": "completed"}))
        .unwrap();

    // Handle layer discovered with BARE layer name (as delivered by LayerChanged)
    let result = runtime.handle_layer_discovered("orders");

    // Should succeed and trigger binding sync
    assert!(
        result.is_ok(),
        "Layer discovered should succeed with bare layer name"
    );

    assert!(
        ui_rx.try_recv().is_ok(),
        "Layer discovered should emit UI mutation via binding sync"
    );
}

#[test]
fn test_wildcard_binding_with_bare_layer_name() {
    let lua_code = r#"
        function on_init()
            scribe:bind("all_dms", "dm/*", {
                transform = function(layer_name, item)
                    return {
                        id = item.id,
                        text = item.text,
                        from = layer_name
                    }
                end
            })
        end
    "#;

    let (runtime, _cmd_tx, mut ui_rx) = create_test_runtime_with_binding("page-dm-test", lua_code);

    // Verify wildcard binding was registered with prefixed pattern
    {
        let manager = runtime.binding_manager.lock();
        let binding = manager.get_binding("all_dms").unwrap();
        assert_eq!(binding.expanded_pattern, "page-dm-test/dm/*");
        assert!(binding.is_wildcard);
    }

    // Create layer and add data
    runtime.scribe.ensure_list("dm/alice").unwrap();
    runtime
        .scribe
        .list_push("dm/alice", "", json!({"id": "m1", "text": "Hello"}))
        .unwrap();
    runtime
        .scribe
        .list_push("dm/alice", "", json!({"id": "m2", "text": "World"}))
        .unwrap();

    // Get layer data
    let layer_data = runtime.scribe.get_layer_json("dm/alice").unwrap().unwrap();

    // Process bindings with BARE layer name (as delivered by LayerChanged)
    let processed = runtime.process_bindings("dm/alice", Some(&layer_data), None);

    // Should find the wildcard binding and process it
    assert!(
        processed,
        "Wildcard binding should be found and processed with bare layer name"
    );

    // Should have emitted a UI mutation
    let mutation = ui_rx.try_recv();
    assert!(
        mutation.is_ok(),
        "Should emit UI mutation when wildcard binding is processed"
    );
}

#[test]
fn test_dynamic_path_binding_with_bare_layer_name() {
    let lua_code = r#"
        function on_init()
            scribe:bind("user_orders", "orders/{me}")
        end
    "#;

    let (runtime, _cmd_tx, mut ui_rx) = create_test_runtime_with_binding("page-user", lua_code);

    // Verify binding was registered with expanded pattern (with {me} replaced)
    {
        let manager = runtime.binding_manager.lock();
        let binding = manager.get_binding("user_orders").unwrap();
        // {me} should be expanded to user_did
        assert_eq!(binding.expanded_pattern, "page-user/orders/did:key:test");
    }

    // Create layer and add data
    runtime.scribe.ensure_list("orders/did:key:test").unwrap();
    runtime
        .scribe
        .list_push(
            "orders/did:key:test",
            "",
            json!({"id": "o1", "product": "Widget"}),
        )
        .unwrap();
    runtime
        .scribe
        .list_push(
            "orders/did:key:test",
            "",
            json!({"id": "o2", "product": "Gadget"}),
        )
        .unwrap();

    // Get layer data
    let layer_data = runtime
        .scribe
        .get_layer_json("orders/did:key:test")
        .unwrap()
        .unwrap();

    // Process bindings with BARE layer name (as delivered by LayerChanged)
    let processed = runtime.process_bindings("orders/did:key:test", Some(&layer_data), None);

    // Should find the binding and process it
    assert!(
        processed,
        "Dynamic path binding should be found and processed with bare layer name"
    );

    // Should have emitted a UI mutation
    let mutation = ui_rx.try_recv();
    assert!(
        mutation.is_ok(),
        "Should emit UI mutation when dynamic path binding is processed"
    );
}

#[test]
fn test_no_binding_registered_returns_false() {
    let lua_code = r#"
        function on_init()
            -- No bindings registered
        end
    "#;

    let (runtime, _cmd_tx, _ui_rx) = create_test_runtime_with_binding("page-empty", lua_code);

    let layer_data = json!([{"id": "1"}]);

    // Process bindings for a layer with no binding
    let processed = runtime.process_bindings("nonexistent", Some(&layer_data), None);

    // Should return false when no binding found
    assert!(
        !processed,
        "Should return false when no binding is registered"
    );
}

#[test]
fn test_multiple_bindings_same_layer() {
    let lua_code = r#"
        function on_init()
            scribe:bind("products_list", "products")
            scribe:bind("products_count", "products", {
                transform = function(item) return {count = 1} end
            })
        end
    "#;

    let (runtime, _cmd_tx, mut ui_rx) = create_test_runtime_with_binding("page-multi", lua_code);

    // Verify both bindings were registered
    {
        let manager = runtime.binding_manager.lock();
        assert!(manager.get_binding("products_list").is_some());
        assert!(manager.get_binding("products_count").is_some());
    }

    // Create layer and add data
    runtime.scribe.ensure_list("products").unwrap();
    runtime
        .scribe
        .list_push("products", "", json!({"id": "p1", "name": "Product 1"}))
        .unwrap();

    // Get layer data
    let layer_data = runtime.scribe.get_layer_json("products").unwrap().unwrap();

    // Process bindings with bare layer name
    let processed = runtime.process_bindings("products", Some(&layer_data), None);

    // Should find and process both bindings
    assert!(processed, "Should process multiple bindings for same layer");

    // Should have emitted UI mutations (one for each binding)
    assert!(ui_rx.try_recv().is_ok(), "Should emit first UI mutation");
    assert!(ui_rx.try_recv().is_ok(), "Should emit second UI mutation");
}

#[test]
fn test_rebind_stops_updates_from_previous_layer() {
    let lua_code = r#"
        function on_init()
            scribe:bind("messages", "channels/general/messages", { key = "id" })
        end

        function switch_to_random()
            scribe:rebind("messages", "channels/random/messages")
        end
    "#;

    let (runtime, _cmd_tx, mut ui_rx) = create_test_runtime_with_binding("page-chat", lua_code);

    runtime
        .scribe
        .ensure_map("channels/general/messages")
        .unwrap();
    runtime
        .scribe
        .map_insert(
            "channels/general/messages",
            "",
            "g1",
            json!({"id": "g1", "text": "general one"}),
        )
        .unwrap();

    runtime
        .scribe
        .ensure_map("channels/random/messages")
        .unwrap();
    runtime
        .scribe
        .map_insert(
            "channels/random/messages",
            "",
            "r1",
            json!({"id": "r1", "text": "random one"}),
        )
        .unwrap();

    while ui_rx.try_recv().is_ok() {}

    runtime.call_handler("switch_to_random", ()).unwrap();

    while ui_rx.try_recv().is_ok() {}

    let general_data = runtime
        .scribe
        .get_layer_json("channels/general/messages")
        .unwrap()
        .unwrap();

    let processed =
        runtime.process_bindings("channels/general/messages", Some(&general_data), None);

    assert!(
        !processed,
        "Updates for previously bound layer must not match after rebind"
    );
    assert!(
        ui_rx.try_recv().is_err(),
        "No UI mutation should be emitted for stale layer after rebind"
    );
}
