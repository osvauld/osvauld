//! Tests for the group-chat sample app
//!
//! Exercises send_message, empty rejection, multi-peer chat,
//! and UI state management against the real app code.

use std::path::PathBuf;

use app_test::{AppTestRunner, MultiPeerRunner, PeerRole};

fn chat_app_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../sample_apps/osvauld-demos/group-chat")
}

fn chat_lua_code() -> String {
    let app_dir = chat_app_dir();
    std::fs::read_to_string(app_dir.join("app.lua")).unwrap()
}

// -- Single-peer tests --

#[test]
fn test_chat_send_message() {
    let mut runner = AppTestRunner::load(
        &chat_app_dir(),
        "viewer",
        "did:key:alice",
        "Alice",
    )
    .unwrap();

    runner.call_on_init();

    // send_message is exported via api.export, becomes a global
    runner
        .fire_callback("send_message", vec![serde_json::json!("Hello world!")])
        .unwrap();
    runner.tick();

    // Check the message landed in the scribe layer
    let page_id = runner.page_id().to_string();
    let layer = format!("{}/messages", page_id);
    let data = runner.layer_data(&layer).expect("messages layer should exist");
    let messages = data.as_array().unwrap();

    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0]["text"], "Hello world!");
    assert_eq!(messages[0]["sender_did"], "did:key:alice");
    assert_eq!(messages[0]["sender_name"], "Alice");
    assert_eq!(messages[0]["deleted"], false);

    // UI state should be cleared after send
    let draft = runner.eval_sync("return ui:get('draft_text')").unwrap();
    assert!(draft == serde_json::json!("") || draft.is_null());
}

#[test]
fn test_chat_empty_message_rejected() {
    let mut runner = AppTestRunner::load(
        &chat_app_dir(),
        "viewer",
        "did:key:bob",
        "Bob",
    )
    .unwrap();

    runner.call_on_init();

    // Empty string
    runner
        .fire_callback("send_message", vec![serde_json::json!("")])
        .unwrap();
    runner.tick();

    let page_id = runner.page_id().to_string();
    let layer = format!("{}/messages", page_id);
    let data = runner.layer_data(&layer).expect("messages layer should exist");
    assert_eq!(data.as_array().unwrap().len(), 0, "Empty message should be rejected");

    // Nil
    runner
        .fire_callback("send_message", vec![serde_json::Value::Null])
        .unwrap();
    runner.tick();

    let data = runner.layer_data(&layer).unwrap();
    assert_eq!(data.as_array().unwrap().len(), 0, "Nil message should be rejected");
}

#[test]
fn test_chat_multiple_messages() {
    let mut runner = AppTestRunner::load(
        &chat_app_dir(),
        "viewer",
        "did:key:alice",
        "Alice",
    )
    .unwrap();

    runner.call_on_init();

    for i in 1..=5 {
        runner
            .fire_callback("send_message", vec![serde_json::json!(format!("Message {}", i))])
            .unwrap();
        runner.tick();
    }

    let page_id = runner.page_id().to_string();
    let layer = format!("{}/messages", page_id);
    let data = runner.layer_data(&layer).unwrap();
    let messages = data.as_array().unwrap();

    assert_eq!(messages.len(), 5);
    assert_eq!(messages[0]["text"], "Message 1");
    assert_eq!(messages[4]["text"], "Message 5");
}

#[test]
fn test_chat_on_init_ui_state() {
    let mut runner = AppTestRunner::load(
        &chat_app_dir(),
        "viewer",
        "did:key:alice",
        "Alice",
    )
    .unwrap();

    runner.call_on_init();

    // on_init sets these UI properties
    let show_mentions = runner.eval_sync("return ui:get('show_mentions')").unwrap();
    assert_eq!(show_mentions, serde_json::json!(false));

    let auto_scroll = runner.eval_sync("return ui:get('auto_scroll')").unwrap();
    assert_eq!(auto_scroll, serde_json::json!(true));

    let show_online = runner.eval_sync("return ui:get('show_online_panel')").unwrap();
    assert_eq!(show_online, serde_json::json!(false));
}

