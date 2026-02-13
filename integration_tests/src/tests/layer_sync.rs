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
use serde_json::Value;
use tracing::info;
use courier::trace::TraceDirection;

use butler::ScribeMessage;
use crate::fixtures::{init_tracing, wait_for_layer_data, wait_until};
use crate::peer::Peer;
use crate::scenario::Scenario;

async fn get_layer_json(peer: &Peer, page_id: &str, layer_name: &str) -> Result<Value> {
    let scribe = peer.butler.open_page(page_id).await?;
    let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
    scribe
        .cast(ScribeMessage::GetLayerData {
            layer_name: layer_name.to_string(),
            reply: reply_tx,
        })
        .map_err(|e| anyhow::anyhow!("GetLayerData cast failed: {:?}", e))?;

    reply_rx
        .await
        .map_err(|_| anyhow::anyhow!("GetLayerData channel closed"))?
        .map_err(|e| anyhow::anyhow!("GetLayerData failed: {}", e))
}

fn expected_sync_layers_from_permit(permit: &gurkha::Permit, page_id: &str) -> Vec<String> {
    let page_prefix = format!("{}/", page_id);
    let mut layers = Vec::new();
    for (name, config) in permit.layers() {
        if !config.sync || name.starts_with("__sync_meta/") {
            continue;
        }
        let normalized = name.strip_prefix(&page_prefix).unwrap_or(name).to_string();
        layers.push(normalized);
    }
    layers.sort();
    layers.dedup();
    layers
}

fn assert_sync_meta_entries(sync_meta: &Value, expected_layers: &[String]) {
    let meta_obj = sync_meta
        .as_object()
        .expect("sync-meta should be a map object");

    for layer in expected_layers {
        let entry = meta_obj
            .get(layer)
            .unwrap_or_else(|| panic!("sync-meta missing expected layer entry: {}", layer));
        let synced = entry
            .get("synced")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let version = entry
            .get("version")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        assert!(synced, "expected synced=true for layer {}", layer);
        assert_eq!(version, 1, "expected version=1 for layer {}", layer);
    }
}

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

/// Diagnostic: owner page bootstrap creates protocol sync-meta layer locally.
#[tokio::test]
async fn diagnostic_owner_page_bootstrap_creates_sync_meta_layer() -> Result<()> {
    init_tracing();

    let mut s = Scenario::builder()
        .app("osvauld-demos")
        .published()
        .build()
        .await?;

    let page_id = s.space().page_id.clone();
    let owner_did = s.owner().butler.user_info().await?.did;
    let sync_meta_layer = format!("__sync_meta/{}", owner_did);
    let owner_page = s
        .owner()
        .butler
        .pages()
        .get(&page_id)?
        .expect("owner should have page");
    let owner_permit = owner_page
        .permit
        .clone()
        .expect("owner page should have permit");
    let parsed_owner_permit = gurkha::Permit::from_token(&owner_permit)?;
    let expected_sync_layers = expected_sync_layers_from_permit(&parsed_owner_permit, &page_id);

    wait_until("owner sync-meta layer exists", Duration::from_secs(5), || {
        s.owner()
            .butler
            .store()
            .get_layer(&page_id, &sync_meta_layer)
            .ok()
            .flatten()
    })
    .await?;

    let owner_sync_meta_json = get_layer_json(s.owner(), &page_id, &sync_meta_layer).await?;
    assert_sync_meta_entries(&owner_sync_meta_json, &expected_sync_layers);

    wait_until("node sync-meta layer exists", Duration::from_secs(10), || {
        s.node()
            .butler
            .store()
            .get_all_layers(&page_id)
            .ok()
            .and_then(|layers| {
                layers
                    .into_iter()
                    .find(|(name, _)| name == &sync_meta_layer)
                    .map(|(_, data)| data)
            })
    })
    .await?;

    let node_sync_meta_json = get_layer_json(s.node(), &page_id, &sync_meta_layer).await?;
    assert_sync_meta_entries(&node_sync_meta_json, &expected_sync_layers);

    s.shutdown().await;
    Ok(())
}

