# Osvauld Protocol Rebuild: Node-Centric Architecture

## Overview

Rebuild the Osvauld protocol to be node-centric (Kunki), replacing the messy share_records system with UCAN-based cryptographic tracking, and implementing aggressive push for node-to-node sync.

**Key insight**: This is also a data model redesign:
- **Folder** = Application (not just a collection)
- **Resource** = Single document (not multi-document)
- **Template** = New concept between Folder and Resources

## Design Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Node required? | **Optional** - auto-detect | Direct P2P fallback when no node |
| Tracking | **UCAN `fct` field** | Cryptographic, verifiable delegation chain |
| Push strategy | **Node-to-Node: aggressive push** | On every change, immediate sync |
| Direct connections | **Pull-based** | Battery efficient for devices |
| Migration | **Auto-migrate** | When user gets node, existing shares route through it |
| Revocation | **Defer** | Design later, focus on core architecture first |
| Resource model | **1 doc per resource** | Simpler model, comments/submissions = separate resources |

## Target Hardware

**Orange Pi 6 Plus (16GB)**
- 12-core CIX CD8180, 16GB LPDDR5
- Dual 5Gbps Ethernet (limited by 100Mbps internet)
- Iroh tested at 200k concurrent connections
- Estimated capacity: 1,000-5,000 concurrent connections at 100Mbps

---

## Architecture

### Current Problems

1. **Share Records Mess**: Dual-permit pattern in 5+ locations, ephemeral vs persistent confusion
2. **No Node-Centric Design**: Direct P2P to multiple peers, battery drain
3. **Pull-Only Sync**: No proactive push, owner is single point of failure
4. **Multi-Doc Resources**: Resource has `HashMap<String, LoroDoc>` - complex, unclear boundaries

### New Hierarchy

```
Folder (Application)
  └── Templates (HUML templates)
        └── Resources (1 doc each)
              - content_resource   (main Loro doc)
              - comments_resource  (separate Loro doc)
              - submissions_resource (separate Loro doc)
              - assets_resource    (binary blobs)
```

### New Topology

```
┌─────────────────────────────────────────────────────────────┐
│  User Device ──single conn──→ Personal Node (Kunki)         │
│       │                              │                       │
│  (battery                    ┌───────┴───────┐              │
│   efficient)                 ▼               ▼              │
│                         Other Nodes    Direct P2P           │
│                         (aggressive    (if no node,         │
│                          push)         pull-based)          │
└─────────────────────────────────────────────────────────────┘
```

### Connection Routing Detail

```
Scenario A: User HAS a node (Kunki)
┌──────────────────────────────────────────────────────────────┐
│ Device                     Kunki (Node)           Peer Node  │
│    │                           │                      │      │
│    │───── all messages ───────>│                      │      │
│    │                           │──── push on change ─>│      │
│    │                           │<─── push on change ──│      │
│    │<────── responses ─────────│                      │      │
└──────────────────────────────────────────────────────────────┘
- Device maintains SINGLE connection to own Kunki
- Kunki handles all peer communication
- Kunki pushes to other nodes immediately on change
- Battery efficient for device

Scenario B: User has NO node
┌──────────────────────────────────────────────────────────────┐
│ Device                                           Peer Device │
│    │                                                  │      │
│    │<──────────── direct P2P (pull-based) ───────────>│      │
│    │                                                  │      │
└──────────────────────────────────────────────────────────────┘
- Direct Iroh connections to peers
- Pull-based sync (viewer requests data)
- More battery usage, but works without infrastructure
```

---

## Data Model Changes

### 1. New Resource Model (1 doc per resource)

**Current** (`core/src/models/resource.rs`):
```rust
pub struct Resource {
    pub docs: HashMap<String, LoroDoc>,    // Multiple docs per resource
    pub static_assets: HashMap<String, String>,
}
```

