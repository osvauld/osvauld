# Network Layer - P2P Orchestration

**Last Updated:** 2025-11-18
**Status:** Permit-Based Network Layer Implemented

---

## Overview

The network layer (`network/src/p2p/`) orchestrates peer-to-peer communication between devices. It handles:
- WebSocket connection management
- Message routing and dispatching
- Permit-based handshake protocols
- Folder and resource synchronization

**Key Principle:** Network layer is pure orchestration - all business logic (including Permit validation) lives in service layer.

**See Also:**
- [PERMITS_OVERVIEW.md](./PERMITS_OVERVIEW.md) - Permit architecture
- [DELEGATION.md](./DELEGATION.md) - Trust chain and handshake flow
- [SYNC_PROTOCOL.md](./SYNC_PROTOCOL.md) - Permit-driven sync

---

## Architecture

```
network/src/p2p/
├── mod.rs              - Central message dispatcher
├── handshake.rs        - Three-way handshake protocol
├── folder_sync.rs      - Folder publishing orchestration
├── resource_sync.rs    - Resource publishing orchestration
├── sync_handler.rs     - Sync protocol coordination
├── peer_connection.rs  - WebSocket connection management
├── emitter.rs          - Event emission (UI updates)
└── errors.rs           - P2P error types
```

---

## Central Message Dispatcher

**File:** `network/src/p2p/mod.rs`

### Message Routing

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
        Message::Handshake(handshake_msg) => {
            handshake::process_handshake_message(
                &handshake_msg, peer_conn, repo_ctx, crypto_utils
            ).await
        }
        Message::Sync(sync_msg) => {
            sync_handler::process_sync_message(
                &sync_msg, peer_conn, repo_ctx, crypto_utils
            ).await
        }
    }
}
```

**Pattern:**
- Single entry point for all messages
- Delegates to specialized handlers by message type
- Each handler module is focused on one domain

**Status:** ✅ Working

---

## Handshake Protocol

**File:** `network/src/p2p/handshake.rs`

### Bearer Token Handshake (Owner → Node)

**Handshake uses Permits** for mutual authentication - both sides exchange bearer tokens that prove authorization.

**See:** [DELEGATION.md](./DELEGATION.md#adding-hosting-node) for complete handshake flow and Permit structure.

```
┌──────┐                              ┌──────┐
│Owner │                              │ Node │
└──┬───┘                              └───┬──┘
   │                                      │
   │ 1. FirstConnectRequest               │
   │    - one_time_ucan (Permit)          │
   │    - issued_ucan (Permit for Owner)  │
   ├──────────────────────────────────────>
   │                                      │
   │                                      │ Validates one_time_ucan Permit
   │                                      │ Stores issued_ucan Permit
   │                                      │ Creates Node user record
   │                                      │
   │              2. UcanAndUserExchange  │
   │                 - node_user          │
   │                 - ucan_token (Permit)│
   <──────────────────────────────────────┤
   │                                      │
   │ Stores node_user                     │
   │ Stores node's Permit                 │
   │                                      │
   │ 3. UserInfoConfirmation              │
   │    - owner_user                      │
   ├──────────────────────────────────────>
   │                                      │
   │                                      │ Stores owner_user
   │                                      │ Updates connection state
   │                                      │
   │           HANDSHAKE COMPLETE         │
```

**Key Concept:** One-time bearer token (`one_time_ucan`) proves authorization to pair. Node creates this with `operations.own = "allow"` - anyone holding this token is considered owner for the handshake.

**Code:** `kunki/src/main.rs` (node's one-time token generation)

### Message Handlers

**Network layer delegates Permit validation to service layer** - handlers only orchestrate message flow.

```rust
/// Handle FirstConnectRequest (step 1)
pub async fn handle_first_connect_request(
    request: &FirstConnectRequest,
    peer_conn: Arc<PeerConnection>,
    repo_ctx: Arc<RepositoryContext>,
) -> P2PResult<()> {
    // 1. Validate one_time_ucan Permit (service layer checks operations.own)
    // 2. Parse and store issued_ucan Permit (long-lived connection Permit)
    // 3. Update peer connection with validated Permit
    peer_conn.update_token(request.issued_ucan.clone()).await;

    // 4. Create user record for peer (if needed)
    // 5. Send UcanAndUserExchange response
    send_ucan_and_user_exchange(peer_conn, repo_ctx).await?;

    Ok(())
}

