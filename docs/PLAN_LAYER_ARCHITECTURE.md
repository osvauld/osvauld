# Plan: Layer-Centric Architecture

## Problem Statement

The current authorization system has three fundamental issues:

1. **Monolith page permits** — A single page permit lists every layer, every role's permissions, nested `issue_on` delegation chains, and `dynamic_layer_schemas`. The permit_template.json is 957 lines with 6+ copies of the same layer block. Installing a new app requires reissuing every peer's page permit.

2. **Role-based access** — Authorization is tied to role names ("owner", "collaborator", "viewer", "node"). Code checks `allowed_roles.contains(role)` and `role_permissions.get(role)`. This prevents fine-grained capability control.

3. **Eager authorization cache** — `authorized_dids: HashSet<String>` on each LayerUnit is populated from 11+ disconnected call sites, causing race conditions, bugs (viewer writes to dynamic channels fail), and a complex wire protocol chain just to add one DID to a set.

## Design Principles

1. **Layer is the unit of authorization** — Each layer has its own permit. No monolith page permits for data access.
2. **Capabilities, not roles** — Permits say what you CAN DO (read, write, sync), not who you ARE.
3. **Layer + permit travel together** — When sending layer data, include the permit. Authorization is inseparable from data.
4. **Consent is bidirectional** — Layer permit authorizes viewer to write to node. Layer consent authorizes node to write to viewer.
5. **LayerUnit is self-contained** — Owns its CRDT doc, permit config, subscribers, version vectors, and broadcast logic.
6. **page_id is transparent** — Apps never see `{page_id}/` prefixes. Added/stripped at the wire boundary only.

---

## Part 1: New Permit Model

### 1.1 Page Access Permit (minimal)

Authorizes connection to a page's Scribe. No layer data access.

```json
{
  "type": "page_access",
  "page_id": "abc123",
  "presence": { "visible": true, "can_see_others": true },
  "ephemeral_funcs": ["typing"],
  "dynamic_layer_schemas": {
    "channels/{id}/messages": {
      "type": "map",
      "grant": "open",
      "default_capabilities": { "read": true, "write": true, "sync": true }
    },
    "dms/{id}/messages": {
      "type": "map",
      "grant": "explicit",
      "default_capabilities": { "read": true, "write": true, "sync": true }
    }
  }
}
```

What moved OUT:
- `layers` block (now per-layer permits)
- `relationship` field (no roles)
- `operations` (replaced by per-layer capabilities)
- `peer_capabilities` (replaced by per-layer capabilities)
- `issue_on` chains (no delegation nesting)

What stays:
- `presence` config (page-level: are you visible?)
- `ephemeral_funcs` (page-level: what ephemeral events can you send?)
- `dynamic_layer_schemas` (page-level: what patterns can be created?) — but simplified, no role_permissions

### 1.2 Layer Permit

Issued per-layer, per-peer. Authorizes the holder to interact with a specific layer on the issuer's system.

```json
{
  "type": "layer_permit",
  "page_id": "abc123",
  "layer": "channels/general/messages",
  "capabilities": {
    "read": true,
    "write": true,
    "sync": true
  }
}
```

Direction: **Node → Viewer** (or Owner → Node)
Meaning: "I authorize you to read/write/sync this layer on my system"

When the viewer sends an update to the node, the node checks: does this viewer have a layer_permit with `write: true` for this layer?

### 1.3 Layer Consent

Issued by the receiver back to the sender. Authorizes the sender to push updates into the receiver's local data.

```json
{
  "type": "layer_consent",
  "page_id": "abc123",
  "layer": "channels/general/messages",
  "capabilities": {
    "accept_sync": true
  }
}
```

Direction: **Viewer → Node**
Meaning: "I authorize you to push updates for this layer into my local Scribe"

### 1.4 Dynamic Layer Schema (simplified)

No more `role_permissions`. Two grant types:

- `"open"` — anyone with a page_access permit gets a layer_permit when the layer is created (replaces `"role"` with all roles listed)
- `"explicit"` — only explicitly granted DIDs via `add_layer_access()` get a layer_permit

