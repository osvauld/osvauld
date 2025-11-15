# Osvauld Implementation Status

**Last Updated:** 2025-01-15
**Current Phase:** Folder Publishing Complete

---

## Overview

This document tracks the actual implementation status of Osvauld's sync protocol and UCAN-first permission system. It reflects what has been **implemented and working**, not just designed.

---

## Completed Work

### ✅ Phase 1: UCAN Infrastructure (Complete)

**Objective:** Establish UCAN-first permission system with typed tokens

**What Was Built:**
- Three-token architecture (Connection, Folder, Resource)
- Typed token wrappers for compile-time safety
- UCAN domain models with rich query APIs
- Template extraction (frontend is source of truth)

**Key Files:**
- `osvauld_core/src/models/capability.rs` - Domain types (Capability, Role, DocType)
- `osvauld_core/src/models/connection_token.rs` - Connection token domain model
- `osvauld_core/src/models/ucan_domain.rs` - ResourceUcan domain model
- `osvauld_core/src/models/ucan_token.rs` - Typed token wrappers (11 types)
- `services/src/ucan_service.rs` - Rewritten (1559 → 756 lines, zero hardcoded templates)

**Token Types:**
```
Connection Tokens (handshake/auth):
- OneTimeConnectionToken
- OwnerConnectionToken
- NodeConnectionToken
- UserConnectionToken
- ViewerAuthToken

Folder Tokens (folder access):
- FolderOwnerToken
- FolderShareToken
- FolderViewerToken

Resource Tokens (resource access):
- ResourceOwnerToken
- ResourceShareToken
- ResourceViewerToken
```

**Architecture Principles:**
1. **Frontend is source of truth** - All permission templates in `permissions.ts`
2. **Backend never hardcodes** - Extracts templates from delegator UCANs
3. **Data-driven** - Document names are data, not code
4. **Type-safe** - Typed wrappers prevent wrong token usage

**Status:** ✅ **Production Ready**

---

### ✅ Phase 2: Folder Publishing (Owner → Node) (Complete)

**Objective:** Implement simple one-way push of folders and resources from owner to node

**What Was Built:**

#### 2.1 Handshake Protocol (Three-Way)
**File:** `network/src/p2p/handshake.rs`

**Flow:**
```
1. Owner → Node: FirstConnectRequest
   - one_time_ucan: OneTimeConnectionToken (proves pairing)
   - issued_ucan: OwnerConnectionToken (long-lived sync token)

2. Node → Owner: UcanAndUserExchange
   - node_user: User info
   - ucan_token: NodeConnectionToken (node's connection token)

3. Owner → Node: UserInfoConfirmation
   - owner_user: User info
   - Completes handshake
```

**Key Features:**
- ✅ Handshake token update during reconnection (fixes stale token bug)
- ✅ Generic ConnectionToken support (works with Owner, Node, User tokens)
- ✅ UCAN-first validation (capability-based, not role-based)

**Status:** ✅ **Working**

---

#### 2.2 Folder Sync Protocol
**File:** `network/src/p2p/folder_sync.rs`

**Flow:**
```rust
pub async fn send_folder_with_resources(
    folder_id: &str,
    recipient_user_id: &str,
    current_user: &User,
    peer_conn: Arc<PeerConnection>,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: Arc<RwLock<CryptoUtils>>,
) -> P2PResult<()>
```