/// Handle UcanAndUserExchange (step 2)
pub async fn handle_ucan_and_user_exchange(
    exchange: &UcanAndUserExchange,
    peer_conn: Arc<PeerConnection>,
    repo_ctx: Arc<RepositoryContext>,
) -> P2PResult<()> {
    // 1. Parse peer's Permit (service layer validates)
    // 2. Store peer user info
    repo_ctx.user_repo.save_user(&exchange.node_user).await?;

    // 3. Update peer connection
    peer_conn.update_user(exchange.node_user.clone()).await;
    peer_conn.update_token(exchange.ucan_token.clone()).await;

    // 4. Send UserInfoConfirmation
    send_user_info_confirmation(peer_conn, repo_ctx).await?;

    Ok(())
}

/// Handle UserInfoConfirmation (step 3)
pub async fn handle_user_info_confirmation(
    confirmation: &UserInfoConfirmation,
    peer_conn: Arc<PeerConnection>,
    repo_ctx: Arc<RepositoryContext>,
) -> P2PResult<()> {
    // 1. Store owner user info
    repo_ctx.user_repo.save_user(&confirmation.owner_user).await?;

    // 2. Update peer connection
    peer_conn.update_user(confirmation.owner_user.clone()).await;

    // 3. Mark connection as ready
    peer_conn.mark_ready().await;

    // 4. Emit HandshakeComplete event
    peer_conn.event_emitter.emit(P2PEvent::HandshakeComplete {
        peer_user_id: confirmation.owner_user.id.clone(),
    });

    Ok(())
}
```

**Note:** Actual implementation includes full Permit parsing and validation - omitted here for clarity. See `network/src/p2p/handshake.rs` for complete code.

### Handshake Permit Update (Reconnection)

**Problem:** If owner reconnects after Permit rotation, node has stale Permit

**Solution:** Update stored Permit during handshake

```rust
// In handle_first_connect_request:
pub async fn handle_first_connect_request(
    request: &FirstConnectRequest,
    peer_conn: Arc<PeerConnection>,
    repo_ctx: Arc<RepositoryContext>,
) -> P2PResult<()> {
    // ...

    // Update peer connection with NEW Permit (fixes reconnection issue)
    peer_conn.update_token(request.issued_ucan.clone()).await;

    // ...
}
```

**Status:** ✅ Fixed in commit c563d03a

---

## Folder Sync

**File:** `network/src/p2p/folder_sync.rs`

### Folder Publishing Flow

**Folder publishing is Permit-driven** - node validates Permits before accepting folder/resources.

**See:** [DELEGATION.md](./DELEGATION.md#publishing-folder-to-node) for complete Permit flow.

```
┌──────┐                              ┌──────┐
│Owner │                              │ Node │
└──┬───┘                              └───┬──┘
   │                                      │
   │ send_folder_with_resources()         │
   │                                      │
   │ 1. Send FolderDataSync               │
   │    - folder metadata                 │
   │    - folder_share_record (Node Permit)│
   ├──────────────────────────────────────>
   │                                      │
   │                                      │ handle_folder_data_sync()
   │                                      │ - Validate Permit (add_folder)
   │                                      │ - Save folder + share record
   │                                      │
   │ 2. Send ResourceDataSync (for each)  │
   │    - encrypted resource              │
   │    - ALL share records               │
   │    - owner_folder_ucan (Permit)      │
   ├──────────────────────────────────────>
   ├──────────────────────────────────────>
   ├──────────────────────────────────────>
   │                                      │
   │                                      │ handle_resource_data_sync()
   │                                      │ - Validate Permit (add_resources)
   │                                      │ - Save resource + share records
   │                                      │
   │         FOLDER PUBLISHING COMPLETE   │
