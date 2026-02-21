//! Presence heartbeat and multi-user sync tests
//!
//! Verifies that:
//! 1. Presence layer updates propagate between users via node
//! 2. When a user disconnects, their presence becomes stale
//! 3. Heartbeat interval updates last_seen

use std::collections::HashMap;
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

use anyhow::Result;
use ractor::ActorRef;
use tokio::sync::mpsc;
use tokio::time::sleep;

use butler::ScribeMessage;
use domains::{ClockSource, ManualClock, RealClock};
use lua_runtime::{LuaCommand, LuaRuntime, LuaRuntimeConfig, ActorScribeHandle};

use crate::fixtures::init_tracing;
use crate::scenario::Scenario;

/// Holds a running Lua app with presence
struct PresenceApp {
    cmd_tx: mpsc::Sender<LuaCommand>,
    runtime_handle: JoinHandle<()>,
}

impl PresenceApp {
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
    clock: Arc<dyn ClockSource>,
) -> Result<PresenceApp> {
    let lua_code = format!(
        r#"
local page_id = "{page_id}"
local my_did = "{user_did}"
local my_name = "{user_name}"

local presence_layer = scribe:map(page_id .. "/presence")

local function write_presence()
    presence_layer:set(my_did, {{
        did = my_did,
        status = "online",
        name = my_name,
        last_seen = clock:count(),
    }})
end

function on_init()
    write_presence()
    timer.setInterval(2000, function()
        write_presence()
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
        scribe: ActorScribeHandle::new(scribe_ref),
        user_did: user_did.to_string(),
        user_name: user_name.to_string(),
        user_role: "collaborator".to_string(),
        lua_code,
        ui_enabled: false,
        ui_tx: None,
        query_tx: None,
        navigate_tx: None,
        clock,
    })
    .map_err(|e| anyhow::anyhow!("Failed to spawn Lua runtime: {}", e))?;

    Ok(PresenceApp { cmd_tx, runtime_handle })
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

    let map: HashMap<String, serde_json::Value> = data
        .as_object()
        .map(|obj| obj.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
        .unwrap_or_default();

    Ok(map)
}

async fn wait_for_presence_entry(
    scribe_ref: &ActorRef<ScribeMessage>,
    layer_name: &str,
    did: &str,
    timeout: Duration,
) -> Result<HashMap<String, serde_json::Value>> {
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        match query_presence(scribe_ref, layer_name).await {
            Ok(presence) if presence.contains_key(did) => return Ok(presence),
            // Layer doesn't exist yet or DID not present — treat as transient, retry
            Ok(_) | Err(_) => {}
        }
        if tokio::time::Instant::now() >= deadline {
            return Err(anyhow::anyhow!(
                "Timeout waiting for presence entry for DID {} in {}",
                did,
                layer_name
            ));
        }
        sleep(Duration::from_millis(50)).await;
    }
}

/// Count online users (those with last_seen within threshold seconds)
fn count_online(presence: &HashMap<String, serde_json::Value>, threshold_secs: i64) -> usize {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    count_online_with_now(presence, threshold_secs, now)
}

