# Osvauld Documentation

**Last Updated:** 2025-01-15
**Current Status:** Folder Publishing Complete

---

## Quick Links

### Implementation Status
- **[IMPLEMENTATION_STATUS.md](IMPLEMENTATION_STATUS.md)** - Current status, what's working, phase-by-phase progress

### Core Documentation
- **[UCAN_AUTHORIZATION.md](UCAN_AUTHORIZATION.md)** - Complete UCAN permission system (token types, delegation, data-driven design)
- **[NETWORK_LAYER.md](NETWORK_LAYER.md)** - P2P orchestration, message handling, handshake protocol
- **[SERVICE_LAYER.md](SERVICE_LAYER.md)** - Business logic layer (ucan_service, folder_service, resource_service)

### Specifications
- **[SYNC_PROTOCOL.md](SYNC_PROTOCOL.md)** - Complete sync protocol specification (bidirectional, viewer, assets)

---

## Documentation Structure

```
docs/
├── README.md                        (this file)
├── IMPLEMENTATION_STATUS.md         Current status & phase progress
├── UCAN_AUTHORIZATION.md            Complete UCAN system (consolidated)
├── NETWORK_LAYER.md                 P2P orchestration layer
├── SERVICE_LAYER.md                 Business logic layer
└── SYNC_PROTOCOL.md                 Protocol specification

Root (Historical References):
└── UCAN_REFACTOR_DESIGN.md          UCAN refactor design (historical)
```

---

## What's Implemented (Summary)

### ✅ Phase 1: UCAN Infrastructure (Complete)
- 11 typed token wrappers (Connection, Folder, Resource)
- UCAN domain models with rich query APIs
- Template extraction (frontend is source of truth)
- Zero hardcoded templates in backend

**Read:** [UCAN_AUTHORIZATION.md](UCAN_AUTHORIZATION.md)

---

### ✅ Phase 2: Folder Publishing (Owner → Node) (Complete)

**What Works:**
1. **Three-way handshake** - Owner connects to Node with token update on reconnection
2. **Folder sync** - Owner publishes folder to Node with UCAN-first validation
3. **Resource sync** - Owner sends all resources with share records for viewer forwarding
4. **Central dispatcher** - Message routing to specialized handlers (folder/resource)

**Read:**
- [NETWORK_LAYER.md](NETWORK_LAYER.md) - P2P orchestration and message handling
- [SERVICE_LAYER.md](SERVICE_LAYER.md) - Business logic (folder_service, resource_service)

---

## What's Next

### ⏳ Phase 3: Bidirectional Sync (Owner ↔ Node)
**Estimated:** 3-4 days

**Needs:**
- State vector generation
- CRDT update generation
- 2-round protocol (ResourceSyncRequest → UpdatesResponse)
- Asset ID comparison

**Read:** [SYNC_PROTOCOL.md](SYNC_PROTOCOL.md) section 5.1

---

### ⏳ Phase 4: Viewer Sync (Node ↔ Viewer)
**Estimated:** 3-4 days

**Needs:**
- Viewer handshake integration
- Mixed-mode document filtering (collaborator, viewer, submitter)
- Submission isolation (viewer namespaces)
- `no_incoming_updates` and `send_full_snapshot` handling

**Read:** [SYNC_PROTOCOL.md](SYNC_PROTOCOL.md) sections 5.2, 8

---

### ⏳ Phase 5: Folder Discovery
**Estimated:** 2-3 days

**Needs:**
- 3-step protocol (FolderSyncRequest → FolderSyncResponse)
- Resource discovery (which resources are missing)
- Parallel incremental sync (for existing resources)

**Read:** [SYNC_PROTOCOL.md](SYNC_PROTOCOL.md) section 7

---

### ⏳ Phase 6: Asset Sync
**Estimated:** 2-3 days

**Needs:**
- Asset ID extraction from static_assets JSON
- Set operations (HashSet difference) for missing assets
- AssetTransfer message handling
- Binary asset storage and retrieval