**New**:
```rust
pub struct Resource {
    pub id: String,
    pub folder_id: String,
    pub template_id: Option<String>,       // Which template this belongs to
    pub resource_type: ResourceType,       // content | comments | submissions | asset
    pub doc: Option<LoroDoc>,              // Single Loro doc (None for assets)
    pub asset_data: Option<Vec<u8>>,       // Binary data (None for CRDT resources)
    pub ucan_cid: String,                  // Reference to issued UCAN
    pub metadata: Value,
}

pub enum ResourceType {
    Content,      // Main HUML-driven content
    Comments,     // Thread comments (collaborator capability)
    Submissions,  // Form submissions (submitter capability)
    Asset,        // Binary blob (image, video, file)
}
```

### 2. New Template Model

```rust
pub struct Template {
    pub id: String,
    pub folder_id: String,
    pub name: String,
    pub huml_content: String,              // The HUML template source
    pub resource_schema: Value,            // What resources this template creates
    pub created_at: i64,
    pub updated_at: i64,
}
```

### 3. Replace share_records with `issued_ucans` table

The `issued_ucans` table stores all issued UCAN tokens. This replaces both `share_records` and `folder_share_records`.

```sql
CREATE TABLE issued_ucans (
    cid TEXT PRIMARY KEY,                    -- Content-addressed ID (hash of token)
    token TEXT NOT NULL,                     -- Full JWT token string
    issuer_did TEXT NOT NULL,                -- Who issued (did:key:...)
    audience_did TEXT NOT NULL,              -- Who receives (did:key:... or *)

    -- Target (polymorphic)
    target_type TEXT NOT NULL,               -- 'resource' | 'folder' | 'template' | 'connection'
    target_id TEXT,                          -- ID of target (NULL for connections)

    -- Delegation chain tracking (extracted from fct for queries)
    parent_cid TEXT,                         -- CID of parent token we delegated from
    delegation_depth INTEGER DEFAULT 0,      -- Hops from original owner (0 = owner)

    -- Extracted facts (for efficient queries without parsing JWT)
    relationship TEXT NOT NULL,              -- 'owner' | 'node' | 'viewer' | 'user'
    capability TEXT,                         -- 'collaborator' | 'submitter' | 'viewer'
    operations TEXT,                         -- JSON: {"read": "allow", "write": "deny", ...}

    -- Lifecycle
    issued_at INTEGER NOT NULL,
    expires_at INTEGER,                      -- NULL = no expiry
    revoked_at INTEGER,                      -- NULL = not revoked

    FOREIGN KEY (parent_cid) REFERENCES issued_ucans(cid)
);

-- Query: Who has access to this resource?
CREATE INDEX idx_ucans_target ON issued_ucans(target_type, target_id);

-- Query: What can this user access?
CREATE INDEX idx_ucans_audience ON issued_ucans(audience_did);

-- Query: What did this user issue?
CREATE INDEX idx_ucans_issuer ON issued_ucans(issuer_did);

-- Query: Find by relationship type
CREATE INDEX idx_ucans_relationship ON issued_ucans(relationship);

-- Query: Active (non-revoked) tokens only
CREATE INDEX idx_ucans_active ON issued_ucans(revoked_at) WHERE revoked_at IS NULL;
```

### 2. Add `node_registry` table

```sql
CREATE TABLE node_registry (
    user_did TEXT PRIMARY KEY,               -- User's DID
    node_device_id TEXT NOT NULL,            -- Node's Iroh device ID
    node_did TEXT NOT NULL,                  -- Node's DID for UCANs
    node_addr TEXT,                          -- Serialized NodeAddr
    registered_at INTEGER NOT NULL,
    last_seen_at INTEGER,
    is_online BOOLEAN DEFAULT FALSE
);
```

### 3. Add `folder_subscriptions` table (for push)

```sql
CREATE TABLE folder_subscriptions (
    id TEXT PRIMARY KEY,
    subscriber_node_id TEXT NOT NULL,        -- Which node subscribes
    folder_id TEXT NOT NULL,                 -- Which folder
    subscribed_at INTEGER NOT NULL,
    last_push_at INTEGER,

    UNIQUE(subscriber_node_id, folder_id)
);
```

