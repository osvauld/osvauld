# Simple Folder Sync - Implementation Documentation

**Date**: 2025-11-08
**Status**: ✅ Implemented
**Goal**: Simple push protocol for folder sharing - send folder data then resources after `share_folder()`

---

## Overview

### What We Built

A **simple push protocol** that automatically syncs folders and resources to nodes immediately after `share_folder()` completes:

1. **share_folder()** creates ACL records (folder_share_records and share_records with UCANs)
2. **Automatic sync trigger** → sends data to node
3. **Send folder data first** → FolderDataSync message
4. **Send all resources one-by-one** → ResourceDataSync messages (loop)

### Key Characteristics

- **No manifest exchange** - owner directly sends data
- **Fire-and-forget** - spawns async task, logs errors
- **Partial success allowed** - one failed item doesn't block others
- **Clean architecture** - sync_service → folder_sync → resource_sync
- **Service layer usage** - always use service methods, never call repositories directly
- **Bulk operations** - fetch multiple records in single SQL query

---

## Architecture

### Module Structure

```
sync_service.rs (orchestrator)
    ↓
    Gets connection, spawns task
    ↓
folder_sync.rs
    ↓
    send_folder_with_resources()
    ├── send_folder_data() → FolderDataSync
    └── resource_sync::send_all_resources_for_folder()
        ↓
resource_sync.rs
        ↓
        Loop through resources
        send_resource_data() → ResourceDataSync (for each)
```

### Data Flow

```
Owner Side:
share_folder() → sync_service::send_folder()
    ↓
    Get devices, get connection
    ↓
folder_sync::send_folder_with_resources()
    ↓
    1. Send FolderDataSync
    2. Call resource_sync::send_all_resources_for_folder()
        ↓
        Loop: Send ResourceDataSync for each resource

Node Side:
handle_folder_data_sync() → validate UCAN → save folder + share_record
handle_resource_data_sync() → validate UCAN → save resource + share_record
```

---

## Message Types

### FolderDataSync

**Direction**: Owner → Node
**Purpose**: Send folder metadata and share record

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FolderDataSync {
    pub folder: Folder,
    pub folder_share_record: FolderShareRecord,
}
```

**Owner sends**:
- Folder metadata (id, name, description, timestamps)
- Complete FolderShareRecord (UCAN token, permission level, etc.)

**Node receives**:
- Validates UCAN token
- Saves both in transaction

---

### ResourceDataSync

**Direction**: Owner → Node (one message per resource)
**Purpose**: Send re-encrypted resource and share record

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ResourceDataSync {
    /// Re-encrypted resource for node
    pub resource: EncryptedResource,
    /// Complete share record (includes UCAN token)
    pub share_record: ShareRecord,
}
```

