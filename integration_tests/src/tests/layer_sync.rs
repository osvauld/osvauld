//! Layer sync protocol tests
//!
//! Tests that layer data flows between peers after publish:
//! - Owner edits map layer → node receives data
//! - Owner edits list layer (chat messages) → node receives data
//! - Viewer connects → receives page metadata with encrypted key
//! - Full chain: owner writes → node relay → viewer receives

use std::time::Duration;

use anyhow::Result;
use tokio::time::sleep;

use butler::ScribeMessage;
use crate::fixtures::{init_tracing, wait_for_layer_data};
use crate::scenario::Scenario;

/// Owner writes to map layer (reactions), node receives via SyncOffer
///
/// **Bug coverage**: "Permission denied: cannot write to layer" where
/// node's Scribe rejects owner SyncOffers because Permissions::can_write()
/// fails on all 3 paths (subscriber, sync_target, stored permit).
///
/// Uses "reactions" — the map layer defined in osvauld-demos's permit_template.json.
#[tokio::test]
async fn test_owner_layer_sync_to_node() -> Result<()> {
    init_tracing();

    let mut s = Scenario::builder()
        .app("osvauld-demos")
        .published()
        .build()
        .await?;

    let page_id = s.space().page_id.clone();

    // Give time for PageAnnounceAck + Scribe subscription
    sleep(Duration::from_millis(500)).await;

    // Owner opens page and writes to a map layer
    let owner_scribe = s.owner().butler.open_page(&page_id).await?;

    // Write data to "reactions" (a map layer in the osvauld-demos permit)
    let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
    owner_scribe.cast(ScribeMessage::EnsureLoroMap {
        layer_name: "reactions".to_string(),
        reply: reply_tx,
    }).map_err(|e| anyhow::anyhow!("Failed to ensure map: {:?}", e))?;
    reply_rx.await??;

    owner_scribe.cast(ScribeMessage::MapInsert {
        layer_name: "reactions".to_string(),
        path: String::new(),
        key: "msg1".to_string(),
        value: serde_json::json!({"emoji": "thumbs_up", "user": "owner"}),
    }).map_err(|e| anyhow::anyhow!("Failed to insert: {:?}", e))?;

    // Wait for layer data to arrive on node
    let node_data = wait_for_layer_data(
        &s.node().butler,
        &page_id,
        "reactions",
        Duration::from_secs(10),
    ).await?;

    assert!(!node_data.is_empty(), "Node should have received reactions layer data");

    s.shutdown().await;
    Ok(())
}

/// Owner writes chat message (list layer), node receives via SyncOffer
///
/// **Bug coverage**: Pattern-based layer names ({page_id}/messages) require
/// gurkha pattern expansion in can_write_layer(). Catches mismatches between
/// actual layer names and permit patterns.
#[tokio::test]
async fn test_owner_chat_layer_sync_to_node() -> Result<()> {
    init_tracing();

    let mut s = Scenario::builder()
        .app("osvauld-demos")
        .published()
        .build()
        .await?;

    let page_id = s.space().page_id.clone();
    let messages_layer = "messages".to_string();

    sleep(Duration::from_millis(500)).await;

    // Owner opens page and writes a chat message
    let owner_scribe = s.owner().butler.open_page(&page_id).await?;

    let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
    owner_scribe.cast(ScribeMessage::EnsureLoroList {
        layer_name: messages_layer.clone(),
        reply: reply_tx,
    }).map_err(|e| anyhow::anyhow!("Failed to ensure list: {:?}", e))?;
    reply_rx.await??;

    owner_scribe.cast(ScribeMessage::ListPush {
        layer_name: messages_layer.clone(),
        path: String::new(),
        item: serde_json::json!({
            "text": "Hello from owner",
            "sender": "owner",
            "timestamp": 1234567890
        }),
    }).map_err(|e| anyhow::anyhow!("Failed to push message: {:?}", e))?;

    // Wait for message to arrive on node
    let node_data = wait_for_layer_data(
        &s.node().butler,
        &page_id,
        &messages_layer,
        Duration::from_secs(10),
    ).await?;

    assert!(!node_data.is_empty(), "Node should have received chat layer data");

    s.shutdown().await;
    Ok(())
}

/// Viewer connects to node and receives page with encrypted key
///
/// **Bug coverage**: SpaceData carries Vec<PublishedPageMeta> and the viewer
/// creates page shells with generated AES keys. Verifies viewer-side page creation.
#[tokio::test]
async fn test_viewer_receives_page() -> Result<()> {
    init_tracing();

    let mut s = Scenario::builder()
        .app("osvauld-demos")
        .published()
        .viewers(1)
        .build()
        .await?;

    let page_id = s.space().page_id.clone();
    let space_id = s.space().space_id.clone();

    sleep(Duration::from_millis(500)).await;

    // Get viewer link and add viewer
    let viewer_link = s.get_viewer_link(&space_id).await?;
    s.add_viewer(0, &viewer_link, &page_id).await?;

    // Verify viewer has the page
    let viewer_page = s.viewer(0).butler.pages().get(&page_id)?;
    assert!(viewer_page.is_some(), "Viewer should have the page");

    let page = viewer_page.unwrap();
    assert_eq!(page.meta.space_id, space_id, "Page should be in correct space");
    assert!(!page.meta.encrypted_key.is_empty(), "Viewer page should have an encrypted AES key");

    s.shutdown().await;
    Ok(())
}

/// Full flow — owner writes layer data, viewer receives via node relay
///
/// **Bug coverage**: End-to-end test of owner → node → viewer data flow.
/// Catches permission, subscription, or encryption issues in the full chain.
#[tokio::test]
async fn test_owner_to_viewer_layer_flow() -> Result<()> {
    init_tracing();

    let mut s = Scenario::builder()
        .app("osvauld-demos")
        .published()
        .viewers(1)
        .build()
        .await?;

    let page_id = s.space().page_id.clone();
    let space_id = s.space().space_id.clone();
    let messages_layer = "messages".to_string();

    sleep(Duration::from_millis(500)).await;

    // Owner writes data BEFORE viewer connects
    let owner_scribe = s.owner().butler.open_page(&page_id).await?;
    let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
    owner_scribe.cast(ScribeMessage::EnsureLoroList {
        layer_name: messages_layer.clone(),
        reply: reply_tx,
    }).map_err(|e| anyhow::anyhow!("EnsureLoroList failed: {:?}", e))?;
    reply_rx.await??;

    owner_scribe.cast(ScribeMessage::ListPush {
        layer_name: messages_layer.clone(),
        path: String::new(),
        item: serde_json::json!({
            "text": "Pre-viewer message",
            "sender": "owner"
        }),
    }).map_err(|e| anyhow::anyhow!("ListPush failed: {:?}", e))?;

    // Wait for data to reach node first
    wait_for_layer_data(
        &s.node().butler,
        &page_id,
        &messages_layer,
        Duration::from_secs(10),
    ).await?;

    // Now connect viewer
    let viewer_link = s.get_viewer_link(&space_id).await?;
    s.add_viewer(0, &viewer_link, &page_id).await?;

    // Wait for layer data to arrive on viewer
    let viewer_data = wait_for_layer_data(
        &s.viewer(0).butler,
        &page_id,
        &messages_layer,
        Duration::from_secs(10),
    ).await?;

    assert!(!viewer_data.is_empty(), "Viewer should have received layer data");

    s.shutdown().await;
    Ok(())
}