**Read:** [SYNC_PROTOCOL.md](SYNC_PROTOCOL.md) section 6

---

## Documentation Guide

### For New Developers

**Start Here:**
1. Read [IMPLEMENTATION_STATUS.md](IMPLEMENTATION_STATUS.md) - Understand current state
2. Read [UCAN_ARCHITECTURE.md](UCAN_ARCHITECTURE.md) - Understand permission system
3. Read [NETWORK_LAYER.md](NETWORK_LAYER.md) - Understand P2P orchestration
4. Read [SERVICE_LAYER.md](SERVICE_LAYER.md) - Understand business logic

**Then:**
- Browse [SYNC_PROTOCOL.md](SYNC_PROTOCOL.md) for protocol details
- Check phase progress in [IMPLEMENTATION_STATUS.md](IMPLEMENTATION_STATUS.md)

---

### For Understanding Specific Areas

**UCAN Tokens and Permissions:**
- [UCAN_AUTHORIZATION.md](UCAN_AUTHORIZATION.md) - Complete authorization system with token types and delegation patterns

**P2P Network Layer:**
- [NETWORK_LAYER.md](NETWORK_LAYER.md) - Handshake, folder sync, resource sync
- [SYNC_PROTOCOL.md](SYNC_PROTOCOL.md) - Message protocol specifications

**Service Layer (Business Logic):**
- [SERVICE_LAYER.md](SERVICE_LAYER.md) - ucan_service, folder_service, resource_service
- Code: `services/src/ucan_service.rs`, `services/src/folder_service.rs`, `services/src/resource_service.rs`

**Implementation Progress:**
- [IMPLEMENTATION_STATUS.md](IMPLEMENTATION_STATUS.md) - What's working now & phase tracker

---

### For Implementing New Features

**Before Starting:**
1. Check [SYNC_PROTOCOL.md](SYNC_PROTOCOL.md) for protocol specification
2. Check [IMPLEMENTATION_STATUS.md](IMPLEMENTATION_STATUS.md) for current phase and next steps

**During Implementation:**
1. Follow UCAN-first design (see [UCAN_ARCHITECTURE.md](UCAN_ARCHITECTURE.md))
2. Use typed tokens (see examples in [SERVICE_LAYER.md](SERVICE_LAYER.md))
3. Delegate business logic to service layer (see [NETWORK_LAYER.md](NETWORK_LAYER.md) for orchestration pattern)
4. Never hardcode templates or document names (see [UCAN_ARCHITECTURE.md](UCAN_ARCHITECTURE.md) delegation pattern)

**After Implementation:**
1. Update [IMPLEMENTATION_STATUS.md](IMPLEMENTATION_STATUS.md)
2. Add examples to relevant documentation

---

## Key Design Principles

### 1. UCAN-First Permission System
✅ **All permission logic in UCAN tokens, not in service code**

**Example:**
```rust
// GOOD (UCAN-first):
let folder_resource = format!("{}:folder:*", domain);
crypto_utils::ucan_utils::check_capability(
    peer_token.parsed(),
    &folder_resource,
    "add_folder",
)?;

// BAD (role-based):
if peer_role == "owner" || peer_role == "node" {
    // Allow folder creation
}
```

**Read:** [UCAN_AUTHORIZATION.md](UCAN_AUTHORIZATION.md)

---

### 2. Frontend is Source of Truth
✅ **Frontend defines all permissions, backend never hardcodes**

**Flow:**
```
1. Frontend (permissions.ts) defines permission templates
2. Frontend generates complete owner UCAN with all templates
3. Backend stores UCAN as-is (no modification)
4. Backend extracts templates when delegating
```

**Read:** [UCAN_AUTHORIZATION.md](UCAN_AUTHORIZATION.md) section "Template Extraction Pattern"

---

