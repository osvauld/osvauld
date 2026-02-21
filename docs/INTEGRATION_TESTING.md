# Integration Testing

Rust-level protocol integration tests using in-memory mock connections. No real networking, no processes — everything runs in a single `cargo test` invocation.

For Python/control-server E2E tests (real processes, real QUIC), see `docs/app-dev/TESTING.md`.

## Quick Start

```bash
# Run all integration tests
cargo test -p integration_tests

# Run a specific test with trace output
RUST_LOG=info cargo test -p integration_tests test_owner_first_connection -- --nocapture

# Run a test category
cargo test -p integration_tests layer_sync
```

## Architecture

```
integration_tests/
├── Cargo.toml
└── src/
    ├── lib.rs                   # Re-exports
    ├── fixtures.rs              # Paths, wait helpers, constants
    ├── peer.rs                  # MockConnection-only Peer (no generics)
    ├── tracer.rs                # Protocol message capture + assertions
    ├── scenario.rs              # Builder pattern for multi-peer scenarios
    └── tests/
        ├── mod.rs
        ├── handshake.rs         # 3 tests — connection, reconnect, invalid permit
        ├── publish.rs           # 4 tests — page/space/app sync after publish
        ├── layer_sync.rs        # 8 tests — static/dynamic layer sync, viewer relay, custom channels
        ├── app_sync.rs          # 3 tests — app file sync owner→node→viewer
        ├── permission.rs        # 3 tests — permit enforcement, scoped access
        ├── presence.rs          # 2 tests — presence heartbeat, stale detection
        ├── derivation.rs        # 1 test  — ecomm derivation auto-trigger
        └── validation.rs        # 7 tests — standalone Lua validation rules
```

### Key Components

**Peer** (`peer.rs`) — A single participant with its own Butler, Coordinator, identity, and temp database. Uses `MockConnection` only (in-memory channels, no QUIC). Pass an optional `message_tx` from `Tracer::new()` to enable protocol tracing.

**Scenario** (`scenario.rs`) — Builder pattern that wires up peers, handles connection, publish, and viewer flows. The builder methods compose:

- `.owner()` / `.node()` / `.viewers(n)` — add peers
- `.connected()` — implies owner + node + mock connection + handshake
- `.published()` — implies connected + app import + publish + wait for page on node
- `.app("name")` — which sample app to use (defaults to `"osvauld-demos"`)
- `.with_tracer()` — enable protocol message capture

**Tracer** (`tracer.rs`) — Wraps `courier::MessageTrace` channel. Collects all protocol messages (Hello, Welcome, PublishSpace, SyncOffer, etc.) and provides assertion methods:

- `assert_sequence(&["A", "B", "C"])` — exact match
- `assert_contains_sequence(&["A", "C"])` — subsequence (other messages allowed between)
- `assert_count("SyncOffer", 3)` — count occurrences
- `dump()` — print full trace with timestamps

**Fixtures** (`fixtures.rs`) — Helper functions and constants:

- `app_dir("osvauld-demos")` → `<workspace>/sample_apps/osvauld-demos`
- `wait_for_layer_data(butler, page_id, layer, timeout)` → polls `store().get_layer()`
- `wait_for_app_files(butler, page_id, app_name, timeout)` → polls `apps().get_files()`
- `MOCK_TIMEOUT` (500ms), `PAGE_SYNC_TIMEOUT` (10s)

## Writing a New Test

### Protocol test (handshake, sync, publish)

```rust
use anyhow::Result;
use crate::fixtures::init_tracing;
use crate::scenario::Scenario;

#[tokio::test]
async fn test_my_protocol_feature() -> Result<()> {
    init_tracing();

    let mut s = Scenario::builder()
        .app("osvauld-demos")
        .published()           // owner + node + connected + published
        .with_tracer()         // capture protocol messages
        .build()
        .await?;

    // Assert on protocol message sequence
    s.tracer().assert_contains_sequence(&[
        "Hello", "Welcome",
        "PublishSpace", "PublishSpaceAck",
    ]);

    // Access butler APIs on any peer
    let page = s.node().butler.pages().get(&s.space().page_id)?;
    assert!(page.is_some());

    s.shutdown().await;
    Ok(())
}
```

### Layer sync test (Scribe writes, cross-peer data flow)

