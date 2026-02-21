//! Layer sync integration tests
//!
//! Focused tests for static and dynamic channel layer sync using group-chat app.
//! Tests cover: owner→node, owner→node→viewer, dynamic layer creation,
//! second message propagation, viewer writes, and late joiners.

use std::collections::HashMap;
use std::time::Duration;

use anyhow::Result;
use serde_json::Value;
use tokio::time::sleep;
use tracing::info;

use crate::fixtures::{init_tracing, wait_for_layer_data};
use crate::peer::Peer;
use crate::scenario::Scenario;
use butler::ScribeMessage;

fn day_period(offset_days: i32) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock before unix epoch")
        .as_secs() as i64;
    domains::format_period("day", now, offset_days).expect("valid day period")
}

async fn get_layer_json(peer: &Peer, page_id: &str, layer_name: &str) -> Result<Value> {
    let scribe = peer.butler.open_page(page_id).await?;
    let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
    scribe
        .cast(ScribeMessage::GetLayerData {
            layer_name: layer_name.to_string(),
            reply: reply_tx,
        })
        .map_err(|e| anyhow::anyhow!("GetLayerData cast failed: {:?}", e))?;

    let sthithi = reply_rx
        .await
        .map_err(|_| anyhow::anyhow!("GetLayerData channel closed"))?
        .map_err(|e| anyhow::anyhow!("GetLayerData failed: {}", e))?;

    Ok(serde_json::Value::from(&sthithi))
}