### 3. Type-Safe Token System
✅ **Typed token wrappers prevent wrong token usage**

**Example:**
```rust
// Compile-time type safety:
let owner_token = ResourceOwnerToken::from_token(&token)?;
let node_token = delegate_resource_to_node(&owner_token, node_id, ...)?;

// Can't pass wrong type (compile error):
let viewer_token = ResourceViewerToken::from_token(&token)?;
let node_token = delegate_resource_to_node(&viewer_token, ...)?;  // ❌ Compile error!
```

**Read:** [UCAN_AUTHORIZATION.md](UCAN_AUTHORIZATION.md) section "Token Type Hierarchy"

---

### 4. Network Layer is Pure Orchestration
✅ **No business logic in P2P handlers, delegate to service layer**

**Pattern:**
```rust
// Network layer (orchestration only):
pub async fn handle_folder_data_sync(payload, peer_conn, repo_ctx) {
    // Delegate to service layer for business logic
    services::accept_folder_from_peer(
        &payload.folder,
        &payload.folder_share_record,
        peer_connection_token,
        domain,
        repo_ctx,
    ).await?;

    // Emit event for UI
    peer_conn.event_emitter.emit(P2PEvent::FolderSynced { ... });
}
```

**Read:** [NETWORK_LAYER.md](NETWORK_LAYER.md)

---

## File Structure

### Core Models
```
osvauld_core/src/models/
├── capability.rs           - Domain types (Capability, Role, DocType)
├── connection_token.rs     - Connection token domain model
├── ucan_domain.rs          - ResourceUcan domain model
├── ucan_token.rs           - Typed token wrappers (11 types)
└── p2p.rs                  - P2P message types
```

### Network Layer (P2P Orchestration)
```
network/src/p2p/
├── mod.rs                  - Central message dispatcher
├── handshake.rs            - Three-way handshake
├── folder_sync.rs          - Folder publishing
├── resource_sync.rs        - Resource publishing
├── peer_connection.rs      - WebSocket management
└── emitter.rs              - Event emission
```

### Service Layer (Business Logic)
```
services/src/
├── ucan_service.rs         - UCAN generation & validation (756 lines)
├── folder_service.rs       - Folder lifecycle (430 lines)
├── resource_service.rs     - Resource lifecycle (1255 lines)
└── merge_service.rs        - CRDT operations (602 lines)
```

### Frontend (Source of Truth)
```
sthalam/frontend/desktop/src/config/
└── permissions.ts          - Permission templates (FOLDER_TEMPLATE, RESOURCE_TEMPLATE)
```

---

## Recent Commits

```
8704779e - feat: Implement UCAN-first folder publishing (Owner → Node sync)
c563d03a - fix: Update UCAN capability names and folder token parsing
d8cd7ed2 - feat: Implement three-way handshake protocol for peer connections
```

---

## Summary

**What We Have:**
- ✅ UCAN-first permission system (typed tokens, template extraction)
- ✅ Folder publishing (Owner → Node) working end-to-end
- ✅ Clean architecture (network orchestration, service logic, data persistence)
- ✅ Type-safe token system (11 typed wrappers, compile-time safety)
- ✅ Frontend is source of truth (zero hardcoded templates)

**What We Need:**
- ⏳ Bidirectional sync (CRDT merge, 2 rounds)
- ⏳ Viewer sync (mixed-mode, isolation)
- ⏳ Folder discovery (3-step protocol)
- ⏳ Asset sync (binary transfer)

**Where to Start:**
1. Read [IMPLEMENTATION_STATUS.md](IMPLEMENTATION_STATUS.md)
2. Read [UCAN_ARCHITECTURE.md](UCAN_ARCHITECTURE.md)
3. Read [NETWORK_LAYER.md](NETWORK_LAYER.md)
4. Read [SERVICE_LAYER.md](SERVICE_LAYER.md)

**Status:** ✅ **Folder Publishing Complete - Ready for Bidirectional Sync**