/// Viewer add_website bootstrap creates and syncs protocol sync-meta layer to node.
#[tokio::test]
async fn diagnostic_viewer_add_website_bootstrap_sync_meta_to_node() -> Result<()> {
    init_tracing();

    let mut s = Scenario::builder()
        .app("osvauld-demos")
        .published()
        .viewers(1)
        .build()
        .await?;

    let page_id = s.space().page_id.clone();
    let space_id = s.space().space_id.clone();

    let viewer_link = s.get_viewer_link(&space_id).await?;
    s.add_viewer(0, &viewer_link, &page_id).await?;

    let viewer_did = s.viewer(0).butler.user_info().await?.did;
    let owner_did = s.owner().butler.user_info().await?.did;
    let viewer_sync_meta_layer = format!("__sync_meta/{}", viewer_did);
    let owner_sync_meta_layer = format!("__sync_meta/{}", owner_did);

    // Wait for viewer bootstrap sync-meta to reach node.
    wait_until("node has viewer sync-meta layer", Duration::from_secs(10), || {
        s.node()
            .butler
            .store()
            .get_layer(&page_id, &viewer_sync_meta_layer)
            .ok()
            .flatten()
    })
    .await?;

    // Viewer should not receive owner's sync-meta layer.
    let viewer_has_owner_sync_meta = s
        .viewer(0)
        .butler
        .store()
        .get_layer(&page_id, &owner_sync_meta_layer)?
        .is_some();
    assert!(
        !viewer_has_owner_sync_meta,
        "viewer must not receive owner's __sync_meta layer"
    );

    let viewer_page = s
        .viewer(0)
        .butler
        .pages()
        .get(&page_id)?
        .expect("viewer should have page");
    let viewer_permit = viewer_page
        .permit
        .clone()
        .expect("viewer page should have permit");
    let parsed_viewer_permit = gurkha::Permit::from_token(&viewer_permit)?;
    let expected_sync_layers = expected_sync_layers_from_permit(&parsed_viewer_permit, &page_id);

    let viewer_sync_meta_json = get_layer_json(s.viewer(0), &page_id, &viewer_sync_meta_layer).await?;
    assert_sync_meta_entries(&viewer_sync_meta_json, &expected_sync_layers);

    let node_sync_meta_json = get_layer_json(s.node(), &page_id, &viewer_sync_meta_layer).await?;
    assert_sync_meta_entries(&node_sync_meta_json, &expected_sync_layers);

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

/// Static channel layer (channels/general/messages) syncs via scribe:map()
///
/// **Bug coverage**: Default channel layers are now static in the permit template.
/// Writing via implicit creation (scribe:map) should sync because the layer is
/// declared in the permit's layers section — no create_layer() or DID-namespace needed.
#[tokio::test]
async fn test_static_channel_layer_syncs_to_node() -> Result<()> {
    init_tracing();

    let mut s = Scenario::builder()
        .app("osvauld-demos")
        .published()
        .build()
        .await?;

    let page_id = s.space().page_id.clone();
    sleep(Duration::from_millis(500)).await;

    // Owner opens page and writes to channels/general/messages (a static layer)
    let owner_scribe = s.owner().butler.open_page(&page_id).await?;

    let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
    owner_scribe.cast(ScribeMessage::EnsureLoroMap {
        layer_name: "channels/general/messages".to_string(),
        reply: reply_tx,
    }).map_err(|e| anyhow::anyhow!("Failed to ensure map: {:?}", e))?;
    reply_rx.await??;

    owner_scribe.cast(ScribeMessage::MapInsert {
        layer_name: "channels/general/messages".to_string(),
        path: String::new(),
        key: "msg1".to_string(),
        value: serde_json::json!({
            "id": "msg1",
            "text": "Hello from owner",
            "sender_did": "did:key:owner",
            "timestamp": 1234567890
        }),
    }).map_err(|e| anyhow::anyhow!("Failed to insert: {:?}", e))?;

    // Node should receive this because channels/general/messages is a static layer
    // in the permit template — no create_layer() needed
    let node_data = wait_for_layer_data(
        &s.node().butler,
        &page_id,
        "channels/general/messages",
        Duration::from_secs(10),
    ).await?;

    assert!(!node_data.is_empty(), "Node should receive static channel layer data");

    s.shutdown().await;
    Ok(())
}

/// Static channel layer syncs owner → node → viewer (full chain)
///
/// **Bug coverage**: Default channel messages (general) should sync to viewers
/// because they're static layers in the permit template. This is the main chat flow.
#[tokio::test]
async fn test_static_channel_layer_syncs_to_viewer() -> Result<()> {
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

    // Owner writes to static channel layer BEFORE viewer connects
    let owner_scribe = s.owner().butler.open_page(&page_id).await?;

    let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
    owner_scribe.cast(ScribeMessage::EnsureLoroMap {
        layer_name: "channels/general/messages".to_string(),
        reply: reply_tx,
    }).map_err(|e| anyhow::anyhow!("EnsureLoroMap failed: {:?}", e))?;
    reply_rx.await??;

    owner_scribe.cast(ScribeMessage::MapInsert {
        layer_name: "channels/general/messages".to_string(),
        path: String::new(),
        key: "msg1".to_string(),
        value: serde_json::json!({
            "id": "msg1",
            "text": "Hello from general channel",
            "sender_did": "did:key:owner",
            "timestamp": 1234567890
        }),
    }).map_err(|e| anyhow::anyhow!("MapInsert failed: {:?}", e))?;

    // Wait for data to reach node first
    wait_for_layer_data(
        &s.node().butler,
        &page_id,
        "channels/general/messages",
        Duration::from_secs(10),
    ).await?;

    // Now connect viewer
    let viewer_link = s.get_viewer_link(&space_id).await?;
    s.add_viewer(0, &viewer_link, &page_id).await?;

    // Wait for layer data to arrive on viewer (static layer — no special permit needed)
    let viewer_data = wait_for_layer_data(
        &s.viewer(0).butler,
        &page_id,
        "channels/general/messages",
        Duration::from_secs(10),
    ).await?;

    assert!(!viewer_data.is_empty(), "Viewer should receive static channel layer data");

    s.shutdown().await;
    Ok(())
}

/// Dynamic layer syncs from owner → node → viewer (full chain)
///
/// **Bug coverage**: Node receives dynamic layer from owner but doesn't forward to viewers
/// because SubscriberInfo.layer_permits is empty. The fix: emit_dynamic_layer_permits
/// adds permits to node's SubscriberInfo before emitting SyncEvent::NewDynamicLayer.
#[tokio::test]
async fn test_dynamic_layer_syncs_to_viewer() -> Result<()> {
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

    // Connect viewer first
    let viewer_link = s.get_viewer_link(&space_id).await?;
    s.add_viewer(0, &viewer_link, &page_id).await?;

    sleep(Duration::from_millis(500)).await;

    // Owner creates dynamic layer and writes data
    let owner_scribe = s.owner().butler.open_page(&page_id).await?;

    let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
    owner_scribe.cast(ScribeMessage::CreateDynamicLayer {
        schema_key: "channels/{id}/messages".to_string(),
        layer_id: "general".to_string(),
        reply: reply_tx,
    }).map_err(|e| anyhow::anyhow!("Failed to create dynamic layer: {:?}", e))?;

    let full_layer_name = reply_rx.await
        .map_err(|_| anyhow::anyhow!("CreateDynamicLayer channel closed"))?
        .map_err(|e| anyhow::anyhow!("CreateDynamicLayer failed: {}", e))?;

    let bare_layer = full_layer_name
        .strip_prefix(&format!("{}/", page_id))
        .unwrap_or(&full_layer_name);

    owner_scribe.cast(ScribeMessage::MapInsert {
        layer_name: bare_layer.to_string(),
        path: String::new(),
        key: "msg1".to_string(),
        value: serde_json::json!({
            "id": "msg1",
            "text": "Hello from dynamic channel",
            "sender_did": "did:key:owner",
            "timestamp": 1234567890
        }),
    }).map_err(|e| anyhow::anyhow!("Failed to insert: {:?}", e))?;

    // Wait for data to reach node first
    wait_for_layer_data(
        &s.node().butler,
        &page_id,
        bare_layer,
        Duration::from_secs(10),
    ).await?;

    // Now wait for data to reach viewer (node must issue permits and forward)
    let viewer_data = wait_for_layer_data(
        &s.viewer(0).butler,
        &page_id,
        bare_layer,
        Duration::from_secs(10),
    ).await?;

    assert!(!viewer_data.is_empty(), "Viewer should receive dynamic layer data via node relay");

    s.shutdown().await;
    Ok(())
}

/// Dynamic layer created via create_layer() syncs to node
///
/// **Context**: Proper flow — owner calls create_layer("channels/{id}/messages", "general")
/// which generates a DID-namespaced path and the schema fallback grants access.
#[tokio::test]
async fn test_dynamic_layer_create_layer_syncs_to_node() -> Result<()> {
    init_tracing();

    let mut s = Scenario::builder()
        .app("osvauld-demos")
        .published()
        .build()
        .await?;

    let page_id = s.space().page_id.clone();
    sleep(Duration::from_millis(500)).await;

    let owner_scribe = s.owner().butler.open_page(&page_id).await?;

    // Create dynamic layer using the proper API
    let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
    owner_scribe.cast(ScribeMessage::CreateDynamicLayer {
        schema_key: "channels/{id}/messages".to_string(),
        layer_id: "general".to_string(),
        reply: reply_tx,
    }).map_err(|e| anyhow::anyhow!("Failed to create dynamic layer: {:?}", e))?;

    let full_layer_name = reply_rx.await
        .map_err(|_| anyhow::anyhow!("CreateDynamicLayer channel closed"))?
        .map_err(|e| anyhow::anyhow!("CreateDynamicLayer failed: {}", e))?;

    // Extract bare layer name (strip page_id prefix)
    let bare_layer = full_layer_name
        .strip_prefix(&format!("{}/", page_id))
        .unwrap_or(&full_layer_name);

    // Write data to the dynamic layer
    owner_scribe.cast(ScribeMessage::MapInsert {
        layer_name: bare_layer.to_string(),
        path: String::new(),
        key: "msg1".to_string(),
        value: serde_json::json!({
            "id": "msg1",
            "text": "Hello from dynamic channel",
            "sender_did": "did:key:owner",
            "timestamp": 1234567890
        }),
    }).map_err(|e| anyhow::anyhow!("Failed to insert: {:?}", e))?;

    // Node should receive this because create_layer() sets up proper DID-namespaced path
    // and the schema fallback in can_access_with_layer_permits grants creator access
    let node_data = wait_for_layer_data(
        &s.node().butler,
        &page_id,
        bare_layer,
        Duration::from_secs(10),
    ).await?;

    assert!(!node_data.is_empty(), "Node should receive dynamic layer data created via create_layer()");

    s.shutdown().await;
    Ok(())
}

/// Node requests missing layer when sync-meta announces new dynamic layer.
///
/// **Flow**: Owner creates dynamic layer -> sync-meta entry arrives on node -> node sends SyncReset
/// to request full snapshot of the missing layer.
#[tokio::test]
async fn test_node_requests_missing_layer_from_sync_meta_update() -> Result<()> {
    init_tracing();

    let mut s = Scenario::builder()
        .app("osvauld-demos")
        .published()
        .with_tracer()
        .build()
        .await?;

    let page_id = s.space().page_id.clone();
    sleep(Duration::from_millis(500)).await;

    let owner_scribe = s.owner().butler.open_page(&page_id).await?;

    // Create dynamic layer only (no data write) so node learns about it via sync-meta first.
    let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
    owner_scribe.cast(ScribeMessage::CreateDynamicLayer {
        schema_key: "channels/{id}/messages".to_string(),
        layer_id: "sync-meta-request-check".to_string(),
        reply: reply_tx,
    }).map_err(|e| anyhow::anyhow!("Failed to create dynamic layer: {:?}", e))?;

    let full_layer_name = reply_rx.await
        .map_err(|_| anyhow::anyhow!("CreateDynamicLayer channel closed"))?
        .map_err(|e| anyhow::anyhow!("CreateDynamicLayer failed: {}", e))?;

    let bare_layer = full_layer_name
        .strip_prefix(&format!("{}/", page_id))
        .unwrap_or(&full_layer_name)
        .to_string();

    // Node should request this missing layer via SyncReset after applying sync-meta update.
    wait_until(
        "node sends SyncReset for newly announced dynamic layer",
        Duration::from_secs(8),
        || {
            let traces = s.tracer().messages();
            traces.into_iter().find(|t| {
                t.msg_name == "SyncReset"
                    && t.direction == TraceDirection::Sent
                    && t.node_id == s.node().node_id
                    && t.layer_name.as_deref() == Some(bare_layer.as_str())
            })
        },
    )
    .await?;

    s.shutdown().await;
    Ok(())
}

/// Node fans out sync-meta entries for recipients after receiving bundled layer authority.
#[tokio::test]
async fn test_node_fanout_sync_meta_from_layer_authority() -> Result<()> {
    init_tracing();

    let mut s = Scenario::builder()
        .app("osvauld-demos")
        .published()
        .viewers(1)
        .build()
        .await?;

    let page_id = s.space().page_id.clone();
    let space_id = s.space().space_id.clone();

    // Connect viewer so node has recipient permit context.
    let viewer_link = s.get_viewer_link(&space_id).await?;
    s.add_viewer(0, &viewer_link, &page_id).await?;
    sleep(Duration::from_millis(600)).await;

    let owner_scribe = s.owner().butler.open_page(&page_id).await?;
    let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
    owner_scribe.cast(ScribeMessage::CreateDynamicLayer {
        schema_key: "channels/{id}/messages".to_string(),
        layer_id: "fanout-check".to_string(),
        reply: reply_tx,
    }).map_err(|e| anyhow::anyhow!("Failed to create dynamic layer: {:?}", e))?;

    let full_layer_name = reply_rx.await
        .map_err(|_| anyhow::anyhow!("CreateDynamicLayer channel closed"))?
        .map_err(|e| anyhow::anyhow!("CreateDynamicLayer failed: {}", e))?;

    let bare_layer = full_layer_name
        .strip_prefix(&format!("{}/", page_id))
        .unwrap_or(&full_layer_name)
        .to_string();

    // Ensure layer changes are pushed (authority permit bundled on sync offer).
    owner_scribe.cast(ScribeMessage::MapInsert {
        layer_name: bare_layer.clone(),
        path: String::new(),
        key: "ping".to_string(),
        value: serde_json::json!({"ok": true}),
    }).map_err(|e| anyhow::anyhow!("Failed to write dynamic layer: {:?}", e))?;

    let viewer_did = s.viewer(0).butler.user_info().await?.did;
    let viewer_sync_meta_layer = format!("__sync_meta/{}", viewer_did);

    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    let mut found = false;
    while tokio::time::Instant::now() < deadline {
        if let Ok(sync_meta) = get_layer_json(s.node(), &page_id, &viewer_sync_meta_layer).await {
            if let Some(entry) = sync_meta.get(&bare_layer) {
                let synced = entry.get("synced").and_then(Value::as_bool).unwrap_or(true);
                if !synced {
                    found = true;
                    break;
                }
            }
        }
        sleep(Duration::from_millis(100)).await;
    }

    assert!(found, "node should write recipient sync-meta entry with synced=false");

    s.shutdown().await;
    Ok(())
}

/// Required viewer receives newly created dynamic layer data.
///
/// **Deterministic outcome check**: Viewer is already subscribed, owner creates a
/// new dynamic layer and writes data, viewer must materialize the layer and mark
/// its local sync-meta entry as synced=true.
#[tokio::test]
async fn test_required_viewer_receives_new_dynamic_layer_after_subscribe() -> Result<()> {
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

    // Subscribe the required viewer first.
    let viewer_link = s.get_viewer_link(&space_id).await?;
    s.add_viewer(0, &viewer_link, &page_id).await?;

    sleep(Duration::from_millis(500)).await;

    let owner_scribe = s.owner().butler.open_page(&page_id).await?;

    // Create a new dynamic channel layer.
    let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
    owner_scribe
        .cast(ScribeMessage::CreateDynamicLayer {
            schema_key: "channels/{id}/messages".to_string(),
            layer_id: "required-viewer".to_string(),
            reply: reply_tx,
        })
        .map_err(|e| anyhow::anyhow!("Failed to create dynamic layer: {:?}", e))?;

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
                "text": "hello required viewer",
                "sender_did": "did:key:owner",
                "timestamp": 1234567890
            }),
        })
        .map_err(|e| anyhow::anyhow!("Failed to write dynamic layer: {:?}", e))?;

    // Ensure node has authoritative copy.
    wait_for_layer_data(
        &s.node().butler,
        &page_id,
        &bare_layer,
        Duration::from_secs(10),
    )
    .await?;

    // Deterministic assertion #1: viewer materializes the new layer.
    let viewer_data = wait_for_layer_data(
        &s.viewer(0).butler,
        &page_id,
        &bare_layer,
        Duration::from_secs(12),
    )
    .await?;
    assert!(!viewer_data.is_empty(), "required viewer should receive new dynamic layer");

    // Deterministic assertion #2: viewer marks local sync-meta entry as synced=true.
    let viewer_did = s.viewer(0).butler.user_info().await?.did;
    let viewer_sync_meta_layer = format!("__sync_meta/{}", viewer_did);
    wait_until(
        "viewer marks dynamic layer synced=true",
        Duration::from_secs(8),
        || {
            if let Ok(sync_meta) = s
                .viewer(0)
                .butler
                .store()
                .get_layer(&page_id, &viewer_sync_meta_layer)
            {
                return sync_meta;
            }
            None
        },
    )
    .await?;

    let viewer_sync_meta_json = get_layer_json(s.viewer(0), &page_id, &viewer_sync_meta_layer).await?;
    let synced = viewer_sync_meta_json
        .get(&bare_layer)
        .and_then(|v| v.get("synced"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    assert!(synced, "required viewer sync-meta entry should be synced=true");

    s.shutdown().await;
    Ok(())
}

/// Custom channel: metadata syncs but message data should also sync
///
/// **Scenario**: Owner creates a custom channel via create_layer(), writes
/// channel metadata to channels_meta (static layer) and a message to the
/// dynamic channel layer.
///
/// **Expected**: Both the channel metadata AND message data sync to the viewer.
/// All user-created channels are shared with all members (DMs/private channels
/// are a future feature).
///
/// **Current bug**: channels_meta syncs (static layer) but the dynamic channel's
/// message data does not reach the viewer.
#[tokio::test]
async fn test_custom_channel_data_syncs_to_viewer() -> Result<()> {
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

    // Connect viewer first
    let viewer_link = s.get_viewer_link(&space_id).await?;
    s.add_viewer(0, &viewer_link, &page_id).await?;

    sleep(Duration::from_millis(500)).await;

    // Owner opens page
    let owner_scribe = s.owner().butler.open_page(&page_id).await?;

    // 1. Owner creates a custom channel via create_layer
    let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
    owner_scribe.cast(ScribeMessage::CreateDynamicLayer {
        schema_key: "channels/{id}/messages".to_string(),
        layer_id: "project-updates".to_string(),
        reply: reply_tx,
    }).map_err(|e| anyhow::anyhow!("CreateDynamicLayer failed: {:?}", e))?;

    let full_layer_name = reply_rx.await
        .map_err(|_| anyhow::anyhow!("CreateDynamicLayer channel closed"))?
        .map_err(|e| anyhow::anyhow!("CreateDynamicLayer failed: {}", e))?;

    let bare_channel_layer = full_layer_name
        .strip_prefix(&format!("{}/", page_id))
        .unwrap_or(&full_layer_name);

    // 2. Owner writes channel metadata to channels_meta (static layer)
    let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
    owner_scribe.cast(ScribeMessage::EnsureLoroMap {
        layer_name: "channels_meta".to_string(),
        reply: reply_tx,
    }).map_err(|e| anyhow::anyhow!("EnsureLoroMap failed: {:?}", e))?;
    reply_rx.await??;

    owner_scribe.cast(ScribeMessage::MapInsert {
        layer_name: "channels_meta".to_string(),
        path: String::new(),
        key: "project-updates".to_string(),
        value: serde_json::json!({
            "id": "project-updates",
            "name": "project-updates",
            "topic": "",
            "created_by": "did:key:owner",
            "layer_path": bare_channel_layer,
        }),
    }).map_err(|e| anyhow::anyhow!("MapInsert channels_meta failed: {:?}", e))?;

    // 3. Owner writes a message to the dynamic channel
    owner_scribe.cast(ScribeMessage::MapInsert {
        layer_name: bare_channel_layer.to_string(),
        path: String::new(),
        key: "msg1".to_string(),
        value: serde_json::json!({
            "id": "msg1",
            "text": "First message in custom channel",
            "sender_did": "did:key:owner",
            "timestamp": 1234567890
        }),
    }).map_err(|e| anyhow::anyhow!("MapInsert message failed: {:?}", e))?;

    // 4. Verify channels_meta syncs to viewer (static layer — should work)
    let viewer_meta = wait_for_layer_data(
        &s.viewer(0).butler,
        &page_id,
        "channels_meta",
        Duration::from_secs(10),
    ).await?;

    assert!(!viewer_meta.is_empty(), "Viewer should receive channels_meta (channel metadata)");

    // 5. Verify message data syncs to viewer (dynamic layer — currently broken)
    let viewer_data = wait_for_layer_data(
        &s.viewer(0).butler,
        &page_id,
        bare_channel_layer,
        Duration::from_secs(10),
    ).await?;

    assert!(!viewer_data.is_empty(), "Viewer should receive custom channel message data");

    s.shutdown().await;
    Ok(())
}

/// Offline subscriber receives layer permit + layer data at subscribe time.
#[tokio::test]
async fn test_offline_viewer_gets_layer_on_subscribe_via_authority_permit() -> Result<()> {
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

    let owner_scribe = s.owner().butler.open_page(&page_id).await?;

    let (create_tx, create_rx) = tokio::sync::oneshot::channel();
    owner_scribe
        .cast(ScribeMessage::CreateDynamicLayer {
            schema_key: "channels/{id}/messages".to_string(),
            layer_id: "late-subscriber".to_string(),
            reply: create_tx,
        })
        .map_err(|e| anyhow::anyhow!("CreateDynamicLayer failed: {:?}", e))?;

    let full_layer_name = create_rx
        .await
        .map_err(|_| anyhow::anyhow!("CreateDynamicLayer channel closed"))?
        .map_err(|e| anyhow::anyhow!("CreateDynamicLayer failed: {}", e))?;

    let bare_layer_name = full_layer_name
        .strip_prefix(&format!("{}/", page_id))
        .unwrap_or(&full_layer_name)
        .to_string();

    owner_scribe
        .cast(ScribeMessage::MapInsert {
            layer_name: bare_layer_name.clone(),
            path: String::new(),
            key: "msg1".to_string(),
            value: serde_json::json!({
                "id": "msg1",
                "text": "hello offline viewer",
                "sender_did": "did:key:owner",
                "timestamp": 1234567890
            }),
        })
        .map_err(|e| anyhow::anyhow!("MapInsert failed: {:?}", e))?;

    wait_for_layer_data(
        &s.node().butler,
        &page_id,
        &bare_layer_name,
        Duration::from_secs(10),
    )
    .await?;

    let viewer_did = s.viewer(0).butler.user_info().await?.did;
    let owner_signing_key = s.owner().butler.signing_key().await?;
    let owner_page = s
        .owner()
        .butler
        .pages()
        .get(&page_id)?
        .expect("owner should have page");
    let owner_page_permit = owner_page
        .permit
        .clone()
        .expect("owner page should have permit");

    let (authority_token, _) = gurkha::issue_layer_permit(
        &owner_signing_key,
        &owner_page_permit,
        &viewer_did,
        &page_id,
        &full_layer_name,
        gurkha::LayerConfig {
            sync: true,
            write: true,
            layer_type: Some("map".to_string()),
        },
        None,
    )
    .await?;

    s.node().butler.permits().store_layer_authority_permit(
        &page_id,
        &full_layer_name,
        &viewer_did,
        1,
        &authority_token,
    )?;

    let viewer_link = s.get_viewer_link(&space_id).await?;
    s.add_viewer(0, &viewer_link, &page_id).await?;

    let permit_deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    loop {
        let has_permit = s
            .node()
            .butler
            .permits()
            .has_layer_permit(&page_id, &viewer_did, &full_layer_name)
            .unwrap_or(false);
        if has_permit {
            break;
        }
        if tokio::time::Instant::now() > permit_deadline {
            return Err(anyhow::anyhow!(
                "Node did not issue layer permit on subscribe for {}",
                full_layer_name
            ));
        }
        sleep(Duration::from_millis(100)).await;
    }

    // Trigger a post-subscribe update so the newly authorized layer gets delivered.
    owner_scribe
        .cast(ScribeMessage::MapInsert {
            layer_name: bare_layer_name.clone(),
            path: String::new(),
            key: "msg2".to_string(),
            value: serde_json::json!({
                "id": "msg2",
                "text": "post-subscribe message",
                "sender_did": "did:key:owner",
                "timestamp": 1234567891
            }),
        })
        .map_err(|e| anyhow::anyhow!("MapInsert post-subscribe failed: {:?}", e))?;

    let viewer_data = wait_for_layer_data(
        &s.viewer(0).butler,
        &page_id,
        &bare_layer_name,
        Duration::from_secs(10),
    )
    .await?;

    assert!(!viewer_data.is_empty());

    let owner_len_before_reply = wait_for_layer_data(
        &s.owner().butler,
        &page_id,
        &bare_layer_name,
        Duration::from_secs(10),
    )
    .await?
    .len();

    let viewer_scribe = s.viewer(0).butler.open_page(&page_id).await?;
    viewer_scribe
        .cast(ScribeMessage::MapInsert {
            layer_name: bare_layer_name.clone(),
            path: String::new(),
            key: "msg3".to_string(),
            value: serde_json::json!({
                "id": "msg3",
                "text": "viewer reply",
                "sender_did": "did:key:viewer",
                "timestamp": 1234567892
            }),
        })
        .map_err(|e| anyhow::anyhow!("Viewer MapInsert failed: {:?}", e))?;

    let reply_deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    loop {
        let owner_bytes = wait_for_layer_data(
            &s.owner().butler,
            &page_id,
            &bare_layer_name,
            Duration::from_secs(2),
        )
        .await?;

        if owner_bytes.len() > owner_len_before_reply {
            break;
        }

        if tokio::time::Instant::now() > reply_deadline {
            return Err(anyhow::anyhow!(
                "Owner did not receive viewer reply on {}",
                bare_layer_name
            ));
        }

        sleep(Duration::from_millis(100)).await;
    }

    s.shutdown().await;
    Ok(())
}

/// AddLayerAccess issues permit when authority permit exists and peer is subscribed.
#[tokio::test]
async fn test_add_layer_access_issues_permit_from_authority() -> Result<()> {
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

    let viewer_link = s.get_viewer_link(&space_id).await?;
    s.add_viewer(0, &viewer_link, &page_id).await?;

    let owner_scribe = s.owner().butler.open_page(&page_id).await?;

    let (create_tx, create_rx) = tokio::sync::oneshot::channel();
    owner_scribe
        .cast(ScribeMessage::CreateDynamicLayer {
            schema_key: "dms/{id}/messages".to_string(),
            layer_id: "add-access".to_string(),
            reply: create_tx,
        })
        .map_err(|e| anyhow::anyhow!("CreateDynamicLayer failed: {:?}", e))?;

    let full_layer_name = create_rx
        .await
        .map_err(|_| anyhow::anyhow!("CreateDynamicLayer channel closed"))?
        .map_err(|e| anyhow::anyhow!("CreateDynamicLayer failed: {}", e))?;

    let bare_layer_name = full_layer_name
        .strip_prefix(&format!("{}/", page_id))
        .unwrap_or(&full_layer_name)
        .to_string();

    owner_scribe
        .cast(ScribeMessage::MapInsert {
            layer_name: bare_layer_name.clone(),
            path: String::new(),
            key: "msg1".to_string(),
            value: serde_json::json!({
                "id": "msg1",
                "text": "before add access",
                "sender_did": "did:key:owner",
                "timestamp": 1234567890
            }),
        })
        .map_err(|e| anyhow::anyhow!("MapInsert failed: {:?}", e))?;

    wait_for_layer_data(
        &s.node().butler,
        &page_id,
        &bare_layer_name,
        Duration::from_secs(10),
    )
    .await?;

    let viewer_did = s.viewer(0).butler.user_info().await?.did;
    let owner_signing_key = s.owner().butler.signing_key().await?;
    let owner_page = s
        .owner()
        .butler
        .pages()
        .get(&page_id)?
        .expect("owner should have page");
    let owner_page_permit = owner_page
        .permit
        .clone()
        .expect("owner page should have permit");

    let (authority_token, _) = gurkha::issue_layer_permit(
        &owner_signing_key,
        &owner_page_permit,
        &viewer_did,
        &page_id,
        &full_layer_name,
        gurkha::LayerConfig {
            sync: true,
            write: true,
            layer_type: Some("map".to_string()),
        },
        None,
    )
    .await?;

    s.node().butler.permits().store_layer_authority_permit(
        &page_id,
        &full_layer_name,
        &viewer_did,
        1,
        &authority_token,
    )?;

    let node_scribe = s.node().butler.open_page(&page_id).await?;
    let (add_tx, add_rx) = tokio::sync::oneshot::channel();
    node_scribe
        .cast(ScribeMessage::AddLayerAccess {
            layer_name: full_layer_name.clone(),
            did: viewer_did.clone(),
            reply: add_tx,
        })
        .map_err(|e| anyhow::anyhow!("AddLayerAccess cast failed: {:?}", e))?;

    add_rx
        .await
        .map_err(|_| anyhow::anyhow!("AddLayerAccess channel closed"))?
        .map_err(|e| anyhow::anyhow!("AddLayerAccess failed: {}", e))?;

    let permit_deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    loop {
        let has_permit = s
            .node()
            .butler
            .permits()
            .has_layer_permit(&page_id, &viewer_did, &full_layer_name)
            .unwrap_or(false);
        if has_permit {
            break;
        }
        if tokio::time::Instant::now() > permit_deadline {
            return Err(anyhow::anyhow!(
                "Node did not issue layer permit after AddLayerAccess for {}",
                full_layer_name
            ));
        }
        sleep(Duration::from_millis(100)).await;
    }

    owner_scribe
        .cast(ScribeMessage::MapInsert {
            layer_name: bare_layer_name.clone(),
            path: String::new(),
            key: "msg2".to_string(),
            value: serde_json::json!({
                "id": "msg2",
                "text": "after add access",
                "sender_did": "did:key:owner",
                "timestamp": 1234567891
            }),
        })
        .map_err(|e| anyhow::anyhow!("MapInsert post-add-access failed: {:?}", e))?;

    let viewer_data = wait_for_layer_data(
        &s.viewer(0).butler,
        &page_id,
        &bare_layer_name,
        Duration::from_secs(10),
    )
    .await?;

    assert!(!viewer_data.is_empty());

    s.shutdown().await;
    Ok(())
}

/// Creator-side AddLayerAccess should issue and store a layer permit locally.
#[tokio::test]
async fn test_creator_add_layer_access_issues_local_permit() -> Result<()> {
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

    let viewer_link = s.get_viewer_link(&space_id).await?;
    s.add_viewer(0, &viewer_link, &page_id).await?;

    let owner_scribe = s.owner().butler.open_page(&page_id).await?;

    let (create_tx, create_rx) = tokio::sync::oneshot::channel();
    owner_scribe
        .cast(ScribeMessage::CreateDynamicLayer {
            schema_key: "dms/{id}/messages".to_string(),
            layer_id: "owner-self-issue".to_string(),
            reply: create_tx,
        })
        .map_err(|e| anyhow::anyhow!("CreateDynamicLayer failed: {:?}", e))?;

    let full_layer_name = create_rx
        .await
        .map_err(|_| anyhow::anyhow!("CreateDynamicLayer channel closed"))?
        .map_err(|e| anyhow::anyhow!("CreateDynamicLayer failed: {}", e))?;

    let viewer_did = s.viewer(0).butler.user_info().await?.did;

    let (add_tx, add_rx) = tokio::sync::oneshot::channel();
    owner_scribe
        .cast(ScribeMessage::AddLayerAccess {
            layer_name: full_layer_name.clone(),
            did: viewer_did.clone(),
            reply: add_tx,
        })
        .map_err(|e| anyhow::anyhow!("AddLayerAccess cast failed: {:?}", e))?;

    add_rx
        .await
        .map_err(|_| anyhow::anyhow!("AddLayerAccess channel closed"))?
        .map_err(|e| anyhow::anyhow!("AddLayerAccess failed: {}", e))?;

    let permit_deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    loop {
        let has_local_permit = s
            .owner()
            .butler
            .permits()
            .has_layer_permit(&page_id, &viewer_did, &full_layer_name)
            .unwrap_or(false);
        if has_local_permit {
            break;
        }
        if tokio::time::Instant::now() > permit_deadline {
            return Err(anyhow::anyhow!(
                "Owner did not issue/store local layer permit after AddLayerAccess for {}",
                full_layer_name
            ));
        }
        sleep(Duration::from_millis(100)).await;
    }

    s.shutdown().await;
    Ok(())
}

/// Diagnostic (temporary): explicit custom layer with multiple participant DIDs
/// should produce node-issued layer permits once authority facts are present.
#[tokio::test]
async fn diagnostic_custom_layer_multiple_dids_issue_permits_from_authority_facts() -> Result<()> {
    init_tracing();

    let mut s = Scenario::builder()
        .app("osvauld-demos")
        .published()
        .viewers(2)
        .build()
        .await?;

    let page_id = s.space().page_id.clone();
    let space_id = s.space().space_id.clone();

    sleep(Duration::from_millis(500)).await;

    let viewer_link = s.get_viewer_link(&space_id).await?;
    s.add_viewer(0, &viewer_link, &page_id).await?;
    s.add_viewer(1, &viewer_link, &page_id).await?;

    let owner_scribe = s.owner().butler.open_page(&page_id).await?;

    let (create_tx, create_rx) = tokio::sync::oneshot::channel();
    owner_scribe
        .cast(ScribeMessage::CreateDynamicLayer {
            schema_key: "dms/{id}/messages".to_string(),
            layer_id: "diagnostic-multi-dids".to_string(),
            reply: create_tx,
        })
        .map_err(|e| anyhow::anyhow!("CreateDynamicLayer failed: {:?}", e))?;

    let full_layer_name = create_rx
        .await
        .map_err(|_| anyhow::anyhow!("CreateDynamicLayer channel closed"))?
        .map_err(|e| anyhow::anyhow!("CreateDynamicLayer failed: {}", e))?;

    let bare_layer_name = full_layer_name
        .strip_prefix(&format!("{}/", page_id))
        .unwrap_or(&full_layer_name)
        .to_string();

    owner_scribe
        .cast(ScribeMessage::MapInsert {
            layer_name: bare_layer_name.clone(),
            path: String::new(),
            key: "msg1".to_string(),
            value: serde_json::json!({
                "id": "msg1",
                "text": "seed message",
                "sender_did": "did:key:owner",
                "timestamp": 1234567890
            }),
        })
        .map_err(|e| anyhow::anyhow!("MapInsert failed: {:?}", e))?;

    wait_for_layer_data(
        &s.node().butler,
        &page_id,
        &bare_layer_name,
        Duration::from_secs(10),
    )
    .await?;

    let audience_dids = vec![
        s.viewer(0).butler.user_info().await?.did,
        s.viewer(1).butler.user_info().await?.did,
    ];

    let owner_signing_key = s.owner().butler.signing_key().await?;
    let owner_page = s
        .owner()
        .butler
        .pages()
        .get(&page_id)?
        .expect("owner should have page");
    let owner_page_permit = owner_page
        .permit
        .clone()
        .expect("owner page should have permit");

    for did in &audience_dids {
        let (authority_token, _) = gurkha::issue_layer_permit(
            &owner_signing_key,
            &owner_page_permit,
            did,
            &page_id,
            &full_layer_name,
            gurkha::LayerConfig {
                sync: true,
                write: true,
                layer_type: Some("map".to_string()),
            },
            None,
        )
        .await?;

        s.node().butler.permits().store_layer_authority_permit(
            &page_id,
            &full_layer_name,
            did,
            1,
            &authority_token,
        )?;

        let has_authority = s
            .node()
            .butler
            .permits()
            .get_layer_authority_permit(&page_id, &full_layer_name, did)?
            .is_some();
        assert!(
            has_authority,
            "Node should store authority fact for audience {did}"
        );
    }

    let node_scribe = s.node().butler.open_page(&page_id).await?;
    for did in &audience_dids {
        let (add_tx, add_rx) = tokio::sync::oneshot::channel();
        node_scribe
            .cast(ScribeMessage::AddLayerAccess {
                layer_name: full_layer_name.clone(),
                did: did.clone(),
                reply: add_tx,
            })
            .map_err(|e| anyhow::anyhow!("AddLayerAccess cast failed: {:?}", e))?;

        add_rx
            .await
            .map_err(|_| anyhow::anyhow!("AddLayerAccess channel closed"))?
            .map_err(|e| anyhow::anyhow!("AddLayerAccess failed: {}", e))?;
    }

    let permit_deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    loop {
        let issued_for_all = audience_dids.iter().all(|did| {
            s.node()
                .butler
                .permits()
                .has_layer_permit(&page_id, did, &full_layer_name)
                .unwrap_or(false)
        });

        if issued_for_all {
            break;
        }

        if tokio::time::Instant::now() > permit_deadline {
            return Err(anyhow::anyhow!(
                "Node did not issue layer permits for all participant DIDs"
            ));
        }

        sleep(Duration::from_millis(100)).await;
    }

    owner_scribe
        .cast(ScribeMessage::MapInsert {
            layer_name: bare_layer_name.clone(),
            path: String::new(),
            key: "msg2".to_string(),
            value: serde_json::json!({
                "id": "msg2",
                "text": "post-access message",
                "sender_did": "did:key:owner",
                "timestamp": 1234567891
            }),
        })
        .map_err(|e| anyhow::anyhow!("MapInsert post-access failed: {:?}", e))?;

    let viewer0_data = wait_for_layer_data(
        &s.viewer(0).butler,
        &page_id,
        &bare_layer_name,
        Duration::from_secs(10),
    )
    .await?;
    assert!(
        !viewer0_data.is_empty(),
        "Authorized viewer0 should receive dynamic layer data"
    );

    let viewer1_data = wait_for_layer_data(
        &s.viewer(1).butler,
        &page_id,
        &bare_layer_name,
        Duration::from_secs(10),
    )
    .await?;
    assert!(
        !viewer1_data.is_empty(),
        "Authorized viewer1 should receive dynamic layer data"
    );

    s.shutdown().await;
    Ok(())
}

/// Regression: DM layer created by owner should be visible to online participant.
///
/// This captures the current bug from e2e DM flow where Alice creates a DM layer,
/// writes a message, and Bob does not receive the layer/message.
#[tokio::test]
async fn test_dm_owner_created_layer_visible_to_online_participant() -> Result<()> {
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

    let viewer_link = s.get_viewer_link(&space_id).await?;
    s.add_viewer(0, &viewer_link, &page_id).await?;

    sleep(Duration::from_millis(500)).await;

    let owner_scribe = s.owner().butler.open_page(&page_id).await?;

    let (create_tx, create_rx) = tokio::sync::oneshot::channel();
    owner_scribe
        .cast(ScribeMessage::CreateDynamicLayer {
            schema_key: "dms/{id}/messages".to_string(),
            layer_id: "alice-bob".to_string(),
            reply: create_tx,
        })
        .map_err(|e| anyhow::anyhow!("CreateDynamicLayer failed: {:?}", e))?;

    let full_layer_name = create_rx
        .await
        .map_err(|_| anyhow::anyhow!("CreateDynamicLayer channel closed"))?
        .map_err(|e| anyhow::anyhow!("CreateDynamicLayer failed: {}", e))?;

    let bare_layer_name = full_layer_name
        .strip_prefix(&format!("{}/", page_id))
        .unwrap_or(&full_layer_name)
        .to_string();

    owner_scribe
        .cast(ScribeMessage::MapInsert {
            layer_name: bare_layer_name.clone(),
            path: String::new(),
            key: "msg1".to_string(),
            value: serde_json::json!({
                "id": "msg1",
                "text": "hello bob",
                "sender_did": "did:key:owner",
                "timestamp": 1234567890
            }),
        })
        .map_err(|e| anyhow::anyhow!("MapInsert failed: {:?}", e))?;

    wait_for_layer_data(
        &s.node().butler,
        &page_id,
        &bare_layer_name,
        Duration::from_secs(10),
    )
    .await?;

    let viewer_data = wait_for_layer_data(
        &s.viewer(0).butler,
        &page_id,
        &bare_layer_name,
        Duration::from_secs(10),
    )
    .await?;

    assert!(
        !viewer_data.is_empty(),
        "Bob should receive owner-created DM layer data"
    );

    s.shutdown().await;
    Ok(())
}

/// Regression: offline participant should receive owner-created DM layer on subscribe.
///
/// This captures the current bug from e2e DM flow for an offline participant who
/// reconnects after the DM layer was created.
#[tokio::test]
async fn test_dm_offline_participant_receives_layer_on_subscribe() -> Result<()> {
    init_tracing();

    let mut s = Scenario::builder()
        .app("osvauld-demos")
        .published()
        .viewers(2)
        .build()
        .await?;

    let page_id = s.space().page_id.clone();
    let space_id = s.space().space_id.clone();

    sleep(Duration::from_millis(500)).await;

    // Keep viewer1 offline; connect only viewer0 first.
    let viewer_link = s.get_viewer_link(&space_id).await?;
    s.add_viewer(0, &viewer_link, &page_id).await?;

    sleep(Duration::from_millis(500)).await;

    let owner_scribe = s.owner().butler.open_page(&page_id).await?;

    let (create_tx, create_rx) = tokio::sync::oneshot::channel();
    owner_scribe
        .cast(ScribeMessage::CreateDynamicLayer {
            schema_key: "dms/{id}/messages".to_string(),
            layer_id: "alice-bob-carol".to_string(),
            reply: create_tx,
        })
        .map_err(|e| anyhow::anyhow!("CreateDynamicLayer failed: {:?}", e))?;

    let full_layer_name = create_rx
        .await
        .map_err(|_| anyhow::anyhow!("CreateDynamicLayer channel closed"))?
        .map_err(|e| anyhow::anyhow!("CreateDynamicLayer failed: {}", e))?;

    let bare_layer_name = full_layer_name
        .strip_prefix(&format!("{}/", page_id))
        .unwrap_or(&full_layer_name)
        .to_string();

    owner_scribe
        .cast(ScribeMessage::MapInsert {
            layer_name: bare_layer_name.clone(),
            path: String::new(),
            key: "msg1".to_string(),
            value: serde_json::json!({
                "id": "msg1",
                "text": "hello offline carol",
                "sender_did": "did:key:owner",
                "timestamp": 1234567890
            }),
        })
        .map_err(|e| anyhow::anyhow!("MapInsert failed: {:?}", e))?;

    wait_for_layer_data(
        &s.node().butler,
        &page_id,
        &bare_layer_name,
        Duration::from_secs(10),
    )
    .await?;

    // Carol subscribes after layer creation.
    s.add_viewer(1, &viewer_link, &page_id).await?;

    let offline_viewer_data = wait_for_layer_data(
        &s.viewer(1).butler,
        &page_id,
        &bare_layer_name,
        Duration::from_secs(10),
    )
    .await?;

    assert!(
        !offline_viewer_data.is_empty(),
        "Offline participant should receive owner-created DM layer data on subscribe"
    );

    s.shutdown().await;
    Ok(())
}

/// Diagnostic: explicit DM layer is not shared until access is granted.
///
/// Confirms the current gap by asserting that a connected participant does not
/// receive owner-created DM layer data and node has no issued layer permit.
#[tokio::test]
async fn diagnostic_dm_explicit_layer_requires_access_grant_for_online_participant() -> Result<()> {
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

    let viewer_link = s.get_viewer_link(&space_id).await?;
    s.add_viewer(0, &viewer_link, &page_id).await?;

    sleep(Duration::from_millis(500)).await;

    let owner_scribe = s.owner().butler.open_page(&page_id).await?;

    let (create_tx, create_rx) = tokio::sync::oneshot::channel();
    owner_scribe
        .cast(ScribeMessage::CreateDynamicLayer {
            schema_key: "dms/{id}/messages".to_string(),
            layer_id: "diag-online".to_string(),
            reply: create_tx,
        })
        .map_err(|e| anyhow::anyhow!("CreateDynamicLayer failed: {:?}", e))?;

    let full_layer_name = create_rx
        .await
        .map_err(|_| anyhow::anyhow!("CreateDynamicLayer channel closed"))?
        .map_err(|e| anyhow::anyhow!("CreateDynamicLayer failed: {}", e))?;

    let bare_layer_name = full_layer_name
        .strip_prefix(&format!("{}/", page_id))
        .unwrap_or(&full_layer_name)
        .to_string();

    owner_scribe
        .cast(ScribeMessage::MapInsert {
            layer_name: bare_layer_name.clone(),
            path: String::new(),
            key: "msg1".to_string(),
            value: serde_json::json!({
                "id": "msg1",
                "text": "diagnostic message",
                "sender_did": "did:key:owner",
                "timestamp": 1234567890
            }),
        })
        .map_err(|e| anyhow::anyhow!("MapInsert failed: {:?}", e))?;

    wait_for_layer_data(
        &s.node().butler,
        &page_id,
        &bare_layer_name,
        Duration::from_secs(10),
    )
    .await?;

    let owner_did = s.owner().butler.user_info().await?.did;
    let node_did = s.node().butler.user_info().await?.did;
    let viewer_did = s.viewer(0).butler.user_info().await?.did;

    let owner_has_local_layer_permit = s
        .owner()
        .butler
        .permits()
        .has_layer_permit(&page_id, &owner_did, &full_layer_name)
        .unwrap_or(false);
    let node_has_owner_layer_permit = s
        .node()
        .butler
        .permits()
        .has_layer_permit(&page_id, &owner_did, &full_layer_name)
        .unwrap_or(false);
    let node_has_node_layer_permit = s
        .node()
        .butler
        .permits()
        .has_layer_permit(&page_id, &node_did, &full_layer_name)
        .unwrap_or(false);
    let node_has_viewer_layer_permit = s
        .node()
        .butler
        .permits()
        .has_layer_permit(&page_id, &viewer_did, &full_layer_name)
        .unwrap_or(false);

    let node_has_owner_authority = s
        .node()
        .butler
        .permits()
        .get_layer_authority_permit(&page_id, &full_layer_name, &owner_did)?
        .is_some();
    let node_has_viewer_authority = s
        .node()
        .butler
        .permits()
        .get_layer_authority_permit(&page_id, &full_layer_name, &viewer_did)?
        .is_some();

    info!(
        page_id = %page_id,
        layer = %full_layer_name,
        owner_did = %owner_did,
        node_did = %node_did,
        viewer_did = %viewer_did,
        owner_has_local_layer_permit,
        node_has_owner_layer_permit,
        node_has_node_layer_permit,
        node_has_viewer_layer_permit,
        node_has_owner_authority,
        node_has_viewer_authority,
        "DM explicit diagnostic permit snapshot"
    );

    let viewer_data = wait_for_layer_data(
        &s.viewer(0).butler,
        &page_id,
        &bare_layer_name,
        Duration::from_secs(3),
    )
    .await;
    assert!(
        viewer_data.is_err(),
        "Viewer should not receive explicit DM data before access grant"
    );

    let has_layer_permit = s
        .node()
        .butler
        .permits()
        .has_layer_permit(&page_id, &viewer_did, &full_layer_name)
        .unwrap_or(false);
    assert!(
        !has_layer_permit,
        "Node should not have issued DM layer permit without explicit grant"
    );

    s.shutdown().await;
    Ok(())
}

/// Diagnostic: disconnected participant does not get DM layer on subscribe
/// without pre-stored authority permit.
#[tokio::test]
async fn diagnostic_dm_offline_subscriber_without_authority_does_not_receive_layer() -> Result<()> {
    init_tracing();

    let mut s = Scenario::builder()
        .app("osvauld-demos")
        .published()
        .viewers(2)
        .build()
        .await?;

    let page_id = s.space().page_id.clone();
    let space_id = s.space().space_id.clone();

    sleep(Duration::from_millis(500)).await;

    let viewer_link = s.get_viewer_link(&space_id).await?;
    s.add_viewer(0, &viewer_link, &page_id).await?;

    sleep(Duration::from_millis(500)).await;

    let owner_scribe = s.owner().butler.open_page(&page_id).await?;

    let (create_tx, create_rx) = tokio::sync::oneshot::channel();
    owner_scribe
        .cast(ScribeMessage::CreateDynamicLayer {
            schema_key: "dms/{id}/messages".to_string(),
            layer_id: "diag-offline".to_string(),
            reply: create_tx,
        })
        .map_err(|e| anyhow::anyhow!("CreateDynamicLayer failed: {:?}", e))?;

    let full_layer_name = create_rx
        .await
        .map_err(|_| anyhow::anyhow!("CreateDynamicLayer channel closed"))?
        .map_err(|e| anyhow::anyhow!("CreateDynamicLayer failed: {}", e))?;

    let bare_layer_name = full_layer_name
        .strip_prefix(&format!("{}/", page_id))
        .unwrap_or(&full_layer_name)
        .to_string();

    owner_scribe
        .cast(ScribeMessage::MapInsert {
            layer_name: bare_layer_name.clone(),
            path: String::new(),
            key: "msg1".to_string(),
            value: serde_json::json!({
                "id": "msg1",
                "text": "diagnostic offline message",
                "sender_did": "did:key:owner",
                "timestamp": 1234567890
            }),
        })
        .map_err(|e| anyhow::anyhow!("MapInsert failed: {:?}", e))?;

    wait_for_layer_data(
        &s.node().butler,
        &page_id,
        &bare_layer_name,
        Duration::from_secs(10),
    )
    .await?;

    s.add_viewer(1, &viewer_link, &page_id).await?;

    let offline_data = wait_for_layer_data(
        &s.viewer(1).butler,
        &page_id,
        &bare_layer_name,
        Duration::from_secs(3),
    )
    .await;
    assert!(
        offline_data.is_err(),
        "Offline subscriber should not receive explicit DM data without authority permit"
    );

    let offline_did = s.viewer(1).butler.user_info().await?.did;
    let has_layer_permit = s
        .node()
        .butler
        .permits()
        .has_layer_permit(&page_id, &offline_did, &full_layer_name)
        .unwrap_or(false);
    assert!(
        !has_layer_permit,
        "Node should not issue offline subscriber DM layer permit without authority"
    );

    s.shutdown().await;
    Ok(())
}

/// Diagnostic: two peers creating the same logical DM id create different
/// creator-namespaced physical layers.
#[tokio::test]
async fn diagnostic_dm_same_logical_id_creates_distinct_creator_namespaced_layers() -> Result<()> {
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

    let viewer_link = s.get_viewer_link(&space_id).await?;
    s.add_viewer(0, &viewer_link, &page_id).await?;

    sleep(Duration::from_millis(500)).await;

    let owner_scribe = s.owner().butler.open_page(&page_id).await?;
    let viewer_scribe = s.viewer(0).butler.open_page(&page_id).await?;

    let (owner_tx, owner_rx) = tokio::sync::oneshot::channel();
    owner_scribe
        .cast(ScribeMessage::CreateDynamicLayer {
            schema_key: "dms/{id}/messages".to_string(),
            layer_id: "same-dm-id".to_string(),
            reply: owner_tx,
        })
        .map_err(|e| anyhow::anyhow!("Owner CreateDynamicLayer failed: {:?}", e))?;

    let owner_full_layer = owner_rx
        .await
        .map_err(|_| anyhow::anyhow!("Owner CreateDynamicLayer channel closed"))?
        .map_err(|e| anyhow::anyhow!("Owner CreateDynamicLayer failed: {}", e))?;

    let (viewer_tx, viewer_rx) = tokio::sync::oneshot::channel();
    viewer_scribe
        .cast(ScribeMessage::CreateDynamicLayer {
            schema_key: "dms/{id}/messages".to_string(),
            layer_id: "same-dm-id".to_string(),
            reply: viewer_tx,
        })
        .map_err(|e| anyhow::anyhow!("Viewer CreateDynamicLayer failed: {:?}", e))?;

    let viewer_full_layer = viewer_rx
        .await
        .map_err(|_| anyhow::anyhow!("Viewer CreateDynamicLayer channel closed"))?
        .map_err(|e| anyhow::anyhow!("Viewer CreateDynamicLayer failed: {}", e))?;

    assert_ne!(
        owner_full_layer, viewer_full_layer,
        "Same logical DM id should currently resolve to different creator-namespaced layers"
    );

    let owner_bare_layer = owner_full_layer
        .strip_prefix(&format!("{}/", page_id))
        .unwrap_or(&owner_full_layer)
        .to_string();
    let viewer_bare_layer = viewer_full_layer
        .strip_prefix(&format!("{}/", page_id))
        .unwrap_or(&viewer_full_layer)
        .to_string();

    owner_scribe
        .cast(ScribeMessage::MapInsert {
            layer_name: owner_bare_layer.clone(),
            path: String::new(),
            key: "owner-msg".to_string(),
            value: serde_json::json!({
                "id": "owner-msg",
                "text": "owner message",
                "sender_did": "did:key:owner",
                "timestamp": 1234567890
            }),
        })
        .map_err(|e| anyhow::anyhow!("Owner MapInsert failed: {:?}", e))?;

    viewer_scribe
        .cast(ScribeMessage::MapInsert {
            layer_name: viewer_bare_layer.clone(),
            path: String::new(),
            key: "viewer-msg".to_string(),
            value: serde_json::json!({
                "id": "viewer-msg",
                "text": "viewer message",
                "sender_did": "did:key:viewer",
                "timestamp": 1234567891
            }),
        })
        .map_err(|e| anyhow::anyhow!("Viewer MapInsert failed: {:?}", e))?;

    wait_for_layer_data(
        &s.node().butler,
        &page_id,
        &owner_bare_layer,
        Duration::from_secs(10),
    )
    .await?;
    wait_for_layer_data(
        &s.node().butler,
        &page_id,
        &viewer_bare_layer,
        Duration::from_secs(10),
    )
    .await?;

    let owner_sees_viewer_layer = wait_for_layer_data(
        &s.owner().butler,
        &page_id,
        &viewer_bare_layer,
        Duration::from_secs(3),
    )
    .await;
    assert!(
        owner_sees_viewer_layer.is_err(),
        "Owner should not automatically receive viewer-created explicit DM layer"
    );

    let viewer_sees_owner_layer = wait_for_layer_data(
        &s.viewer(0).butler,
        &page_id,
        &owner_bare_layer,
        Duration::from_secs(3),
    )
    .await;
    assert!(
        viewer_sees_owner_layer.is_err(),
        "Viewer should not automatically receive owner-created explicit DM layer"
    );

    s.shutdown().await;
    Ok(())
}
