# User-to-Node Connection & Authorization Flow

This document explains how a user (sthalam) connects to a node (kunki) and gets authorized in the Osvauld system.

---

## 1. What happens when sthalam calls `add_node` with a connection string?

### Connection String Structure

A connection string contains everything needed for first connection:

```rust
pub struct ConnectionString {
    pub node_public_key: [u8; 32],      // Node's signing key (for DID derivation)
    pub node_encryption_key: [u8; 32],  // Node's X25519 key (for ECDH)
    pub device_public_key: [u8; 32],    // Device's iroh NodeId public key
    pub permit: String,                  // UCAN permit token
}
```

The connection string is base64-encoded JSON containing these fields.

### Flow when user calls `add_node`:

1. **User decodes connection string** → extracts `device_public_key` (NodeId) and `permit`
2. **User stores node info** → Butler stores `SovereignNode` record with:
   - `node_id`: Device public key (iroh NodeId)
   - `did`: Derived from `node_public_key` using `Identity::did_from_public_key()`
   - `permit`: The UCAN permit from connection string
3. **User initiates connection** → Sends `CoordinatorMessage::Connect { node_id, permit }` to Courier
4. **Coordinator marks as pending** → Stores in `pending_connections` and `pending_permits`
5. **Transport connects** → CourierRunner calls `transport.connect(node_id)`
6. **On connection established** → Coordinator receives `Connected { node_id, conn }` event
7. **PeerActor spawned** → Coordinator spawns PeerActor with the stored permit
8. **Handshake auto-starts** → PeerActor sees permit in args, calls `initiate_handshake()`

---

## 2. Handshake Flow: User → Node

The handshake is a **4-step protocol** for first connection, **3-step** for reconnection.

### First Connection (4-step)

```
User (sthalam)                          Node (kunki)
     │                                       │
     │  1. Hello                             │
     │  ─────────────────────────────────>   │
     │     - DID, username, public keys      │
     │     - permit (with first_connection)  │
     │                                       │
     │                                       │  ✓ Validate permit
     │                                       │  ✓ Store OwnerInfo
     │                                       │  ✓ Issue permit_for_user
     │                                       │
     │  2. Welcome                           │
     │  <─────────────────────────────────   │
     │     - Node ID, public keys            │
     │     - permit_for_user                 │
     │                                       │
     │  ✓ Validate node identity             │
     │  ✓ Validate permit audience           │
     │  ✓ Issue permit_for_node              │
     │                                       │
     │  3. PermitGrant                       │
     │  ─────────────────────────────────>   │
     │     - permit_for_node                 │
     │                                       │
     │                                       │  ✓ Validate permit
     │                                       │  ✓ Store permit
     │                                       │
     │  4. Ack                               │
     │  <─────────────────────────────────   │
     │                                       │
     │  ✅ AUTHENTICATED                     │  ✅ AUTHENTICATED
```

### Reconnection (3-step)

When the user reconnects with the **same permit** that the node already has stored:

```
User (sthalam)                          Node (kunki)
     │                                       │
     │  1. Hello                             │
     │  ─────────────────────────────────>   │
     │     - permit (no first_connection)    │
     │                                       │
     │                                       │  ✓ Recognize owner DID
     │                                       │  ✓ Retrieve stored permit
     │                                       │
     │  2. Welcome                           │
     │  <─────────────────────────────────   │
     │     - stored permit_for_user          │
     │                                       │
     │  ✓ Permit matches stored permit       │
     │  ✓ Skip PermitGrant step              │
     │                                       │
     │  3. Ack                               │
     │  ─────────────────────────────────>   │
     │                                       │
     │  ✅ AUTHENTICATED                     │  ✅ AUTHENTICATED
```

**Key difference**: If the received permit matches the stored permit, the user sends `Ack` directly instead of `PermitGrant`. This is the **bifurcated path** in `complete_welcome_flow()`.

---

