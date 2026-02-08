//! Test presence heartbeat and multi-user sync
//!
//! Verifies that:
//! 1. Presence layer updates propagate between users via node
//! 2. When a user disconnects, their presence becomes stale
//! 3. When a user reconnects, they see updated presence from others

use std::collections::HashMap;
use std::thread::JoinHandle;
use std::time::Duration;

use anyhow::Result;
use ractor::ActorRef;
use tokio::sync::mpsc;
use tokio::time::sleep;

use butler::ScribeMessage;
use lua_runtime::{LuaCommand, LuaRuntime, LuaRuntimeConfig};

use crate::scenario::{init_tracing, MockScenario, CHAT_PAGE_LAYERS, CHAT_PAGE_TEMPLATE, SPACE_TEMPLATE};

/// Holds a running Lua app with presence
struct PresenceApp {
    cmd_tx: mpsc::Sender<LuaCommand>,
    runtime_handle: JoinHandle<()>,
    user_did: String,
    user_name: String,
}

impl PresenceApp {
    /// Shutdown the app
    async fn shutdown(self) {
        let _ = self.cmd_tx.send(LuaCommand::Shutdown).await;
        let _ = self.runtime_handle.join();
    }
}

/// Spawn a Lua runtime with fast presence heartbeat (2 seconds)
fn spawn_presence_app(
    page_id: &str,
    user_did: &str,
    user_name: &str,
    scribe_ref: ActorRef<ScribeMessage>,
) -> Result<PresenceApp> {
    let lua_code = format!(
        r#"
-- Fast presence test with 2-second heartbeat
local page_id = "{page_id}"
local my_did = "{user_did}"
local my_name = "{user_name}"

-- Create presence layer
local presence_layer = scribe:map(page_id .. "/presence")

-- Write presence
local function write_presence()
    presence_layer:set(my_did, {{
        did = my_did,
        status = "online",
        name = my_name,
        last_seen = os.time(),
    }})
end

function on_init()
    write_presence()
    print("[" .. my_name .. "] Presence initialized")

    -- Fast heartbeat for testing (2 seconds)
    timer.setInterval(2000, function()
        write_presence()
        print("[" .. my_name .. "] Heartbeat fired")
    end)
end
"#,
        page_id = page_id,
        user_did = user_did,
        user_name = user_name
    );

    let (runtime_handle, cmd_tx) = LuaRuntime::spawn(LuaRuntimeConfig {
        page_id: page_id.to_string(),
        app_name: format!("presence-{}", user_name),
        scribe_ref,
        user_did: user_did.to_string(),
        user_name: user_name.to_string(),
        user_role: "collaborator".to_string(),
        lua_code,
        ui_enabled: false,
        ui_tx: None,
        query_tx: None,
    })
    .map_err(|e| anyhow::anyhow!("Failed to spawn Lua runtime: {}", e))?;

    Ok(PresenceApp {
        cmd_tx,
        runtime_handle,
        user_did: user_did.to_string(),
        user_name: user_name.to_string(),
    })
}

/// Query presence layer and return all entries
async fn query_presence(
    scribe_ref: &ActorRef<ScribeMessage>,
    layer_name: &str,
) -> Result<HashMap<String, serde_json::Value>> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    scribe_ref
        .cast(ScribeMessage::GetLayerJson {
            layer_name: layer_name.to_string(),
            reply: tx,
        })
        .map_err(|e| anyhow::anyhow!("Failed to send GetLayerJson: {:?}", e))?;

    let data = rx
        .await
        .map_err(|_| anyhow::anyhow!("Failed to receive response"))?
        .ok_or_else(|| anyhow::anyhow!("Presence layer '{}' does not exist", layer_name))?;

    // Convert to HashMap
    let map: HashMap<String, serde_json::Value> = data
        .as_object()
        .map(|obj| obj.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
        .unwrap_or_default();

    Ok(map)
}

/// Count online users (those with last_seen within threshold seconds)
fn count_online(presence: &HashMap<String, serde_json::Value>, threshold_secs: i64) -> usize {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    presence
        .values()
        .filter(|entry| {
            entry
                .get("last_seen")
                .and_then(|v| v.as_i64())
                .map(|last_seen| now - last_seen < threshold_secs)
                .unwrap_or(false)
        })
        .count()
}