```rust
use butler::ScribeMessage;

#[tokio::test]
async fn test_my_layer_sync() -> Result<()> {
    init_tracing();

    let mut s = Scenario::builder()
        .app("osvauld-demos")
        .published()
        .build()
        .await?;

    let page_id = s.space().page_id.clone();
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Open page on owner → returns ActorRef<ScribeMessage>
    let scribe = s.owner().butler.open_page(&page_id).await?;

    // Write to a layer (must be declared in the app's app.osv policy)
    let (tx, rx) = tokio::sync::oneshot::channel();
    scribe.cast(ScribeMessage::EnsureLoroMap {
        layer_name: "reactions".to_string(),
        reply: tx,
    })?;
    rx.await??;

    scribe.cast(ScribeMessage::MapInsert {
        layer_name: "reactions".to_string(),
        path: String::new(),
        key: "msg1".to_string(),
        value: serde_json::json!({"emoji": "thumbs_up"}),
    })?;

    // Wait for data to arrive on node
    let data = wait_for_layer_data(
        &s.node().butler, &page_id, "reactions", Duration::from_secs(10),
    ).await?;
    assert!(!data.is_empty());

    s.shutdown().await;
    Ok(())
}
```

### Viewer test (connect viewer, verify data flow)

```rust
#[tokio::test]
async fn test_viewer_gets_data() -> Result<()> {
    init_tracing();

    let mut s = Scenario::builder()
        .app("osvauld-demos")
        .published()
        .viewers(1)          // allocate 1 viewer peer
        .build()
        .await?;

    let page_id = s.space().page_id.clone();
    let space_id = s.space().space_id.clone();

    // Get viewer link from node, then connect + handshake + request space
    let viewer_link = s.get_viewer_link(&space_id).await?;
    s.add_viewer(0, &viewer_link, &page_id).await?;

    // Viewer now has the page
    let page = s.viewer(0).butler.pages().get(&page_id)?;
    assert!(page.is_some());

    s.shutdown().await;
    Ok(())
}
```

### Dynamic layer test (create_layer + permit issuance + sync)

```rust
#[tokio::test]
async fn test_dynamic_layer_syncs() -> Result<()> {
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

    // Connect viewer
    let viewer_link = s.get_viewer_link(&space_id).await?;
    s.add_viewer(0, &viewer_link, &page_id).await?;
    sleep(Duration::from_millis(500)).await;

    // Owner creates dynamic layer via schema
    let owner_scribe = s.owner().butler.open_page(&page_id).await?;
    let (tx, rx) = tokio::sync::oneshot::channel();
    owner_scribe.cast(ScribeMessage::CreateDynamicLayer {
        schema_key: "channels/{id}/messages".to_string(),
        placeholders: [("id".to_string(), "my-channel".to_string())]
            .into_iter()
            .collect(),
        authorized_peers: None,
        reply: tx,
    })?;
    let full_name = rx.await??;

    // Strip page_id prefix for bare layer name
    let bare = full_name.strip_prefix(&format!("{}/", page_id))
        .unwrap_or(&full_name);

    // Write data to the dynamic layer
    owner_scribe.cast(ScribeMessage::MapInsert {
        layer_name: bare.to_string(),
        path: String::new(),
        key: "msg1".to_string(),
        value: serde_json::json!({"text": "Hello"}),
    })?;

    // Node issues layer permits automatically → viewer receives data
    let viewer_data = wait_for_layer_data(
        &s.viewer(0).butler, &page_id, bare, Duration::from_secs(10),
    ).await?;
    assert!(!viewer_data.is_empty());

    s.shutdown().await;
    Ok(())
}
```

Dynamic layers require:
- A matching dynamic layer schema in `app.osv`
- Role/action policy that allows the sync path for participating peers
- Node mode Scribe with `PermitIssuer` wired (handled automatically by Butler)

### Validation test (standalone Lua, no P2P)

Validation tests load `validation.lua` from sample apps and test directly with mlua. No Scenario needed.

```rust
use mlua::{Lua, Function, Value};

#[test]
fn test_my_validation_rule() -> Result<()> {
    let lua = Lua::new();
    let code = std::fs::read_to_string(
        workspace_root().join("sample_apps/my-shop/shared/validation.lua")
    )?;
    lua.load(&code).exec()?;

    let validate_fn: Function = lua.globals().get("validate_ops")?;
    let ops = lua.create_table()?;
    // ... set up ops ...

    let result: bool = validate_fn.call(("products", ops, "did:key:owner", "owner", "page1"))?;
    assert!(result);
    Ok(())
}
```

### Presence / LuaRuntime test

Tests that need a running Lua app use `LuaRuntime::spawn()` with `ActorScribeHandle`:

