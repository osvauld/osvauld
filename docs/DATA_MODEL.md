# Data Model

Space, Page, Layer hierarchy and encryption model in osvauld.

## Overview

Osvauld organizes data in a hierarchical structure:

```
Identity (DID)
    │
    └── Space
            │
            ├── Page 1
            │   ├── Layer A (map)
            │   ├── Layer B (list)
            │   └── Layer C (text)
            │
            └── Page 2
                ├── Layer X
                └── Layer Y
```

## Identity

Every user has a cryptographic identity:

```
┌─────────────────────────────────────────┐
│  Identity                               │
│  ┌─────────────────────────────────────┐│
│  │ DID: did:key:z6MkhaXg...            ││
│  │                                      ││
│  │ Ed25519 Keypair (signing)           ││
│  │ X25519 Keypair (encryption)         ││
│  └─────────────────────────────────────┘│
└─────────────────────────────────────────┘
```

- **DID**: Decentralized identifier derived from public key
- **Ed25519**: For signing messages and permits
- **X25519**: For ECDH encryption of layer data

## Space

A Space is a container for related Pages, like a project or workspace.

```rust
struct Space {
    id: String,           // Unique identifier
    name: String,         // Display name
    description: String,  // Optional description
    owner_did: String,    // Creator's DID
    created_at: i64,      // Unix timestamp
    apps: Vec<AppRef>,    // Apps available in this space
}
```

**Characteristics**:
- Has a single owner (creator)
- Can be shared with nodes and viewers
- Contains one or more pages
- Defines which apps are available

## Page

A Page is a collaborative document within a Space.

```rust
struct Page {
    id: String,           // Unique identifier
    space_id: String,     // Parent space
    name: String,         // Display name
    app_id: String,       // App that renders this page
    owner_did: String,    // Creator's DID
    created_at: i64,
    layers: HashMap<String, Layer>,
}
```

**Characteristics**:
- Belongs to exactly one space
- Has a designated app for rendering
- Contains multiple layers for data storage
- Can have different sharing settings than parent space

## Layer