/// Test multi-user presence with disconnect/reconnect
///
/// **Scenario**:
/// 1. Owner creates space/page with presence layer
/// 2. Owner + 2 viewers connect to node
/// 3. All 3 run presence apps with 2-second heartbeat
/// 4. Verify all 3 are online
/// 5. Kill viewer1's app
/// 6. Wait 15 seconds, verify online count decreased
/// 7. Wait 30 seconds total
/// 8. Restart viewer1's app
/// 9. Verify viewer1 sees updated presence from others
#[tokio::test]
async fn test_presence_multi_user_disconnect_reconnect() -> Result<()> {
    init_tracing();

    // 1. Create scenario: owner + node + 2 viewers
    let mut s = MockScenario::new_mock(true, true, 2).await?;

    // Connect owner to node
    s.connect_owner_to_node_mock().await?;
    tracing::info!("Owner connected to node");

    // 2. Setup owner with chat space (includes presence layer)
    let space_info = s.setup_owner_chat("Presence Multi-User Test").await?;
    let page_id = space_info.page_id.clone();
    tracing::info!(page_id = %page_id, "Chat space created");

    // 3. Publish space to node
    s.publish_space(&space_info.id).await?;
    sleep(Duration::from_millis(500)).await;
    tracing::info!("Space published to node");

    // 4. Get viewer link and connect viewers
    let viewer_link = s.get_viewer_link(&space_info.id).await?;

    s.add_viewer_mock(0, &viewer_link, &page_id).await?;
    tracing::info!("Viewer0 connected and has page");

    s.add_viewer_mock(1, &viewer_link, &page_id).await?;
    tracing::info!("Viewer1 connected and has page");

    // 5. Open pages and spawn presence apps for all 3 users
    let owner = s.owner();
    let owner_info = owner.butler.user_info().await?;
    let owner_scribe = owner.butler.open_page(&page_id).await?;
    let owner_app = spawn_presence_app(&page_id, &owner_info.did, "owner", owner_scribe.clone())?;
    tracing::info!("Owner presence app started");

    let viewer0 = s.viewer(0);
    let viewer0_info = viewer0.butler.user_info().await?;
    let viewer0_scribe = viewer0.butler.open_page(&page_id).await?;
    let viewer0_app = spawn_presence_app(&page_id, &viewer0_info.did, "viewer0", viewer0_scribe.clone())?;
    tracing::info!("Viewer0 presence app started");

    let viewer1 = s.viewer(1);
    let viewer1_info = viewer1.butler.user_info().await?;
    let viewer1_scribe = viewer1.butler.open_page(&page_id).await?;
    let viewer1_app = spawn_presence_app(&page_id, &viewer1_info.did, "viewer1", viewer1_scribe.clone())?;
    tracing::info!("Viewer1 presence app started");

    // 6. Wait for initial presence writes and sync
    sleep(Duration::from_secs(3)).await;

    // 7. Query presence from owner's perspective
    let presence_layer = format!("{}/presence", page_id);
    let presence = query_presence(&owner_scribe, &presence_layer).await?;
    let online_count = count_online(&presence, 10);

    tracing::info!(
        entries = presence.len(),
        online = online_count,
        "[TEST] Initial presence state"
    );

    assert_eq!(
        presence.len(),
        3,
        "Should have 3 presence entries, got: {:?}",
        presence.keys().collect::<Vec<_>>()
    );
    assert_eq!(online_count, 3, "All 3 users should be online");

    // 8. Kill viewer1's app
    tracing::info!("[TEST] Killing viewer1's app...");
    viewer1_app.shutdown().await;

    // 9. Wait 15 seconds - viewer1's presence should become stale
    tracing::info!("[TEST] Waiting 15 seconds for viewer1 to go stale...");
    sleep(Duration::from_secs(15)).await;

    // Query presence again - viewer1 should be stale (last_seen > 10s ago)
    let presence = query_presence(&owner_scribe, &presence_layer).await?;
    let online_count = count_online(&presence, 10);

    tracing::info!(
        entries = presence.len(),
        online = online_count,
        "[TEST] Presence after viewer1 shutdown (15s)"
    );

    // Still 3 entries, but only 2 should be "online" (recent last_seen)
    assert_eq!(presence.len(), 3, "Should still have 3 presence entries");
    assert_eq!(
        online_count, 2,
        "Only 2 users should be online (viewer1 is stale)"
    );

    // 10. Wait another 15 seconds (30 total)
    tracing::info!("[TEST] Waiting 15 more seconds (30 total)...");
    sleep(Duration::from_secs(15)).await;

    // 11. Restart viewer1's app
    tracing::info!("[TEST] Restarting viewer1's app...");
    let viewer1_scribe_new = s.viewer(1).butler.open_page(&page_id).await?;
    let viewer1_app_new = spawn_presence_app(
        &page_id,
        &viewer1_info.did,
        "viewer1",
        viewer1_scribe_new.clone(),
    )?;
    tracing::info!("Viewer1 presence app restarted");

    // 12. Wait for sync
    sleep(Duration::from_secs(3)).await;

    // 13. Verify viewer1 sees all 3 online again
    let presence = query_presence(&viewer1_scribe_new, &presence_layer).await?;
    let online_count = count_online(&presence, 10);

    tracing::info!(
        entries = presence.len(),
        online = online_count,
        "[TEST] Presence from viewer1's perspective after reconnect"
    );

    assert_eq!(
        online_count, 3,
        "All 3 users should be online again after viewer1 reconnect"
    );

    tracing::info!("[TEST] Multi-user presence disconnect/reconnect test PASSED!");

    // Cleanup
    owner_app.shutdown().await;
    viewer0_app.shutdown().await;
    viewer1_app_new.shutdown().await;
    s.shutdown().await;

    Ok(())
}