```json
{
  "channels/{id}/messages": {
    "type": "map",
    "grant": "open",
    "default_capabilities": { "read": true, "write": true, "sync": true }
  },
  "dms/{id}/messages": {
    "type": "map",
    "grant": "explicit",
    "default_capabilities": { "read": true, "write": true, "sync": true }
  }
}
```

### 1.5 New permit_template.json

```json
{
  "page_access_template": {
    "presence": { "visible": true, "can_see_others": true },
    "ephemeral_funcs": ["typing"],
    "dynamic_layer_schemas": {
      "channels/{id}/messages": {
        "type": "map",
        "grant": "open",
        "default_capabilities": { "read": true, "write": true, "sync": true }
      },
      "dms/{id}/messages": {
        "type": "map",
        "grant": "explicit",
        "default_capabilities": { "read": true, "write": true, "sync": true }
      }
    }
  },
  "layer_templates": {
    "messages": { "type": "list", "capabilities": { "read": true, "write": true, "sync": true } },
    "reactions": { "type": "map", "capabilities": { "read": true, "write": true, "sync": true } },
    "scores": { "type": "list", "capabilities": { "read": true, "write": true, "sync": true } },
    "obstacles": { "type": "map", "capabilities": { "read": true, "write": true, "sync": true } },
    "sim_config": { "type": "map", "capabilities": { "read": true, "write": true, "sync": true } },
    "navigation": { "type": "map", "capabilities": { "read": true, "write": false, "sync": false } },
    "game_state": { "type": "map", "capabilities": { "read": true, "write": false, "sync": false } },
    "sim_state": { "type": "map", "capabilities": { "read": true, "write": false, "sync": false } },
    "channels/general/messages": { "type": "map", "capabilities": { "read": true, "write": true, "sync": true } },
    "channels/random/messages": { "type": "map", "capabilities": { "read": true, "write": true, "sync": true } },
    "channels/dev/messages": { "type": "map", "capabilities": { "read": true, "write": true, "sync": true } }
  },
  "app_layer_template": {
    "capabilities": { "read": true, "write": false, "sync": true }
  }
}
```

957 lines → ~40 lines. No duplication. No roles. No nested `issue_on`.

---

## Part 2: New page.lua

### 2.1 Layer definitions — capabilities, not roles

```lua
page("Demos", "1.0.0")

-- Static layers
layer("messages", "list", { read = true, write = true, sync = true })
layer("reactions", "map", { read = true, write = true, sync = true })
layer("scores", "list", { read = true, write = true, sync = true })
layer("obstacles", "map", { read = true, write = true, sync = true })
layer("sim_config", "map", { read = true, write = true, sync = true })
layer("navigation", "map", { read = true, write = true, sync = false })
layer("game_state", "map", { read = true, write = true, sync = false })
layer("sim_state", "map", { read = true, write = true, sync = false })

-- Static channels
layer("channels/general/messages", "map", { read = true, write = true, sync = true })
layer("channels/random/messages", "map", { read = true, write = true, sync = true })
layer("channels/dev/messages", "map", { read = true, write = true, sync = true })

-- Dynamic layer schemas
dynamic_layer("channels/{id}/messages", "map", {
    grant = "open",
    read = true, write = true, sync = true,
    validate = function(ops, ctx) ... end
})

dynamic_layer("dms/{id}/messages", "map", {
    grant = "explicit",
    read = true, write = true, sync = true,
})

-- Apps
app("Group Chat", { client = { ui = "group-chat/app.slint", logic = "group-chat/app.lua" } })
app("Snake Game", { client = { ui = "snake-game/app.slint", logic = "snake-game/app.lua", tick = true } })
```

What's gone:
- `role()` definitions entirely
- Per-role capability lists on each layer
- `for_role` on apps (all peers with page_access can run any app)
- `can_share`, `can_delegate`, `can_relay` on roles (now per-layer or per-page-access)

### 2.2 App layers are auto-discovered

When an app is installed, its `app:AppName` layer is created automatically. The node issues a layer permit for it to each peer. No page.lua change needed.

---

## Part 3: Rich LayerUnit

### 3.1 Struct definition