**Steps:**
1. Get owner's folder (contains owner's folder UCAN)
2. Send `FolderDataSync` message with:
   - Folder metadata (name, description)
   - Folder share record (contains node's folder UCAN)
3. Delegate to `resource_sync::send_all_resources_for_folder()`

**Message Handler:**
```rust
pub async fn handle_folder_data_sync(
    payload: &FolderDataSync,
    peer_conn: Arc<PeerConnection>,
    repo_ctx: Arc<RepositoryContext>,
) -> P2PResult<()>
```

**Validation:**
- ✅ Peer has `add_folder` capability (UCAN-first check)
- ✅ Folder UCAN structure is valid
- ✅ Saves folder + share record atomically

**Status:** ✅ **Working**

---

#### 2.3 Resource Sync Protocol
**File:** `network/src/p2p/resource_sync.rs`

**Flow:**
```rust
pub async fn send_all_resources_for_folder(
    folder_id: &str,
    recipient_user_id: &str,
    current_user: &User,
    owner_folder_ucan: String,
    peer_conn: Arc<PeerConnection>,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: Arc<RwLock<CryptoUtils>>,
) -> P2PResult<()>
```

**Steps:**
1. Get all resources in folder
2. Get recipient's share records (which resources they have access to)
3. Get recipient's folder share record (contains their folder UCAN and role)
4. For each resource:
   - Get ALL share records (for viewer forwarding)
   - Call `services::prepare_resource_transfer()` (UCAN-first: validates, delegates, filters, encrypts)
   - Send `ResourceDataSync` with:
     - Resource data (encrypted with recipient's key)
     - ALL share records (enables node to forward to viewers)
     - Owner's folder UCAN (proves add_resources permission)

**Message Handler:**
```rust
pub async fn handle_resource_data_sync(
    payload: &ResourceDataSync,
    peer_conn: Arc<PeerConnection>,
    repo_ctx: Arc<RepositoryContext>,
) -> P2PResult<()>
```

**Validation:**
- ✅ Owner has `add_resources` capability in folder UCAN
- ✅ Resource UCAN structure is valid
- ✅ Saves resource + ALL share records

**Status:** ✅ **Working**

---

#### 2.4 Service Layer Integration

**folder_service.rs:**
```rust
/// Accept and save a folder from a peer after validating add_folder capability
pub async fn accept_folder_from_peer(
    folder: &Folder,
    folder_share_record: &FolderShareRecord,
    peer_connection_token: &str,  // Peer's ConnectionToken
    domain: &str,
    repo_ctx: Arc<RepositoryContext>,
) -> ServiceResult<()>
```

**Validation Steps:**
1. Parse peer connection token as `ConnectionToken`
2. Check peer has `add_folder` capability using `check_capability()`
3. Validate folder share UCAN structure
4. Save folder + share record atomically

**Key Fix (commit 8704779e):**
```rust
// BEFORE (WRONG):
let add_folder_resource = format!("{}:add_folder", domain);  // "sthalam:add_folder"
check_capability(peer_token.parsed(), &add_folder_resource, "use")

// AFTER (CORRECT):
let folder_resource = format!("{}:folder:*", domain);  // "sthalam:folder:*"
check_capability(peer_token.parsed(), &folder_resource, "add_folder")
```

**resource_service.rs:**
```rust
/// Prepare resource for transfer to peer (UCAN-first)
pub async fn prepare_resource_transfer(
    resource_id: &str,
    current_user: &User,
    peer_folder_ucan: &str,
    peer_role: &str,
    peer_user: &User,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: &Arc<RwLock<CryptoUtils>>,
) -> ServiceResult<EncryptedResource>
```

**Steps:**
1. Validate folder access (peer's folder UCAN contains resource's folder_id)
2. Get peer's resource share record (contains their resource UCAN)
3. Filter documents based on peer's UCAN capabilities
4. Re-encrypt for peer using their public key

**accept_resource_from_peer.rs:**
```rust
/// Accept and save a resource from a peer (UCAN-first validation)
pub async fn accept_resource_from_peer(
    resource: &EncryptedResource,
    share_records: &[ShareRecord],
    owner_folder_ucan: &str,
    domain: &str,
    repo_ctx: Arc<RepositoryContext>,
) -> ServiceResult<()>
```

**Validation Steps:**
1. Extract folder_id from owner's folder UCAN
2. Verify owner has `add_resources` capability in folder UCAN
3. Validate resource UCAN structure
4. Save resource + ALL share records atomically

**Status:** ✅ **Working**

---

#### 2.5 Message Dispatching

**File:** `network/src/p2p/mod.rs`

**Central Message Dispatcher:**
```rust
pub async fn handle_message(
    message: Message,
    peer_conn: Arc<PeerConnection>,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: Arc<RwLock<CryptoUtils>>,
) -> P2PResult<()> {
    match message {
        Message::Folder(folder_msg) => {
            folder_sync::process_folder_message(
                &folder_msg, peer_conn, repo_ctx, crypto_utils
            ).await
        }
        Message::Resource(resource_msg) => {
            resource_sync::process_resource_message(
                &resource_msg, peer_conn, repo_ctx, crypto_utils
            ).await
        }
        // ... other message types
    }
}
```

**Folder Message Handler:**
```rust
pub async fn process_folder_message(
    folder_msg: &FolderMessage,
    peer_conn: Arc<PeerConnection>,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: Arc<RwLock<CryptoUtils>>,
) -> P2PResult<()> {
    match folder_msg {
        FolderMessage::FolderDataSync(payload) => {
            handle_folder_data_sync(payload, peer_conn, repo_ctx, crypto_utils).await
        }
        // FolderSyncRequest, FolderSyncResponse - not yet implemented
    }
}
```

**Resource Message Handler:**
```rust
pub async fn process_resource_message(
    resource_msg: &ResourceMessage,
    peer_conn: Arc<PeerConnection>,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: Arc<RwLock<CryptoUtils>>,
) -> P2PResult<()> {
    match resource_msg {
        ResourceMessage::ResourceDataSync(payload) => {
            handle_resource_data_sync(payload, peer_conn, repo_ctx, crypto_utils).await
        }
        // MergeUpdate, ResourceSyncRequest - not yet implemented
    }
}
```

**Status:** ✅ **Working**

---

### ✅ Cleanup: Removed Deprecated Code

**Deleted Files:**
- `network/src/p2p/website_sync.rs` (~500 lines) - Viewer-specific ductape
- Deprecated handshake functions in `handshake.rs`
- Old folder/resource sync helpers

**Removed Duplicate Functions:**
- Viewer-specific folder/resource transfer functions
- Hardcoded template generation code
- Role-based branching logic

**Code Reduction:**
- Network package: ~3500+ lines of deprecated code removed
- Services package: ~800 lines removed (hardcoded templates)
- Total reduction: ~4300 lines

**Result:**
- ✅ Single code path for all roles (Owner, Node, User, Viewer)
- ✅ UCAN-driven permissions (no role-based if/else)
- ✅ Cleaner architecture
- ✅ Easier to test and maintain

**Status:** ✅ **Complete**

---

## Current Architecture

### UCAN Token Hierarchy

```
┌─────────────────────────────────────────────────┐
│          CONNECTION TOKENS                      │
│  (Device-to-device handshake and auth)          │
├─────────────────────────────────────────────────┤
│ OneTimeConnectionToken  - Initial pairing       │
│ OwnerConnectionToken    - Owner device sync     │
│ NodeConnectionToken     - Node device sync      │
│ UserConnectionToken     - User P2P collab       │
│ ViewerAuthToken         - Viewer initial auth   │
└─────────────────────────────────────────────────┘
                    ↓
┌─────────────────────────────────────────────────┐
│           FOLDER TOKENS                         │
│   (Folder-level access control)                 │
├─────────────────────────────────────────────────┤
│ FolderOwnerToken   - Full folder control        │
│ FolderShareToken   - Node/User folder access    │
│ FolderViewerToken  - Viewer folder access       │
└─────────────────────────────────────────────────┘
                    ↓
┌─────────────────────────────────────────────────┐
│          RESOURCE TOKENS                        │
│  (Resource-level access control)                │
├─────────────────────────────────────────────────┤
│ ResourceOwnerToken   - Full resource control    │
│ ResourceShareToken   - Node/User access         │
│ ResourceViewerToken  - Viewer resource access   │
└─────────────────────────────────────────────────┘
```

**Key Principles:**
1. **Three separate token hierarchies** - Connection, Folder, Resource
2. **Typed wrappers** - Compile-time safety, can't pass wrong token type
3. **Frontend defines permissions** - Backend extracts from UCANs
4. **Template delegation** - Owner → Node → User → Viewer chain

---

### Network Layer (P2P Orchestration)

```
network/src/p2p/
├── mod.rs              - Central message dispatcher
├── handshake.rs        - Three-way handshake protocol
├── folder_sync.rs      - Folder publishing orchestration
├── resource_sync.rs    - Resource publishing orchestration
├── peer_connection.rs  - WebSocket connection management
└── sync_handler.rs     - Sync protocol coordination
```

**Orchestration Pattern:**
- Network layer handles message routing
- Delegates business logic to service layer
- Uses typed tokens for type safety
- Fire-and-forget for partial failures

**Status:** ✅ **Working**

---

### Service Layer

```
services/src/
├── ucan_service.rs     - UCAN token generation & validation (756 lines)
├── folder_service.rs   - Folder business logic (430 lines)
├── resource_service.rs - Resource business logic (1255 lines)
├── merge_service.rs    - CRDT merge operations (602 lines)
└── website_service.rs  - Viewer-specific operations (271 lines)
```

**Service Layer Principles:**
1. **UCAN-first** - All permission checks use UCAN capabilities
2. **Type-safe** - Functions use typed tokens
3. **Transactional** - Database operations are atomic
4. **Validation** - Business rules enforced before persistence

**Status:** ✅ **Working** (some transitional code remains)

---

## What's Working

### ✅ End-to-End Folder Publishing
1. Owner connects to Node (three-way handshake)
2. Owner publishes folder to Node
3. Node receives folder + all resources
4. Node can now serve resources to viewers

### ✅ UCAN-First Permission System
1. Frontend defines all permissions in `permissions.ts`
2. Backend extracts templates from UCANs
3. Validation uses `check_capability()` with correct resource format
4. No hardcoded templates or permission logic

### ✅ Type-Safe Token System
1. Typed wrappers prevent wrong token usage
2. Compile-time errors for type mismatches
3. Self-documenting function signatures
4. IDE autocomplete works correctly

---

## Not Yet Implemented

### ⏳ Bidirectional Sync (Owner ↔ Node)
**Protocol:** ResourceSyncRequest → UpdatesResponse (2 rounds)

**What's Missing:**
- State vector generation
- Incremental CRDT updates
- 2-round convergence protocol
- Asset ID comparison and transfer

**References:** SYNC_PROTOCOL_DESIGN.md sections 5.1, 6

---

### ⏳ Viewer Sync (Node ↔ Viewer)
**Protocol:** Mixed-mode sync with document-level permissions

**What's Missing:**
- Viewer handshake integration
- Read-only document filtering
- Submission isolation (viewer namespaces)
- Full snapshot sending for submissions

**References:** SYNC_PROTOCOL_DESIGN.md sections 5.2, 8

---

### ⏳ Folder Discovery Protocol
**Protocol:** FolderSyncRequest → FolderSyncResponse (3-step)

**What's Missing:**
- Resource discovery (which resources are missing)
- Parallel resource sync
- Incremental sync for existing resources

**References:** SYNC_PROTOCOL_DESIGN.md section 7

---

### ⏳ Asset Sync Protocol
**Protocol:** Asset IDs in state_vectors, separate AssetTransfer messages

**What's Missing:**
- Asset ID extraction from static_assets JSON
- Set operations for missing assets
- Binary asset transfer messages

**References:** SYNC_PROTOCOL_DESIGN.md section 6

---

## Known Issues

### None (Folder Publishing is Working)

All critical bugs in folder publishing have been fixed:
- ✅ Handshake token update during reconnection
- ✅ Capability validation using correct resource format
- ✅ Generic ConnectionToken support
- ✅ Message dispatching for folder/resource sync

---

## Next Steps

Based on SYNC_PROTOCOL_DESIGN.md implementation plan:

### 1. Phase 3: Bidirectional Sync (Owner ↔ Node)
**Estimated:** 3-4 days

**Tasks:**
- Implement state vector generation
- Implement CRDT update generation
- Implement 2-round protocol (ResourceSyncRequest)
- Add asset ID comparison logic
- Test convergence

**Priority:** High (required for real-time collaboration)

---

### 2. Phase 4: Viewer Sync (Node ↔ Viewer)
**Estimated:** 3-4 days

**Tasks:**
- Implement viewer handshake flow
- Implement mixed-mode document filtering
- Implement submission isolation (viewer namespaces)
- Test viewer workflows

**Priority:** High (required for viewer access)

---

### 3. Phase 5: Folder Discovery
**Estimated:** 2-3 days

**Tasks:**
- Implement FolderSyncRequest/Response
- Implement resource discovery
- Implement parallel resource sync
- Test folder sync scenarios

**Priority:** Medium (optimization, not critical path)

---

### 4. Phase 6: Asset Sync
**Estimated:** 2-3 days

**Tasks:**
- Implement asset ID extraction
- Implement set operations
- Implement AssetTransfer messages
- Test large asset transfers

**Priority:** Medium (required for images/files)

---

## References

- **SYNC_PROTOCOL_DESIGN.md** - Complete protocol specification
- **UCAN_REFACTOR_DESIGN.md** - UCAN architecture and token types
- **permissions.ts** - Frontend permission templates
- **Recent commits:**
  - `8704779e` - Implement UCAN-first folder publishing (Owner → Node sync)
  - `c563d03a` - Fix UCAN capability names and folder token parsing
  - `d8cd7ed2` - Implement three-way handshake protocol

---

## Summary

**What We Have:**
- ✅ UCAN-first permission system (typed tokens, template extraction)
- ✅ Folder publishing (Owner → Node) - Simple push flow
- ✅ Handshake protocol (three-way, token update on reconnection)
- ✅ Message dispatching (central dispatcher, folder/resource handlers)
- ✅ Service layer integration (UCAN-first validation, atomic transactions)

**What We Need:**
- ⏳ Bidirectional sync (CRDT merge, 2 rounds)
- ⏳ Viewer sync (mixed-mode, isolation)
- ⏳ Folder discovery (3-step protocol)
- ⏳ Asset sync (binary transfer)

**Status:** **Folder Publishing Complete** - Ready for bidirectional sync implementation
