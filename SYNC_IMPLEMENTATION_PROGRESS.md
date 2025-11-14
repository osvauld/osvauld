# Sync Protocol Implementation Progress

**Status:** IN PROGRESS
**Started:** 2025-01-14
**Current Phase:** Folder Publishing (Owner → Node)

---

## Overview

This document tracks the implementation of the sync protocol as designed in `SYNC_PROTOCOL_DESIGN.md`. We're following a UCAN-first design principle where all permission logic is embedded in tokens, not in service code.

---

## Current State

### ✅ Completed
- UCAN infrastructure (token types, delegation functions)
- Folder service (create, share, accept from peer)
- Resource service (CRUD operations, core helpers)
- P2P infrastructure (auth, peer_connection, message handling)
- Basic folder_sync.rs (send/receive folder data)
- Basic resource_sync.rs (send/receive resource data)

### 🚧 In Progress
- **Folder Publishing (Owner → Node)** - Simple push flow
  - Fixing UCAN retrieval logic in `get_resource_ucans_for_sync()`
  - Implementing proper `accept_resource_from_peer()` validation
  - Wiring up network layer with connection_type

### ⏳ Not Started
- Bidirectional sync (Owner ↔ Node) - CRDT merge
- Viewer sync (Node ↔ Viewer) - Mixed mode
- Asset sync protocol
- Submission isolation for viewers

---

## Implementation Phases

### Phase 1: Folder Publishing (Owner → Node) [IN PROGRESS]

**Goal:** Simple one-way push of folder + resources from owner to node

**Flow:**
1. Owner calls `send_folder_with_resources(folder_id, node_user_id, ...)`
2. Send folder data with node's folder_share_record
3. For each resource shared with node:
   - Fetch node's UCAN from share_records
   - Decrypt resource, filter documents based on UCANs
   - Re-encrypt for node using node's public key
   - Send ResourceDataSync with resource + ALL share_records + owner_folder_ucan
4. Node validates and saves folder + resources

**Current Work:**
- [x] Folder sync handlers (send/receive)
- [ ] Fix `get_resource_ucans_for_sync()` - UCAN retrieval
- [ ] Implement `accept_resource_from_peer()` - validation & save
- [ ] Wire up network layer with connection_type
- [ ] Remove duplicate viewer-specific functions
- [ ] Test end-to-end flow

**Files:**
- `network/src/p2p/folder_sync.rs` - Folder orchestration
- `network/src/p2p/resource_sync.rs` - Resource orchestration
- `services/src/folder_service.rs` - Folder business logic
- `services/src/resource_service/sync.rs` - Resource sync helpers

---

### Phase 2: Bidirectional Sync (Owner ↔ Node) [NOT STARTED]

**Goal:** Two-way CRDT merge for collaborative editing

**Protocol:** ResourceSyncRequest → UpdatesResponse (2 rounds)

**Key Features:**
- State vectors for incremental updates
- Loro CRDT merge for all documents
- Asset ID comparison and transfer
- Always 2 rounds for convergence

**TODO:**
- Implement state vector generation
- Implement CRDT update generation
- Implement update application
- Add asset comparison logic
- Test convergence

---

### Phase 3: Viewer Sync (Node ↔ Viewer) [NOT STARTED]

**Goal:** Mixed-mode sync with read-only, collaborative, and submit-only documents

**Protocol:** Same as Phase 2 but with per-document capability checks

**Key Features:**
- `crud/readonly`: Node → Viewer only
- `crud/merge`: Bidirectional CRDT merge
- `crud/submit`: Viewer sends full document, node never sends back
- Submission isolation (viewer namespaces)

**TODO:**
- Implement submission document handling
- Test viewer isolation
- Test mixed-mode filtering

---

### Phase 4: Asset Sync [NOT STARTED]

**Goal:** Efficient binary asset transfer

**Protocol:** Asset IDs in state_vectors, separate AssetTransfer messages

**TODO:**
- Implement asset ID extraction from static_assets JSON
- Implement set operations for missing assets
- Implement AssetTransfer message handling
- Test large asset transfers

---

## Design Principles

### UCAN-First Design

**Core Principle:** Authorization logic lives in tokens, not in service code.

**Bad (role-based branching):**
```rust
if connection_type == Viewer {
    // Special viewer logic
} else if connection_type == Node {
    // Special node logic
}
```

**Good (UCAN-driven):**
```rust
let (our_ucan, peer_ucan, cid) = get_resource_ucans_for_sync(...);
let sync_context = create_sync_context(our_ucan, peer_ucan).await?;
filter_and_encrypt_for_peer(&resource, &sync_context, peer_public_key)
// sync_context contains ALL permission logic from UCANs
```

**Benefits:**
- Single code path for all roles
- No duplication (no separate viewer/node functions)
- Permission logic is declarative (in UCAN facts)
- Easier to test and reason about

---

## Known Issues & TODOs

### High Priority
- [ ] Fix `get_resource_ucans_for_sync()` - currently just clones owner token
- [ ] Implement `accept_resource_from_peer()` - currently a stub
- [ ] Remove `folder_publish_helpers` module reference
- [ ] Delete obsolete `handshake.rs` file

### Medium Priority
- [ ] Add user_id to UCAN facts for viewer optimization
- [ ] Deprecate `prepare_resource_for_viewer()` function
- [ ] Deprecate `create_viewer_resource_token()` wrapper
- [ ] Deprecate `issue_delegated_folder_token()` wrapper

### Low Priority
- [ ] Add comprehensive logging for sync operations
- [ ] Add metrics/telemetry for sync performance
- [ ] Document UCAN delegation chains

---

## Testing Strategy

### Unit Tests
- UCAN token parsing and validation
- Document filtering based on capabilities
- Asset ID extraction and comparison

### Integration Tests
- Owner → Node folder publishing
- Bidirectional resource sync
- Viewer submission isolation

### End-to-End Tests
- Multi-device sync scenarios
- Viewer workflows
- Asset sync with large files

---

## Notes

### Folder Publishing Simplification
The initial folder share uses simple push (ResourceDataSync), not the CRDT merge protocol. This is intentional:
- First sync: Complete resource transfer (no previous state)
- Subsequent syncs: Incremental CRDT updates (ResourceSyncRequest)

### Share Records
ALL share_records are sent during folder publishing to enable:
- Node can forward resources to viewers
- Proper UCAN chain reconstruction
- Multi-hop delegation support

### Connection Types
Set during auth handshake, stored in `peer_connection.connection_type`:
- `ConnectionType::Owner` - Owner ↔ Owner or Owner ↔ User
- `ConnectionType::Node` - Node ↔ Owner
- `ConnectionType::Viewer` - Viewer ↔ Node

Used to determine UCAN retrieval strategy (fetch from DB vs delegate on-demand).

---

## References

- `SYNC_PROTOCOL_DESIGN.md` - Complete protocol specification
- `network/src/p2p/` - P2P orchestration layer
- `services/src/` - Business logic layer
- `core/src/models/` - Data models and UCAN types
