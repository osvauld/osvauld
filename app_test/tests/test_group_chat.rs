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

/// Helper: get message count from the channels/general/messages map layer
fn message_count(runner: &AppTestRunner) -> usize {
    let data = runner.layer_data("channels/general/messages");
    match data {
        Some(obj) => obj.as_object().map(|m| m.len()).unwrap_or(0),
        None => 0,
    }
}

/// Helper: get all messages as a sorted-by-timestamp vec
fn get_messages(runner: &AppTestRunner) -> Vec<serde_json::Value> {
    let data = runner.layer_data("channels/general/messages");
    match data {
        Some(obj) => {
            let map = obj.as_object().unwrap();
            let mut msgs: Vec<serde_json::Value> = map.values().cloned().collect();
            msgs.sort_by_key(|m| m["timestamp"].as_i64().unwrap_or(0));
            msgs
        }
        None => vec![],
    }
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
    let messages = get_messages(&runner);
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

    assert_eq!(message_count(&runner), 0, "Empty message should be rejected");

    // Nil
    runner
        .fire_callback("send_message", vec![serde_json::Value::Null])
        .unwrap();
    runner.tick();

    assert_eq!(message_count(&runner), 0, "Nil message should be rejected");
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

    assert_eq!(message_count(&runner), 5);

    // Collect all message texts (order may vary since os.time() is same-second)
    let messages = get_messages(&runner);
    let texts: Vec<&str> = messages.iter().map(|m| m["text"].as_str().unwrap()).collect();
    for i in 1..=5 {
        assert!(texts.contains(&format!("Message {}", i).as_str()), "Missing Message {}", i);
    }
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
    let auto_scroll = runner.eval_sync("return ui:get('auto_scroll')").unwrap();
    assert_eq!(auto_scroll, serde_json::json!(true));

    let show_emoji = runner.eval_sync("return ui:get('show_emoji_picker')").unwrap();
    assert_eq!(show_emoji, serde_json::json!(false));

    let thread_open = runner.eval_sync("return ui:get('thread_open')").unwrap();
    assert_eq!(thread_open, serde_json::json!(false));
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

    let messages = get_messages(&runner);
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
    let messages = get_messages(&runner);
    let msg_id = messages[0]["id"].as_str().unwrap().to_string();

    // Set reply state + draft text, then call on_submit (like pressing Enter)
    runner.eval_sync(&format!("ui:set('replying_to_id', '{}')", msg_id)).unwrap();
    runner.eval_sync("ui:set('replying_to_text', 'Original message')").unwrap();
    runner.eval_sync("ui:set('draft_text', 'This is a reply')").unwrap();
    runner
        .fire_callback("on_submit", vec![])
        .unwrap();
    runner.tick();

    assert_eq!(message_count(&runner), 2);

    // Find the reply message (not the original)
    let messages = get_messages(&runner);
    let reply = messages.iter().find(|m| m["text"] == "This is a reply").unwrap();
    assert_eq!(reply["reply_to"], msg_id);
    assert_eq!(reply["reply_preview"], "Original message");
}

// -- Multi-peer test --

#[test]
fn test_chat_multi_peer() {
    let mut multi = MultiPeerRunner::from_dir(
        &chat_app_dir(),
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
    let messages = get_messages(multi.peer(1));
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
    assert_eq!(message_count(multi.peer(0)), 2);
    assert_eq!(message_count(multi.peer(1)), 2);

    // Verify Bob's message exists
    let messages = get_messages(multi.peer(1));
    let bob_msg = messages.iter().find(|m| m["sender_name"] == "Bob").unwrap();
    assert_eq!(bob_msg["text"], "Hi Alice!");
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