```

### Send Folder Data

```rust
pub async fn send_folder_with_resources(
    folder_id: &str,
    recipient_user_id: &str,
    current_user: &User,
    peer_conn: Arc<PeerConnection>,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: Arc<RwLock<CryptoUtils>>,
) -> P2PResult<()> {
    info!("📤 Sending folder {} to node {}", folder_id, recipient_user_id);

    // 1. Get owner's folder to extract UCAN
    let owner_folder = get_folder_by_id(folder_id, repo_ctx.clone())
        .await
        .map_err(|e| P2PError::InvalidState(format!("Failed to get folder: {}", e)))?;
    let owner_folder_ucan = owner_folder.ucan.clone();

    // 2. Send folder first
    send_folder_data(folder_id, recipient_user_id, peer_conn.clone(), repo_ctx.clone()).await?;

    // 3. Send all resources with owner's folder UCAN
    resource_sync::send_all_resources_for_folder(
        folder_id,
        recipient_user_id,
        current_user,
        owner_folder_ucan,
        peer_conn,
        repo_ctx,
        crypto_utils,
    )
    .await?;

    info!("✅ Successfully sent folder {} to node", folder_id);
    Ok(())
}
```

### Folder Message Handler

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
        FolderMessage::FolderSyncRequest(_) => {
            info!("FolderSyncRequest not yet implemented (CRDT sync)");
            Ok(())
        }
        FolderMessage::FolderSyncResponse(_) => {
            info!("FolderSyncResponse not yet implemented (CRDT sync)");
            Ok(())
        }
        FolderMessage::FolderTokenRequest(_) => {
            info!("FolderTokenRequest not yet implemented (shareable links)");
            Ok(())
        }
        FolderMessage::FolderTokenResponse(_) => {
            info!("FolderTokenResponse not yet implemented (shareable links)");
            Ok(())
        }
    }
}
```

### Handle Folder Data Sync

```rust
pub async fn handle_folder_data_sync(
    payload: &FolderDataSync,
    peer_conn: Arc<PeerConnection>,
    repo_ctx: Arc<RepositoryContext>,
    _crypto_utils: Arc<RwLock<CryptoUtils>>,
) -> P2PResult<()> {
    info!("📥 Received folder {} from peer", payload.folder.id);

    // Get the peer user to access their connection token
    let peer_user_guard = peer_conn.user.read().await;
    let peer_user_id = &peer_user_guard.id;
    let peer_connection_token = &peer_user_guard.ucan_token;
    let domain = &peer_conn.domain;

    // Delegate to folder_service for validation and saving
    services::accept_folder_from_peer(
        &payload.folder,
        &payload.folder_share_record,
        peer_connection_token,
        domain,
        repo_ctx,
    )
    .await
    .map_err(|e| {
        error!("❌ Failed to accept folder from peer: {}", e);
        error!("   Folder ID: {}", payload.folder.id);
        error!("   Peer user ID: {}", peer_user_id);
        P2PError::InvalidState(format!("Failed to accept folder: {}", e))
    })?;

    info!("✅ Accepted and saved folder {}", payload.folder.id);

    // Emit FolderSynced event
    peer_conn.event_emitter.emit(P2PEvent::FolderSynced {
        folder_id: payload.folder.id.clone(),
        folder_name: payload.folder.name.clone(),
    });

    Ok(())
}
```

**Status:** ✅ Working

---

## Resource Sync

**File:** `network/src/p2p/resource_sync.rs`

**Resource sync uses Permit-based filtering** - service layer filters documents based on dual-Permit validation.