```rust
pub struct LayerUnit {
    // === CRDT ===
    doc: Layer,                          // Loro CRDT document
    dirty: bool,                         // needs persistence
    loro_sub: Option<loro::Subscription>, // observer handle

    // === Authorization (from permit) ===
    config: LayerConfig,                 // cached capabilities: read, write, sync
    permit_token: Option<String>,        // raw permit token (viewer side)
    is_dynamic: bool,                    // created via dynamic schema

    // === Subscriber state (populated on node side) ===
    subscribers: HashMap<String, LayerSubscriber>,  // did → subscriber

    // === Lifecycle ===
    last_accessed: std::time::Instant,   // for cold/warm eviction
}

/// Per-subscriber state within a LayerUnit (node side)
pub struct LayerSubscriber {
    /// What this subscriber can do (issued by us)
    pub permit_capabilities: Capabilities,
    /// Their consent (authorizes us to push to them)
    pub consent_token: Option<String>,
    /// Their last known version vector for incremental sync
    pub version_vector: Vec<u8>,
    /// Channel to send broadcasts
    pub broadcast_tx: mpsc::Sender<BroadcastPayload>,
}

/// Cached capabilities from a permit
#[derive(Debug, Clone)]
pub struct Capabilities {
    pub read: bool,
    pub write: bool,
    pub sync: bool,
}

/// Cached page-level config from page_access permit
pub struct LayerConfig {
    pub capabilities: Capabilities,     // our capabilities for this layer
    pub is_local_only: bool,            // sync: false
}
```

### 3.2 Core methods

```rust
impl LayerUnit {
    // --- Creation ---
    pub fn new(doc: Layer, config: LayerConfig) -> Self;
    pub fn new_empty(config: LayerConfig) -> Self;
    pub fn from_snapshot(snapshot: &[u8], config: LayerConfig) -> Result<Self>;

    // --- Authorization ---
    /// Can we accept a write from this DID?
    pub fn can_accept_write_from(&self, did: &str) -> bool {
        self.subscribers.get(did)
            .map(|s| s.permit_capabilities.write)
            .unwrap_or(false)
    }

    /// Can we push updates to this DID?
    pub fn can_push_to(&self, did: &str) -> bool {
        self.subscribers.get(did)
            .map(|s| s.consent_token.is_some())
            .unwrap_or(false)
    }

    /// Can WE write to this layer? (viewer side check)
    pub fn can_local_write(&self) -> bool {
        self.config.capabilities.write
    }

    // --- Subscriber management ---
    pub fn add_subscriber(&mut self, did: String, capabilities: Capabilities, tx: mpsc::Sender<BroadcastPayload>);
    pub fn remove_subscriber(&mut self, did: &str) -> Option<LayerSubscriber>;
    pub fn set_consent(&mut self, did: &str, consent_token: String);
    pub fn update_subscriber_vector(&mut self, did: &str, vector: Vec<u8>);

    // --- CRDT operations ---
    pub fn apply_update(&mut self, data: &[u8], from_did: Option<&str>) -> Result<()>;
    pub fn export_snapshot(&self) -> Vec<u8>;
    pub fn export_update_for(&self, did: &str) -> Vec<u8>;

    // --- Broadcast ---
    /// Broadcast to all subscribers with consent (exclude sender)
    pub fn broadcast_to_subscribers(&self, exclude_did: Option<&str>);

    // --- Lifecycle ---
    pub fn touch(&mut self);                       // update last_accessed
    pub fn is_cold(&self, threshold: Duration) -> bool;
    pub fn freeze(&mut self) -> Vec<u8>;           // export + drop doc
    pub fn has_subscribers(&self) -> bool;

    // --- Dirty tracking ---
    pub fn mark_dirty(&mut self);
    pub fn take_dirty_snapshot(&mut self) -> Option<Vec<u8>>;
}
```

### 3.3 What moves INTO LayerUnit (from Scribe/subscription/broadcast/apply)