### 4. Remove (after migration)

- `share_records` table
- `folder_share_records` table

---

## New Rust Types

### IssuedUcan (replaces ShareRecord)

```rust
// core/src/models/issued_ucan.rs

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssuedUcan {
    pub cid: String,
    pub token: String,
    pub issuer_did: String,
    pub audience_did: String,
    pub target_type: TargetType,
    pub target_id: Option<String>,
    pub parent_cid: Option<String>,
    pub delegation_depth: u32,
    pub relationship: String,
    pub operations: serde_json::Value,
    pub issued_at: i64,
    pub expires_at: Option<i64>,
    pub revoked_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TargetType {
    Resource,
    Folder,
    Connection,
}
```

### NodeRegistration

```rust
// core/src/models/node_registry.rs

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeRegistration {
    pub user_did: String,
    pub node_device_id: String,
    pub node_did: String,
    pub node_addr: Option<String>,
    pub registered_at: i64,
    pub last_seen_at: Option<i64>,
    pub is_online: bool,
}
```

### RouteInfo (for connection routing)

```rust
// core/src/models/routing.rs

#[derive(Debug, Clone)]
pub enum RouteInfo {
    ViaNode {
        node_device_id: String,
        node_addr: Option<String>,
    },
    DirectP2P {
        devices: Vec<Device>,
    },
}
```

---

## New Services

### 1. AccessService (replaces ShareService)

```rust
// services/src/access_service.rs

/// Get all users with access to a resource
pub async fn get_resource_access_list(resource_id: &str) -> Vec<IssuedUcan>

/// Get all resources a user can access
pub async fn get_user_access_list(user_did: &str) -> Vec<IssuedUcan>

/// Get full delegation chain for audit
pub async fn get_delegation_chain(cid: &str) -> Vec<IssuedUcan>

/// Check if user has specific access
pub async fn check_access(user_did: &str, target_id: &str, operation: &str) -> bool
```

### 2. NodeRegistryService

```rust
// services/src/node_registry_service.rs

/// Check if user has a registered node
pub async fn user_has_node(user_did: &str) -> Option<NodeRegistration>

/// Register current device as user's node
pub async fn register_as_node(user_did: &str, device_id: &str, did: &str) -> NodeRegistration

/// Get route for reaching a user (via node or direct)
pub async fn get_route_for_user(target_user_did: &str) -> RouteInfo
```

### 3. PushService

```rust
// services/src/push_service.rs

/// Subscribe to folder changes
pub async fn subscribe_to_folder(node_id: &str, folder_id: &str)

/// Called when resource changes - push to all subscribed nodes
pub async fn notify_subscribers(folder_id: &str, change: ChangeNotification)
```

---

## P2P Changes

### New Message Types

```rust
// core/src/models/p2p.rs

pub enum Message {
    // Existing...
    Ping, Pong, Error,
    Handshake(HandshakeMessage),
    Resource(ResourceMessage),
    Folder(FolderMessage),

    // New
    Push(PushMessage),
    Routed(RoutedMessage),
}

pub enum PushMessage {
    Subscribe { folder_id: String, permit: String },
    Unsubscribe { folder_id: String },
    ChangeNotification { folder_id: String, resource_id: String, change_type: ChangeType },
    ResourcePush(ResourceDataSync),
}

pub struct RoutedMessage {
    pub target_user_did: String,
    pub inner_message: Box<Message>,
}
```

### P2PService Modifications

```rust
// network/src/p2p/p2p_service.rs

impl P2PService {
    /// Connect routing through node if available
    pub async fn connect_routed(&self, target_user_did: &str) -> Arc<PeerConnection>

    /// Detect and connect to own node
    pub async fn connect_to_own_node(&self) -> Option<Arc<PeerConnection>>

    /// Smart send - routes through node when available
    pub async fn send_smart(&self, target_user_did: &str, message: Message)
}
```