A Layer is a CRDT document for storing app data. Layers use [Loro](https://loro.dev/) for conflict-free synchronization.

### Layer Types

| Type | Loro Container | Use Case |
|------|----------------|----------|
| `map` | `LoroMap` | Key-value data, objects |
| `list` | `LoroList` | Ordered collections, arrays |
| `text` | `LoroText` | Rich text, collaborative editing |

### Layer Naming

Layers are named with a hierarchical path:

```
{page_id}/shapes          # Map of shapes in canvas
{page_id}/messages        # List of chat messages
{page_id}/orders/{did}    # Per-user order layer
app:Canvas                # App-level shared config
```

**Patterns**:
- `{page_id}/*` - Page-specific data
- `{page_id}/*/*` - Per-user data within page (e.g., `{page_id}/orders/{user_did}`)
- `app:*` - App-level configuration

### Layer Configuration

Each layer has sync and write settings:

```json
{
  "{page_id}/shapes": {
    "type": "map",
    "sync": true,    // Sync to peers
    "write": true    // Allow mutations
  },
  "{page_id}/local_state": {
    "type": "map",
    "sync": false,   // Local only
    "write": true
  }
}
```

## Loro CRDT Operations

### Map Operations

```lua
-- Get or create a map layer
local shapes = loro:get_or_create_layer(page_id .. "/shapes", "map")

-- Set value
shapes:set("shape_1", { x = 100, y = 200, width = 50 })

-- Get value
local shape = shapes:get("shape_1")

-- Delete key
shapes:delete("shape_1")

-- Get all keys
local keys = shapes:keys()
```

### List Operations

```lua
-- Get or create a list layer
local messages = loro:get_or_create_layer(page_id .. "/messages", "list")

-- Append to list
messages:push({ text = "Hello", sender = my_did })

-- Get by index (0-based)
local msg = messages:get(0)

-- Update at index
messages:set(0, { text = "Hello, edited", sender = my_did })

-- Get length
local count = messages:length()
```

### Text Operations

```lua
-- Get or create a text layer
local doc = loro:get_or_create_layer(page_id .. "/document", "text")

-- Insert text at position
doc:insert(0, "Hello ")

-- Delete range
doc:delete(0, 6)

-- Get full text
local content = doc:to_string()
```

## Sync Model

### State Vectors

Loro tracks changes using version vectors. Each peer has a unique version number, and state vectors record the latest version seen from each peer.

```
State Vector: { peer_a: 5, peer_b: 3, peer_c: 7 }
```

### Sync Protocol

1. **SyncOffer**: Sender sends their state vector and changes since receiver's known state
2. **SyncAccept**: Receiver merges changes, sends their state vector
3. **SyncAck**: Sender confirms receipt, may send more if vectors diverged

### Conflict Resolution

Loro handles conflicts automatically:
- **Map**: Last-writer-wins for same key
- **List**: Interleaving for concurrent inserts
- **Text**: Character-level merging

### sync_meta Protocol Layer

Each peer has a `__sync_meta:{did}` layer that serves as:
- **Discovery catalog**: What layers exist and should be synced
- **Access intent**: Which layers a peer should discover (written by node for users)
- **Sync status tracker**: `false` = discovered but not synced, `true` = synced

#### Ownership and Node/User Modes

| Layer | Created By | Written By | Syncs To |
|-------|------------|------------|----------|
| `__sync_meta:{creator_did}` | Node | Creator | Creator→Node |
| `__sync_meta:{user_did}` | Node | Node | Node→User |

**Creator**: The owner who creates dynamic layers. Their `__sync_meta` layer is created by the Node but written to by themselves.

**Node**: Creates `__sync_meta` layers for both creators and users. For creators, it subscribes to the creator's `__sync_meta`. For users, it writes entries to their `__sync_meta`.

**User**: Has their `__sync_meta` layer created and written by the Node. They subscribe to it to discover layers.

#### Flow Example: Creator → Node → User

```
Creator                         Node                        User
  │                              │                           │
  │ create_dynamic_layer()       │                           │
  │ ────────────────────────────>│                           │
  │   writes to __sync_meta:self │                           │
  │   (synced=false)             │                           │
  │                              │                           │
  │    SyncOffer(__sync_meta)    │                           │
  │ ────────────────────────────>│                           │
  │                              │ detect_new_dynamic_layers()│
  │                              │ issue LayerSubscribe ─────>│
  │                              │                           │
  │ <──── LayerSubscribe ────────│                           │
  │   (from Node)                │                           │
  │                              │                           │
  │ LayerSubscribeAck ──────────>│                           │
  │   (authority permit)         │                           │
  │                              │ store authority           │
  │                              │ mark creator's entry synced│
  │                              │ fan out to users ────────>│
  │                              │   writes to __sync_meta:user│
  │                              │                           │
  │                              │<──── SyncOffer ────────────│
  │                              │    (user's __sync_meta)   │
  │                              │                           │
  │                              │  check_sync_meta_and_subscribe()
  │                              │<──── LayerSubscribe ──────│
  │                              │                           │
  │                              │ LayerSubscribeAck ────────>│
  │                              │   (layer_permit)          │
  │                              │                           │
  │                              │           mark own entry synced
```

#### Entry Marking Responsibility

- **Creator's `__sync_meta`**: Node marks entries synced in `handle_store_layer_authority`
- **User's `__sync_meta`**: User marks entries synced in `handle_layer_permit_ack` via `MarkSyncMetaSynced`

The same `scribe` crate code runs on both Nodes and Users (Creator-side), with behavior switching based on:
- **Node mode**: Creates `__sync_meta` layers for others, writes to users' layers
- **User mode**: Subscribes to their own `__sync_meta`, marks entries synced upon reception

## Encryption

### At Rest

All data is encrypted before storage:

```
┌────────────────────────────────────────────┐
│  Encrypted Layer Data                       │
│  ┌────────────────────────────────────────┐│
│  │ Nonce (12 bytes)                       ││
│  │ Ciphertext (ChaCha20Poly1305)          ││
│  │ Tag (16 bytes)                         ││
│  └────────────────────────────────────────┘│
└────────────────────────────────────────────┘
```

### In Transit

Sync data is encrypted using per-connection session keys:

```
Handshake (once per connection)
  │
  │  Both sides derive session key:
  │  session_key = HKDF(
  │    ECDH(our_static_secret, peer_static_public),
  │    "herald-session-v1"
  │  )
  │
Per-message (reuses session key)
  │
Sender                            Receiver
  │                                  │
  │  encrypted = AES-GCM(            │
  │    session_key,                  │
  │    random_nonce,                 │
  │    plaintext                     │
  │  )                               │
  │  - nonce || ciphertext || tag    │
  │─────────────────────────────────>│
  │                                  │
  │              Uses same session_key  │
  │              (derived at handshake) │
  │              decrypt                │
```
Sender                              Receiver
  │                                     │
  │  Generate ephemeral keypair         │
  │  ephemeral_pub, ephemeral_priv      │
  │                                     │
  │  ECDH: shared = ephemeral_priv      │
  │        × receiver_public            │
  │                                     │
  │  SyncOffer                          │
  │  - ephemeral_public                 │
  │  - encrypted_data                   │
  │───────────────────────────────────>│
  │                                     │
  │              ECDH: shared =         │
  │              receiver_priv ×        │
  │              ephemeral_public       │
  │                                     │
  │              Decrypt data           │
```

### Key Hierarchy

```
Identity Key (long-term)
    │
    └── Page Key (derived per page)
            │
            └── Layer Key (derived per layer)
```

## AppManifest

Every app directory contains a `manifest.json` that is deserialized as `domains::AppManifest`:

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `name` | string | (required) | Display name |
| `version` | string | (required) | Semver version |
| `entry_logic` | string | (required) | Path to app.lua |
| `renderer` | string | `"slint"` | `"slint"` or `"raylib"` |
| `entry_ui` | string | `None` | Path to .slint file (Slint only) |
| `models` | string[] | `[]` | VecModel names (Slint only) |
| `width` | u32 | `800` | Window width (Raylib) |
| `height` | u32 | `600` | Window height (Raylib) |
| `target_fps` | u32 | `60` | Target FPS (Raylib) |
| `entry_node` | string | `None` | Path to node.lua (kunki/headless runtime) |

The `domains::AppManifest` type is shared across sthalam (desktop app), kunki (headless node runtime), and both renderers (renderer_slint, renderer_raylib). See [MANIFEST.md](app-dev/MANIFEST.md) for usage patterns.

---

## Derivations

Nodes can run derivation rules to aggregate data:

```lua
-- Aggregate orders from all users into summary
derivation:register({
    source = page_id .. "/orders/*",
    target = page_id .. "/derived/orders_summary",

    key_fn = function(order)
        return order.id
    end,

    transform = function(source_layer, order)
        return {
            id = order.id,
            customer = extract_did(source_layer),
            status = order.status,
            total = order.total,
        }
    end,

    filter = function(order)
        return order.status ~= "draft"
    end,
})
```

Derivations:
- Run only on nodes (headless runtime)
- Create read-only computed layers
- Update automatically when source changes
- Enable aggregation across per-user layers

## Examples

### E-commerce App

```
my-shop/
  {page_id}/products         # map: all products (owner writes)
  {page_id}/orders/{did}     # list: per-customer orders
  {page_id}/derived/orders_summary  # map: aggregated (node derives)
```

### Collaborative Canvas

```
canvas/
  {page_id}/shapes           # map: all shapes
  {page_id}/connectors       # map: connections between shapes
```

### Chat App

```
group-chat/
  {page_id}/messages         # list: all messages
  {page_id}/reactions        # map: message_id -> reactions
```
