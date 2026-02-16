# Scribe Internals

Deep dive into the scribe crate: LayerUnit architecture, sync_meta protocol, observer patterns, and authorization model.

## Overview

Scribe is osvauld's CRDT document actor built on ractor and Loro. One Scribe instance per open page, managing all layer documents, bidirectional sync with permission-aware merge logic, and broadcast to peers and UI subscribers.

**Architecture**: Actor model with `ScribeMessage` (42 variants), `ScribeState`, and per-layer `LayerUnit` instances.

---

## LayerUnit Architecture

### Design Philosophy

LayerUnit is a **self-contained compute unit** that consolidates:
- Loro CRDT document
- Dirty tracking for persistence
- Per-subscriber authorization and state
- Observer subscription handle
- Layer-level configuration

**Key insight**: Scribe becomes a **thin router** over LayerUnit instances. Authorization, broadcast, and subscriber management are delegated to each LayerUnit.

### LayerUnit Struct

```rust
pub struct LayerUnit {
    // === CRDT ===
    layer: Layer,                     // Loro CRDT document
    dirty: bool,                      // Needs flush to storage?
    loro_sub: Option<loro::Subscription>, // Observer handle
    
    // === Authorization ===
    config: LayerConfig,              // Our write permission, local-only flag
    pub is_dynamic: bool,             // Created via dynamic schema?
    
    // === Subscriber state ===
    // Arc<RwLock> because observer async task needs shared read access
    subscribers: Arc<RwLock<HashMap<(String, String), LayerSubscriber>>>,
}
```

**Subscriber keying**: `(user_did, device_id)` allows same user on multiple devices with independent version vectors.

### LayerConfig

```rust
pub struct LayerConfig {
    pub can_write: bool,       // Can we write to this layer locally?
    pub is_local_only: bool,   // sync: false — never broadcast to peers
}
```

Set once at layer creation. Describes our own permission for this layer (viewer side) or default permission (node side).

### LayerSubscriber

```rust
pub struct LayerSubscriber {
    pub can_write: bool,              // Can this subscriber write?
    pub version_vector: Vec<u8>,      // Their last known version
    pub broadcast_tx: mpsc::Sender<BroadcastPayload>, // Channel to send updates
}
```

Per-subscriber state created when a peer subscribes with a valid layer permit.

### Core Methods

```rust
impl LayerUnit {
    // --- Creation ---
    pub fn new(layer: Layer) -> Self
    pub fn new_empty() -> Self
    
    // --- Layer access ---
    pub fn layer(&self) -> &Layer
    pub fn replace_layer(&mut self, layer: Layer)
    
    // --- Configuration ---
    pub fn config(&self) -> &LayerConfig
    pub fn is_local_only(&self) -> bool
    pub fn set_local_only(&mut self, val: bool)
    pub fn is_dynamic(&self) -> bool
    
    // --- Dirty tracking ---
    pub fn mark_dirty(&mut self)
    pub fn is_dirty(&self) -> bool
    pub fn take_dirty_snapshot(&mut self) -> Option<Vec<u8>>
    
    // --- Observer lifecycle ---
    pub fn has_observer(&self) -> bool
    pub fn set_observer(&mut self, sub: loro::Subscription)
    
    // --- Subscriber management ---
    pub fn add_subscriber(user_did, device_id, can_write, broadcast_tx)
    pub fn remove_subscriber(user_did, device_id) -> Option<LayerSubscriber>
    pub fn update_subscriber_vector(user_did, device_id, vector)
    pub fn has_subscribers(&self) -> bool
    pub fn subscribers(&self) -> &Arc<RwLock<HashMap<...>>>
    
    // --- Authorization ---
    pub fn can_push_to(user_did, device_id) -> bool
    pub fn can_local_write(&self) -> bool
}
```

### Threading Boundaries

**Critical constraint**: `subscribers` is `Arc<RwLock<HashMap<...>>>` because:
1. LayerUnit is owned by ScribeState (actor state)
2. Loro observer fires synchronously on `commit()` and `import()`
3. Observer callback spawns async broadcast task
4. Broadcast task needs to read subscriber list **concurrently**

This is the **key threading boundary** in scribe.

---

## ScribeState as Thin Router

### Structure

```rust
pub struct ScribeState {
    pub page_id: String,
    pub our_did: String,
    
    // === Layer management ===
    pub units: HashMap<String, LayerUnit>,  // layer_name → LayerUnit
    
    // === Connection registry ===
    pub connections: HashMap<String, ConnectionInfo>,
    
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
```

### Routing Pattern

**Every message handler**:
1. Calls `normalize_layer_name()` at boundary
2. Internal lookups use **bare names** (no `page_id/` prefix)
3. Delegates to LayerUnit for authorization and operations

