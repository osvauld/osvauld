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

### 1. User → Node (First Connection)

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
  │                           Validate permit
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
  │  Validate permit audience             │
  │  Issue permit_for_node                │
  │                                       │
  │  PermitGrant                          │
  │  - permit_for_node                    │
  │──────────────────────────────────────>│
  │                                       │
  │                           Validate & store
  │                                       │
  │  Ack                                  │
  │<──────────────────────────────────────│
  │                                       │
  │  [AUTHENTICATED]                      │
```

### 2. Reconnection

Same flow but with stored `owner_connection` permit instead of `one_time_connection`.

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
  │  - ephemeral_public (ECDH key)        │
  │  - layers (transit encrypted)         │
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
  │  - data (ECDH encrypted)              │
  │  - state_vector (sender's)            │
  │  - ephemeral_public                   │
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

- Sender generates ephemeral X25519 keypair
- ECDH: ephemeral_private × receiver_public = shared_secret
- ChaCha20Poly1305 encrypt: nonce (12) || ciphertext || tag (16)

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
| `PermitGrant` | Initiator → Acceptor | Grant capabilities to peer |
| `Ack` | Either | Acknowledge |
| `Rejected` | Either | Reject with reason |

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