fn count_online_with_now(
    presence: &HashMap<String, serde_json::Value>,
    threshold_secs: i64,
    now: i64,
) -> usize {

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

/// Local presence write + stale detection
///
/// **Note**: Cross-peer presence sync requires a presence layer in the permit
/// template. The osvauld-demos app doesn't define one, so this test verifies
/// local presence behavior: write, heartbeat, and stale detection on a single peer.
///
/// Spawns two presence apps on the owner's Scribe. Verifies both entries exist,
/// kills one app, and checks that its presence becomes stale.
#[tokio::test]
async fn test_presence_local_stale_detection() -> Result<()> {
    init_tracing();

    let mut s = Scenario::builder()
        .app("osvauld-demos")
        .published()
        .build()
        .await?;

    let page_id = s.space().page_id.clone();
    let owner_info = s.owner().butler.user_info().await?;
    let scribe_ref = s.owner().butler.open_page(&page_id).await?;

    // Spawn two presence apps writing to the same Scribe (simulating multi-user)
    let app1 = spawn_presence_app(
        &page_id,
        &owner_info.did,
        "user1",
        scribe_ref.clone(),
        Arc::new(RealClock),
    )?;
    let app2 = spawn_presence_app(
        &page_id,
        "did:key:z6MkTestUser2",
        "user2",
        scribe_ref.clone(),
        Arc::new(RealClock),
    )?;

    // Wait for initial writes
    sleep(Duration::from_secs(2)).await;

    let presence_layer = format!("{}/presence", page_id);
    let presence = query_presence(&scribe_ref, &presence_layer).await?;

    assert_eq!(presence.len(), 2, "Should have 2 presence entries");
    assert_eq!(count_online(&presence, 10), 2, "Both users should be online");

    // Kill user2's app
    app2.shutdown().await;

    // Wait for user2 to become stale (>10 seconds without heartbeat)
    sleep(Duration::from_secs(15)).await;

    let presence = query_presence(&scribe_ref, &presence_layer).await?;
    assert_eq!(presence.len(), 2, "Should still have 2 presence entries");
    assert_eq!(count_online(&presence, 10), 1, "Only 1 user should be online (user2 is stale)");

    app1.shutdown().await;
    s.shutdown().await;

    Ok(())
}

/// Fast heartbeat test — verify heartbeat updates last_seen
#[tokio::test]
async fn test_presence_heartbeat_fast() -> Result<()> {
    init_tracing();

    let mut s = Scenario::builder()
        .app("osvauld-demos")
        .published()
        .build()
        .await?;

    let page_id = s.space().page_id.clone();
    let owner_info = s.owner().butler.user_info().await?;
    let user_did = owner_info.did.clone();

    let scribe_ref = s.owner().butler.open_page(&page_id).await?;
    let app = spawn_presence_app(
        &page_id,
        &user_did,
        "test_user",
        scribe_ref.clone(),
        Arc::new(RealClock),
    )?;

    // Wait for initial write
    sleep(Duration::from_secs(1)).await;

    // Query initial
    let presence_layer = format!("{}/presence", page_id);
    let presence = query_presence(&scribe_ref, &presence_layer).await?;
    let initial_last_seen = presence
        .get(&user_did)
        .and_then(|v| v.get("last_seen"))
        .and_then(|v| v.as_i64())
        .expect("Should have last_seen");

    // Wait for heartbeat (2s interval + buffer)
    sleep(Duration::from_secs(3)).await;

    // Query updated
    let presence = query_presence(&scribe_ref, &presence_layer).await?;
    let updated_last_seen = presence
        .get(&user_did)
        .and_then(|v| v.get("last_seen"))
        .and_then(|v| v.as_i64())
        .expect("Should have last_seen");

    assert!(
        updated_last_seen > initial_last_seen,
        "Fast heartbeat should update last_seen: {} -> {} (expected diff >= 2s)",
        initial_last_seen,
        updated_last_seen
    );

    app.shutdown().await;
    s.shutdown().await;

    Ok(())
}

/// Manual clock test — advance time without sleeping real seconds.
#[tokio::test]
async fn test_presence_heartbeat_manual_clock_advance() -> Result<()> {
    init_tracing();

    let mut s = Scenario::builder()
        .app("osvauld-demos")
        .published()
        .build()
        .await?;

    let page_id = s.space().page_id.clone();
    let owner_info = s.owner().butler.user_info().await?;
    let user_did = owner_info.did.clone();

    let scribe_ref = s.owner().butler.open_page(&page_id).await?;
    let manual_clock = Arc::new(ManualClock::new(1_740_009_600)); // 2025-02-20T00:00:00Z

    let app = spawn_presence_app(
        &page_id,
        &user_did,
        "sim_user",
        scribe_ref.clone(),
        manual_clock.clone(),
    )?;

    let presence_layer = format!("{}/presence", page_id);
    let initial = wait_for_presence_entry(
        &scribe_ref,
        &presence_layer,
        &user_did,
        Duration::from_secs(2),
    )
    .await?;
    let initial_last_seen = initial
        .get(&user_did)
        .and_then(|v| v.get("last_seen"))
        .and_then(|v| v.as_i64())
        .expect("Should have initial last_seen");

    assert_eq!(initial_last_seen, manual_clock.now_unix());

    // Jump forward in simulated time; heartbeat interval is 2s.
    manual_clock.advance(Duration::from_secs(5));
    sleep(Duration::from_millis(150)).await;

    let updated = query_presence(&scribe_ref, &presence_layer).await?;
    let updated_last_seen = updated
        .get(&user_did)
        .and_then(|v| v.get("last_seen"))
        .and_then(|v| v.as_i64())
        .expect("Should have updated last_seen");

    assert!(
        updated_last_seen >= initial_last_seen + 2,
        "Heartbeat should fire after simulated time advance: {} -> {}",
        initial_last_seen,
        updated_last_seen
    );

    assert_eq!(
        count_online_with_now(&updated, 10, manual_clock.now_unix()),
        1,
        "User should remain online after simulated heartbeat"
    );

    app.shutdown().await;
    s.shutdown().await;
    Ok(())
}