/// Shorter test with reduced heartbeat interval for faster CI
///
/// Uses a custom Lua script that sets a 2-second heartbeat instead of 15s.
#[tokio::test]
async fn test_presence_heartbeat_fast() -> Result<()> {
    init_tracing();

    // 1. Create owner peer
    let mut s = MockScenario::new_mock(true, false, 0).await?;
    let owner = s.owner();

    // 2. Create space and page
    let owner_info = owner.butler.user_info().await?;
    let space = owner
        .butler
        .spaces()
        .create(
            "Fast Presence Test".to_string(),
            owner_info.did.clone(),
            SPACE_TEMPLATE,
        )
        .await?;

    let layer_names: Vec<String> = CHAT_PAGE_LAYERS.iter().map(|s| s.to_string()).collect();
    let page = owner
        .butler
        .pages()
        .create(&space.id, "test-page", layer_names, CHAT_PAGE_TEMPLATE)
        .await?;
    let page_id = page.id.clone();
    let user_did = owner_info.did.clone();

    tracing::info!(page_id = %page_id, "Page created for fast presence test");

    // 3. Open page
    let scribe_ref = owner.butler.open_page(&page_id).await?;

    // 4. Spawn presence app
    let app = spawn_presence_app(&page_id, &user_did, "test_user", scribe_ref.clone())?;
    tracing::info!("Lua runtime spawned with fast presence test");

    // 5. Wait for initial write
    sleep(Duration::from_secs(1)).await;

    // 6. Query initial
    let presence_layer = format!("{}/presence", page_id);
    let presence = query_presence(&scribe_ref, &presence_layer).await?;
    let initial_last_seen = presence
        .get(&user_did)
        .and_then(|v| v.get("last_seen"))
        .and_then(|v| v.as_i64())
        .expect("Should have last_seen");

    tracing::info!(initial = %initial_last_seen, "[TEST] Initial presence");

    // 7. Wait for heartbeat (2s interval + buffer)
    tracing::info!("[TEST] Waiting 3 seconds for fast heartbeat...");
    sleep(Duration::from_secs(3)).await;

    // 8. Query updated
    let presence = query_presence(&scribe_ref, &presence_layer).await?;
    let updated_last_seen = presence
        .get(&user_did)
        .and_then(|v| v.get("last_seen"))
        .and_then(|v| v.as_i64())
        .expect("Should have last_seen");

    tracing::info!(
        initial = %initial_last_seen,
        updated = %updated_last_seen,
        diff = %(updated_last_seen - initial_last_seen),
        "[TEST] After heartbeat"
    );

    // 9. Verify
    assert!(
        updated_last_seen > initial_last_seen,
        "Fast heartbeat should update last_seen: {} -> {} (expected diff >= 2s)",
        initial_last_seen,
        updated_last_seen
    );

    tracing::info!("[TEST] Fast presence heartbeat working!");

    // Cleanup
    app.shutdown().await;
    s.shutdown().await;

    Ok(())
}