/// Poll `get_layer_json` until a specific key appears in the map.
async fn wait_for_layer_json_key(
    peer: &Peer,
    page_id: &str,
    layer_name: &str,
    key: &str,
    timeout: Duration,
) -> Result<Value> {
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        if let Ok(json) = get_layer_json(peer, page_id, layer_name).await {
            if let Some(val) = json.get(key) {
                return Ok(val.clone());
            }
        }
        if tokio::time::Instant::now() > deadline {
            return Err(anyhow::anyhow!(
                "Timeout waiting for key '{}' in layer '{}' on page '{}'",
                key,
                layer_name,
                page_id
            ));
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

/// Static channel (channels/general/messages) syncs from owner to node.
#[tokio::test]
async fn test_static_channel_syncs_owner_to_node() -> Result<()> {
    init_tracing();

    let mut s = Scenario::builder()
        .app("group-chat")
        .published()
        .build()
        .await?;

    let page_id = s.space().page_id.clone();
    sleep(Duration::from_millis(500)).await;

    let owner_scribe = s.owner().butler.open_page(&page_id).await?;

    let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
    owner_scribe
        .cast(ScribeMessage::EnsureLoroMap {
            layer_name: "channels/general/messages".to_string(),
            reply: reply_tx,
        })
        .map_err(|e| anyhow::anyhow!("EnsureLoroMap failed: {:?}", e))?;
    reply_rx.await??;

    owner_scribe
        .cast(ScribeMessage::MapInsert {
            layer_name: "channels/general/messages".to_string(),
            path: String::new(),
            key: "msg1".to_string(),
            value: serde_json::json!({
                "id": "msg1",
                "text": "Hello from owner",
                "sender_did": "did:key:owner",
                "timestamp": 1234567890
            }).into(),
        })
        .map_err(|e| anyhow::anyhow!("MapInsert failed: {:?}", e))?;

    let node_data = wait_for_layer_data(
        &s.node().butler,
        &page_id,
        "channels/general/messages",
        Duration::from_secs(10),
    )
    .await?;

    assert!(
        !node_data.is_empty(),
        "Node should receive static channel layer data"
    );

    s.shutdown().await;
    Ok(())
}

/// Static channel syncs owner → node → viewer (viewer connects after write).
#[tokio::test]
async fn test_static_channel_syncs_owner_to_viewer() -> Result<()> {
    init_tracing();

    let mut s = Scenario::builder()
        .app("group-chat")
        .published()
        .viewers(1)
        .build()
        .await?;

    let page_id = s.space().page_id.clone();
    let space_id = s.space().space_id.clone();
    sleep(Duration::from_millis(500)).await;

    // Owner writes BEFORE viewer connects
    let owner_scribe = s.owner().butler.open_page(&page_id).await?;

    let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
    owner_scribe
        .cast(ScribeMessage::EnsureLoroMap {
            layer_name: "channels/general/messages".to_string(),
            reply: reply_tx,
        })
        .map_err(|e| anyhow::anyhow!("EnsureLoroMap failed: {:?}", e))?;
    reply_rx.await??;

    owner_scribe
        .cast(ScribeMessage::MapInsert {
            layer_name: "channels/general/messages".to_string(),
            path: String::new(),
            key: "msg1".to_string(),
            value: serde_json::json!({
                "id": "msg1",
                "text": "Pre-viewer message",
                "sender_did": "did:key:owner",
                "timestamp": 1234567890
            }).into(),
        })
        .map_err(|e| anyhow::anyhow!("MapInsert failed: {:?}", e))?;

    // Wait for data on node first
    wait_for_layer_data(
        &s.node().butler,
        &page_id,
        "channels/general/messages",
        Duration::from_secs(10),
    )
    .await?;

    // Connect viewer
    let viewer_link = s.get_viewer_link(&space_id).await?;
    s.add_viewer(0, &viewer_link, &page_id).await?;

    // Viewer should receive the data
    let viewer_data = wait_for_layer_data(
        &s.viewer(0).butler,
        &page_id,
        "channels/general/messages",
        Duration::from_secs(10),
    )
    .await?;

    assert!(
        !viewer_data.is_empty(),
        "Viewer should receive static channel data"
    );

    s.shutdown().await;
    Ok(())
}

/// Dynamic channel: first message from owner reaches viewer via node relay.
///
/// Viewer connects first, then owner creates dynamic layer + writes msg1.
#[tokio::test]
async fn test_dynamic_channel_first_message_reaches_viewer() -> Result<()> {
    init_tracing();

    let mut s = Scenario::builder()
        .app("group-chat")
        .published()
        .viewers(1)
        .build()
        .await?;

    let page_id = s.space().page_id.clone();
    let space_id = s.space().space_id.clone();
    sleep(Duration::from_millis(500)).await;

    // Connect viewer first
    let viewer_link = s.get_viewer_link(&space_id).await?;
    s.add_viewer(0, &viewer_link, &page_id).await?;
    sleep(Duration::from_millis(500)).await;

    // Owner creates dynamic layer and writes msg1
    let owner_scribe = s.owner().butler.open_page(&page_id).await?;

    let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
    owner_scribe
        .cast(ScribeMessage::CreateDynamicLayer {
            schema_key: "channels/{id}/messages".to_string(),
            placeholders: [("id".to_string(), "project-x".to_string())]
                .into_iter()
                .collect(),
            authorized_peers: None,
            reply: reply_tx,
        })
        .map_err(|e| anyhow::anyhow!("CreateDynamicLayer failed: {:?}", e))?;

    let full_layer_name = reply_rx
        .await
        .map_err(|_| anyhow::anyhow!("CreateDynamicLayer channel closed"))?
        .map_err(|e| anyhow::anyhow!("CreateDynamicLayer failed: {}", e))?;

    let bare_layer = full_layer_name
        .strip_prefix(&format!("{}/", page_id))
        .unwrap_or(&full_layer_name);

    owner_scribe
        .cast(ScribeMessage::MapInsert {
            layer_name: bare_layer.to_string(),
            path: String::new(),
            key: "msg1".to_string(),
            value: serde_json::json!({
                "id": "msg1",
                "text": "First message in dynamic channel",
                "sender_did": "did:key:owner",
                "timestamp": 1234567890
            }).into(),
        })
        .map_err(|e| anyhow::anyhow!("MapInsert failed: {:?}", e))?;

    // Node should receive it
    wait_for_layer_data(
        &s.node().butler,
        &page_id,
        bare_layer,
        Duration::from_secs(10),
    )
    .await?;

    // Viewer should receive msg1
    wait_for_layer_json_key(
        s.viewer(0),
        &page_id,
        bare_layer,
        "msg1",
        Duration::from_secs(10),
    )
    .await?;

    info!("Viewer received msg1 on dynamic channel");

    s.shutdown().await;
    Ok(())
}

/// Dynamic channel: second message reaches viewer (reproduces known bug).
///
/// After viewer receives msg1, owner writes msg2. Currently msg2 may fail
/// to propagate due to sync-meta chatter or missing observer.
#[tokio::test]
async fn test_dynamic_channel_second_message_reaches_viewer() -> Result<()> {
    init_tracing();

    let mut s = Scenario::builder()
        .app("group-chat")
        .published()
        .viewers(1)
        .build()
        .await?;

    let page_id = s.space().page_id.clone();
    let space_id = s.space().space_id.clone();
    sleep(Duration::from_millis(500)).await;

    // Connect viewer first
    let viewer_link = s.get_viewer_link(&space_id).await?;
    s.add_viewer(0, &viewer_link, &page_id).await?;
    sleep(Duration::from_millis(500)).await;

    // Owner creates dynamic layer and writes msg1
    let owner_scribe = s.owner().butler.open_page(&page_id).await?;

    let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
    owner_scribe
        .cast(ScribeMessage::CreateDynamicLayer {
            schema_key: "channels/{id}/messages".to_string(),
            placeholders: [("id".to_string(), "project-y".to_string())]
                .into_iter()
                .collect(),
            authorized_peers: None,
            reply: reply_tx,
        })
        .map_err(|e| anyhow::anyhow!("CreateDynamicLayer failed: {:?}", e))?;

    let full_layer_name = reply_rx
        .await
        .map_err(|_| anyhow::anyhow!("CreateDynamicLayer channel closed"))?
        .map_err(|e| anyhow::anyhow!("CreateDynamicLayer failed: {}", e))?;

    let bare_layer = full_layer_name
        .strip_prefix(&format!("{}/", page_id))
        .unwrap_or(&full_layer_name)
        .to_string();

    owner_scribe
        .cast(ScribeMessage::MapInsert {
            layer_name: bare_layer.clone(),
            path: String::new(),
            key: "msg1".to_string(),
            value: serde_json::json!({
                "id": "msg1",
                "text": "First message",
                "sender_did": "did:key:owner",
                "timestamp": 1234567890
            }).into(),
        })
        .map_err(|e| anyhow::anyhow!("MapInsert msg1 failed: {:?}", e))?;

    // Wait for viewer to receive msg1
    wait_for_layer_json_key(
        s.viewer(0),
        &page_id,
        &bare_layer,
        "msg1",
        Duration::from_secs(10),
    )
    .await?;

    info!("Viewer received msg1, now sending msg2");

    // Owner writes msg2
    owner_scribe
        .cast(ScribeMessage::MapInsert {
            layer_name: bare_layer.clone(),
            path: String::new(),
            key: "msg2".to_string(),
            value: serde_json::json!({
                "id": "msg2",
                "text": "Second message",
                "sender_did": "did:key:owner",
                "timestamp": 1234567891
            }).into(),
        })
        .map_err(|e| anyhow::anyhow!("MapInsert msg2 failed: {:?}", e))?;

    // Viewer should receive msg2
    wait_for_layer_json_key(
        s.viewer(0),
        &page_id,
        &bare_layer,
        "msg2",
        Duration::from_secs(10),
    )
    .await?;

    // Both keys present
    let viewer_json = get_layer_json(s.viewer(0), &page_id, &bare_layer).await?;
    assert!(
        viewer_json.get("msg1").is_some(),
        "Viewer should still have msg1"
    );
    assert!(viewer_json.get("msg2").is_some(), "Viewer should have msg2");

    s.shutdown().await;
    Ok(())
}

/// Viewer writes to dynamic channel — node and owner receive (reproduces known bug).
///
/// Owner creates dynamic channel + writes msg1, viewer receives it,
/// then viewer writes msg2. Currently viewer writes may silently fail.
#[tokio::test]
async fn test_viewer_writes_to_dynamic_channel() -> Result<()> {
    init_tracing();

    let mut s = Scenario::builder()
        .app("group-chat")
        .published()
        .viewers(1)
        .build()
        .await?;

    let page_id = s.space().page_id.clone();
    let space_id = s.space().space_id.clone();
    sleep(Duration::from_millis(500)).await;

    // Connect viewer
    let viewer_link = s.get_viewer_link(&space_id).await?;
    s.add_viewer(0, &viewer_link, &page_id).await?;
    sleep(Duration::from_millis(500)).await;

    // Owner creates dynamic channel + writes msg1
    let owner_scribe = s.owner().butler.open_page(&page_id).await?;

    let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
    owner_scribe
        .cast(ScribeMessage::CreateDynamicLayer {
            schema_key: "channels/{id}/messages".to_string(),
            placeholders: [("id".to_string(), "project-z".to_string())]
                .into_iter()
                .collect(),
            authorized_peers: None,
            reply: reply_tx,
        })
        .map_err(|e| anyhow::anyhow!("CreateDynamicLayer failed: {:?}", e))?;

    let full_layer_name = reply_rx
        .await
        .map_err(|_| anyhow::anyhow!("CreateDynamicLayer channel closed"))?
        .map_err(|e| anyhow::anyhow!("CreateDynamicLayer failed: {}", e))?;

    let bare_layer = full_layer_name
        .strip_prefix(&format!("{}/", page_id))
        .unwrap_or(&full_layer_name)
        .to_string();

    owner_scribe
        .cast(ScribeMessage::MapInsert {
            layer_name: bare_layer.clone(),
            path: String::new(),
            key: "msg1".to_string(),
            value: serde_json::json!({
                "id": "msg1",
                "text": "Owner's first message",
                "sender_did": "did:key:owner",
                "timestamp": 1234567890
            }).into(),
        })
        .map_err(|e| anyhow::anyhow!("MapInsert failed: {:?}", e))?;

    // Viewer receives msg1
    wait_for_layer_json_key(
        s.viewer(0),
        &page_id,
        &bare_layer,
        "msg1",
        Duration::from_secs(10),
    )
    .await?;

    info!("Viewer received msg1, now viewer writes msg2");

    // Viewer writes msg2 to the same dynamic channel
    let viewer_scribe = s.viewer(0).butler.open_page(&page_id).await?;
    viewer_scribe
        .cast(ScribeMessage::MapInsert {
            layer_name: bare_layer.clone(),
            path: String::new(),
            key: "msg2".to_string(),
            value: serde_json::json!({
                "id": "msg2",
                "text": "Viewer's reply",
                "sender_did": "did:key:viewer",
                "timestamp": 1234567891
            }).into(),
        })
        .map_err(|e| anyhow::anyhow!("Viewer MapInsert failed: {:?}", e))?;

    // Node should receive viewer's msg2
    wait_for_layer_json_key(
        s.node(),
        &page_id,
        &bare_layer,
        "msg2",
        Duration::from_secs(10),
    )
    .await?;

    // Owner should also receive viewer's msg2
    wait_for_layer_json_key(
        s.owner(),
        &page_id,
        &bare_layer,
        "msg2",
        Duration::from_secs(10),
    )
    .await?;

    s.shutdown().await;
    Ok(())
}

/// Dynamic channel: viewer connects AFTER owner creates + writes (late joiner).
#[tokio::test]
async fn test_dynamic_channel_late_joiner() -> Result<()> {
    init_tracing();

    let mut s = Scenario::builder()
        .app("group-chat")
        .published()
        .viewers(1)
        .build()
        .await?;

    let page_id = s.space().page_id.clone();
    let space_id = s.space().space_id.clone();
    sleep(Duration::from_millis(500)).await;

    // Owner creates dynamic layer + writes BEFORE viewer connects
    let owner_scribe = s.owner().butler.open_page(&page_id).await?;

    let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
    owner_scribe
        .cast(ScribeMessage::CreateDynamicLayer {
            schema_key: "channels/{id}/messages".to_string(),
            placeholders: [("id".to_string(), "late-join".to_string())]
                .into_iter()
                .collect(),
            authorized_peers: None,
            reply: reply_tx,
        })
        .map_err(|e| anyhow::anyhow!("CreateDynamicLayer failed: {:?}", e))?;

    let full_layer_name = reply_rx
        .await
        .map_err(|_| anyhow::anyhow!("CreateDynamicLayer channel closed"))?
        .map_err(|e| anyhow::anyhow!("CreateDynamicLayer failed: {}", e))?;

    let bare_layer = full_layer_name
        .strip_prefix(&format!("{}/", page_id))
        .unwrap_or(&full_layer_name)
        .to_string();

    owner_scribe
        .cast(ScribeMessage::MapInsert {
            layer_name: bare_layer.clone(),
            path: String::new(),
            key: "msg1".to_string(),
            value: serde_json::json!({
                "id": "msg1",
                "text": "Message before viewer joins",
                "sender_did": "did:key:owner",
                "timestamp": 1234567890
            }).into(),
        })
        .map_err(|e| anyhow::anyhow!("MapInsert failed: {:?}", e))?;

    // Wait for node to receive the data first
    wait_for_layer_data(
        &s.node().butler,
        &page_id,
        &bare_layer,
        Duration::from_secs(10),
    )
    .await?;

    // Now connect late-joining viewer
    let viewer_link = s.get_viewer_link(&space_id).await?;
    s.add_viewer(0, &viewer_link, &page_id).await?;

    // Viewer should discover and receive the dynamic layer data
    wait_for_layer_json_key(
        s.viewer(0),
        &page_id,
        &bare_layer,
        "msg1",
        Duration::from_secs(12),
    )
    .await?;

    info!("Late-joining viewer received dynamic channel data");

    s.shutdown().await;
    Ok(())
}

/// Owner creates dynamic channel while node is disconnected; data syncs after reconnect.
#[tokio::test]
async fn test_dynamic_channel_node_offline_sync() -> Result<()> {
    init_tracing();

    let mut s = Scenario::builder()
        .app("group-chat")
        .published()
        .build()
        .await?;

    let page_id = s.space().page_id.clone();
    sleep(Duration::from_millis(500)).await;

    // Disconnect owner from node
    s.disconnect_owner_from_node().await?;

    // Owner creates dynamic layer + writes while node is offline
    let owner_scribe = s.owner().butler.open_page(&page_id).await?;

    let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
    owner_scribe
        .cast(ScribeMessage::CreateDynamicLayer {
            schema_key: "channels/{id}/messages".to_string(),
            placeholders: [("id".to_string(), "offline-test".to_string())]
                .into_iter()
                .collect(),
            authorized_peers: None,
            reply: reply_tx,
        })
        .map_err(|e| anyhow::anyhow!("CreateDynamicLayer failed: {:?}", e))?;

    let full_layer_name = reply_rx
        .await
        .map_err(|_| anyhow::anyhow!("CreateDynamicLayer channel closed"))?
        .map_err(|e| anyhow::anyhow!("CreateDynamicLayer failed: {}", e))?;

    let bare_layer = full_layer_name
        .strip_prefix(&format!("{}/", page_id))
        .unwrap_or(&full_layer_name)
        .to_string();

    owner_scribe
        .cast(ScribeMessage::MapInsert {
            layer_name: bare_layer.clone(),
            path: String::new(),
            key: "msg1".to_string(),
            value: serde_json::json!({
                "id": "msg1",
                "text": "Written while node was offline",
                "sender_did": "did:key:owner",
                "timestamp": 1234567890
            }).into(),
        })
        .map_err(|e| anyhow::anyhow!("MapInsert failed: {:?}", e))?;

    // Give local write time to settle
    sleep(Duration::from_millis(200)).await;

    // Reconnect owner to node
    s.reconnect_owner_to_node().await?;

    // Node should receive the dynamic channel data after reconnect
    wait_for_layer_json_key(
        s.node(),
        &page_id,
        &bare_layer,
        "msg1",
        Duration::from_secs(10),
    )
    .await?;

    info!("Node received dynamic channel data after reconnect");

    s.shutdown().await;
    Ok(())
}

/// DM (explicit grant): owner creates DM layer, grants access to viewer,
/// viewer discovers and receives data.
#[tokio::test]
async fn test_dm_explicit_grant_syncs_to_viewer() -> Result<()> {
    init_tracing();

    let mut s = Scenario::builder()
        .app("group-chat")
        .published()
        .viewers(1)
        .build()
        .await?;

    let page_id = s.space().page_id.clone();
    let space_id = s.space().space_id.clone();
    sleep(Duration::from_millis(500)).await;

    // Connect viewer
    let viewer_link = s.get_viewer_link(&space_id).await?;
    s.add_viewer(0, &viewer_link, &page_id).await?;
    sleep(Duration::from_millis(500)).await;

    // Get viewer's DID for the explicit grant
    let viewer_did = s.viewer(0).butler.get_identity().await?.did().to_string();

    // Owner creates DM layer (explicit grant type: "dms/{id}/messages")
    let owner_scribe = s.owner().butler.open_page(&page_id).await?;

    let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
    owner_scribe
        .cast(ScribeMessage::CreateDynamicLayer {
            schema_key: "dms/{id}/messages".to_string(),
            placeholders: [("id".to_string(), "dm-test".to_string())]
                .into_iter()
                .collect(),
            authorized_peers: None,
            reply: reply_tx,
        })
        .map_err(|e| anyhow::anyhow!("CreateDynamicLayer failed: {:?}", e))?;

    let full_layer_name = reply_rx
        .await
        .map_err(|_| anyhow::anyhow!("CreateDynamicLayer channel closed"))?
        .map_err(|e| anyhow::anyhow!("CreateDynamicLayer failed: {}", e))?;

    let bare_layer = full_layer_name
        .strip_prefix(&format!("{}/", page_id))
        .unwrap_or(&full_layer_name)
        .to_string();

    // Owner writes a message to the DM layer
    owner_scribe
        .cast(ScribeMessage::MapInsert {
            layer_name: bare_layer.clone(),
            path: String::new(),
            key: "dm1".to_string(),
            value: serde_json::json!({
                "id": "dm1",
                "text": "Private DM message",
                "sender_did": "did:key:owner",
                "timestamp": 1234567890
            }).into(),
        })
        .map_err(|e| anyhow::anyhow!("MapInsert failed: {:?}", e))?;

    // Wait for data to reach the node first
    wait_for_layer_data(
        &s.node().butler,
        &page_id,
        &bare_layer,
        Duration::from_secs(10),
    )
    .await?;

    // Grant viewer explicit access to the DM layer
    let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
    owner_scribe
        .cast(ScribeMessage::AddLayerAccess {
            layer_name: full_layer_name.clone(),
            dids: vec![viewer_did.clone()],
            reply: reply_tx,
        })
        .map_err(|e| anyhow::anyhow!("AddLayerAccess failed: {:?}", e))?;

    reply_rx
        .await
        .map_err(|_| anyhow::anyhow!("AddLayerAccess channel closed"))?
        .map_err(|e| anyhow::anyhow!("AddLayerAccess failed: {}", e))?;

    info!(
        "Granted viewer {} access to DM layer {}",
        viewer_did, full_layer_name
    );

    // Viewer should receive the DM data via LayerSync
    wait_for_layer_json_key(
        s.viewer(0),
        &page_id,
        &bare_layer,
        "dm1",
        Duration::from_secs(10),
    )
    .await?;

    info!("Viewer received DM data via explicit grant");

    s.shutdown().await;
    Ok(())
}

/// DM (explicit grant): non-granted viewer cannot see DM data.
///
/// Owner creates DM, grants access to viewer0 only.
/// Viewer1 should NOT receive the DM data.
#[tokio::test]
async fn test_dm_explicit_grant_isolation() -> Result<()> {
    init_tracing();

    let mut s = Scenario::builder()
        .app("group-chat")
        .published()
        .viewers(2)
        .build()
        .await?;

    let page_id = s.space().page_id.clone();
    let space_id = s.space().space_id.clone();
    sleep(Duration::from_millis(500)).await;

    // Connect both viewers
    let viewer_link_0 = s.get_viewer_link(&space_id).await?;
    s.add_viewer(0, &viewer_link_0, &page_id).await?;
    let viewer_link_1 = s.get_viewer_link(&space_id).await?;
    s.add_viewer(1, &viewer_link_1, &page_id).await?;
    sleep(Duration::from_millis(500)).await;

    let viewer0_did = s.viewer(0).butler.get_identity().await?.did().to_string();

    // Owner creates DM layer with explicit access for viewer0 only
    let owner_scribe = s.owner().butler.open_page(&page_id).await?;

    let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
    owner_scribe
        .cast(ScribeMessage::CreateDynamicLayer {
            schema_key: "dms/{id}/messages".to_string(),
            placeholders: [("id".to_string(), "private-dm".to_string())]
                .into_iter()
                .collect(),
            authorized_peers: Some(vec![viewer0_did.clone()]),
            reply: reply_tx,
        })
        .map_err(|e| anyhow::anyhow!("CreateDynamicLayer failed: {:?}", e))?;

    let full_layer_name = reply_rx
        .await
        .map_err(|_| anyhow::anyhow!("CreateDynamicLayer channel closed"))?
        .map_err(|e| anyhow::anyhow!("CreateDynamicLayer failed: {}", e))?;

    let bare_layer = full_layer_name
        .strip_prefix(&format!("{}/", page_id))
        .unwrap_or(&full_layer_name)
        .to_string();

    owner_scribe
        .cast(ScribeMessage::MapInsert {
            layer_name: bare_layer.clone(),
            path: String::new(),
            key: "secret".to_string(),
            value: serde_json::json!({
                "id": "secret",
                "text": "Only viewer0 should see this",
                "sender_did": "did:key:owner",
                "timestamp": 1234567890
            }).into(),
        })
        .map_err(|e| anyhow::anyhow!("MapInsert failed: {:?}", e))?;

    // Wait for data on node
    wait_for_layer_data(
        &s.node().butler,
        &page_id,
        &bare_layer,
        Duration::from_secs(10),
    )
    .await?;

    // Grant access ONLY to viewer0
    let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
    owner_scribe
        .cast(ScribeMessage::AddLayerAccess {
            layer_name: full_layer_name.clone(),
            dids: vec![viewer0_did.clone()],
            reply: reply_tx,
        })
        .map_err(|e| anyhow::anyhow!("AddLayerAccess failed: {:?}", e))?;

    reply_rx
        .await
        .map_err(|_| anyhow::anyhow!("AddLayerAccess channel closed"))?
        .map_err(|e| anyhow::anyhow!("AddLayerAccess failed: {}", e))?;

    // Viewer0 should receive the DM data
    wait_for_layer_json_key(
        s.viewer(0),
        &page_id,
        &bare_layer,
        "secret",
        Duration::from_secs(10),
    )
    .await?;

    info!("Viewer0 received DM data (granted)");

    // Viewer1 should NOT receive the data — wait briefly and verify absence
    sleep(Duration::from_secs(2)).await;
    let viewer1_result = get_layer_json(s.viewer(1), &page_id, &bare_layer).await;
    match viewer1_result {
        Ok(json) => {
            assert!(
                json.get("secret").is_none(),
                "Viewer1 should NOT have access to DM data, but got: {:?}",
                json
            );
        }
        Err(_) => {
            // Layer doesn't exist on viewer1 at all — expected
            info!("Viewer1 correctly has no access to DM layer");
        }
    }

    s.shutdown().await;
    Ok(())
}

/// Time-sharded channel layers: two day shards both sync to viewer.
#[tokio::test]
async fn test_time_sharded_daily_layers_sync_to_viewer() -> Result<()> {
    init_tracing();

    let mut s = Scenario::builder()
        .app("group-chat")
        .published()
        .viewers(1)
        .build()
        .await?;

    let page_id = s.space().page_id.clone();
    let space_id = s.space().space_id.clone();

    let viewer_link = s.get_viewer_link(&space_id).await?;
    s.add_viewer(0, &viewer_link, &page_id).await?;

    let owner_scribe = s.owner().butler.open_page(&page_id).await?;
    let day0 = day_period(0);
    let day1 = day_period(1);

    let (reply0_tx, reply0_rx) = tokio::sync::oneshot::channel();
    let mut placeholders0 = HashMap::new();
    placeholders0.insert("channel".to_string(), "general".to_string());
    placeholders0.insert("period".to_string(), day0.clone());
    owner_scribe
        .cast(ScribeMessage::CreateDynamicLayer {
            schema_key: "channels/{channel}/messages/{period}".to_string(),
            placeholders: placeholders0,
            authorized_peers: None,
            reply: reply0_tx,
        })
        .map_err(|e| anyhow::anyhow!("CreateDynamicLayer day0 failed: {:?}", e))?;
    let full0 = reply0_rx
        .await
        .map_err(|_| anyhow::anyhow!("CreateDynamicLayer day0 channel closed"))?
        .map_err(|e| anyhow::anyhow!("CreateDynamicLayer day0 failed: {}", e))?;

    let (reply1_tx, reply1_rx) = tokio::sync::oneshot::channel();
    let mut placeholders1 = HashMap::new();
    placeholders1.insert("channel".to_string(), "general".to_string());
    placeholders1.insert("period".to_string(), day1.clone());
    owner_scribe
        .cast(ScribeMessage::CreateDynamicLayer {
            schema_key: "channels/{channel}/messages/{period}".to_string(),
            placeholders: placeholders1,
            authorized_peers: None,
            reply: reply1_tx,
        })
        .map_err(|e| anyhow::anyhow!("CreateDynamicLayer day1 failed: {:?}", e))?;
    let full1 = reply1_rx
        .await
        .map_err(|_| anyhow::anyhow!("CreateDynamicLayer day1 channel closed"))?
        .map_err(|e| anyhow::anyhow!("CreateDynamicLayer day1 failed: {}", e))?;

    let bare0 = full0
        .strip_prefix(&format!("{}/", page_id))
        .unwrap_or(&full0)
        .to_string();
    let bare1 = full1
        .strip_prefix(&format!("{}/", page_id))
        .unwrap_or(&full1)
        .to_string();

    owner_scribe
        .cast(ScribeMessage::MapInsert {
            layer_name: bare0.clone(),
            path: String::new(),
            key: "msg-day0".to_string(),
            value: serde_json::json!({"id":"msg-day0","text":"d0","timestamp":1}).into(),
        })
        .map_err(|e| anyhow::anyhow!("MapInsert day0 failed: {:?}", e))?;

    owner_scribe
        .cast(ScribeMessage::MapInsert {
            layer_name: bare1.clone(),
            path: String::new(),
            key: "msg-day1".to_string(),
            value: serde_json::json!({"id":"msg-day1","text":"d1","timestamp":2}).into(),
        })
        .map_err(|e| anyhow::anyhow!("MapInsert day1 failed: {:?}", e))?;

    wait_for_layer_json_key(
        s.viewer(0),
        &page_id,
        &bare0,
        "msg-day0",
        Duration::from_secs(10),
    )
    .await?;
    wait_for_layer_json_key(
        s.viewer(0),
        &page_id,
        &bare1,
        "msg-day1",
        Duration::from_secs(10),
    )
    .await?;

    s.shutdown().await;
    Ok(())
}

/// Time-sharded layer offline writes on both sides merge after reconnect.
#[tokio::test]
async fn test_time_sharded_daily_layer_offline_merge() -> Result<()> {
    init_tracing();

    let mut s = Scenario::builder()
        .app("group-chat")
        .published()
        .build()
        .await?;

    let page_id = s.space().page_id.clone();
    let day = day_period(0);

    let owner_scribe = s.owner().butler.open_page(&page_id).await?;
    let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
    let mut placeholders = HashMap::new();
    placeholders.insert("channel".to_string(), "general".to_string());
    placeholders.insert("period".to_string(), day);
    owner_scribe
        .cast(ScribeMessage::CreateDynamicLayer {
            schema_key: "channels/{channel}/messages/{period}".to_string(),
            placeholders,
            authorized_peers: None,
            reply: reply_tx,
        })
        .map_err(|e| anyhow::anyhow!("CreateDynamicLayer failed: {:?}", e))?;
    let full = reply_rx
        .await
        .map_err(|_| anyhow::anyhow!("CreateDynamicLayer channel closed"))?
        .map_err(|e| anyhow::anyhow!("CreateDynamicLayer failed: {}", e))?;
    let bare = full
        .strip_prefix(&format!("{}/", page_id))
        .unwrap_or(&full)
        .to_string();

    s.disconnect_owner_from_node().await?;

    owner_scribe
        .cast(ScribeMessage::MapInsert {
            layer_name: bare.clone(),
            path: String::new(),
            key: "owner-msg".to_string(),
            value: serde_json::json!({"id":"owner-msg","text":"owner offline","timestamp":1}).into(),
        })
        .map_err(|e| anyhow::anyhow!("Owner offline insert failed: {:?}", e))?;

    let node_scribe = s.node().butler.open_page(&page_id).await?;
    node_scribe
        .cast(ScribeMessage::MapInsert {
            layer_name: bare.clone(),
            path: String::new(),
            key: "node-msg".to_string(),
            value: serde_json::json!({"id":"node-msg","text":"node offline","timestamp":2}).into(),
        })
        .map_err(|e| anyhow::anyhow!("Node offline insert failed: {:?}", e))?;

    s.reconnect_owner_to_node().await?;

    wait_for_layer_json_key(
        s.owner(),
        &page_id,
        &bare,
        "node-msg",
        Duration::from_secs(12),
    )
    .await?;
    wait_for_layer_json_key(
        s.node(),
        &page_id,
        &bare,
        "owner-msg",
        Duration::from_secs(12),
    )
    .await?;

    s.shutdown().await;
    Ok(())
}