| Currently in | Method | Moves to LayerUnit |
|---|---|---|
| subscription.rs | `authorize_subscriber_for_layers()` | `add_subscriber()` |
| subscription.rs | `send_initial_state_to_subscriber()` | `export_update_for()` |
| broadcast.rs | `broadcast_update()` | `broadcast_to_subscribers()` |
| loro_observer.rs | authorized_dids check in observer | `can_push_to()` in observer |
| apply.rs | `create_layer_from_peer()` auth loop | removed (no eager auth) |
| apply.rs | `update_sender_vector()` | `update_subscriber_vector()` |
| state.rs | `pending_layer_authorizations` | removed entirely |
| actor.rs | `AuthorizeLayerSubscriber` handler | removed entirely |

---

## Part 4: New Scribe (thin router)

### 4.1 Simplified ScribeState

```rust
pub struct ScribeState {
    pub page_id: String,
    pub our_did: String,

    // === Layer management ===
    pub units: HashMap<String, LayerUnit>,

    // === Connection registry (peer lifecycle) ===
    pub connections: HashMap<String, ConnectionInfo>,  // did → connection

    // === Page-level config (from page_access permit) ===
    pub page_config: PageConfig,

    // === Storage ===
    pub layer_storage: LayerStorageRef,
    pub vector_storage: PeerVectorStorageRef,
    pub permit_issuer: Option<PermitIssuerRef>,

    // === Sync ===
    pub sync_config: Option<SyncConfig>,
    pub sync_event_tx: Option<mpsc::Sender<SyncEvent>>,

    // === Observability ===
    pub capture_tx: Option<broadcast::Sender<String>>,
    pub page_update_subscribers: Arc<RwLock<Vec<mpsc::Sender<PageUpdate>>>>,
}

/// Connection-level info (not per-layer)
pub struct ConnectionInfo {
    pub did: String,
    pub device_id: String,
    pub ephemeral_tx: Option<mpsc::Sender<EphemeralOutbound>>,
}

/// Page-level config from page_access permit
pub struct PageConfig {
    pub presence: PresenceConfig,
    pub ephemeral_funcs: Vec<String>,
    pub dynamic_layer_schemas: HashMap<String, DynamicLayerSchema>,
}
```

What's REMOVED from ScribeState:
- `subscribers: Arc<RwLock<HashMap<...>>>` → split into connections (Scribe) + per-layer subscribers (LayerUnit)
- `our_permit` → page_config (page-level) + per-layer config in each LayerUnit
- `pending_layer_authorizations` → gone
- `pending_update_source` → handled within LayerUnit
- `validation_handle` → passed to LayerUnit on creation

### 4.2 Scribe message routing

```rust
// Before: 50+ lines of permission checks, auth, broadcast in each handler
ScribeMessage::ApplyUpdate { layer_name, update, from_peer, .. } => {
    let unit = self.ensure_warm(&layer_name)?;
    unit.apply_update(&update, from_peer.as_deref())?;
    // LayerUnit handles: permission check, CRDT merge, dirty marking
    // LayerUnit's observer handles: broadcast to subscribers, UI notification
}

ScribeMessage::Subscribe { did, device_id, broadcast_tx, ephemeral_tx, page_access_permit, layer_permits } => {
    // 1. Validate page_access_permit
    // 2. Store connection info
    state.connections.insert(did.clone(), ConnectionInfo { did, device_id, ephemeral_tx });

    // 3. For each layer_permit: add subscriber to the LayerUnit
    for (layer_name, permit) in layer_permits {
        let unit = self.ensure_warm(&layer_name)?;
        let capabilities = parse_capabilities(&permit);
        unit.add_subscriber(did.clone(), capabilities, broadcast_tx.clone());
        // Send initial state
        let update = unit.export_update_for(&did);
        broadcast_tx.send(BroadcastPayload { layer_name, update, .. });
    }
}
```

### 4.3 Lifecycle management