---

## Implementation Phases

### Phase 1: UCAN-Based Tracking Foundation

1. Create `issued_ucans` table migration
2. Create `IssuedUcan` model and repository
3. Modify `gurkha/src/service.rs` to auto-persist UCANs on delegation
4. Create `services/src/access_service.rs`
5. Update callers to use new access queries

**Files to modify:**
- `persistance/src/database/migrations/` - new migration
- `persistance/src/repositories/` - new `ucan_repository.rs`
- `core/src/models/` - new `issued_ucan.rs`
- `gurkha/src/service.rs` - auto-persist on delegation
- `services/src/` - new `access_service.rs`

### Phase 2: Node Registry

1. Create `node_registry` table migration
2. Create `NodeRegistration` model and repository
3. Create `services/src/node_registry_service.rs`
4. Modify Kunki to register itself on startup
5. Add node detection to P2PService

**Files to modify:**
- `persistance/src/database/migrations/` - new migration
- `persistance/src/repositories/` - new `node_registry_repository.rs`
- `core/src/models/` - new `node_registry.rs`
- `services/src/` - new `node_registry_service.rs`
- `kunki/src/main.rs` - call register_as_node

### Phase 3: Connection Routing

1. Add `RouteInfo` type
2. Implement `get_route_for_user()` in node registry service
3. Add `connect_routed()` to P2PService
4. Add `RoutedMessage` type and handler
5. Implement `send_smart()` for auto-routing

**Files to modify:**
- `core/src/models/` - new `routing.rs`
- `network/src/p2p/p2p_service.rs` - routing methods
- `network/src/p2p/peer_connection.rs` - handle routed messages
- `core/src/models/p2p.rs` - new message types

### Phase 4: Aggressive Push

1. Create `folder_subscriptions` table migration
2. Create subscription repository
3. Add `PushMessage` types
4. Create `services/src/push_service.rs`
5. Wire push notifications on resource create/update/delete
6. Implement subscription handler in P2P layer

**Files to modify:**
- `persistance/src/database/migrations/` - new migration
- `persistance/src/repositories/` - new `subscription_repository.rs`
- `core/src/models/p2p.rs` - push message types
- `services/src/` - new `push_service.rs`
- `services/src/resource_service/` - trigger push on changes
- `network/src/p2p/` - new `push_handler.rs`

### Phase 5: Cleanup

1. Remove share_records tables
2. Delete `ShareRecord`, `FolderShareRecord` types
3. Delete `share_service.rs`
4. Simplify `folder_sync.rs` (remove dual-permit mess)
5. Simplify `resource_sync.rs`

**Files to delete:**
- `core/src/models/share_record.rs`
- `core/src/models/folder_share_record.rs`
- `services/src/share_service.rs`
- `persistance/src/repositories/share_repository.rs`
- `persistance/src/repositories/folder_share_repository.rs`

**Files to simplify:**
- `network/src/p2p/folder_sync.rs`
- `network/src/p2p/resource_sync.rs`

---

## Critical Files Summary

### Create New
- `core/src/models/issued_ucan.rs`
- `core/src/models/node_registry.rs`
- `core/src/models/routing.rs`
- `services/src/access_service.rs`
- `services/src/node_registry_service.rs`
- `services/src/push_service.rs`
- `persistance/src/repositories/ucan_repository.rs`
- `persistance/src/repositories/node_registry_repository.rs`
- `persistance/src/repositories/subscription_repository.rs`
- `network/src/p2p/push_handler.rs`

### Major Modifications
- `gurkha/src/service.rs` - auto-persist UCANs
- `network/src/p2p/p2p_service.rs` - routing
- `network/src/p2p/folder_sync.rs` - simplify
- `network/src/p2p/resource_sync.rs` - simplify
- `kunki/src/main.rs` - register as node