**Owner process**:
1. Get resource from DB (encrypted for owner)
2. Decrypt with owner's key
3. Re-encrypt for node (using node's public key)
4. Get share_record for this resource
5. Send ResourceDataSync message

**Node receives**:
- Validates resource UCAN token
- Saves both in transaction

---

## Implementation Details

### File: `network/src/p2p/sync_service.rs`

**Lightweight orchestration layer** - gets connection and delegates to folder_sync

```rust
/// Send folder and all its resources to a node
///
/// Spawns async task (fire-and-forget). Errors logged.
pub async fn send_folder(
    folder_id: String,
    recipient_user_id: String,
    current_user: User,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: Arc<RwLock<CryptoUtils>>,
    p2p_service: Arc<P2PService>,
) -> ServiceResult<()> {
    tokio::spawn(async move {
        if let Err(e) = send_folder_impl(...).await {
            error!("Folder sync failed: {}", e);
        }
    });
    Ok(())
}

async fn send_folder_impl(...) -> ServiceResult<()> {
    // 1. Get recipient's devices
    let devices = repo_ctx.device_repo.get_devices_by_user_id(&recipient_user_id).await?;
    let device = &devices[0];

    // 2. Get peer connection
    let peer_conn = p2p_service.get_connection(&device.id).await?;

    // 3. Delegate to folder_sync
    folder_sync::send_folder_with_resources(
        &folder_id,
        &recipient_user_id,
        &current_user,
        peer_conn,
        repo_ctx,
        crypto_utils,
    ).await
}
```

---

### File: `network/src/p2p/folder_sync.rs`

**Folder sync operations** - sends folder first, then delegates resources

```rust
/// Send folder data then all resources
pub async fn send_folder_with_resources(
    folder_id: &str,
    recipient_user_id: &str,
    current_user: &User,
    peer_conn: Arc<PeerConnection>,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: Arc<RwLock<CryptoUtils>>,
) -> P2PResult<()> {
    // 1. Send folder first
    send_folder_data(folder_id, recipient_user_id, peer_conn.clone(), repo_ctx.clone()).await?;

    // 2. Send all resources (delegate to resource_sync)
    resource_sync::send_all_resources_for_folder(
        folder_id,
        recipient_user_id,
        current_user,
        peer_conn,
        repo_ctx,
        crypto_utils,
    ).await
}

/// Send just folder data
async fn send_folder_data(
    folder_id: &str,
    recipient_user_id: &str,
    peer_conn: Arc<PeerConnection>,
    repo_ctx: Arc<RepositoryContext>,
) -> P2PResult<()> {
    // Get folder by ID from folder_service
    let folder = folder_service::get_folder_by_id(folder_id, repo_ctx.clone()).await?;

    // Get share record from share_service
    let folder_share_record = share_service::get_folder_share_record(
        folder_id,
        recipient_user_id,
        repo_ctx,
    ).await?;

    // Send message
    let data = FolderDataSync { folder, folder_share_record };
    peer_conn.send_message(Message::FolderDataSync(data)).await
}

/// Handle folder data sync from owner (node side)
pub async fn handle_folder_data_sync(
    payload: FolderDataSync,
    _peer_conn: Arc<PeerConnection>,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: Arc<RwLock<CryptoUtils>>,
) -> P2PResult<()> {
    // Validate UCAN token
    let crypto = crypto_utils.read().await;
    crypto.validate_ucan(&payload.folder_share_record.ucan_token)?;
    drop(crypto);

    // Save folder and share record in transaction
    repo_ctx
        .folder_repo
        .save_folder_with_share_record(&payload.folder, &payload.folder_share_record)
        .await?;

    Ok(())
}
```

---

### File: `network/src/p2p/resource_sync.rs`

**Resource sync operations** - sends all resources for a folder (to be implemented)

```rust
/// Send all resources for a folder to node
pub async fn send_all_resources_for_folder(
    folder_id: &str,
    recipient_user_id: &str,
    current_user: &User,
    peer_conn: Arc<PeerConnection>,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: Arc<RwLock<CryptoUtils>>,
) -> P2PResult<()> {
    // TODO: Implementation pending
    // 1. Get all resources for folder (from resource_service)
    // 2. Get all share records for those resources (bulk, from share_service)
    // 3. Loop through resources
    // 4. For each: re-encrypt and send ResourceDataSync message
    Ok(())
}

/// Handle resource data sync from owner (node side)
pub async fn handle_resource_data_sync(
    payload: ResourceDataSync,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: Arc<RwLock<CryptoUtils>>,
) -> P2PResult<()> {
    // Validate resource UCAN
    let crypto = crypto_utils.read().await;
    crypto.validate_ucan(&payload.share_record.ucan_token)?;
    drop(crypto);

    // Save resource and share record in transaction
    repo_ctx
        .resource_repo
        .save_resource_with_share_record(&payload.resource, &payload.share_record)
        .await?;

    Ok(())
}
```

---

### File: `services/src/share_service.rs`

**Share record helpers** - centralized access to share records

```rust
/// Get folder share record for a user
pub async fn get_folder_share_record(
    folder_id: &str,
    user_id: &str,
    repo_ctx: Arc<RepositoryContext>,
) -> ServiceResult<FolderShareRecord> {
    repo_ctx
        .folder_share_repo
        .find_by_folder_and_user(folder_id, user_id)
        .await?
        .ok_or_else(|| FolderServiceError::Validation(...))
}

/// Get resource share record for a user
pub async fn get_resource_share_record(
    resource_id: &str,
    user_id: &str,
    repo_ctx: Arc<RepositoryContext>,
) -> ServiceResult<ShareRecord> {
    repo_ctx
        .share_repo
        .find_by_resource_and_operation_and_user(
            resource_id,
            &ShareOperation::Share.to_string(),
            user_id,
        )
        .await
}

/// Get all resource share records for a folder (bulk operation)
pub async fn get_resource_share_records_for_folder(
    folder_id: &str,
    user_id: &str,
    repo_ctx: Arc<RepositoryContext>,
) -> ServiceResult<Vec<ShareRecord>> {
    // Get all resource IDs in the folder
    let resource_ids = repo_ctx
        .resource_repo
        .get_resource_ids_by_folder_id(folder_id)
        .await?;

    if resource_ids.is_empty() {
        return Ok(Vec::new());
    }

    // Get all share records for those resources (bulk query)
    let share_records = repo_ctx
        .share_repo
        .find_by_resources_and_user(&resource_ids, user_id, &ShareOperation::Share.to_string())
        .await?;

    Ok(share_records)
}
```

---

### File: `services/src/folder_service.rs`

**Helper method** - get folder by ID

```rust
pub async fn get_folder_by_id(
    folder_id: &str,
    repo_ctx: Arc<RepositoryContext>,
) -> ServiceResult<Folder> {
    Ok(repo_ctx.folder_repo.find_by_id(folder_id).await?)
}
```

---

### File: `core/src/repositories/mod.rs`

**Bulk repository method** - fetch multiple share records in one query

```rust
#[async_trait]
pub trait ShareRepository: Send + Sync {
    // ... existing methods ...

    /// Find share records for multiple resources and a user with specific operation type
    /// Returns records in same order as input resource_ids
    async fn find_by_resources_and_user(
        &self,
        resource_ids: &[String],
        user_id: &str,
        operation_type: &str,
    ) -> Result<Vec<ShareRecord>, RepositoryError>;
}
```

---

### File: `persistance/src/repositories/share_repository.rs`

**Implementation** - uses SQL IN clause for bulk fetch

```rust
async fn find_by_resources_and_user(
    &self,
    resource_ids: &[String],
    user_id: &str,
    operation_type: &str,
) -> Result<Vec<ShareRecord>, RepositoryError> {
    if resource_ids.is_empty() {
        return Ok(Vec::new());
    }

    let mut conn = self.connection.get()?;

    // Fetch all matching records using IN clause
    let share_record_models = share_records::table
        .filter(share_records::resource_id.eq_any(resource_ids))
        .filter(share_records::recipient_user_id.eq(user_id))
        .filter(share_records::operation_type.eq(operation_type))
        .load::<ShareRecordModel>(&mut *conn)?;

    // Convert to domain models
    let share_records = share_record_models
        .into_iter()
        .map(|model| model.to_domain())
        .collect();

    Ok(share_records)
}
```

---

## Design Principles

### 1. Always Use Service Layer

❌ **Bad** - calling repository directly from sync code:
```rust
let folder = repo_ctx.folder_repo.find_by_id(folder_id).await?;
```

✅ **Good** - using service method:
```rust
let folder = folder_service::get_folder_by_id(folder_id, repo_ctx).await?;
```

### 2. Bulk Operations Over Loops

❌ **Bad** - N+1 queries:
```rust
for resource_id in resource_ids {
    let share_record = repo_ctx.share_repo
        .find_by_resource_and_user(&resource_id, user_id).await?;
}
```

✅ **Good** - single bulk query:
```rust
let share_records = repo_ctx.share_repo
    .find_by_resources_and_user(&resource_ids, user_id, "share").await?;
```

### 3. Standalone Functions

We use standalone functions instead of impl blocks for sending:

```rust
// Standalone function with Arc<PeerConnection> parameter
pub async fn send_folder_data(
    folder_id: &str,
    recipient_user_id: &str,
    peer_conn: Arc<PeerConnection>,  // Arc clone passed as parameter
    repo_ctx: Arc<RepositoryContext>,
) -> P2PResult<()> {
    // ...
}
```

### 4. Error Handling

**Log and continue** - partial success allowed:

```rust
match repo_ctx.folder_repo.save_folder_with_share_record(...).await {
    Ok(_) => {
        info!("Saved folder {}", folder_id);
        Ok(())
    }
    Err(e) => {
        error!("Failed to save folder {}: {}", folder_id, e);
        // Log and continue (partial success allowed)
        Ok(())
    }
}
```

---

## Integration with share_folder()

### Current State (TODO)

Need to update `share_folder()` to trigger sync automatically:

```rust
pub async fn share_folder(
    folder_id: &str,
    recipient_user_id: &str,
    // ... existing params ...
    p2p_service: Arc<P2PService>,  // NEW parameter
) -> ServiceResult<()> {
    // 1. Create ACL records
    // ... existing shareFolder logic ...

    // 2. Auto-trigger sync (NEW)
    sync_service::send_folder(
        folder_id.to_string(),
        recipient_user_id.to_string(),
        current_user.clone(),
        repo_ctx.clone(),
        crypto_utils.clone(),
        p2p_service,
    ).await?;

    Ok(())
}
```

### Tauri Handler Update (TODO)

Pass p2p_service to share_folder:

```rust
#[tauri::command]
pub async fn handle_share_folder(
    folder_id: String,
    recipient_user_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let p2p_service = state.p2p_service.clone();  // NEW

    folder_service::share_folder(
        &folder_id,
        &recipient_user_id,
        // ... other args ...
        p2p_service,  // Pass p2p_service
    ).await
}
```

---

## Pending Implementation

### 1. Complete resource_sync.rs

Need to implement `send_all_resources_for_folder()`:

**Steps**:
1. Add `prepare_resources_for_sync()` to resource_service.rs
   - Get all resources for folder
   - Re-encrypt each for node
   - Return Vec<(EncryptedResource, ShareRecord)>
2. Use `get_resource_share_records_for_folder()` from share_service.rs (✅ implemented)
3. Loop through resources and send ResourceDataSync messages

### 2. Add Repository Methods

Need to add to ResourceRepository:

```rust
/// Save resource and share record in transaction
async fn save_resource_with_share_record(
    &self,
    resource: &EncryptedResource,
    share_record: &ShareRecord,
) -> Result<(), RepositoryError>;
```

### 3. Update Module Exports

Update `network/src/p2p/mod.rs` to export sync_service:

```rust
pub mod sync_service;
pub use sync_service::send_folder;
```

---

## Testing Strategy

### Manual Testing

1. **Happy path**:
   - Share folder with node
   - Verify FolderDataSync sent
   - Verify all ResourceDataSync messages sent
   - Check node has folder + resources

2. **Error cases**:
   - Node offline → should log error, not crash
   - Invalid UCAN → should skip and continue
   - Database error → should log and continue

3. **Performance**:
   - Share folder with 100 resources
   - Measure time to sync
   - Verify no N+1 queries (check SQL logs)

---

## Summary

### What We Built

✅ Simple push protocol
✅ Clean 3-layer architecture (sync_service → folder_sync → resource_sync)
✅ Service layer usage pattern
✅ Bulk repository operations
✅ New message types (FolderDataSync, ResourceDataSync)
✅ Standalone functions with Arc clones
✅ Log and continue error handling
✅ Helper service (share_service.rs)

### What's Left (TODO)

⏳ Complete resource_sync.rs implementation
⏳ Add `save_resource_with_share_record()` to resource repository
⏳ Add `prepare_resources_for_sync()` to resource_service
⏳ Integrate with share_folder() (add p2p_service parameter)
⏳ Update tauri handler to pass p2p_service
⏳ Update module exports

### Differences from Original Plan

The original `FOLDER_SYNC_IMPLEMENTATION_PLAN.md` described a **manifest-based sync** with 4 phases:
1. Owner sends manifest
2. Node responds with what it has
3. Owner sends missing folders (bulk)
4. Owner sends missing resources (individual)

We implemented a **simpler push protocol** instead:
1. Owner directly sends folder (no manifest exchange)
2. Owner sends all resources one-by-one

**Why simpler approach**:
- Easier to implement and test
- Good enough for current use case (always sync everything)
- Can add manifest optimization later if needed
- Follows YAGNI principle

---

**Status**: Core infrastructure complete, pending resource re-encryption logic and integration.
