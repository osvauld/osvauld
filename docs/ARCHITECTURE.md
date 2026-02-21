# Osvauld Architecture

System architecture, crate boundaries, and data flow patterns.

## Overview

Osvauld is a layered P2P platform built on QUIC (via iroh) with capability-based authorization (UCAN permits). The system separates concerns into distinct crates with clear boundaries.

```
┌─────────────────────────────────────────────────────────────────┐
│  Application Layer                                               │
│  Lua apps, Slint/Raylib renderers, business logic               │
├─────────────────────────────────────────────────────────────────┤
│  Policy Declaration + Typed Model                                │
│  - osv_decl compiles app.osv                                     │
│  - policy_model defines shared typed policy contracts            │
├─────────────────────────────────────────────────────────────────┤
│  Domains (Shared Types)                                          │
│  - Shared domain types across crates                             │
├─────────────────────────────────────────────────────────────────┤
│  Butler (Storage + Services)                                     │
│  - PageService: page lifecycle, layer management                │
│  - BlobService: asset storage via iroh-blobs                    │
├─────────────────────────────────────────────────────────────────┤
│  Scribe (CRDT Document Actor)                                    │
│  - Loro CRDT documents per layer (extracted from butler)         │
├─────────────────────────────────────────────────────────────────┤
│  Courier (P2P Orchestration)                                     │
│  - Coordinator: manages all peer connections                    │
│  - PeerActor: one per connected peer, handles messages          │
│  - SyncManager: 3-step sync protocol                            │
├─────────────────────────────────────────────────────────────────┤
│  Gurkha (Authorization)                                          │
│  - Permit parsing and validation                                │
│  - Capability extraction from UCAN facts                        │
│  - Authorization decisions                                      │
├─────────────────────────────────────────────────────────────────┤
│  Transport (Networking)                                          │
│  - QUIC connections via iroh                                    │
│  - Bidirectional streams for messages                           │
│  - Datagrams for ephemeral data                                 │
│  - Blob transfer protocol                                       │
├─────────────────────────────────────────────────────────────────┤
│  Herald (Identity)                                               │
│  - Ed25519 keypairs for signing                                 │
│  - X25519 for ECDH encryption                                   │
│  - DID generation and parsing                                   │
└─────────────────────────────────────────────────────────────────┘
```

## Crate Boundaries

### Herald (Foundation)

**Purpose**: Cryptographic identity primitives.

**Public Types**:
- `Identity`: Ed25519 + X25519 keypair bundle
- `KeyPair`: Individual keypair
- `Did`: Decentralized identifier (did:key:...)

**Operations**:
- `sign(message)` / `verify(message, signature)`
- `encrypt(plaintext, recipient_public_key)` / `decrypt(ciphertext)`
- `did_from_public_key()` / `public_key_from_did()`

**Dependencies**: None (foundation crate)

**Boundary Rule**: Herald knows nothing about permits, networking, or storage.

---

### Gurkha (Authorization)

**Purpose**: Parse and validate UCAN permits, extract capabilities, and evaluate typed policy facts.

**Public Types**:
- `Permit`: Parsed UCAN token with extracted capabilities
- `Capabilities`: What a permit allows (read, write, relay, etc.)
- `PermitValidator`: Validates permit chains

**Operations**:
- `parse_permit(token_string)` → `Permit`
- `validate_permit(permit, expected_audience)` → `Result<()>`
- `extract_capabilities(permit)` → `Capabilities`
- `can_write_layer(permit, layer_name)` → `bool`

**Dependencies**: Herald (for signature verification)

**Boundary Rule**: Gurkha only parses and validates. It doesn't store permits or make protocol decisions.

---

### policy_model (Shared Typed Policy Contract)

**Purpose**: Canonical typed policy schema shared between compilation and authorization.

**Public Types**:
- `PolicyFacts`, `PolicyRule`, `DelegationRule`
- `DynamicLayerSchema`, `ResourceSelector`, `Subject`, `Action`

**Dependencies**: serde only

**Boundary Rule**: Pure data model crate. No runtime, storage, or transport logic.

---