### Delete
- `core/src/models/share_record.rs`
- `core/src/models/folder_share_record.rs`
- `services/src/share_service.rs`
- Related repositories

---

## UCAN Structure Detail

### Current UCAN `fct` Field (from `gurkha/src/decision.rs`)

The current implementation already has rich facts:
```json
{
  "token_type": "resource_owner",
  "relationship": "owner",
  "resource_id": "...",
  "user_id": "base64_pubkey",
  "operations": {
    "own": "allow",
    "read": "allow",
    "write": "allow",
    "share_resource": "allow"
  },
  "auth_capabilities": {
    "can_connect": true,
    "persist_share": true,
    "can_delegate": true,
    "sync_enabled": true
  },
  "documents": {
    "main_doc": { "type": "crdt", "capability": "collaborator" }
  },
  "delegation": {
    "node": { /* template */ },
    "viewer": { /* template */ }
  },
  "cel_rules": { /* CEL expressions */ }
}
```

### New UCAN `fct` Field (with tracking)

Add delegation tracking to existing structure:
```json
{
  // Existing fields...
  "token_type": "resource_viewer",
  "relationship": "viewer",
  "resource_id": "...",

  // NEW: Delegation tracking
  "delegation": {
    "parent_cid": "bafyreif...",           // CID of token we derived from
    "depth": 2,                             // Hops from owner (0=owner, 1=node, 2=viewer)
    "chain": ["bafyreif...", "bafyreig..."], // Full chain of CIDs to root
    "delegated_by": "did:key:z6Mk...",     // Who gave us this token
    "delegated_at": 1732534800000          // When delegation happened
  },

  // NEW: Transparency info
  "transparency": {
    "original_owner": "did:key:z6Mk...",   // Who created the resource
    "delegation_path": [                    // Human-readable path
      {"did": "did:key:owner", "role": "owner"},
      {"did": "did:key:node", "role": "node"},
      {"did": "did:key:viewer", "role": "viewer"}
    ]
  }
}
```

### Token Storage Strategy

1. **On delegation**: Gurkha service auto-persists to `issued_ucans` table
2. **On receive**: When receiving a token, store it and extract to DB
3. **Query "who has access"**: Query `issued_ucans` by target_id
4. **Query "delegation chain"**: Follow parent_cid links or use `fct.delegation.chain`

---

## Push Mechanism Detail

### Subscription Flow

```
1. Node A connects to Node B (after handshake)
2. Node A sends: PushMessage::Subscribe { folder_id, permit }
3. Node B validates permit has access to folder
4. Node B stores subscription in folder_subscriptions table
5. Node B acknowledges subscription

Later:
6. User on Node B updates a resource in the folder
7. Node B's resource_service calls push_service::notify_subscribers()
8. push_service finds all subscribed nodes for this folder
9. For each subscriber: send PushMessage::ResourcePush with updated data
```

### Push Message Flow

```
Owner Device → Kunki (Owner's Node) → Subscribed Nodes
     │                   │                    │
     │  ResourceUpdate   │                    │
     │──────────────────>│                    │
     │                   │                    │
     │                   │  For each subscriber:
     │                   │──PushMessage::ResourcePush──>│
     │                   │                              │
     │                   │<─────Acknowledge─────────────│
```

### Push vs Pull Decision

```rust
// In P2PService
async fn send_to_peer(&self, target_did: &str, message: Message) {
    let route = node_registry::get_route_for_user(target_did).await;

    match route {
        RouteInfo::ViaNode { node_id, .. } => {
            // Target has a node - they will receive via push
            // Just ensure we're subscribed to their node
            self.ensure_subscription(node_id, folder_id).await;
        }
        RouteInfo::DirectP2P { devices } => {
            // No node - direct connection, pull-based
            // Send directly when they connect and request
        }
    }
}
```

### Subscription Table Usage