## 3. How does the user get a permit? Who issues it? When?

### Permit Exchange (Bidirectional)

**Both sides issue permits to each other during handshake:**

#### User → Node (in Hello message)

- **Who issues**: User's device (sthalam)
- **When**: Before connection (embedded in connection string)
- **Issuer**: User's DID
- **Audience**: Node's public key (base64-encoded)
- **Capabilities**: `accept_publish: true` (for owner), `first_connection: true` (first time only)
- **Relationship**: `owner_node` (user is owner, node is recipient)

#### Node → User (in Welcome message)

- **Who issues**: Node (kunki)
- **When**: During handshake (step 2 - Welcome)
- **Issuer**: Node's DID
- **Audience**: User's public key (base64-encoded)
- **Capabilities**: Derived from user's permit (mirrors `accept_publish`)
- **Relationship**: `node_owner` (node is issuer, owner is recipient)

#### User → Node (in PermitGrant message)

- **Who issues**: User's device (sthalam)
- **When**: During handshake (step 3 - PermitGrant)
- **Issuer**: User's DID
- **Audience**: Node's public key (base64-encoded)
- **Capabilities**: Based on user's role (`accept_publish` determines relationship)
- **Relationship**: `owner_node` (user is owner, node is recipient)

### Permit Issuance Code Paths

**Node issues permit to user** (`on_hello` → `handle_first_connection`):

```rust
// courier/src/peer_actor/handshake.rs:295-314
let relationship = if peer_can_publish {
    "node_owner"  // Node → Owner
} else {
    "node_viewer" // Node → Viewer
};
let (permit_for_peer, _our_pubkey) = state
    .butler
    .issue_peer_connection_permit(&peer_pubkey, relationship)
    .await?;
```

**User issues permit to node** (`on_welcome` → `complete_welcome_flow`):

```rust
// courier/src/peer_actor/handshake.rs:664-682
let relationship = if our_can_publish {
    "owner_node"  // Owner → Node
} else {
    "viewer_node" // Viewer → Node
};
let (permit_for_node, _our_pubkey) = state
    .butler
    .issue_peer_connection_permit(&node_pubkey_b64, relationship)
    .await?;
```

### Permit Storage

- **Node stores user's permit**: In `OwnerInfo` (for owner) or `connection_permits` table (for viewers)
- **User stores node's permit**: In `SovereignNode.permit` field

---

## 4. What does `CourierMode::User` do differently from `CourierMode::Node`?

`CourierMode` determines **who initiates handshake** and **who can accept certain operations**.

### Mode Differences

| Aspect | `CourierMode::User` | `CourierMode::Node` |
|--------|---------------------|---------------------|
| **Handshake initiation** | Sends `Hello` first | Waits for `Hello` |
| **Receives** | `Welcome`, `Ack` | `Hello`, `PermitGrant` |
| **Can publish spaces** | ✅ Yes (via `publish_space`) | ❌ No (rejects with mode guard) |
| **Can accept publish** | ❌ No | ✅ Yes (via `on_publish_space`) |
| **Viewer onboarding** | Can request spaces | Can serve spaces |
| **Relay datagrams** | ❌ No | ✅ Yes (forwards to other peers) |
| **__sync_meta filtering** | Skips `__sync_meta:` layers | Processes all layers |
| **Subscription model** | Subscribes to node's Scribes | Accepts subscriptions from users |

### Mode Guards

The codebase uses **mode guards** to enforce these rules:

```rust
// courier/src/peer_actor/guards.rs:26-31
pub fn require_user_mode(actual: CourierMode, action: &str) -> Option<()> {
    require_mode(actual, CourierMode::User, action)
}

pub fn require_node_mode(actual: CourierMode, action: &str) -> Option<()> {
    require_mode(actual, CourierMode::Node, action)
}
```

**Example usage**:

