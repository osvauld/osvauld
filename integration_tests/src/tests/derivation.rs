//! E-Commerce derivation tests
//!
//! Tests the derivation pipeline: customer order → node derivation → orders_summary.
//!
//! **Bug coverage**: "order made on viewer not visible on owner" — derivation
//! on_source_change pattern matching fails because Scribe normalizes layer names
//! to bare form but Lua registers prefixed patterns.

use std::time::Duration;

use anyhow::Result;
use tokio::time::sleep;

use butler::{PageUpdate, ScribeMessage};
use lua_runtime::{ActorScribeHandle, LuaCommand, LuaRuntime, LuaRuntimeConfig};

use crate::fixtures::{app_dir, init_tracing};
use crate::scenario::Scenario;

/// Simulate what kunki's NodeRuntimeManager does: get app files, find entry_node,
/// start LuaRuntime with init.lua + node.lua, and set up PageUpdate bridge
async fn start_node_runtime(
    butler: &std::sync::Arc<butler::Butler>,
    page_id: &str,
    scribe_ref: &ractor::ActorRef<ScribeMessage>,
) -> Result<(
    std::thread::JoinHandle<()>,
    tokio::sync::mpsc::Sender<LuaCommand>,
)> {
    let apps = butler
        .apps()
        .list(page_id)
        .map_err(|e| anyhow::anyhow!("Failed to list apps: {}", e))?;

    if apps.is_empty() {
        return Err(anyhow::anyhow!("Page has no apps"));
    }

    for app_name in &apps {
        let (tx, rx) = tokio::sync::oneshot::channel();
        scribe_ref
            .cast(ScribeMessage::GetAppFiles {
                app_name: app_name.clone(),
                reply: tx,
            })
            .map_err(|e| anyhow::anyhow!("Failed to get app files: {:?}", e))?;

        let files = rx
            .await?
            .map_err(|e| anyhow::anyhow!("GetAppFiles error: {}", e))?;

        let manifest_str = match files.get("manifest.json") {
            Some(m) => m,
            None => continue,
        };

        let manifest: serde_json::Value = serde_json::from_str(manifest_str)?;
        let entry_node = match manifest.get("entry_node").and_then(|v| v.as_str()) {
            Some(e) => e.to_string(),
            None => continue,
        };

        let node_script = files
            .get(&entry_node)
            .ok_or_else(|| anyhow::anyhow!("Node script '{}' not found", entry_node))?;

        let init_code = files.get("init.lua").cloned();
        let lua_code = match &init_code {
            Some(init) => format!("{}\n{}", init, node_script),
            None => node_script.clone(),
        };

        let identity = butler.user_info().await?;

        let (thread, cmd_tx) = LuaRuntime::spawn(LuaRuntimeConfig {
            page_id: page_id.to_string(),
            app_name: app_name.clone(),
            scribe: ActorScribeHandle::new(scribe_ref.clone()),
            user_did: identity.did.clone(),
            user_name: "node".to_string(),
            user_role: "node".to_string(),
            lua_code,
            ui_enabled: false,
            ui_tx: None,
            query_tx: None,
            navigate_tx: None,
            clock: std::sync::Arc::new(domains::RealClock),
        })
        .map_err(|e| anyhow::anyhow!("Failed to spawn LuaRuntime: {}", e))?;

        // Trigger initial derivation rebuild (same as kunki)
        if init_code.is_some() {
            cmd_tx
                .send(LuaCommand::RebuildDerivation)
                .await
                .map_err(|e| anyhow::anyhow!("Failed to send RebuildDerivation: {}", e))?;
        }

        // Subscribe to page updates and set up bridge (same as kunki's node_runtime)
        let (page_update_tx, mut page_update_rx) = tokio::sync::mpsc::channel(256);
        scribe_ref
            .cast(ScribeMessage::SubscribeToPageUpdates { tx: page_update_tx })
            .map_err(|e| anyhow::anyhow!("Failed to subscribe to page updates: {:?}", e))?;

        let bridge_cmd_tx = cmd_tx.clone();
        tokio::spawn(async move {
            while let Some(update) = page_update_rx.recv().await {
                match update {
                    PageUpdate::LayerChanged {
                        layer,
                        full_data,
                        delta,
                        created,
                        dynamic_ref,
                        ..
                    } => {
                        let _ = bridge_cmd_tx
                            .send(LuaCommand::LayerChanged {
                                layer_name: layer,
                                created,
                                delta,
                                full_data,
                                dynamic_ref,
                            })
                            .await;
                    }
                    _ => {}
                }
            }
        });

        return Ok((thread, cmd_tx));
    }

    Err(anyhow::anyhow!("No app with entry_node found"))
}