```rust
impl Scribe {
    /// Load layer from storage if not in memory
    fn ensure_warm(&mut self, layer_name: &str) -> Result<&mut LayerUnit> {
        if self.state.units.contains_key(layer_name) {
            let unit = self.state.units.get_mut(layer_name).unwrap();
            unit.touch();
            return Ok(unit);
        }

        // Load from storage
        let snapshot = self.state.layer_storage.load_layer(layer_name)?;
        let config = self.state.layer_storage.load_layer_config(layer_name)?;
        let unit = match snapshot {
            Some(data) => LayerUnit::from_snapshot(&data, config)?,
            None => LayerUnit::new_empty(config),
        };
        self.state.units.insert(layer_name.to_string(), unit);
        // Set up Loro observer
        setup_layer_observer(&mut self.state, layer_name);
        Ok(self.state.units.get_mut(layer_name).unwrap())
    }

    /// Evict cold layers (called periodically, e.g. every 60s)
    fn evict_cold_layers(&mut self) {
        let threshold = Duration::from_secs(300);
        let cold: Vec<String> = self.state.units.iter()
            .filter(|(_, u)| u.is_cold(threshold) && !u.has_subscribers())
            .map(|(name, _)| name.clone())
            .collect();

        for name in cold {
            if let Some(mut unit) = self.state.units.remove(&name) {
                if let Some(snapshot) = unit.take_dirty_snapshot() {
                    let _ = self.state.layer_storage.save_layer(&name, &snapshot);
                }
            }
        }
    }
}
```

---

## Part 5: Wire Protocol — Unified Layer Sync

### 5.1 The single sync pattern

Every layer sync follows the same pattern:

```
Sender → Receiver:  LayerSync { layer_name, data, permit }
Receiver → Sender:  LayerConsent { layer_name, consent }
```

This replaces:
- Initial state delivery (send_initial_state_to_subscriber)
- LayerPermit message
- LayerConsentGrant message
- AuthorizeLayerSubscriber internal message
- DistributeLayerPermits coordinator message

### 5.2 New wire messages

```rust
/// Sent when establishing sync for a layer (initial + ongoing discovery)
pub struct LayerSyncMsg {
    pub request_id: String,
    pub page_id: String,
    pub layer_name: String,
    pub data: Vec<u8>,           // snapshot or incremental update
    pub state_vector: Vec<u8>,
    pub permit: String,          // layer permit token
}

/// Response: receiver consents to bidirectional sync
pub struct LayerConsentMsg {
    pub request_id: String,
    pub page_id: String,
    pub layer_name: String,
    pub consent: String,         // consent permit token
}

/// Ongoing updates (after initial LayerSync + Consent)
pub struct LayerUpdateMsg {
    pub page_id: String,
    pub layer_name: String,
    pub data: Vec<u8>,
    pub state_vector: Vec<u8>,
}
```

### 5.3 Flows

**Initial connection:**
```
1. Handshake (page_access permit exchange — auth only)
2. Node issues layer permits for all layers the peer should access
3. For each layer:
     Node → Peer:  LayerSync { data, permit }
     Peer → Node:  LayerConsent { consent }
4. Both sides now have: data + authorization
5. Ongoing: LayerUpdate messages (no permit needed)
```

**New app installed:**
```
1. Node creates app layer, issues layer permits to each peer
2. For each peer:
     Node → Peer:  LayerSync { data: app_code, permit: { read: true, write: false, sync: true } }
     Peer → Node:  LayerConsent { consent }
```

**Dynamic channel created:**
```
1. Owner creates "channels/did:key:alice/general/messages" locally
2. Owner → Node: LayerUpdate (sync_target rule — no permit needed)
3. Node detects dynamic schema match (grant: "open")
4. Node issues layer permits to all peers with page_access
5. For each peer:
     Node → Peer:  LayerSync { data, permit }
     Peer → Node:  LayerConsent { consent }
```

**DM (explicit grant):**
```
1. Creator creates "dms/did:key:alice/room1/messages"
2. Creator → Node: LayerUpdate (sync_target)
3. Creator calls add_layer_access(layer, "did:key:bob")
4. Node issues layer permit for Bob only
5. Node → Bob:  LayerSync { data, permit }
   Bob → Node:  LayerConsent { consent }
```

### 5.4 Removed wire messages