```sql
-- When resource changes, find who to push to:
SELECT subscriber_node_id
FROM folder_subscriptions
WHERE folder_id = ?;

-- When node connects, restore subscriptions:
SELECT folder_id
FROM folder_subscriptions
WHERE subscriber_node_id = ?;
```

---

## Resource Model Migration

### Current → New Mapping

| Current | New |
|---------|-----|
| `Resource.docs["blocksuite_doc"]` | `Resource` (type=Content) |
| `Resource.docs["thread_comments_doc"]` | `Resource` (type=Comments) |
| `Resource.docs["form_submissions_doc"]` | `Resource` (type=Submissions) |
| `Resource.static_assets["asset_id"]` | `Resource` (type=Asset) |

### Per-Resource Access Control

**Key insight**: Each resource has its **own UCAN token** with independent access rules.

```
Folder (Application) ─── folder_ucan (who can see folder exists)
  │
  └── Template A
        ├── public_content_resource ─── ucan (viewer: all subscribers)
        ├── comments_resource ─────────── ucan (collaborator: all subscribers)
        ├── submissions_resource ──────── ucan (submitter: all subscribers)
        └── private_chat_resource ──────── ucan (viewer: ONLY owner + specific user)
```

**Example: Private conversation in a public app**
```json
// public_content_resource UCAN
{
  "resource_id": "content_123",
  "relationship": "viewer",
  "capability": "viewer"
  // Delegated to all folder subscribers
}

// private_chat_resource UCAN
{
  "resource_id": "chat_456",
  "relationship": "viewer",
  "capability": "collaborator",
  "restricted_to": ["did:key:owner", "did:key:specific_user"]
  // Only these two DIDs have access
}
```

### Resource-Level vs Folder-Level Permissions

| Level | Controls | Example |
|-------|----------|---------|
| **Folder UCAN** | Who can see folder exists, subscribe | Public folder = shareable link |
| **Resource UCAN** | Who can read/write specific resource | Private chat = owner + 1 person |

This allows:
- Public app with private direct messages
- Shared workspace with private notes
- Public form with private admin-only submissions view

### Template Creates Resources

When a HUML template is deployed, it creates multiple resources with appropriate UCANs:
```rust
// template_service.rs
pub async fn deploy_template(folder_id: &str, template: &Template) -> Vec<Resource> {
    let mut resources = vec![];

    // Main content resource - follows folder permissions
    let (content_ucan, _) = ucan_service.issue_owner_token(
        &content_id,
        template.content_permissions_json()
    ).await?;
    resources.push(Resource::new_content(folder_id, template.id, content_ucan));

    // Private resources get restricted UCANs
    if let Some(private_config) = template.private_resources() {
        for config in private_config {
            let (private_ucan, _) = ucan_service.issue_owner_token(
                &config.id,
                &config.restricted_permissions_json() // Only specific DIDs
            ).await?;
            resources.push(Resource::new_private(folder_id, template.id, private_ucan));
        }
    }

    resources
}
```

### Sync Filtering by Resource UCAN

When syncing to a peer, each resource is checked independently:
```rust
// In resource_sync.rs
for resource in folder_resources {
    // Check THIS resource's UCAN, not folder's
    let peer_has_access = access_service::check_access(
        &peer_did,
        &resource.id,
        "read"
    ).await?;

    if peer_has_access {
        push_resource_to_peer(peer, resource).await?;
    }
    // Private resources simply not sent to unauthorized peers
}
```

---

---

## Caching Strategy

### Current State
- **No application-level caching** currently exists
- Only SQLite's internal 64MB cache via PRAGMA
- Connection pool (r2d2, 20 connections max)

### Proposed Caching Layers