### osv_decl (app.osv Compiler)

**Purpose**: Parse/lower `app.osv` declarations into runtime artifacts and typed policy facts.

**Operations**:
- `compile_source(...)` / lowering pipeline
- Emit `permit.osv_policy` as typed `PolicyFacts`

**Dependencies**: policy_model, serde

**Boundary Rule**: Compile-time declaration processing only; no permit issuance/network I/O.

---

### Transport (Networking)

**Purpose**: QUIC connections, streams, and blob transfer.

**Public Types**:
- `Transport`: Main networking handle
- `Connection`: Single QUIC connection to a peer
- `Stream`: Bidirectional stream for messages

**Operations**:
- `connect(node_addr)` → `Connection`
- `accept()` → `Connection`
- `open_stream()` → `Stream`
- `send_datagram(data)` / `recv_datagram()` → ephemeral data
- `download_blob(hash)` / `serve_blob(data)`

**Dependencies**: iroh, iroh-blobs

**Boundary Rule**: Transport is a "dumb byte pipe". It has no knowledge of message types, permits, or sync protocol. It just moves bytes.

---

### Butler (Storage + Services)

**Purpose**: Application storage, services API, Loro CRDT management.

**Public Types**:
- `Butler`: Main service container
- `PageService`: Page lifecycle management
- `Scribe`: Actor that manages a Loro document for a layer
- `BlobService`: Asset storage

**Operations**:
- `create_space(metadata)` / `get_space(id)`
- `create_page(space_id, metadata)` / `get_page(id)`
- `get_or_create_layer(page_id, layer_name, type)` → `LayerHandle`
- `apply_sync_data(layer, delta)` / `get_sync_data(layer)` → Loro operations

**Dependencies**: Herald, Gurkha, Loro

**Boundary Rule**: Butler exposes a services API. External code never accesses stores directly. Butler validates permits before mutations.

---

### Scribe (CRDT Document Actor)

**Purpose**: Per-page CRDT document management with Loro.

**Core Design**: LayerUnit-based architecture with per-subscriber authorization, sync_meta discovery protocol, and storage-agnostic trait boundaries.

**See**: `docs/SCRIBE_INTERNALS.md` for deep dive on LayerUnit architecture, sync_meta protocol, observer patterns, and authorization model.

**Dependencies**: Domains, Gurkha, Herald

**Boundary Rule**: Storage-agnostic via 4 trait boundaries (LayerStorage, PeerVectorStorage, PeerResolver, PermitIssuer). Never holds signing keys.

---

### Courier (P2P Orchestration)

**Purpose**: P2P protocol implementation, handshakes, sync orchestration.

**Components**:

```
                    ┌─────────────────────────┐
                    │     Coordinator         │
                    │  (mod.rs + state.rs)    │
                    │  - PeerEntry tracking   │
                    │  - Connection routing   │
                    └──────────┬──────────────┘
                               │
           ┌───────────────────┼───────────────┐
           │                   │               │
    ┌──────┴──────┐     ┌──────┴──────┐ ┌──────┴──────┐
    │  PeerActor  │     │  PeerActor  │ │  PeerActor  │
    │  (Node A)   │     │  (Node B)   │ │  (Viewer)   │
    │             │     │             │ │             │
    │ - handshake │     │ - handshake │ │ - handshake │
    │ - sync      │     │ - sync      │ │ - sync      │
    │ - publish   │     │ - publish   │ │ - publish   │
    └─────────────┘     └─────────────┘ └─────────────┘
```

**Coordinator** (`courier/src/coordinator/`):
- **mod.rs**: Main coordinator logic, message routing, event handling
- **state.rs**: Peer lifecycle management via `PeerEntry` and `PeerInfo`
- Spawns/removes PeerActors dynamically
- Routes app-level events (publish, sync requests) to appropriate peers
- Tracks connection state (AwaitingHandshake, Authenticated, Failed)

**PeerEntry** (coordinator state abstraction):
```rust
struct PeerEntry {
    info: PeerInfo,              // DID, pubkey, role
    actor_handle: ActorHandle,   // Channel to PeerActor
    state: ConnectionState,      // Handshake progress
    last_connected: Timestamp,   // For reconnection logic
}
```

