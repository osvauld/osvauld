//! Integration tests for AppTestRunner

use app_test::AppTestRunner;

#[test]
fn test_basic_lua_execution() {
    let mut runner = AppTestRunner::from_code(
        r#"
        function on_init()
            local messages = scribe:list(scribe:page_id() .. "/messages")
            messages:push({ text = "hello from init", sender = "test" })
        end
        "#,
        "basic-test",
        "viewer",
        "did:key:test_user",
        "TestUser",
    )
    .unwrap();

    runner.call_on_init();

    // Verify the message was pushed to the mock scribe
    let page_id = runner.page_id().to_string();
    let layer_name = format!("{}/messages", page_id);
    let data = runner.layer_data(&layer_name);
    assert!(data.is_some(), "Messages layer should exist after on_init");

    let messages = data.unwrap();
    let arr = messages.as_array().unwrap();
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["text"], "hello from init");
    assert_eq!(arr[0]["sender"], "test");
}

#[test]
fn test_callback_execution() {
    let mut runner = AppTestRunner::from_code(
        r#"
        local messages = nil

        function on_init()
            messages = scribe:list(scribe:page_id() .. "/messages")
        end

        function send_message(text)
            if not text or text == "" then return end
            messages:push({ text = text, sender = scribe:my_did() })
        end
        "#,
        "callback-test",
        "viewer",
        "did:key:alice",
        "Alice",
    )
    .unwrap();

    runner.call_on_init();

    // Fire callback
    runner
        .fire_callback(
            "send_message",
            vec![serde_json::json!("hello from callback")],
        )
        .unwrap();
    runner.tick();

    // Check result
    let page_id = runner.page_id().to_string();
    let layer_name = format!("{}/messages", page_id);
    let data = runner.layer_data(&layer_name).unwrap();
    let arr = data.as_array().unwrap();
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["text"], "hello from callback");
    assert_eq!(arr[0]["sender"], "did:key:alice");
}

#[test]
fn test_empty_message_rejected() {
    let mut runner = AppTestRunner::from_code(
        r#"
        local messages = nil

        function on_init()
            messages = scribe:list(scribe:page_id() .. "/messages")
        end

        function send_message(text)
            if not text or text == "" then return end
            messages:push({ text = text })
        end
        "#,
        "reject-test",
        "viewer",
        "did:key:test",
        "Test",
    )
    .unwrap();

    runner.call_on_init();

    // Try empty message
    runner
        .fire_callback("send_message", vec![serde_json::json!("")])
        .unwrap();
    runner.tick();

    // Should be empty
    let page_id = runner.page_id().to_string();
    let layer_name = format!("{}/messages", page_id);
    let data = runner.layer_data(&layer_name).unwrap();
    assert_eq!(data.as_array().unwrap().len(), 0);

    // Try nil message
    runner
        .fire_callback("send_message", vec![serde_json::Value::Null])
        .unwrap();
    runner.tick();
    let data = runner.layer_data(&layer_name).unwrap();
    assert_eq!(data.as_array().unwrap().len(), 0);
}

#[test]
fn test_map_operations() {
    let mut runner = AppTestRunner::from_code(
        r#"
        local config = nil

        function on_init()
            config = scribe:map(scribe:page_id() .. "/config")
            config:set("theme", "dark")
            config:set("language", "en")
        end

        function update_theme(theme)
            config:set("theme", theme)
        end
        "#,
        "map-test",
        "owner",
        "did:key:owner",
        "Owner",
    )
    .unwrap();

    runner.call_on_init();

    let page_id = runner.page_id().to_string();
    let layer_name = format!("{}/config", page_id);
    let data = runner.layer_data(&layer_name).unwrap();
    assert_eq!(data["theme"], "dark");
    assert_eq!(data["language"], "en");

    // Update theme
    runner
        .fire_callback("update_theme", vec![serde_json::json!("light")])
        .unwrap();
    runner.tick();

    let data = runner.layer_data(&layer_name).unwrap();
    assert_eq!(data["theme"], "light");
}