**Critical invariant**: Layer names are normalized at message entry points.

### Storage Agnosticism

Scribe is storage-agnostic via 4 trait boundaries:
- `LayerStorage` - Persist/load layer data
- `PeerVectorStorage` - Version vector tracking
- `PeerResolver` - Lookup peer permits
- `PermitIssuer` - Delegate permit creation

Butler implements all 4. Tests use null implementations.

---

## Sync Meta Protocol

### What is `__sync_meta:{did}`?

A **regular Loro Map layer** that serves as:
- Discovery catalog (what layers exist)
- Access intent (who should get access)
- Sync status tracker (what's been synced)

**Entry format**: `layer_name → bool`
- `false` = layer discovered but not yet synced
- `true` = layer data has been synced

### Why CRDT for Discovery?

**Both peer and node write** to the same `__sync_meta` CRDT:
- Peer: Creates entries for locally-created dynamic layers
- Node: Populates entries for layers the peer should discover

**Merge conflicts handled naturally by Loro** - no coordination needed, eventually consistent discovery.

### Discovery Flow (9 Steps)

```
1. Creator writes to their local __sync_meta (synced=false)
2. Syncs to node via normal layer sync
3. Node detects new entry on __sync_meta apply, emits SubscribeLayers
4. Coordinator sends LayerSubscribe to creator
5. Creator handles: issues permit, exports snapshot
6. Node receives StoreLayerAuthority: stores, applies data
7. Node writes to each user's __sync_meta (fan-out)
8. Users detect entries in their __sync_meta, send LayerSubscribe
9. Node issues permits, returns snapshots
```

### Late Joiner Population

When a new viewer connects:
1. Node populates their `__sync_meta` with all accessible layers
2. Viewer discovers via normal flow
3. No special case - uses same discovery mechanism

### Key Functions

```rust
// sync_meta.rs

pub fn is_sync_meta_layer(name: &str) -> bool
pub fn sync_meta_layer_name(peer_did: &str) -> String
pub fn ensure_sync_meta_layer(state: &mut ScribeState, peer_did: &str) -> String
pub fn write_sync_meta_entry(state, peer_did, layer_name, synced: bool)
pub fn read_sync_meta_entries(state, peer_did) -> Vec<SyncMetaEntry>
```

**Implementation**: `scribe/src/sync/sync_meta.rs` (~494 lines)

---

## Observer Pattern & Broadcast

### Loro Observer Mechanics

**Loro's `subscribe_root()`** fires **synchronously** on `commit()` and `import()`:
- Callback runs on the same thread that called commit/import
- Runs **during** the CRDT operation, before returning
- Must be fast - spawns async task for actual broadcast

### Dual Broadcast Paths

**Path 1: Local changes**
```
commit() 
  → observer fires synchronously
  → sends through unbounded channel
  → async broadcast task reads channel
  → broadcasts to LayerUnit subscribers
  → emits PageUpdate to UI
```

**Path 2: Remote changes**
```
set pending_update_source before import()
  → import()
  → observer fires (skips peer broadcast due to from_peer flag)
  → apply.rs calls broadcast_update with sender exclusion
```

### pending_update_source Pattern

```rust
// Before importing remote update
state.pending_update_source = Some(from_did);

// Import (observer fires synchronously)
layer.import(&update);

// Clear flag
state.pending_update_source = None;
```

Observer callback checks `pending_update_source` to distinguish local vs remote changes.

### Threading Model

```
ScribeState (actor thread)
    ↓
LayerUnit.commit() (synchronous)
    ↓
Loro observer callback (synchronous, same thread)
    ↓
Send via unbounded channel
    ↓
Async broadcast task (separate tokio task)
    ↓
Read subscribers from Arc<RwLock<...>> (concurrent)
    ↓
Send BroadcastPayload to each subscriber
```

**Why unbounded channel?** Fast CRDT operations shouldn't block on backpressure.

---

## Authorization Model

### Per-Layer Capabilities

**No eager authorization cache**. Authorization checked at operation time:

1. **Local writes** (viewer side):
   ```rust
   if !unit.can_local_write() {
       return Err("Cannot write to this layer");
   }
   ```

2. **Remote writes** (node side):
   ```rust
   if !unit.can_push_to(user_did, device_id) {
       return Err("No write permission");
   }
   ```

### 5-Path Permission Chain

For remote writes, checked in order:

1. **`__sync_meta:{peer_did}` ownership** - Is this their meta layer?
2. **Subscriber permit's can_write** - Per-layer subscriber check
3. **Sync target trust** - Viewer → node unconditional (trusted relay)
4. **Stored permit via PeerResolver** - Lookup cached permit
5. **Dynamic schema check** - via `gurkha::matches_dynamic_schema()`

### Consent Model

**Bidirectional authorization**:
- **Permit**: Authorizes peer to read/write layer
- **Consent**: Authorizes us to push updates to peer

LayerUnit tracks consent per subscriber. Broadcast only to subscribers with consent.

---

## Dynamic Layer Lifecycle

### 9-Step Creation Flow

1. **Lua calls** `scribe:create_layer()`
2. **Scribe validates** schema, generates DID-namespaced path
3. **Creates LayerUnit**, issues self-permit via PermitIssuer
4. **Writes entry** to creator's `__sync_meta` (synced=false)
5. **Node detects** on sync, emits SubscribeLayers event
6. **Coordinator sends** LayerSubscribe to creator
7. **Creator handles**: issues permit, exports snapshot
8. **Node receives** StoreLayerAuthority: stores permit, applies data, fans out
9. **Users detect** entries in their `__sync_meta`, send LayerSubscribe

### Schema Matching

**Dynamic layer schemas** in permit template:
```json
{
  "channels/{id}/messages": {
    "type": "map",
    "grant": "role",
    "role_permissions": { ... }
  }
}
```

**Runtime path** generation:
```
Pattern: "channels/{id}/messages"
Creator DID: "did:key:alice"
Result: "channels/did:key:alice/general/messages"
```

Node matches pattern → determines eligible peers → issues permits.

### Grant Types

- **`"role"`**: All peers with matching role get permit
- **`"explicit"`**: Only creator (+ named participants in future)

---

## Layer Name Normalization

### The Invariant

**Internal lookups always use bare names** (no `page_id/` prefix).

**At message boundaries**, every handler calls:
```rust
let bare = normalize_layer_name(layer_name, &state.page_id);
```

**Why**: Consistency. Scribe's internal `units` map keys are bare names.

### normalize_layer_name()

```rust
pub fn normalize_layer_name(name: &str, page_id: &str) -> String {
    // Strip page_id prefix if present
    let prefix = format!("{}/", page_id);
    name.strip_prefix(&prefix)
        .map(|s| s.to_string())
        .unwrap_or_else(|| name.to_string())
}
```

**Usage**: Every `ScribeMessage` handler calls this before lookups.

---

## Key Files

| File | Lines | Purpose |
|------|-------|---------|
| `actor.rs` | ~1325 | Main message dispatch, pre_start initialization |
| `state.rs` | ~561 | ScribeState, SubscriberInfo, SyncMode |
| `message.rs` | ~726 | 42 ScribeMessage variants, BroadcastPayload, PageUpdate |
| `layer_unit/mod.rs` | ~680 | LayerUnit implementation |
| `layer_unit/dynamic.rs` | ~371 | Dynamic layer creation, schema matching |
| `sync/apply.rs` | ~540 | Update application pipeline (5-step) |
| `sync/subscription.rs` | ~299 | Subscribe/unsubscribe, initial state delivery |
| `sync/broadcast.rs` | ~118 | Per-subscriber incremental export |
| `sync/sync_meta.rs` | ~494 | Sync metadata protocol |
| `loro_observer.rs` | ~561 | Observer setup, delta extraction, broadcast task |
| `operations.rs` | ~334 | Typed CRDT operations (list/map/counter) |
| `permit.rs` | ~430 | PermitContext, glob_match, Permissions |
| `storage.rs` | ~290 | Four storage traits + Null implementations |
| `validation_handle.rs` | ~78 | Channel-based validation delegation |
| `ephemeral.rs` | ~197 | Ephemeral routing, structured ephemeral |

---

## Gotchas

- **Timing race mitigation**: `HandleLayerSubscribe` creates empty dynamic layer if schema matches but data hasn't arrived yet
- **Reconciliation**: Runs every 30s only in Broadcast mode (prevents sync storms in ToSource mode)
- **Flush interval**: Every 10s for dirty layers
- **Observer subscriptions**: Dropped = unsubscribed (RAII cleanup)
- **ensure_sender_subscribed**: Handles timing race where LayerUnit exists but sender isn't in subscriber list yet
- **pre_start workload**: Permit parsing, timer spawning, observer setup, static layer pre-creation

---

## Cross-References

- [ARCHITECTURE.md](ARCHITECTURE.md) - High-level crate overview
- [PROTOCOL.md](PROTOCOL.md) - Wire protocol messages (SyncOffer, LayerSubscribe, etc.)
- [DATA_MODEL.md](DATA_MODEL.md) - Layer/Page/Space hierarchy, sync_meta protocol
- [GURKHA_INTERNALS.md](GURKHA_INTERNALS.md) - Permit authorization engine
- `.opencode/agents/scribe.md` - Scribe specialist agent

---

## Related Agent

See `.opencode/agents/scribe.md` for the scribe specialist agent with detailed crate knowledge.