**See:** [SYNC_PROTOCOL.md](./SYNC_PROTOCOL.md#document-filtering) for complete filtering logic.

### Send All Resources for Folder

```rust
pub async fn send_all_resources_for_folder(
    folder_id: &str,
    recipient_user_id: &str,
    current_user: &User,
    owner_folder_ucan: String,
    peer_conn: Arc<PeerConnection>,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: Arc<RwLock<CryptoUtils>>,
) -> P2PResult<()> {
    info!("📦 Sending all resources for folder {} to user {}", folder_id, recipient_user_id);

    // 1. Get all resources in folder
    let resources = repo_ctx
        .resource_repo
        .get_resource_ids_by_folder_id(folder_id)
        .await
        .map_err(|e| {
            error!("Failed to get resources for folder {}: {}", folder_id, e);
            P2PError::InvalidState(format!("Failed to get resources: {}", e))
        })?;

    if resources.is_empty() {
        info!("No resources to send for folder {}", folder_id);
        return Ok(());
    }

    info!("Found {} resources to send", resources.len());

    // 2. Get recipient's share records to know which resources to send
    let recipient_share_records = services::get_resource_share_records_for_folder(
        folder_id, recipient_user_id, repo_ctx.clone()
    )
    .await
    .map_err(|e| {
        error!("Failed to get recipient share records for folder {}: {}", folder_id, e);
        P2PError::InvalidState(format!("Failed to get share records: {}", e))
    })?;

    info!("Found {} resources shared with recipient", recipient_share_records.len());

    // 3. Get recipient user
    let recipient = repo_ctx
        .user_repo
        .get_user_by_id(recipient_user_id)
        .await
        .map_err(|e| {
            error!("Failed to get recipient user {}: {}", recipient_user_id, e);
            P2PError::InvalidState(format!("Failed to get recipient user: {}", e))
        })?;

    // 4. Get recipient's folder_share_record (contains their folder UCAN and role)
    let recipient_folder_share = services::get_folder_share_record(
        folder_id, recipient_user_id, repo_ctx.clone()
    )
    .await
    .map_err(|e| {
        error!("Failed to get recipient folder share record: {}", e);
        P2PError::InvalidState(format!("Failed to get folder share record: {}", e))
    })?;

    let recipient_folder_ucan = &recipient_folder_share.ucan_token;

    // Extract role from folder UCAN (stored in facts)
    let peer_role = services::ucan_service::extract_facts(recipient_folder_ucan)
        .await?
        .and_then(|facts| facts.get("role").and_then(|v| v.as_str()).map(String::from))
        .unwrap_or_else(|| "node".to_string());

    info!("  Recipient role: {}", peer_role);

    // 5. Loop through recipient's share records and send each resource
    for recipient_share in recipient_share_records {
        info!("   Sending resource: {}", recipient_share.resource_id);

        // Get ALL share records for this resource (for forwarding viewer updates)
        let all_share_records = services::get_all_share_records_for_resource(
            &recipient_share.resource_id, repo_ctx.clone()
        )
        .await
        .map_err(|e| {
            error!("Failed to get all share records for resource {}: {}", recipient_share.resource_id, e);
            P2PError::InvalidState(format!("Failed to get all share records: {}", e))
        })?;

        info!("     Including {} share records (for viewer forwarding)", all_share_records.len());

        // Prepare resource for peer (UCAN-first: validates folder access, delegates, filters, encrypts)
        let peer_encrypted_resource = match services::prepare_resource_transfer(
            &recipient_share.resource_id,
            current_user,
            recipient_folder_ucan,
            &peer_role,
            &recipient,
            repo_ctx.clone(),
            &crypto_utils,
        )
        .await
        {
            Ok(res) => res,
            Err(e) => {
                error!("Failed to prepare resource {} for peer: {}, skipping", recipient_share.resource_id, e);
                continue; // Partial success - continue with other resources
            }
        };

        // Create ResourceDataSync message with ALL share records
        let resource_data = ResourceDataSync {
            resource: peer_encrypted_resource,
            share_records: all_share_records,
            owner_folder_ucan: owner_folder_ucan.clone(),
        };

        // Send to peer (fire-and-forget pattern, log errors)
        if let Err(e) = send_resource_data(peer_conn.clone(), resource_data).await {
            error!("Failed to send resource {} to peer: {}, continuing", recipient_share.resource_id, e);
            // Continue with other resources even if one fails
        } else {
            info!("     ✓ Successfully sent resource");
        }
    }

    info!("✅ Finished sending resources for folder {} to user {}", folder_id, recipient_user_id);
    Ok(())
}
```

### Resource Message Handler

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
        ResourceMessage::MergeUpdate(_) => {
            info!("MergeUpdate not yet implemented (CRDT merge)");
            Ok(())
        }
        ResourceMessage::ResourceSyncRequest(_) => {
            info!("ResourceSyncRequest not yet implemented (CRDT sync)");
            Ok(())
        }
        ResourceMessage::ResourceNotFoundRequest(_) => {
            info!("ResourceNotFoundRequest not yet implemented");
            Ok(())
        }
        ResourceMessage::ResourceTransfer(_) => {
            info!("ResourceTransfer not yet implemented");
            Ok(())
        }
        ResourceMessage::ResourceTransferAck => {
            info!("ResourceTransferAck not yet implemented");
            Ok(())
        }
        ResourceMessage::AssetTransfer(_) => {
            info!("AssetTransfer not yet implemented");
            Ok(())
        }
    }
}
```

### Handle Resource Data Sync

```rust
pub async fn handle_resource_data_sync(
    payload: &ResourceDataSync,
    peer_conn: Arc<PeerConnection>,
    repo_ctx: Arc<RepositoryContext>,
    _crypto_utils: Arc<RwLock<CryptoUtils>>,
) -> P2PResult<()> {
    info!("📥 Received resource {} from peer", payload.resource.id);

    let domain = &peer_conn.domain;

    // Delegate to resource_service for validation and saving
    services::accept_resource_from_peer(
        &payload.resource,
        &payload.share_records,
        &payload.owner_folder_ucan,
        domain,
        repo_ctx,
    )
    .await
    .map_err(|e| {
        error!("Failed to accept resource from peer: {}", e);
        P2PError::InvalidState(format!("Failed to accept resource: {}", e))
    })?;

    info!("✓ Accepted and saved resource {}", payload.resource.id);

    // Extract metadata and emit ResourceSynced event
    let metadata = &payload.resource.metadata;
    let title = metadata
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("Untitled")
        .to_string();
    let resource_type = metadata
        .get("type")
        .and_then(|v| v.as_str())
        .unwrap_or("website")
        .to_string();
    let last_modified = metadata
        .get("last_modified")
        .and_then(|v| v.as_i64())
        .unwrap_or_else(|| payload.resource.updated_at);

    let metadata_json = serde_json::json!({
        "id": payload.resource.id,
        "title": title,
        "resourceType": resource_type,
        "folderId": payload.resource.folder_id,
        "lastModified": last_modified,
        "favourite": false,
        "preview": null,
    });

    peer_conn.event_emitter.emit(P2PEvent::ResourceSynced {
        metadata_json: metadata_json.to_string(),
    });

    Ok(())
}
```

**Status:** ✅ Working

---

## Error Handling

**File:** `network/src/p2p/errors.rs`

### P2P Error Types

```rust
pub enum P2PError {
    InvalidState(String),
    MessageError(String),
    ValidationError(String),
    NetworkError(String),
    ServiceError(String),
}
```

### Fire-and-Forget Pattern

Network layer uses partial success pattern for resource publishing:

```rust
// Send to peer (fire-and-forget pattern, log errors)
if let Err(e) = send_resource_data(peer_conn.clone(), resource_data).await {
    error!("Failed to send resource {} to peer: {}, continuing", resource_id, e);
    // Continue with other resources even if one fails
} else {
    info!("     ✓ Successfully sent resource");
}
```

**Rationale:**
- One resource failure shouldn't stop entire folder publishing
- Log errors for debugging
- Continue with other resources
- UI can show partial success

---

## Event Emission

**File:** `network/src/p2p/emitter.rs`

### P2P Events

```rust
pub enum P2PEvent {
    HandshakeComplete {
        peer_user_id: String,
    },
    FolderSynced {
        folder_id: String,
        folder_name: String,
    },
    ResourceSynced {
        metadata_json: String,
    },
}
```

### Usage

```rust
// Emit event after successful operation
peer_conn.event_emitter.emit(P2PEvent::FolderSynced {
    folder_id: folder.id.clone(),
    folder_name: folder.name.clone(),
});
```

**Purpose:** Update UI with sync progress without blocking network layer

---

## Summary

**Network Layer Principles:**
- ✅ Pure orchestration (no business logic)
- ✅ Delegates to service layer for Permit validation
- ✅ Fire-and-forget for partial failures
- ✅ Event emission for UI updates
- ✅ Type-safe message handling
- ✅ Permit-based authentication and authorization

**Current Implementation:**
- ✅ Three-way handshake with bearer tokens (Permit update on reconnection)
- ✅ Folder publishing (Owner → Node) with Permit validation
- ✅ Resource publishing with Permit-driven document filtering
- ✅ Dual-Permit validation (SyncContext in service layer)
- ⏳ Bidirectional sync (partially implemented)
- ⏳ Viewer sync (in progress)

**Key Concepts:**
- **Bearer tokens**: Handshake uses one-time Permits for mutual authentication
- **Permit validation**: Service layer validates operations using Permit facts
- **Document filtering**: Resources filtered based on document-level permissions
- **No hardcoded roles**: All authorization decisions from Permit facts

**Files:**
- `network/src/p2p/mod.rs` - Central dispatcher (150 lines)
- `network/src/p2p/handshake.rs` - Bearer token handshake (400 lines)
- `network/src/p2p/folder_sync.rs` - Folder sync with Permits (238 lines)
- `network/src/p2p/resource_sync.rs` - Resource sync with filtering (345 lines)

**Status:** ✅ **Permit-Based Network Layer Working**

**See Also:**
- [PERMITS_OVERVIEW.md](./PERMITS_OVERVIEW.md) - Core Permit architecture
- [DELEGATION.md](./DELEGATION.md) - Complete delegation flows
- [SYNC_PROTOCOL.md](./SYNC_PROTOCOL.md) - Permit-driven sync mechanics
- [SERVICE_LAYER.md](./SERVICE_LAYER.md) - Service integration patterns