```rust
use lua_runtime::{LuaRuntime, LuaRuntimeConfig, LuaCommand, ActorScribeHandle};
use butler::ScribeMessage;

let scribe_ref = s.owner().butler.open_page(&page_id).await?;

let (thread, cmd_tx) = LuaRuntime::spawn(LuaRuntimeConfig {
    page_id: page_id.to_string(),
    app_name: "test-app".to_string(),
    scribe: ActorScribeHandle::new(scribe_ref.clone()),
    user_did: "did:key:...".to_string(),
    user_name: "test".to_string(),
    user_role: "collaborator".to_string(),
    lua_code: r#"function on_init() ... end"#.to_string(),
    ui_enabled: false,
    ui_tx: None,
    query_tx: None,
})?;

// ... test logic ...

cmd_tx.send(LuaCommand::Shutdown).await?;
thread.join().unwrap();
```

## Available Sample Apps

Tests use real sample apps from `sample_apps/`. Each app's `app.osv` defines which layers exist and which roles can read, write, and sync them.

| App | Layers | Use for |
|-----|--------|---------|
| `osvauld-demos` | `messages` (list), `reactions` (map), `channels_meta` (map), `channels/*/messages` (map), `app:Group Chat` (map) | Most protocol tests, dynamic layer sync |
| `my-shop` | `products` (map), `orders/{aud}` (list), `derived/orders_summary` (map), multiple app layers | Derivation, validation, ecomm sync |
| `my-booking` | `schedule` (map), `bookings/{aud}` (list), `derived/calendar` (map) | Validation tests |

### Layer constraints

Layer names in test writes are bare names (for example, `"messages"`, `"reactions"`). Scribe normalizes layer names against page context internally.

**Only layers declared in app policy can sync between peers.** If you write to a layer that is not declared in `app.osv`, the write can succeed locally but the node will reject sync for that layer. This is a common gotcha when porting tests.

### Adding a new sample app for testing

If your test requires layers not in any existing app:

1. Create `sample_apps/my-test-app/`
2. Add `app.osv` with roles, layer definitions, and app declarations
3. Add at least one app subdirectory with `manifest.json`
4. Use `.app("my-test-app")` in the Scenario builder

## Scenario Builder Reference

### Builder methods

| Method | Effect |
|--------|--------|
| `.owner()` | Add an owner peer |
| `.node()` | Add a node peer |
| `.viewers(n)` | Allocate `n` viewer peers |
| `.app("name")` | Set sample app directory (default: `"osvauld-demos"`) |
| `.space_name("name")` | Override space name (default: `"Test Space"`) |
| `.connected()` | Implies `owner + node`, runs mock connection + handshake |
| `.published()` | Implies `connected + app`, runs import + publish + wait |
| `.with_tracer()` | Enable protocol message tracing |

### Scenario operations

| Method | Description |
|--------|-------------|
| `s.owner()` / `s.node()` / `s.viewer(i)` | Access peers |
| `s.space()` | Get `SpaceInfo { space_id, page_id }` (panics if not published) |
| `s.tracer()` | Get `Tracer` (panics if `.with_tracer()` not called) |
| `s.get_viewer_link(&space_id)` | Generate viewer connection string from node |
| `s.add_viewer(i, &conn_string, &page_id)` | Full viewer flow: connect + handshake + request space + wait |
| `s.setup_viewer(i, &conn_string)` | Connect + handshake only (no space request) |
| `s.import_app(space_name, app_name)` | Manually import an app (`.published()` does this automatically) |
| `s.publish_space(&space_id)` | Manually publish (`.published()` does this automatically) |
| `s.wait_for_page_on_node(&page_id)` | Poll until node has the page |
| `s.shutdown()` | Shutdown all peers |

### Peer fields

Each `Peer` exposes:

| Field | Type | Description |
|-------|------|-------------|
| `name` | `String` | Peer name ("owner", "node", "viewer0") |
| `node_id` | `NodeId` | Transport-level node ID |
| `mode` | `CourierMode` | `User` or `Node` |
| `coordinator` | `ActorRef<CoordinatorMessage>` | Coordinator actor |
| `butler` | `Arc<Butler>` | Butler facade for storage/services |

## Protocol Message Tracing

The tracer captures every protocol message sent and received by all peers in the scenario. Messages are captured via `courier::trace::MessageTrace`:

```rust
pub struct MessageTrace {
    pub direction: TraceDirection,  // Sent or Received
    pub msg_name: &'static str,    // "Hello", "Welcome", "SyncOffer", etc.
    pub node_id: NodeId,           // Our node
    pub peer_node_id: NodeId,      // The other side
    pub timestamp: Instant,
}
```

### Common message sequences

**Handshake (owner → node):**
```
Hello → Welcome → PermitGrant → Ack
```

