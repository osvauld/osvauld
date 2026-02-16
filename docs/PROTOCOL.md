# Osvauld P2P Protocol

## Overview

Osvauld uses a layered P2P protocol built on QUIC (via iroh) with capability-based authorization via UCAN permits.

```
┌─────────────────────────────────────────────────────────────────┐
│  Application Layer                                               │
│  Lua apps, UI state, business logic                             │
├─────────────────────────────────────────────────────────────────┤
│  Butler                                                          │
│  Storage, services, Scribe actors (Loro CRDT)                   │
├─────────────────────────────────────────────────────────────────┤
│  Courier                                                         │
│  P2P orchestration, handshakes, sync protocol                   │
│  Actor model: Coordinator → PeerActors                          │
├─────────────────────────────────────────────────────────────────┤
│  Gurkha                                                          │
│  Permit parsing, capability extraction, authorization decisions │
├─────────────────────────────────────────────────────────────────┤
│  Transport                                                       │
│  QUIC connections, streams, datagrams, blob transfer            │
│  "Dumb byte pipe" - no protocol knowledge                       │
├─────────────────────────────────────────────────────────────────┤
│  iroh                                                            │
│  QUIC, relay, NAT traversal, blob protocol                      │
└─────────────────────────────────────────────────────────────────┘
```

---

## Roles

| Role | Description | Permit Type |
|------|-------------|-------------|
| **Owner** | Creates spaces/pages, publishes to nodes | `space_owner`, `page_owner` |
| **Node (Kunki)** | Relay, stores published data, serves viewers | `space_node`, `page_node` |
| **Viewer** | Receives data via shareable link | `space_viewer`, `page_viewer` |

---

## Connection Flow

### 1. First Connection (4-step)

When a user connects to a node for the first time, both sides exchange and store new permits.

```
User                                    Node
  │                                       │
  │  QUIC connect (iroh)                  │
  │──────────────────────────────────────>│
  │                                       │
  │  Hello                                │
  │  - did, username                      │
  │  - public_key, encryption_key         │
  │  - signature, timestamp               │
  │  - permit (one_time_connection)       │
  │──────────────────────────────────────>│
  │                                       │
  │                           decide_hello_response()
  │                           ✓ Validate permit signature
  │                           ✓ Check first_connection flag
  │                           ✓ Verify no existing owner
  │                           → AcceptFirstConnection
  │                           
  │                           Store OwnerInfo
  │                           Issue permit_for_user
  │                                       │
  │  Welcome                              │
  │  - node_id, node_public_key           │
  │  - node_encryption_key                │
  │  - signature, timestamp               │
  │  - permit_for_peer                    │
  │<──────────────────────────────────────│
  │                                       │
  │  decide_welcome_response()            │
  │  ✓ Validate node pubkey matches       │
  │  ✓ Validate permit audience (our DID) │
  │  → AcceptWelcome                      │
  │                                       │
  │  Issue permit_for_node                │
  │                                       │
  │  PermitGrant                          │
  │  - permit_for_node                    │
  │──────────────────────────────────────>│
  │                                       │
  │                           Validate & store permit
  │                                       │
  │  Ack                                  │
  │<──────────────────────────────────────│
  │                                       │
  │  [AUTHENTICATED]                      │
  │  Subscribe to active scribes          │
```

### 2. Reconnection (3-step)

When reconnecting with stored permits, PermitGrant is skipped.

```
User                                    Node
  │                                       │
  │  QUIC connect (iroh)                  │
  │──────────────────────────────────────>│
  │                                       │
  │  Hello                                │
  │  - did, username                      │
  │  - public_key, encryption_key         │
  │  - signature, timestamp               │
  │  - permit (owner_connection)          │
  │──────────────────────────────────────>│
  │                                       │
  │                           decide_hello_response()
  │                           ✓ Validate permit signature
  │                           ✓ Check !first_connection flag
  │                           ✓ Verify owner DID matches stored
  │                           → AcceptReconnection(stored_permit)
  │                           
  │                           Update last_connected timestamp
  │                                       │
  │  Welcome                              │
  │  - node_id, node_public_key           │
  │  - signature, timestamp               │
  │  - permit_for_peer (from storage)     │
  │<──────────────────────────────────────│
  │                                       │
  │  decide_welcome_response()            │
  │  ✓ Permit matches stored permit       │
  │  → Skip PermitGrant step              │
  │                                       │
  │  Ack                                  │
  │──────────────────────────────────────>│
  │                                       │
  │  [AUTHENTICATED]                      │
  │  Subscribe to active scribes          │
```

### 3. Rejection Paths

The handshake includes validation at each step with explicit rejection reasons.

**Hello Decision Rejections** (`courier/src/handshake/decision.rs:96-124`):
- `RejectAlreadyHasOwner`: First-connection permit but owner already exists
- `RejectOwnerMismatch`: Reconnection permit but DID doesn't match stored owner
- `RejectNoOwnerForReconnection`: Reconnection permit but no owner stored
- `RejectInvalidPermit`: Signature invalid or permit malformed