#[test]
fn test_chat_on_submit() {
    let mut runner = AppTestRunner::load(
        &chat_app_dir(),
        "viewer",
        "did:key:alice",
        "Alice",
    )
    .unwrap();

    runner.call_on_init();

    // Set draft text, then call on_submit (simulates Enter key)
    runner.eval_sync("ui:set('draft_text', 'typed message')").unwrap();
    runner
        .fire_callback("on_submit", vec![])
        .unwrap();
    runner.tick();

    let page_id = runner.page_id().to_string();
    let layer = format!("{}/messages", page_id);
    let data = runner.layer_data(&layer).unwrap();
    let messages = data.as_array().unwrap();

    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0]["text"], "typed message");

    // Draft should be cleared
    let draft = runner.eval_sync("return ui:get('draft_text')").unwrap();
    assert!(draft == serde_json::json!("") || draft.is_null());
}

#[test]
fn test_chat_reply_to_message() {
    let mut runner = AppTestRunner::load(
        &chat_app_dir(),
        "viewer",
        "did:key:alice",
        "Alice",
    )
    .unwrap();

    runner.call_on_init();

    // Send first message
    runner
        .fire_callback("send_message", vec![serde_json::json!("Original message")])
        .unwrap();
    runner.tick();

    // Get the message ID
    let page_id = runner.page_id().to_string();
    let layer = format!("{}/messages", page_id);
    let data = runner.layer_data(&layer).unwrap();
    let msg_id = data.as_array().unwrap()[0]["id"].as_str().unwrap().to_string();

    // Set reply state, then send reply
    runner.eval_sync(&format!("ui:set('replying_to_id', '{}')", msg_id)).unwrap();
    runner.eval_sync("ui:set('replying_to_text', 'Original message')").unwrap();
    runner
        .fire_callback("send_message", vec![serde_json::json!("This is a reply")])
        .unwrap();
    runner.tick();

    let data = runner.layer_data(&layer).unwrap();
    let messages = data.as_array().unwrap();
    assert_eq!(messages.len(), 2);

    let reply = &messages[1];
    assert_eq!(reply["text"], "This is a reply");
    assert_eq!(reply["reply_to"], msg_id);
    assert_eq!(reply["reply_preview"], "Original message");
}

// -- Multi-peer test --

#[test]
fn test_chat_multi_peer() {
    let lua_code = chat_lua_code();

    let mut multi = MultiPeerRunner::from_code(
        &lua_code,
        "Group Chat",
        "shared-page",
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
        .fire_callback("send_message", vec![serde_json::json!("Hello from Alice")])
        .unwrap();
    multi.tick_all();

    // Bob can see Alice's message (shared MockScribeState)
    let data = multi.peer(1).layer_data("shared-page/messages").unwrap();
    let messages = data.as_array().unwrap();
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0]["text"], "Hello from Alice");
    assert_eq!(messages[0]["sender_name"], "Alice");

    // Bob replies
    multi
        .peer(1)
        .fire_callback("send_message", vec![serde_json::json!("Hi Alice!")])
        .unwrap();
    multi.tick_all();

    // Both see 2 messages
    let data = multi.peer(0).layer_data("shared-page/messages").unwrap();
    assert_eq!(data.as_array().unwrap().len(), 2);

    let data = multi.peer(1).layer_data("shared-page/messages").unwrap();
    let messages = data.as_array().unwrap();
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[1]["sender_name"], "Bob");
}

#[test]
fn test_chat_get_message_count() {
    let mut runner = AppTestRunner::load(
        &chat_app_dir(),
        "viewer",
        "did:key:alice",
        "Alice",
    )
    .unwrap();

    runner.call_on_init();

    // get_message_count is exported via api.export
    let count = runner.eval_sync("return get_message_count()").unwrap();
    assert_eq!(count, serde_json::json!(0));

    runner
        .fire_callback("send_message", vec![serde_json::json!("msg1")])
        .unwrap();
    runner.tick();

    let count = runner.eval_sync("return get_message_count()").unwrap();
    assert_eq!(count, serde_json::json!(1));
}