- `LayerPermitMsg` → merged into `LayerSyncMsg`
- `LayerConsentGrantMsg` → replaced by `LayerConsentMsg`
- `LayerConsentAckMsg` → no separate ack needed (consent IS the ack)
- `DistributeLayerPermits` coordinator msg → coordinator sends `LayerSync` directly

### 5.5 Sync target rule

The viewer always syncs all its layers to its sync_target (the node) without needing a permit. The node is a trusted relay. The viewer's LayerUnit doesn't check `can_push_to(node)` — it pushes unconditionally.

This is the ONE special case. Everything else goes through permit + consent.

---

## Part 6: Courier Changes

### 6.1 PeerActor subscription flow

```rust
// Old: Subscribe with one page permit, then complex auth dance
// New: Subscribe with page_access permit + batch of layer permits

pub struct SubscriptionRequest {
    pub page_id: String,
    pub page_access_permit: String,
    pub layer_permits: Vec<(String, String)>,  // (layer_name, layer_permit_token)
}
```

### 6.2 Coordinator changes

- `DistributeLayerPermits` → replaced by direct `LayerSync` messages
- `NewDynamicLayer` SyncEvent → simplified to trigger `LayerSync` to eligible peers
- `LayerAccessChanged` SyncEvent → triggers `LayerSync` to the specific DID

### 6.3 consent.rs simplification

Most of consent.rs becomes the LayerConsent handler:
- Remove `issue_sync_consent` (batched page-level consent)
- Remove `issue_space_consent` / `issue_page_consent`
- Keep only `on_layer_sync` (receive data + permit, issue consent back)
- Keep only `on_layer_consent` (receive consent, store it)

---

## Part 7: Gurkha Changes

### 7.1 New permit types

```rust
pub enum PermitType {
    PageAccess,     // access to Scribe, presence, ephemeral
    LayerPermit,    // per-layer capabilities
    LayerConsent,   // authorization to push to receiver
}

pub struct Capabilities {
    pub read: bool,
    pub write: bool,
    pub sync: bool,
}

pub struct DynamicLayerSchema {
    pub layer_type: String,
    pub grant: DynamicGrant,      // Open or Explicit (not Role)
    pub default_capabilities: Capabilities,
}

pub enum DynamicGrant {
    Open,       // everyone with page_access gets a permit
    Explicit,   // only via add_layer_access()
}
```

### 7.2 Removed from gurkha

- `GrantType::Role` → replaced by `DynamicGrant::Open`
- `role_permissions: HashMap<String, LayerConfig>` → gone
- `relationship()` method → gone (no roles)
- `PeerCapabilities` struct → replaced by per-layer Capabilities
- `can_read_layer()` / `can_write_layer()` with pattern matching → simplified (direct lookup per-layer permit)
- `static_layers()` → moved to page definition, not permit

### 7.3 Kept in gurkha

- UCAN signing/verification
- Permit parsing
- CID computation
- Token encoding/decoding

---

## Part 8: Butler Storage Changes

### 8.1 Per-layer permit storage

```rust
// Old: one page permit per (page_id, did)
store_page_permit(page_id, did, token)

// New: layer permits per (page_id, layer_name, did)
store_layer_permit(page_id, layer_name, did, token)
list_layer_permits(page_id, did) -> Vec<(layer_name, token)>
```

### 8.2 Consent storage

```rust
// Per-layer consent per (page_id, layer_name, did)
store_layer_consent(page_id, layer_name, did, consent_token)
get_layer_consent(page_id, layer_name, did) -> Option<token>
```

### 8.3 What's removed

- `store_viewer_layer_consent` (old consent model)
- `store_viewer_consent` (old batched consent)
- Role-based permit issuance helpers

---

## Part 9: Implementation Phases

### Phase 1: Rich LayerUnit (scribe only)

**Goal**: Make LayerUnit self-contained. No external API changes.