```rust
// courier/src/peer_actor/publish.rs:708
if state.mode != CourierMode::User {
    warn!("initiate_get_shareable_link only available in User mode");
    return;
}
```

### Behavioral Differences

#### User Mode (`sthalam`)

1. **Initiates handshake** when permit is provided in `PeerActorArgs`
2. **Publishes spaces** to node via `PublishSpace` message
3. **Requests shareable links** from node
4. **Subscribes to node's Scribes** for live sync after authentication

#### Node Mode (`kunki`)

1. **Waits for Hello** from connecting peers
2. **Stores owner info** on first connection
3. **Issues permits** to connecting users
4. **Accepts published spaces** from owner
5. **Generates shareable links** for viewers
6. **Relays datagrams** between authenticated peers
7. **Serves space data** to viewers via `SpaceRequest` flow

---

## 5. How does `publish_space` work - what permits does it create?

### Publish Flow (Owner → Node)

```
Owner (User mode)                       Node (Node mode)
     │                                       │
     │  1. initiate_publish_space()          │
     │     - Get space from Butler           │
     │     - Delegate permit to node         │
     │                                       │
     │  PublishSpace                         │
     │  ─────────────────────────────────>   │
     │     - space metadata                  │
     │     - space_permit (delegated)        │
     │                                       │
     │                                       │  2. on_publish_space()
     │                                       │     - Validate permit
     │                                       │     - Store space + permit
     │                                       │     - Issue permit back to owner
     │                                       │
     │  PublishSpaceAck                      │
     │  <─────────────────────────────────   │
     │     - node_permit (for owner)         │
     │     - pages[] (existing page IDs)     │
     │                                       │
```

### Permits Created During `publish_space`

#### 1. Owner → Node Delegation (before PublishSpace)

**Created by**: Owner's Butler (`spaces().delegate_to_node()`)

```rust
// butler/src/services/spaces.rs
pub async fn delegate_to_node(&self, space_id: &str, node_pubkey: &str) -> Result<String>
```

**Permit structure**:
- **Issuer**: Owner's DID
- **Audience**: Node's public key (base64)
- **Capabilities**: `accept_publish: true`, `share: true`
- **Scope**: `space_id: <space_id>`
- **Relationship**: Owner delegates to node (node can accept publish)

**Purpose**: Proves to node that owner authorizes this space to be published.

#### 2. Node → Owner Permit (in PublishSpaceAck)

**Created by**: Node's Butler (`publish().issue_space_permit_to_owner()`)

```rust
// butler/src/services/publish.rs
pub async fn issue_space_permit_to_owner(&self, space_id: &str, owner_pubkey: &str) -> Result<String>
```

**Permit structure**:
- **Issuer**: Node's DID
- **Audience**: Owner's public key (base64)
- **Capabilities**: Mirrors owner's capabilities
- **Scope**: `space_id: <space_id>`
- **Relationship**: Node acknowledges ownership

**Purpose**: Proves that node has accepted the space (receipt of publication).

### Page Publishing (Separate Flow)

After space is published, owner publishes **each page** separately:

```
Owner                                   Node
  │  PageAnnounce                        │
  │  ─────────────────────────────────>  │
  │     - page metadata                  │
  │     - page_permit (delegated)        │
  │     - owner_permit (for sync auth)   │
  │                                      │
  │  PageAnnounceAck                     │
  │  <─────────────────────────────────  │
  │     - node_permit (for page)         │
```

**Permits created**:

1. **Owner → Node page delegation** (via `gurkha::delegate_page()`)
   - Issuer: Owner's DID
   - Audience: Node's public key
   - Scope: `page_id: <page_id>`
   - Relationship: `node` (owner delegates to node)

2. **Node → Owner page permit** (in PageAnnounceAck)
   - Issuer: Node's DID
   - Audience: Owner's public key
   - Scope: `page_id: <page_id>`
   - Relationship: Acknowledgment of page receipt

### Permit Storage