```
┌─────────────────────────────────────────────────────────────┐
│                    Application Layer                         │
├─────────────────────────────────────────────────────────────┤
│  L1: In-Memory Cache (per-process)                          │
│  - Hot UCANs (recently validated)                           │
│  - Active connections metadata                               │
│  - Frequently accessed resource metadata                     │
│  TTL: 5 minutes, LRU eviction                               │
├─────────────────────────────────────────────────────────────┤
│  L2: Database (Sled or SQLite)                              │
│  - All issued UCANs                                         │
│  - Node registry                                            │
│  - Subscriptions                                            │
│  - Resource metadata                                        │
├─────────────────────────────────────────────────────────────┤
│  L3: Encrypted Storage (Loro docs, assets)                  │
│  - Actual document content (encrypted)                      │
│  - Binary assets                                            │
└─────────────────────────────────────────────────────────────┘
```

### L1 Cache Implementation

```rust
// services/src/cache.rs
use moka::sync::Cache;  // or dashmap for simpler case

pub struct AppCache {
    // UCAN cache: cid -> IssuedUcan
    ucans: Cache<String, IssuedUcan>,

    // Access check cache: (user_did, resource_id) -> bool
    access_checks: Cache<(String, String), bool>,

    // Node registry cache: user_did -> NodeRegistration
    nodes: Cache<String, Option<NodeRegistration>>,
}

impl AppCache {
    pub fn new() -> Self {
        Self {
            ucans: Cache::builder()
                .max_capacity(10_000)
                .time_to_live(Duration::from_secs(300))
                .build(),
            access_checks: Cache::builder()
                .max_capacity(50_000)
                .time_to_live(Duration::from_secs(60))
                .build(),
            nodes: Cache::builder()
                .max_capacity(1_000)
                .time_to_live(Duration::from_secs(300))
                .build(),
        }
    }
}
```

### Cache Invalidation

```rust
// On UCAN delegation/revocation
cache.ucans.invalidate(&cid);
cache.access_checks.invalidate_all(); // Conservative: clear all access checks

// On node registration change
cache.nodes.invalidate(&user_did);
```

---

## Database Options

### Current: SQLite + Diesel

**Pros:**
- Already implemented and working
- SQL queries for complex filtering
- Well-tested, stable (20+ years)
- Good tooling (migrations, schema)

**Cons:**
- ORM overhead (Diesel)
- Not optimal for key-value patterns
- Sync operations wrapped in async

### Option: Sled (Pure Rust Key-Value)

**Pros:**
- Pure Rust, no C dependencies
- Lock-free concurrent access
- Natural key-value API (perfect for UCANs by CID)
- Smaller binary (~500KB)
- "Just works" for concurrent programs

**Cons:**
- No SQL queries
- Higher disk space usage than SQLite
- Still in development (rewrite planned)
- Less mature than SQLite

**Sled API Example:**
```rust
let db = sled::open("osvauld.db")?;

// Store UCAN by CID
let ucans = db.open_tree("issued_ucans")?;
ucans.insert(cid.as_bytes(), serde_json::to_vec(&ucan)?)?;

// Get UCAN
let ucan: IssuedUcan = ucans.get(cid)?
    .map(|bytes| serde_json::from_slice(&bytes))
    .transpose()?;

// Range scan by prefix
for result in ucans.scan_prefix(b"resource:") {
    let (key, value) = result?;
    // Process each UCAN for resources
}
```

### Recommendation: Hybrid Approach

| Data Type | Storage | Rationale |
|-----------|---------|-----------|
| **UCANs** | Sled | Key-value by CID, high concurrency |
| **Node Registry** | Sled | Simple key-value (user_did → node) |
| **Subscriptions** | Sled | Key-value (folder_id → subscribers) |
| **Resources metadata** | SQLite | Complex queries, relations |
| **Templates** | SQLite | Relational (folder → templates) |
| **Loro docs** | File system | Large binary, separate from DB |

```rust
// persistance/src/lib.rs
pub struct Storage {
    // Key-value store (Sled)
    pub kv: sled::Db,

    // Relational store (SQLite)
    pub sql: DbPool,

    // In-memory cache
    pub cache: AppCache,
}
```

---

## Implementation Order (Revised)

### Phase 0: Resource Model Split (FIRST) ← START HERE
**Goal**: Split multi-doc Resource into single-doc Resources with types.