**Welcome Decision Rejections** (`courier/src/handshake/decision.rs:130-162`):
- `RejectNodeMismatch`: Node pubkey doesn't match expected
- `RejectAudienceMismatch`: Permit audience doesn't match our DID

When rejection occurs, a `Rejected` message is sent with the reason.

---

## Publishing Flow

### Owner → Node

```
Owner                                   Node
  │                                       │
  │  PublishSpace                         │
  │  - space metadata                     │
  │  - space_permit                       │
  │──────────────────────────────────────>│
  │                                       │
  │                           Validate permit
  │                           Store space
  │                                       │
  │  PublishSpaceAck                      │
  │<──────────────────────────────────────│
  │                                       │
  │  PublishPage (for each page)          │
  │  - page metadata                      │
  │  - page_permit, owner_permit          │
  │  - layers (session encrypted)         │
  │──────────────────────────────────────>│
  │                                       │
  │                           Decrypt layers
  │                           Create Loro docs
  │                           Store page
  │                                       │
  │  PublishPageAck                       │
  │<──────────────────────────────────────│
```

---

## Viewer Flow

### 1. Get Shareable Link

```
Owner                                   Node
  │                                       │
  │  GetShareableLinkRequest              │
  │  - space_id                           │
  │──────────────────────────────────────>│
  │                                       │
  │                           Issue viewer permit
  │                           (aud:* wildcard)
  │                                       │
  │  GetShareableLinkResponse             │
  │  - permit (viewer_auth)               │
  │<──────────────────────────────────────│
```

### 2. Viewer Connects

```
Viewer                                  Node
  │                                       │
  │  [Has permit from shareable link]     │
  │                                       │
  │  SpaceRequest                         │
  │  - space_id                           │
  │  - viewer_did, viewer_public_key      │
  │  - viewer_permit (aud:*)              │
  │──────────────────────────────────────>│
  │                                       │
  │                           Validate aud:* permit
  │                           Delegate real permit
  │                           (aud:viewer_did)
  │                                       │
  │  SpaceData                            │
  │  - delegated_permit                   │
  │  - space metadata                     │
  │  - page_ids                           │
  │<──────────────────────────────────────│
  │                                       │
  │  SpaceDataAck                         │
  │  - echoed permit                      │
  │──────────────────────────────────────>│
  │                                       │
  │  PageData (for each page)             │
  │  - meta, permit                       │
  │  - layers (encrypted)                 │
  │<──────────────────────────────────────│
  │                                       │
  │  SyncConsentGrant                     │
  │  - space_consent_permit               │
  │  - page_consent_permits               │
  │──────────────────────────────────────>│
  │                                       │
  │                           Store consent permits
  │                           Can now sync
  │                                       │
  │  SyncConsentAck                       │
  │<──────────────────────────────────────│
```

---

## Sync Protocol (3-Step)

Works for all roles (owner ↔ node ↔ viewer). Permit determines authorization.

```
Sender                                  Receiver
  │                                       │
  │  SyncOffer                            │
  │  - page_id, layer_name                │
  │  - data (session encrypted)           │
  │  - state_vector (sender's)            │
  │  - permit (consent)                   │
  │──────────────────────────────────────>│
  │                                       │
  │                           Decrypt data
  │                           Apply to Loro
  │                           Get state_vector
  │                                       │
  │  SyncAccept                           │
  │  - state_vector (receiver's)          │
  │<──────────────────────────────────────│
  │                                       │
  │  Compare vectors                      │
  │  If match: done                       │
  │  If diverged: send new SyncOffer      │
  │                                       │
  │  SyncAck                              │
  │──────────────────────────────────────>│
  │                                       │
  │  [SYNC COMPLETE]                      │
```

### Encryption

Session-based symmetric encryption (derived once per connection):
- During handshake: `session_key = HKDF(ECDH(our_static, peer_static), "herald-session-v1")`
- Per message: `AES-256-GCM(session_key, data)` with random 12-byte nonce
- Wire format: nonce (12) || ciphertext || tag (16)

---

## Asset Sync (Blobs)

Uses iroh-blobs protocol (separate ALPN: `iroh-blobs/v1`).

```
Requester                               Provider
  │                                       │
  │  AssetPrepare                         │
  │  - page_id, hash                      │
  │──────────────────────────────────────>│
  │                                       │
  │                           Ensure blob ready
  │                                       │
  │  AssetReady                           │
  │  - iroh_hash (blake3)                 │
  │<──────────────────────────────────────│
  │                                       │
  │  [iroh-blobs download]                │
  │  transport.download_blob(hash, node)  │
  │──────────────────────────────────────>│
  │                                       │
  │  AssetAck                             │
  │  - success/error                      │
  │──────────────────────────────────────>│
```