**On Node**:
- Space permit: `butler.publish().store_space(space, permit)`
- Page permit: `butler.publish().store_page(page, permit)`
- Owner permit (for sync): `butler.store().put_connection_permit(owner_did, permit)`

**On Owner**:
- Node's space permit: Stored in memory (used for verification)
- Node's page permit: Stored in `PageData.permit` field

---

## 6. Is there a way to auto-authorize a specific DID on the node side?

**No, there is no DID whitelist or auto-authorization mechanism.**

The system uses **permit-based authorization only**. All authorization decisions are based on:

1. **Permit validity** (signature, expiration, chain of delegation)
2. **Permit capabilities** (`accept_publish`, `share`, `relay`)
3. **Permit scope** (space_id, page_id, layer_name)

### Why No DID Whitelist?

From the code comments:

```rust
// courier/src/peer_actor/publish.rs:404
// Store owner's permit for sync authorization (permit-based auth, no DID whitelist)

// butler/src/storage/store/permit.rs:343
// Used for permit-based sync auth (replaces DID whitelist)
```

**Design principle**: Authorization is **capability-based**, not identity-based. A DID without a valid permit has no access, even if it's the "owner" DID.

### How to Grant Access to a Specific DID

To grant access to a specific user:

1. **Issue a permit** with their DID as audience
2. **Deliver the permit** via connection string or shareable link
3. **User connects** with the permit in their Hello message
4. **Node validates permit** and grants access based on capabilities

**Example**: Shareable link flow

```rust
// Node generates viewer permit with wildcard audience
let viewer_permit = butler.issue_viewer_permit(space_id, "*").await?;

// Encode in connection string
let conn_string = ConnectionString::new(
    node_public_key,
    node_encryption_key,
    device_public_key,
    viewer_permit,
);

// User decodes and connects with this permit
```

### First Connection Special Case

The **only exception** is first connection with `first_connection: true` capability:

```rust
// courier/src/handshake/decision.rs:100-106
if caps.accept_publish {
    if is_first {
        match ctx.existing_owner_did {
            Some(_) => HelloDecision::RejectAlreadyHasOwner,
            None => HelloDecision::AcceptFirstConnection { can_publish: true },
        }
    }
}
```

**Rules**:
- First connection with `accept_publish` is accepted **only if no owner exists**
- After first connection, all subsequent connections require **matching DID** (owner reconnection)
- No way to "pre-authorize" a DID before they connect

### Security Model

**Uniform validation for all peers**:

1. Permit must be valid (signature, expiration)
2. Permit must have required capabilities for the operation
3. Permit must match expected scope (space_id, page_id)
4. Permit audience must match peer's public key
5. Permit issuer must be authorized (owner for spaces, node for delegations)

**No shortcuts, no whitelists, no hardcoded DIDs.**

---

## Summary

### Connection Flow

1. User gets connection string from node
2. User calls `add_node(connection_string)`
3. Courier initiates QUIC connection
4. PeerActor spawned with permit from connection string
5. Handshake: Hello → Welcome → PermitGrant → Ack
6. Both sides authenticated with bidirectional permits

### Permit Flow

- **User → Node**: Permit in connection string (first connection) or stored permit (reconnection)
- **Node → User**: Issued during Welcome message
- **Bidirectional**: Both sides issue permits to authenticate each other

### Mode Differences

- **User mode**: Initiates handshake, publishes spaces, requests data
- **Node mode**: Accepts handshake, stores spaces, serves data

### Publishing

- **Space**: Owner delegates permit → Node stores → Node issues receipt permit
- **Page**: Owner delegates permit → Node stores → Node issues receipt permit
- **Layers**: Arrive via Scribe subscription (separate from publishing)

### Authorization

- **Permit-based only**: No DID whitelists, no auto-authorization
- **Capability-driven**: `accept_publish`, `share`, `relay` determine permissions
- **First connection**: Special case for initial owner setup (one-time only)