**Publish:**
```
PublishSpace → PublishSpaceAck → PageAnnounce → PageAnnounceAck
```

**Layer sync:**
```
SyncOffer → SyncRequest → SyncResponse
```

### Using the tracer

```rust
let mut s = Scenario::builder()
    .published()
    .with_tracer()
    .build().await?;

// Exact sequence
s.tracer().assert_sequence(&["Hello", "Welcome", "PublishSpace", "PublishSpaceAck"]);

// Subsequence (allows other messages between)
s.tracer().assert_contains_sequence(&["Hello", "PublishSpaceAck"]);

// Count
s.tracer().assert_count("SyncOffer", 3);

// Debug: print full trace
s.tracer().dump();
```

## How It Works Internally

### MockConnection

Tests use `transport::MockConnection` — in-memory bidirectional channels. When `peer_a.connect_to(&peer_b)` is called, a `mock_connection_pair()` creates two connected mock connections and injects them into each peer's Coordinator via `CoordinatorMessage::Connected`.

### No generics

The old test infrastructure had `Peer<C: Connection>` with both `IrohConnection` and `MockConnection` support. The new crate uses `MockConnection` exclusively — no generics, no type parameters, simpler code.

### Policy source

Page policy comes from real `sample_apps/` directories via `butler.apps().import_page()`. This reads `app.osv` from the app directory and creates roles/layers accordingly. No hardcoded page template constants.

### Sync flow

After `.published()`, the Scenario has:
1. Created a space on owner's Butler
2. Imported the page from the sample app directory
3. Connected owner to node via mock connection + handshake
4. Sent `PublishSpace` from owner to node
5. Waited for the page to appear on node's Butler

Layer data sync happens afterward via the Scribe subscription system:
1. Owner opens page → Scribe actor spawns
2. Owner writes to a layer → Scribe broadcasts SyncOffer to subscribers
3. Node (subscribed via PeerActor) receives SyncOffer → applies update
4. Viewer (if connected) also receives via node relay

**Dynamic layer sync** adds a permit issuance step:
1. Owner calls `create_layer("channels/{id}/messages", "my-channel")` → creates DID-namespaced layer
2. Owner writes data → SyncOffer sent to node
3. Node detects new layer matching a `dynamic_layer_schemas` entry
4. Node issues **layer permits** to connected peers based on `role_permissions`
5. Node adds layer permits to subscriber info → enables `can_receive_layer()` check
6. Node broadcasts layer data to permitted subscribers
7. Node sends `NewDynamicLayer` event via Coordinator → PeerActors deliver permits to peers

## Test Categories

| Category | Files | P2P? | Scribe? | LuaRuntime? |
|----------|-------|------|---------|-------------|
| Handshake | `handshake.rs` | Yes | No | No |
| Publish | `publish.rs` | Yes | No | No |
| Permission | `permission.rs` | Yes | No | No |
| Layer sync | `layer_sync.rs` | Yes | Yes | No |
| App sync | `app_sync.rs` | Yes | Yes | No |
| Presence | `presence.rs` | Yes | Yes | Yes |
| Derivation | `derivation.rs` | Yes | Yes | Yes |
| Validation | `validation.rs` | No | No | mlua only |

## Gotchas

- **Layer names must be in app policy.** Writing to a layer not declared in `app.osv` can work locally but will not sync to other peers. Always check the app policy before writing.

- **Sleep after publish.** After `.published()`, add `sleep(Duration::from_millis(500)).await` before writing layers. This gives time for PageAnnounceAck and Scribe subscription setup.

- **App name in `wait_for_app_files`.** The app name parameter must match the `name` field in `manifest.json`, not the directory name. For `osvauld-demos`, the app name is `"Group Chat"`.

- **Scribe normalizes page-scoped names.** Tests usually write bare names like `"messages"`; Scribe resolves them in page context.

- **`ActorScribeHandle` uses `block_on`.** When passing a Scribe to `LuaRuntime`, use `ActorScribeHandle::new(scribe_ref)`. This bridges sync Lua calls to the async Scribe actor. Don't call it from an async context directly.

- **Dynamic layer names are DID-namespaced.** `create_layer("channels/{id}/messages", "general")` produces `channels/{creator_did}/general/messages`. The bare layer name (without page_id prefix) is what you pass to `wait_for_layer_data`.

- **Dynamic layer sync requires `PermitIssuer`.** The node's Scribe needs a `PermitIssuer` to issue layer permits for dynamic layers. Butler wires this automatically for node mode via `ButlerPermitIssuer`.

- **Presence requires policy entry.** Cross-peer presence sync only works if `presence` is defined and permitted in `app.osv`.