---

## Ephemeral Datagrams

Unreliable, low-latency messages for real-time features.

```rust
struct EphemeralDatagram {
    page_id: String,      // Routing key
    payload: Vec<u8>,     // App-defined (JSON, msgpack, etc.)
}
```

- ~1200 byte MTU
- No length prefix (single datagram)
- Routed by page_id to Scribe, relayed to subscribers
- Use cases: cursor sync, typing indicators, presence

---

## Message Types

### Handshake
| Message | Direction | Purpose |
|---------|-----------|---------|
| `Hello` | Initiator → Acceptor | Introduce identity, present permit |
| `Welcome` | Acceptor → Initiator | Accept connection, issue permit |
| `PermitGrant` | Initiator → Acceptor | Grant capabilities to peer (first-connection only) |
| `Ack` | Either | Acknowledge |
| `Rejected` | Either | Reject with reason (see rejection types below) |

**Rejection Types**:
- `AlreadyHasOwner`: Node rejects first-connection when owner exists
- `OwnerMismatch`: Node rejects reconnection with wrong DID
- `NoOwnerForReconnection`: Node rejects reconnection with no stored owner
- `InvalidPermit`: Permit signature or format invalid
- `NodeMismatch`: User rejects Welcome with wrong node pubkey
- `AudienceMismatch`: User rejects Welcome with wrong permit audience

### Publishing
| Message | Direction | Purpose |
|---------|-----------|---------|
| `PublishSpace` | Owner → Node | Publish space metadata |
| `PublishSpaceAck` | Node → Owner | Confirm space stored |
| `PublishPage` | Owner → Node | Publish page with layers |
| `PublishPageAck` | Node → Owner | Confirm page stored |

### Viewer
| Message | Direction | Purpose |
|---------|-----------|---------|
| `GetShareableLinkRequest` | Owner → Node | Request viewer permit |
| `GetShareableLinkResponse` | Node → Owner | Return aud:* permit |
| `SpaceRequest` | Viewer → Node | Request space data |
| `SpaceData` | Node → Viewer | Space + delegated permit |
| `SpaceDataAck` | Viewer → Node | Confirm receipt |
| `PageData` | Node → Viewer | Page layers (encrypted) |
| `SyncConsentGrant` | Viewer → Node | Consent permits for sync |
| `SyncConsentAck` | Node → Viewer | Confirm consent stored |

### Sync
| Message | Direction | Purpose |
|---------|-----------|---------|
| `SyncOffer` | Sender → Receiver | Offer encrypted update |
| `SyncAccept` | Receiver → Sender | Accept with state vector |
| `SyncAck` | Sender → Receiver | Confirm sync complete |

### Assets
| Message | Direction | Purpose |
|---------|-----------|---------|
| `AssetPrepare` | Requester → Provider | Request asset preparation |
| `AssetReady` | Provider → Requester | Asset ready for download |
| `AssetAck` | Requester → Provider | Confirm receipt |

---

## Permit Types

| Type | Audience | Purpose |
|------|----------|---------|
| `one_time_connection` | owner_did | First connection bootstrap |
| `owner_connection` | owner_did | Reconnection |
| `node_connection` | node_did | Node joins network |
| `viewer_auth` | `*` (wildcard) | Shareable link |
| `space_owner` | owner_did | Full space control |
| `space_node` | node_did | Node can sync space |
| `space_viewer` | viewer_did | Viewer can read space |
| `page_owner` | owner_did | Full page control |
| `page_node` | node_did | Node can sync page |
| `page_viewer` | viewer_did | Viewer can read page |
| `sync_space_consent` | viewer_did | Viewer consents to sync |
| `sync_page_consent` | viewer_did | Viewer consents to page sync |

### Capabilities (from permit facts)

```json
{
  "relay": true,          // Can forward to other peers
  "share": true,          // Can delegate permits
  "accept_publish": true  // Can accept published data
}
```

---

## Crate Responsibilities

| Crate | Responsibility | Dependencies |
|-------|----------------|--------------|
| `transport` | QUIC connections, streams, blobs | iroh |
| `courier` | P2P orchestration, actors, sync | transport, butler, gurkha |
| `gurkha` | Permit parsing, authorization | herald (crypto) |
| `butler` | Storage, services, Scribe | herald, gurkha |
| `herald` | Identity, encryption, signing | - |

---

## Current Status

### Working
- Connection handshake (Hello/Welcome/PermitGrant)
- Space/page publishing (Owner → Node)
- Shareable links (aud:* permits)
- Viewer flow (SpaceRequest → SpaceData → SyncConsent)
- 3-step sync protocol
- Asset sync via iroh-blobs
- Ephemeral datagrams

### In Progress
- Multi-viewer scenarios
- Asset sync reliability (retry logic)

### Planned
- Live streams (audio/video)
- Offline queue
- Conflict resolution UI