**PeerActor** (`courier/src/peer_actor/`):
- One actor per connected peer (spawned via `tokio::spawn`)
- **handshake.rs**: Handshake state machine, decision execution
- **sync/**: 3-step sync protocol (SyncOffer/Accept/Ack)
- **publish.rs**: Publishing flow (PublishSpace/Page)
- Isolated state per peer (no mutex contention)

**Decision Delegation Pattern** (`courier/src/handshake/decision.rs`):
Pure decision functions separate from handlers:
- `decide_hello_response()`: Returns `HelloDecision` (AcceptFirstConnection, AcceptReconnection, Reject*)
- `decide_welcome_response()`: Returns `WelcomeDecision` (Accept, RejectNodeMismatch, RejectAudienceMismatch)
- `extract_capabilities()`: Parses permit facts into `Capabilities`
- **Benefits**: Testable without I/O, clear separation of policy vs execution

**Handshake Flows**:
- **First connection** (4-step): Hello → Welcome → PermitGrant → Ack
- **Reconnection** (3-step): Hello → Welcome → Ack (uses stored permits)
- **Hardening**: Node identity verification, audience validation, owner DID matching

**Dependencies**: Transport, Butler, Gurkha

**Boundary Rule**: Courier has channel-based communication with apps. No UI context leaks in.

---

### Application Stack

**Purpose**: Execute Lua apps with Slint or Raylib rendering.

**Crate hierarchy**:
```
sthalam_shell (entry point, platform main)
  └── sthalam (app browser, page management)
       ├── renderer_slint (Slint UI rendering)
       ├── renderer_raylib (Raylib graphics rendering)
       └── lua_runtime (Mlua VM with osvauld bindings)
            └── scribe (CRDT document actor, extracted from butler)
```

**Components**:
- `lua_runtime`: Mlua VM with bindings for `loro:`, `permit:`, `ui:`, `api:`, `butler:`, `derivation:`
- `renderer_slint`: Compiles .slint files, bridges Lua ↔ Slint AppAPI
- `renderer_raylib`: Immediate-mode rendering via Lua raylib bindings
- `scribe`: Per-page Loro CRDT actor, manages layers, sync, and persistence
- `sthalam`: App browser that loads pages, manages renderers, handles control server

**App Lifecycle**:
1. Load manifest.json (unified `domains::AppManifest` type used by sthalam, kunki, and renderers)
2. Route to renderer: Compile .slint UI (Slint) or initialize window (Raylib)
3. Execute app.lua in Lua VM
4. Call `on_init()`
5. Route events: `on_click()`, `on_layer_discovered()`, `on_ephemeral()`, input callbacks

**Manifest Ownership**: The `domains::AppManifest` type is the canonical manifest representation across the system. Legacy per-crate manifest types (renderer_raylib::GameManifest, kunki local manifest struct) have been removed. The renderer_slint manifest is now a type alias of `domains::AppManifest`.

**Renderer Modularization**: renderer_slint was split into stable modules (launch.rs, prepare.rs, types.rs, value_convert.rs) with unchanged public behavior. lua_runtime internals were split (runtime.rs + runtime/engine.rs + runtime/init_impl.rs), mostly internal refactor.

See `docs/app-dev/` for full app development documentation.

---

## Data Flow Patterns

### App → Storage → Sync

```
┌─────────┐     ┌────────┐     ┌─────────┐     ┌──────────┐
│ Lua App │────>│ Butler │────>│ Scribe  │────>│ Courier  │
│         │     │        │     │ (Loro)  │     │          │
│ layer:  │     │ layer  │     │ apply   │     │ SyncOffer│
│ push()  │     │ write  │     │ delta   │     │ to peer  │
└─────────┘     └────────┘     └─────────┘     └──────────┘
```

### Sync Receive

```
┌──────────┐     ┌─────────┐     ┌────────┐     ┌─────────┐
│ Courier  │────>│ Scribe  │────>│ Butler │────>│ Lua App │
│          │     │ (Loro)  │     │        │     │         │
│ SyncOffer│     │ merge   │     │ notify │     │ on_loro │
│ received │     │ delta   │     │ change │     │ _change │
└──────────┘     └─────────┘     └────────┘     └─────────┘
```

### Ephemeral Messages

```
┌─────────┐     ┌──────────┐     ┌───────────┐     ┌─────────┐
│ Lua App │────>│ Butler   │────>│ Transport │────>│ Peer    │
│         │     │          │     │           │     │         │
│ send_   │     │ route to │     │ datagram  │     │ on_     │
│ephemeral│     │ peer     │     │ send      │     │ephemeral│
└─────────┘     └──────────┘     └───────────┘     └─────────┘
```

## Actor Model

Courier uses an actor model for concurrent peer management:

```rust
// Coordinator spawns PeerActors
let peer_actor = PeerActor::spawn(connection, peer_info);

// Messages via channels
peer_actor.send(Message::SyncOffer { ... }).await;

// Events back to coordinator
match peer_actor.recv().await {
    Event::SyncComplete { page_id } => { ... }
    Event::Disconnected => { ... }
}
```

Benefits:
- Each peer has isolated state
- No mutex contention between peers
- Clean error handling per peer
- Easy to add/remove peers dynamically

## Error Handling

Each crate wraps dependency errors:

```rust
// In courier
#[derive(Error)]
pub enum CourierError {
    #[error("transport error: {0}")]
    Transport(#[from] transport::TransportError),

    #[error("butler error: {0}")]
    Butler(#[from] butler::ButlerError),

    #[error("permit validation failed: {0}")]
    PermitInvalid(String),
}
```

Errors don't leak across boundaries. Each crate presents its own error types.

## App Refresh Lifecycle

When a developer updates app files and triggers `refresh_app` (via control server RPC or Slint UI button):

```
Developer edits files on disk
        │
        ▼
refresh_app RPC (control server socket)
        │
        ▼
Butler AppsApi::update()
  - Reads files from disk
  - Sends ScribeMessage::RefreshAppFiles to Scribe
        │
        ▼
Scribe actor (RefreshAppFiles handler)
  - Diffs incoming files against current app: layer
  - Writes updated files to app:{name} CRDT layer via set_all_files()
  - Commits → Loro observer fires
        │
        ├──► Loro observer broadcasts LayerChanged to all layer subscribers
        │           (connected peers receive updated app files via SyncOffer/SyncAccept)
        │
        └──► Local renderer detects app-layer change → restarts Lua runtime
                  │
                  ▼
            on_init() runs in fresh Lua VM
                  │
                  ▼
            Startup discovery replay:
              runtime fires on_layer_discovered for every existing non-protocol layer
              (DMs, custom channels, etc. reappear without app-specific hydration code)
```

Key properties:
- **Offline-first**: refresh writes locally first; peers catch up via CRDT sync on reconnect
- **No data loss**: only `app:{name}` layer is updated; data layers (`dms/*`, `channels/*`, `presence`, etc.) are untouched
- **Transparent to app devs**: dynamic layer state is restored via startup replay, not manual app code

## Threading Model

```
┌─────────────────────────────────────────────────────────┐
│                    Main Thread                          │
│  - Slint UI event loop                                  │
│  - App lifecycle                                        │
└─────────────────────────────────────────────────────────┘
                          │
                          │ channels
                          ▼
┌─────────────────────────────────────────────────────────┐
│                   Tokio Runtime                         │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐     │
│  │ Coordinator │  │ PeerActor 1 │  │ PeerActor 2 │     │
│  │   (task)    │  │   (task)    │  │   (task)    │     │
│  └─────────────┘  └─────────────┘  └─────────────┘     │
│                                                         │
│  ┌─────────────┐  ┌─────────────┐                      │
│  │   Scribe    │  │ BlobService │                      │
│  │   (task)    │  │   (task)    │                      │
│  └─────────────┘  └─────────────┘                      │
└─────────────────────────────────────────────────────────┘
```

- UI runs on main thread
- All I/O and networking on Tokio runtime
- Communication via async channels
- `tokio::spawn` for async event loops