#[test]
fn test_ephemeral_sending() {
    let mut runner = AppTestRunner::from_code(
        r#"
        function on_init()
        end

        function send_typing()
            scribe:send("typing", { is_typing = true })
        end
        "#,
        "ephemeral-test",
        "viewer",
        "did:key:test",
        "Test",
    )
    .unwrap();

    runner.call_on_init();
    assert_eq!(runner.sent_ephemerals().len(), 0);

    runner
        .fire_callback("send_typing", vec![])
        .unwrap();
    runner.tick();

    let ephemerals = runner.sent_ephemerals();
    assert_eq!(ephemerals.len(), 1);

    let payload: serde_json::Value = serde_json::from_slice(&ephemerals[0]).unwrap();
    assert_eq!(payload["func"], "typing");
    assert_eq!(payload["args"]["is_typing"], true);
}

#[test]
fn test_eval_sync() {
    let mut runner = AppTestRunner::from_code(
        r#"
        function on_init()
        end
        x = 42
        "#,
        "eval-test",
        "viewer",
        "did:key:test",
        "Test",
    )
    .unwrap();

    runner.call_on_init();

    let result = runner.eval_sync("return x").unwrap();
    assert_eq!(result, serde_json::json!(42));

    let result = runner.eval_sync("return 'hello'").unwrap();
    assert_eq!(result, serde_json::json!("hello"));

    let result = runner.eval_sync("return {a=1, b='two'}").unwrap();
    assert_eq!(result["a"], 1);
    assert_eq!(result["b"], "two");
}

#[test]
fn test_identity_bindings() {
    let mut runner = AppTestRunner::from_code(
        r#"
        function on_init()
            local my_did = scribe:my_did()
            local my_name = scribe:my_name()
            local page_id = scribe:page_id()

            local info = scribe:map(page_id .. "/info")
            info:set("did", my_did)
            info:set("name", my_name)
            info:set("page", page_id)
        end
        "#,
        "identity-test",
        "viewer",
        "did:key:alice123",
        "Alice",
    )
    .unwrap();

    runner.call_on_init();

    let page_id = runner.page_id().to_string();
    let layer_name = format!("{}/info", page_id);
    let data = runner.layer_data(&layer_name).unwrap();
    assert_eq!(data["did"], "did:key:alice123");
    assert_eq!(data["name"], "Alice");
    assert_eq!(data["page"], page_id);
}

#[test]
fn test_multi_peer_shared_state() {
    use app_test::{MultiPeerRunner, PeerRole};

    let mut multi = MultiPeerRunner::from_code(
        r#"
        local messages = nil

        function on_init()
            messages = scribe:list(scribe:page_id() .. "/messages")
        end

        function send_message(text)
            messages:push({ text = text, sender = scribe:my_did() })
        end
        "#,
        "multi-peer",
        "test-page",
        &[
            PeerRole::new("did:key:alice", "Alice", "owner"),
            PeerRole::new("did:key:bob", "Bob", "viewer"),
        ],
    )
    .unwrap();

    multi.init_all();

    // Alice sends a message
    multi
        .peer(0)
        .fire_callback("send_message", vec![serde_json::json!("hello from alice")])
        .unwrap();
    multi.tick_all();

    // Bob can see Alice's message (shared state)
    let data = multi.peer(1).layer_data("test-page/messages").unwrap();
    let arr = data.as_array().unwrap();
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["text"], "hello from alice");
    assert_eq!(arr[0]["sender"], "did:key:alice");

    // Bob sends a message
    multi
        .peer(1)
        .fire_callback("send_message", vec![serde_json::json!("hello from bob")])
        .unwrap();
    multi.tick_all();

    // Both see 2 messages
    let data = multi.peer(0).layer_data("test-page/messages").unwrap();
    assert_eq!(data.as_array().unwrap().len(), 2);
}