1. Add `subscribers`, `config`, `permit_token`, `last_accessed` to LayerUnit
2. Add methods: `add_subscriber`, `remove_subscriber`, `can_accept_write_from`, `can_push_to`, `broadcast_to_subscribers`, `export_update_for`, `apply_update`
3. Move subscriber VVs from `SubscriberInfo.vectors` into `LayerSubscriber.version_vector`
4. Move broadcast logic from `broadcast.rs` into `LayerUnit::broadcast_to_subscribers()`
5. Move observer authorization check to use `LayerUnit::can_push_to()`
6. Remove `authorized_dids` HashSet
7. Remove `pending_layer_authorizations`
8. Remove `authorize_subscriber_for_layers()`
9. Remove `AuthorizeLayerSubscriber` message handling (becomes no-op or log)
10. Simplify Scribe message handlers to route to LayerUnit methods

**Tests**: Rewrite unit tests in layer_unit/mod.rs. Integration tests should pass with same intent (layers sync correctly).

### Phase 2: Per-layer permits + capabilities (gurkha + scribe)

**Goal**: Replace role-based access with capability-based.

1. Add `Capabilities` struct to gurkha
2. Add `DynamicGrant::Open` / `DynamicGrant::Explicit` (replace `GrantType::Role`)
3. Add `PermitType::LayerPermit` and `PermitType::LayerConsent`
4. Remove `role_permissions` from `DynamicLayerSchema`
5. Remove `relationship()` usage across codebase
6. Update LayerUnit to parse capabilities from layer permit
7. Update page.lua DSL (remove role definitions, use capability flags)
8. Write new permit_template.json

**Tests**: Update gurkha unit tests. Update integration tests for new permit format.

### Phase 3: Unified wire protocol (courier + scribe)

**Goal**: Layer + permit travel together. Consent replaces complex auth chain.

1. Add `LayerSyncMsg` and `LayerConsentMsg` wire messages
2. Update PeerActor subscription flow (page_access + layer permits batch)
3. Implement `on_layer_sync` handler (receive data + permit, issue consent)
4. Implement `on_layer_consent` handler (store consent in LayerUnit)
5. Remove `LayerPermitMsg`, `LayerConsentGrantMsg`, `LayerConsentAckMsg`
6. Simplify Coordinator (`DistributeLayerPermits` → direct LayerSync)
7. Remove `SyncEvent::NewDynamicLayer` for open grants (node sends LayerSync directly)
8. Keep `SyncEvent::NewDynamicLayer` only for triggering LayerSync to eligible peers

**Tests**: Rewrite integration tests (layer_sync.rs). Same intent: static channels sync, dynamic channels sync, viewer writes work, late joiner works.

### Phase 4: Butler storage + lifecycle (butler + scribe)

**Goal**: Per-layer permit storage. Cold/warm layer management.

1. Add per-layer permit storage to Butler
2. Add consent storage to Butler
3. Implement `ensure_warm()` on Scribe
4. Implement `evict_cold_layers()` periodic timer
5. Update layer persistence to co-locate snapshot + permit
6. Remove old page permit storage (or keep for migration)

**Tests**: Storage unit tests. Lifecycle tests (eviction, reload).

### Phase 5: page_id transparency + cleanup

**Goal**: Apps never see page_id. Clean up dead code.

1. Remove all `normalize_layer_name()` calls inside Scribe (bare names only)
2. Add page_id prefix at PeerActor wire boundary only
3. Strip page_id prefix at PeerActor receive boundary only
4. Update Lua API (scribe:map, scribe:list, scribe:create_layer — no page_id)
5. Remove app-level `page_id .. "/" .. layer_path` concatenation in channels.lua etc.
6. Delete dead code: old consent templates, old authorization functions, etc.

**Tests**: Update all sample apps. E2E tests.

---

## Verification

After each phase, the following should work:

```bash
# Build
cargo build -p scribe -p courier -p gurkha -p butler

# Unit tests
cargo test -p scribe
cargo test -p gurkha

# Integration tests (rewritten for new API)
cargo test -p integration_tests -- layer_sync --nocapture

# E2E
python e2e_tests/test_custom_channel.py
```

Intent of each integration test:
1. Static layer syncs owner → node → viewer
2. Dynamic channel (open grant) syncs to all peers
3. Dynamic channel — viewer can write back
4. Dynamic channel — late joiner receives existing data
5. DM (explicit grant) — only granted DID receives data
6. New app install — peers receive app layer without permit reissue