/// Full ecomm derivation pipeline with automatic trigger
///
/// After customer writes an order, the node's derivation is automatically
/// triggered via on_source_change and produces orders_summary.
#[tokio::test]
async fn test_ecomm_derivation_auto_trigger() -> Result<()> {
    init_tracing();

    let ecomm_dir = app_dir("my-shop");
    assert!(
        ecomm_dir.exists(),
        "sample_apps/my-shop must exist: {:?}",
        ecomm_dir
    );

    let mut s = Scenario::builder()
        .app("my-shop")
        .published()
        .build()
        .await?;

    let page_id = s.space().page_id.clone();
    sleep(Duration::from_millis(500)).await;

    // Node: start runtime with PageUpdate bridge
    let node_scribe = s.node().butler.open_page(&page_id).await?;
    let (_thread, cmd_tx) = start_node_runtime(&s.node().butler, &page_id, &node_scribe).await?;

    // Give runtime time to initialize derivation rules
    sleep(Duration::from_millis(500)).await;

    // Customer writes an order directly to node's scribe
    let customer_did = "did:key:z6MkTestCustomer";
    let order_layer = format!("orders/{}", customer_did);

    let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
    node_scribe
        .cast(ScribeMessage::EnsureLoroList {
            layer_name: order_layer.clone(),
            reply: reply_tx,
        })
        .map_err(|e| anyhow::anyhow!("Failed to ensure list: {:?}", e))?;
    reply_rx.await??;

    node_scribe
        .cast(ScribeMessage::ListPush {
            layer_name: order_layer.clone(),
            path: String::new(),
            item: serde_json::json!({
                "id": "order-001",
                "status": "submitted",
                "total": 199.99,
                "quantity": 2,
                "created_at": 1700000000,
                "submitted_at": 1700000060,
                "updated_at": 1700000060,
            }).into(),
        })
        .map_err(|e| anyhow::anyhow!("Failed to push order: {:?}", e))?;

    // Derivation should auto-trigger via PageUpdate bridge
    sleep(Duration::from_secs(2)).await;

    // Verify: derived/orders_summary should have the order
    let (tx, rx) = tokio::sync::oneshot::channel();
    node_scribe
        .cast(ScribeMessage::GetLayerJson {
            layer_name: "derived/orders_summary".to_string(),
            reply: tx,
        })
        .map_err(|e| anyhow::anyhow!("Failed to get layer: {:?}", e))?;

    let summary = rx.await?;
    let summary_sthithi = summary.expect("derived/orders_summary should exist");
    let summary_data = serde_json::Value::from(&summary_sthithi);
    let summary_obj = summary_data
        .as_object()
        .expect("orders_summary should be a map");

    assert!(
        !summary_obj.is_empty(),
        "derived/orders_summary should have entries — derivation should auto-trigger on layer changes"
    );

    let order_entry = summary_obj
        .get("order-001")
        .expect("Should have order-001 in summary");
    assert_eq!(
        order_entry.get("status").and_then(|v| v.as_str()),
        Some("submitted")
    );
    assert_eq!(
        order_entry.get("total").and_then(|v| v.as_f64()),
        Some(199.99)
    );
    assert_eq!(
        order_entry.get("customer").and_then(|v| v.as_str()),
        Some(customer_did),
        "Derived entry should have customer DID"
    );

    let _ = cmd_tx.send(LuaCommand::Shutdown).await;
    s.shutdown().await;
    Ok(())
}