#### Current Model (to change):
```rust
// core/src/models/resource.rs - CURRENT
pub struct Resource {
    pub id: String,
    pub folder_id: String,
    pub ucan_token: String,
    pub metadata: Value,
    pub docs: HashMap<String, LoroDoc>,      // MULTIPLE docs
    pub static_assets: HashMap<String, String>, // Assets mixed in
}
```

#### New Model:
```rust
// core/src/models/resource.rs - NEW
pub struct Resource {
    pub id: String,
    pub folder_id: String,
    pub template_id: Option<String>,
    pub resource_type: ResourceType,
    pub ucan_token: String,
    pub metadata: Value,

    // ONE of these, based on resource_type
    pub doc: Option<LoroDoc>,           // For CRDT types
    pub asset_data: Option<Vec<u8>>,    // For Asset type
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ResourceType {
    Content,      // Main content (HUML state)
    Comments,     // Thread comments
    Submissions,  // Form submissions
    Asset,        // Binary blob (image, video, file)
}
```

#### Step-by-Step Implementation:

**Step 0.1: Add ResourceType enum**
- Add `ResourceType` enum to `core/src/models/resource.rs`
- Keep old struct for now

**Step 0.2: Create new Resource struct**
- Create `ResourceV2` struct alongside old `Resource`
- Add conversion: `Resource -> Vec<ResourceV2>` (split)

**Step 0.3: Update EncryptedResource**
- Simplify `encrypted_data` format (single doc, not map)
- Add `resource_type` field

**Step 0.4: Update database schema**
- Add `resource_type` column to resources table
- Add `template_id` column (nullable)

**Step 0.5: Update ResourceRepository**
- Update queries for new schema
- Add filtering by resource_type

**Step 0.6: Update resource_service**
- Update CRUD operations
- Update encryption/decryption for single doc

**Step 0.7: Update folder_sync and resource_sync**
- Send multiple resources instead of one multi-doc resource
- Filter by resource_type when syncing

**Step 0.8: Cleanup**
- Remove old `Resource` struct
- Rename `ResourceV2` to `Resource`

#### Files to Modify:

| File | Change |
|------|--------|
| `core/src/models/resource.rs` | Rewrite struct, add ResourceType |
| `core/src/models/mod.rs` | Export new types |
| `persistance/src/models.rs` | Update ResourceModel |
| `persistance/src/database/schema.rs` | Add columns |
| `persistance/migrations/` | New migration |
| `persistance/src/repositories/resource_repository.rs` | Update queries |
| `services/src/resource_service/core.rs` | Update encrypt/decrypt |
| `services/src/resource_service/crud.rs` | Update CRUD |
| `network/src/p2p/resource_sync.rs` | Handle single doc |
| `network/src/p2p/folder_sync.rs` | Send multiple resources |

#### Migration Strategy:
Since there are no production users, we can do a **breaking change**:
1. Drop and recreate resources table
2. No data migration needed

### Phase 1: Add Caching Layer
1. Add `moka` or `dashmap` dependency
2. Create `services/src/cache.rs`
3. Wire cache into service layer
4. Add cache invalidation hooks

### Phase 2: Database Refactor (Optional: Sled)
1. Add Sled dependency
2. Create key-value repository trait
3. Implement Sled backend for UCANs
4. Keep SQLite for relational data

### Phase 3: UCAN-Based Tracking
(As previously planned)

### Phase 4: Node Registry
(As previously planned)

### Phase 5: Push Mechanism
(As previously planned)

### Phase 6: Cleanup
(As previously planned)

---

## Open Questions (Deferred)

1. **Revocation mechanism** - How to revoke UCANs efficiently?
2. **OpenWRT integration** - Port control on Orange Pi?
3. **Auto-migration details** - Exact flow when user gets a node
4. **Template versioning** - How to handle HUML template updates?
5. **Sled vs SQLite** - Full switch or hybrid?
