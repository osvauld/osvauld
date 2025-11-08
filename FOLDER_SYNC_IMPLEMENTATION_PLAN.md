# Folder Manifest Sync - Implementation Plan

**Date**: 2025-11-08
**Status**: Planning Phase
**Goal**: Implement efficient manifest-based sync protocol for folder sharing between owner and node

---

## Table of Contents
1. [Overview](#overview)
2. [Design Decisions](#design-decisions)
3. [Protocol Specification](#protocol-specification)
4. [Data Structures](#data-structures)
5. [Implementation Details](#implementation-details)
6. [File-by-File Changes](#file-by-file-changes)
7. [Testing Strategy](#testing-strategy)
8. [Future TODOs](#future-todos)

---

## Overview

### Problem Statement
After `shareFolder()` creates ACL records (folder_share_records and share_records with UCANs), the node needs to receive the actual folder metadata and encrypted resource data. The previous approach would send all data immediately, which is inefficient.

### Solution
Implement a 4-phase manifest-based protocol:
1. **Phase 1**: Owner sends lightweight manifest (folder_ids + UCANs)
2. **Phase 2**: Node responds with what it already has
3. **Phase 3**: Owner sends missing folders (bulk)
4. **Phase 4**: Owner sends missing resources (individual)

### Key Benefits
- **Efficient**: Only sync what node doesn't have
- **Verifiable**: Both sides can independently calculate and verify diff
- **Resumable**: Can be extended to resume interrupted syncs
- **Scalable**: Works for folders with 1 or 1000 resources

---

## Design Decisions

### Decision 1: Re-encryption and Filtering
**Question**: When re-encrypting resources, should we filter documents based on UCAN capabilities?

**Decision**: ✅ **Send all documents (Option A)**
- Send complete resource data without filtering
- Validate UCAN first to confirm it's our node
- Simpler implementation, no filtering logic needed
- Node can decrypt and access all documents in the resource

**Rationale**: For owner→node sharing, nodes are trusted and should have full access. Filtering is needed for viewer connections, not node connections.

---

### Decision 2: UCAN Validation Level
**Question**: What level of UCAN validation should we perform?

**Decision**: ✅ **Full validation (Option B)**

**Checks to perform**:
1. UCAN signature is valid
2. UCAN is not expired
3. `aud` (audience) matches node's ucan_pub_key
4. `iss` (issuer) matches owner's ucan_pub_key (we issued this token)
5. Capabilities include the folder_id/resource_id
6. Proof chain is valid (if delegated)

**Rationale**: Security is critical. Full validation ensures:
- We only sync to intended recipients
- Tokens haven't been tampered with
- Proof chain integrity for delegated UCANs

---

### Decision 3: Encryption Function
**Question**: Which crypto function to use for re-encrypting resource data?

**Decision**: ✅ **Use `crypto_utils::data_encryption::encrypt_data_for_user()` (Option A)**

**Function signature**:
```rust
pub fn encrypt_data_for_user(
    data: &str,
    public_key: &str,
) -> Result<(String, String), CryptoError>
// Returns: (encrypted_data, encrypted_aes_key)
```

**Process**:
1. Generates new random AES-256 key
2. Encrypts data with AES-GCM
3. Encrypts AES key with recipient's X25519 public key
4. Returns both as base64 strings

**Rationale**: High-level function handles all crypto operations correctly. No need to manually manage AES keys or X25519 encryption.

---

### Decision 4: Share Record Construction
**Question**: Should node construct FolderShareRecord/ShareRecord from UCAN, or should owner send complete records?

**Decision**: ✅ **Owner sends complete records (simplified approach)**

**What gets sent**:
- For folders: `Folder` + `FolderShareRecord` (complete from owner's DB)
- For resources: `EncryptedResource` + `ShareRecord` (complete from owner's DB)

**Node behavior**: Save records directly without parsing UCANs for construction

**Rationale**:
- Simpler implementation for POC
- Avoids UCAN parsing complexity
- Records already exist in owner's DB
- Can optimize later if needed

---

### Decision 5: Resource Sending Strategy
**Question**: Should we wait for ack after each resource, or send all and handle acks asynchronously?

**Decision**: ✅ **Send all resources without waiting (Option B)**

**Implementation**:
```rust
for resource in resources_to_send {
    let encrypted = re_encrypt_resource(resource, node_pub_key)?;
    send(ResourceDataSync { resource, share_record }).await?;
    // Don't wait for ack, continue sending
}
```

**Ack handling**: TODO for later phase (not in POC)

**Rationale**:
- Faster sync (pipeline approach)
- Node can process resources as they arrive
- Ack validation can be added incrementally
- Simpler initial implementation

---

### Decision 6: Folder Validation and Saving
**Question**: Should we validate and save all folders in one transaction, or individually?

**Decision**: ✅ **Validate and save each folder individually (Option B)**

**Implementation**:
```rust
let mut success_count = 0;
for folder_data in payload.folders {
    // Validate UCAN
    if validate_ucan(&folder_data.folder_share_record.ucan_token).is_ok() {
        // Save in individual transaction
        db.transaction(|| {
            save_folder(&folder_data.folder)?;
            save_folder_share_record(&folder_data.folder_share_record)?;
        })?;
        success_count += 1;
    } else {
        log_error("UCAN validation failed for folder {}", folder_data.folder.id);
    }
}
```

**Rationale**:
- Partial success possible (one bad folder doesn't block others)
- Better error isolation and logging
- More resilient to individual validation failures
- Transaction per folder ensures folder + share_record saved atomically

---

### Decision 7: Sync Trigger
**Question**: When should the manifest sync be initiated?

**Decision**: ✅ **Automatic immediately after shareFolder (Option B)**

**Implementation**:
```rust
pub async fn share_folder(
    folder_id: &str,
    recipient_user_id: &str,
    // ... other params
    p2p_service: &P2PService,  // NEW parameter
) -> ServiceResult<()> {
    // 1. Create ACL records
    // ... existing shareFolder logic ...

    // 2. Auto-trigger sync
    initiate_folder_sync(
        folder_id,
        recipient_user_id,
        repo_ctx,
        crypto_utils,
        p2p_service,
    ).await?;

    Ok(())
}
```

**Rationale**:
- Seamless user experience
- No manual sync step needed
- Immediate feedback if node is online
- Can add manual retry later if needed

---

## Protocol Specification

### Phase 1: Manifest Request

**Direction**: Owner → Node

**Message**: `FolderManifestRequest`

**Purpose**: Inform node about folders it should have access to

**Payload**:
```rust
{
  folders: [
    { folder_id: "f1", folder_ucan: "eyJ..." },
    { folder_id: "f2", folder_ucan: "eyJ..." }
  ]
}
```

**Node actions**:
1. Receive manifest request
2. Validate each folder UCAN (full validation)
3. Query database: which folder_ids do I already have?
4. For folders I have, query which resources exist
5. Build manifest response
6. Send response to owner

---

### Phase 2: Manifest Response

**Direction**: Node → Owner

**Message**: `FolderManifestResponse`

**Purpose**: Tell owner what node already has

**Payload**:
```rust
{
  folders_i_have: ["f1"],  // Just IDs
  resources_i_have: [
    { folder_id: "f1", resource_id: "r1", resource_ucan: "eyJ..." },
    { folder_id: "f1", resource_id: "r2", resource_ucan: "eyJ..." }
  ]
}
```

**Owner actions**:
1. Receive manifest response
2. Compute diff:
   ```
   folders_to_send = all_shared_folders - folders_i_have

   For each folder in folders_i_have:
     resources_to_send[folder_id] = all_resources[folder_id] - resources_i_have[folder_id]
   ```
3. Log: "Sending {x} folders, {y} resources to node {node_id}"
4. Prepare folder data (Phase 3)
5. Prepare resource data (Phase 4)

---

### Phase 3: Folder Data Sync (Bulk)

**Direction**: Owner → Node

**Message**: `FolderDataSync`

**Purpose**: Send missing folders with complete metadata and share records

**Payload**:
```rust
{
  folders: [
    {
      folder: Folder {
        id: "f2",
        name: "Work Notes",
        description: "...",
        is_deleted: false,
        created_at: 123456,
        updated_at: 123456
      },
      folder_share_record: FolderShareRecord {
        id: "...",
        folder_id: "f2",
        shared_by_user_id: "owner_id",
        recipient_user_id: "node_id",
        ucan_token: "eyJ...",
        ucan_cid: "bafy...",
        permission_level: Admin,
        operation_type: Share,
        created_at: 123456,
        updated_at: 123456
      }
    }
  ]
}
```

**Node actions**:
1. Receive folder data sync
2. For each folder:
   ```rust
   // Validate UCAN
   validate_ucan(&folder_share_record.ucan_token)?;

   // Save in transaction
   db.transaction(|| {
     save_folder(&folder)?;
     save_folder_share_record(&folder_share_record)?;
   })?;
   ```
3. Log: "Received {x} folders from owner {owner_id}"
4. Send ack (FolderDataAck)

**Ack payload**:
```rust
{
  received_count: 2,
  status: "success"  // or "partial" if some failed
}
```

---

### Phase 4: Resource Data Sync (Individual)

**Direction**: Owner → Node (multiple messages)

**Message**: `ResourceDataSync` (one per resource)

**Purpose**: Send missing resources with re-encrypted data and share records

**Payload**:
```rust
{
  resource: EncryptedResource {
    id: "r3",
    folder_id: "f2",
    encrypted_data: "...",  // Re-encrypted for node
    encrypted_key: "...",   // AES key encrypted with node's pub key
    ucan_token: "eyJ...",   // Node's resource UCAN
    metadata: { "title": "Note 1", "type": "notes" },
    created_at: 123456,
    updated_at: 123456
  },
  share_record: ShareRecord {
    id: "...",
    resource_id: "r3",
    shared_by_user_id: "owner_id",
    recipient_user_id: "node_id",
    ucan_token: "eyJ...",
    ucan_cid: "bafy...",
    permission_level: Admin,
    operation_type: Share,
    created_at: 123456,
    updated_at: 123456
  }
}
```

**Owner process** (for each resource):
```rust
1. Load EncryptedResource from owner's DB
2. Decrypt:
   - Decrypt encrypted_key with owner's private key → AES key
   - Decrypt encrypted_data with AES key → JSON string
3. Re-encrypt for node:
   let (new_encrypted_data, new_encrypted_key) =
     encrypt_data_for_user(&json_string, &node.ucan_pub_key)?;
4. Create new EncryptedResource with:
   - encrypted_data: new_encrypted_data
   - encrypted_key: new_encrypted_key
   - ucan_token: node's resource UCAN (from share_record)
   - Same metadata, id, folder_id
5. Load ShareRecord for this resource and node
6. Send ResourceDataSync message
```

**Node actions** (per message):
```rust
1. Receive ResourceDataSync
2. Validate resource UCAN (full validation)
3. Save in transaction:
   db.transaction(|| {
     save_encrypted_resource(&resource)?;
     save_share_record(&share_record)?;
   })?;
4. Log: "Received resource {resource_id} from owner"
5. (TODO: Send ack - not implemented in POC)
```

---

## Data Structures

### Message Enum Updates

**File**: `core/src/models/p2p.rs`

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Message {
    // ... existing variants ...
    Ping,
    Pong,
    Error,
    Handshake(HandshakeMessage),
    MergeUpdate(ResourceUpdateMsg),
    ResourceAdditionRequest(EncryptedResource),
    ResourceAdditionComplete,
    AssetTransfer(AssetTransferMessage),
    RetryRequest,
    FolderSync(FolderSyncMessage),  // existing
    FolderTokenRequest(FolderTokenRequest),
    FolderTokenResponse(FolderTokenResponse),

    // NEW: Manifest sync messages
    FolderManifestRequest(FolderManifestRequest),
    FolderManifestResponse(FolderManifestResponse),
    FolderDataSync(FolderDataSync),
    FolderDataAck(FolderDataAck),
    ResourceDataSync(ResourceDataSync),
}
```

---

### Phase 1: Manifest Request Structures

```rust
/// Owner sends manifest of folders node should have
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FolderManifestRequest {
    pub folders: Vec<FolderManifestItem>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FolderManifestItem {
    /// Folder ID
    pub folder_id: String,

    /// UCAN token for folder access (for validation)
    pub folder_ucan: String,
}
```

---

### Phase 2: Manifest Response Structures

```rust
/// Node responds with what it already has
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FolderManifestResponse {
    /// Folder IDs that node already has in database
    pub folders_i_have: Vec<String>,

    /// Resources that node already has
    pub resources_i_have: Vec<ResourceManifestItem>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ResourceManifestItem {
    /// Which folder this resource belongs to
    pub folder_id: String,

    /// Resource ID
    pub resource_id: String,

    /// UCAN token for resource access (for validation)
    pub resource_ucan: String,
}
```

---

### Phase 3: Folder Data Sync Structures

```rust
/// Owner sends missing folders with complete data
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FolderDataSync {
    pub folders: Vec<FolderWithShareRecord>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FolderWithShareRecord {
    /// Folder metadata
    pub folder: Folder,

    /// Complete folder share record (no UCAN parsing needed)
    pub folder_share_record: FolderShareRecord,
}

/// Node acknowledges receipt
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FolderDataAck {
    /// How many folders were successfully received
    pub received_count: usize,

    /// "success", "partial", or "failed"
    pub status: String,
}
```

---

### Phase 4: Resource Data Sync Structures

```rust
/// Owner sends one resource at a time
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ResourceDataSync {
    /// Re-encrypted resource for node
    pub resource: EncryptedResource,

    /// Complete share record (no UCAN parsing needed)
    pub share_record: ShareRecord,
}

// Note: ResourceDataAck defined but not used in POC
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ResourceDataAck {
    pub resource_id: String,
    pub status: String,  // "success" or "failed"
}
```

---

## Implementation Details

### Service Layer Functions

**File**: `services/src/folder_service.rs`

#### 1. Build Manifest Request

```rust
/// Get manifest request for folders shared with node
pub async fn get_folder_manifest_for_node(
    node_user_id: &str,
    repo_ctx: Arc<RepositoryContext>,
) -> ServiceResult<FolderManifestRequest> {
    // 1. Get all folders shared with this node
    let folder_share_records = repo_ctx
        .folder_share_repo
        .get_records_by_recipient_user_id(node_user_id)
        .await?;

    // 2. Build manifest items
    let folders = folder_share_records
        .into_iter()
        .map(|record| FolderManifestItem {
            folder_id: record.folder_id,
            folder_ucan: record.ucan_token,
        })
        .collect();

    Ok(FolderManifestRequest { folders })
}
```

---

#### 2. Prepare Missing Folders

```rust
/// Compute diff and prepare folder data to send
pub async fn prepare_missing_folders(
    manifest_response: &FolderManifestResponse,
    shared_folder_ids: &[String],
    repo_ctx: Arc<RepositoryContext>,
) -> ServiceResult<FolderDataSync> {
    // 1. Compute diff: which folders does node need?
    let folders_to_send: Vec<String> = shared_folder_ids
        .iter()
        .filter(|fid| !manifest_response.folders_i_have.contains(fid))
        .cloned()
        .collect();

    // 2. Load folder data for missing folders
    let mut folders_with_records = Vec::new();

    for folder_id in folders_to_send {
        // Load folder
        let folder = repo_ctx.folder_repo.find_by_id(&folder_id).await?;

        // Load folder_share_record
        let folder_share_record = repo_ctx
            .folder_share_repo
            .find_by_folder_and_user(&folder_id, node_user_id)
            .await?
            .ok_or_else(|| FolderServiceError::Validation(
                "Folder share record not found".to_string()
            ))?;

        folders_with_records.push(FolderWithShareRecord {
            folder,
            folder_share_record,
        });
    }

    Ok(FolderDataSync {
        folders: folders_with_records,
    })
}
```

---

#### 3. Prepare Missing Resources

```rust
/// Compute diff, re-encrypt, and prepare resource data to send
pub async fn prepare_missing_resources(
    manifest_response: &FolderManifestResponse,
    shared_folder_ids: &[String],
    node_user: &User,
    current_user: &User,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: &Arc<RwLock<CryptoUtils>>,
) -> ServiceResult<Vec<ResourceDataSync>> {
    let mut resource_syncs = Vec::new();

    // 1. For folders node already has, check which resources are missing
    for folder_id in shared_folder_ids {
        if manifest_response.folders_i_have.contains(folder_id) {
            // Node has this folder, check resources

            // Get all resources in this folder
            let all_resources = repo_ctx
                .resource_repo
                .find_all_by_folder(folder_id, &current_user.id)
                .await?;

            // Filter to resources node doesn't have
            let node_resource_ids: Vec<&String> = manifest_response
                .resources_i_have
                .iter()
                .filter(|r| &r.folder_id == folder_id)
                .map(|r| &r.resource_id)
                .collect();

            let missing_resources: Vec<_> = all_resources
                .into_iter()
                .filter(|r| !node_resource_ids.contains(&&r.id))
                .collect();

            // Re-encrypt and prepare each missing resource
            for encrypted_resource in missing_resources {
                let resource_sync = prepare_single_resource(
                    &encrypted_resource,
                    node_user,
                    current_user,
                    repo_ctx.clone(),
                    crypto_utils,
                ).await?;

                resource_syncs.push(resource_sync);
            }
        } else {
            // Node doesn't have this folder, send ALL resources
            let all_resources = repo_ctx
                .resource_repo
                .find_all_by_folder(folder_id, &current_user.id)
                .await?;

            for encrypted_resource in all_resources {
                let resource_sync = prepare_single_resource(
                    &encrypted_resource,
                    node_user,
                    current_user,
                    repo_ctx.clone(),
                    crypto_utils,
                ).await?;

                resource_syncs.push(resource_sync);
            }
        }
    }

    Ok(resource_syncs)
}
```

---

#### 4. Prepare Single Resource (Helper)

```rust
/// Re-encrypt single resource for node
async fn prepare_single_resource(
    owner_encrypted_resource: &EncryptedResource,
    node_user: &User,
    current_user: &User,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: &Arc<RwLock<CryptoUtils>>,
) -> ServiceResult<ResourceDataSync> {
    // 1. Decrypt owner's resource
    let decrypted_json = {
        let crypto = crypto_utils.read().await;
        crypto.decrypt_data(
            &owner_encrypted_resource.encrypted_data,
            &owner_encrypted_resource.encrypted_key,
        )?
    };

    // 2. Re-encrypt for node
    let (new_encrypted_data, new_encrypted_key) =
        crypto_utils::data_encryption::encrypt_data_for_user(
            &decrypted_json,
            &node_user.ucan_pub_key,
        )?;

    // 3. Get share_record for this resource and node
    let share_record = repo_ctx
        .share_repo
        .find_by_resource_and_user(
            &owner_encrypted_resource.id,
            &node_user.id,
        )
        .await?
        .ok_or_else(|| FolderServiceError::Validation(
            "Share record not found".to_string()
        ))?;

    // 4. Create new EncryptedResource for node
    let resource = EncryptedResource {
        id: owner_encrypted_resource.id.clone(),
        folder_id: owner_encrypted_resource.folder_id.clone(),
        encrypted_data: new_encrypted_data,
        encrypted_key: new_encrypted_key,
        ucan_token: share_record.ucan_token.clone(),
        metadata: owner_encrypted_resource.metadata.clone(),
        created_at: owner_encrypted_resource.created_at,
        updated_at: owner_encrypted_resource.updated_at,
    };

    Ok(ResourceDataSync {
        resource,
        share_record,
    })
}
```

---

#### 5. Process Received Folders (Node Side)

```rust
/// Validate and save folders received from owner
pub async fn process_received_folders(
    payload: &FolderDataSync,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: &Arc<RwLock<CryptoUtils>>,
) -> ServiceResult<usize> {
    let mut success_count = 0;

    for folder_data in &payload.folders {
        // Validate UCAN (full validation)
        let validation_result = {
            let crypto = crypto_utils.read().await;
            crypto.validate_ucan_full(
                &folder_data.folder_share_record.ucan_token,
                &folder_data.folder.id,
            )
        };

        if let Err(e) = validation_result {
            tracing::error!(
                "UCAN validation failed for folder {}: {}",
                folder_data.folder.id,
                e
            );
            continue;
        }

        // Save folder and share record in transaction
        let save_result = repo_ctx
            .folder_repo
            .save_folder_with_share_record(
                &folder_data.folder,
                &folder_data.folder_share_record,
            )
            .await;

        match save_result {
            Ok(_) => {
                success_count += 1;
                tracing::info!("Saved folder {}", folder_data.folder.id);
            }
            Err(e) => {
                tracing::error!(
                    "Failed to save folder {}: {}",
                    folder_data.folder.id,
                    e
                );
            }
        }
    }

    Ok(success_count)
}
```

---

#### 6. Process Received Resource (Node Side)

```rust
/// Validate and save single resource received from owner
pub async fn process_received_resource(
    payload: &ResourceDataSync,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: &Arc<RwLock<CryptoUtils>>,
) -> ServiceResult<()> {
    // 1. Validate resource UCAN (full validation)
    let crypto = crypto_utils.read().await;
    crypto.validate_ucan_full(
        &payload.share_record.ucan_token,
        &payload.resource.id,
    )?;
    drop(crypto);

    // 2. Save resource and share record in transaction
    repo_ctx
        .resource_repo
        .save_resource_with_share_record(
            &payload.resource,
            &payload.share_record,
        )
        .await?;

    tracing::info!("Saved resource {}", payload.resource.id);

    Ok(())
}
```

---

### Network Layer Handlers

**File**: `network/src/p2p/folder_sync.rs` (rewrite)

#### 1. Send Manifest Request

```rust
use crate::p2p::peer_connection::PeerConnection;
use osvauld_core::models::{FolderManifestRequest, Message};
use std::sync::Arc;

/// Send manifest request to node
pub async fn send_manifest_request(
    peer_conn: Arc<PeerConnection>,
    manifest: FolderManifestRequest,
) -> Result<(), P2PError> {
    tracing::info!(
        "Sending manifest request with {} folders",
        manifest.folders.len()
    );

    peer_conn
        .send_message(Message::FolderManifestRequest(manifest))
        .await
}
```

---

#### 2. Handle Manifest Request (Node Side)

```rust
/// Handle manifest request from owner, build and send response
pub async fn handle_manifest_request(
    payload: FolderManifestRequest,
    peer_conn: Arc<PeerConnection>,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: Arc<RwLock<CryptoUtils>>,
) -> Result<(), P2PError> {
    tracing::info!("Received manifest request with {} folders", payload.folders.len());

    let mut folders_i_have = Vec::new();
    let mut resources_i_have = Vec::new();

    // For each folder in manifest
    for folder_item in payload.folders {
        // Validate folder UCAN
        let crypto = crypto_utils.read().await;
        let validation_result = crypto.validate_ucan_full(
            &folder_item.folder_ucan,
            &folder_item.folder_id,
        );
        drop(crypto);

        if validation_result.is_err() {
            tracing::warn!(
                "Invalid UCAN for folder {}, skipping",
                folder_item.folder_id
            );
            continue;
        }

        // Check if we have this folder
        if let Ok(Some(_)) = repo_ctx.folder_repo.find_by_id(&folder_item.folder_id).await {
            folders_i_have.push(folder_item.folder_id.clone());

            // Get resources in this folder
            if let Ok(share_records) = repo_ctx
                .share_repo
                .find_by_folder(&folder_item.folder_id)
                .await
            {
                for share_record in share_records {
                    resources_i_have.push(ResourceManifestItem {
                        folder_id: folder_item.folder_id.clone(),
                        resource_id: share_record.resource_id,
                        resource_ucan: share_record.ucan_token,
                    });
                }
            }
        }
    }

    tracing::info!(
        "Node has {} folders, {} resources",
        folders_i_have.len(),
        resources_i_have.len()
    );

    // Send response
    let response = FolderManifestResponse {
        folders_i_have,
        resources_i_have,
    };

    peer_conn
        .send_message(Message::FolderManifestResponse(response))
        .await
}
```

---

#### 3. Handle Manifest Response (Owner Side)

```rust
/// Handle manifest response from node, prepare and send data
pub async fn handle_manifest_response(
    payload: FolderManifestResponse,
    peer_conn: Arc<PeerConnection>,
    shared_folder_ids: Vec<String>,
    node_user: User,
    current_user: User,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: Arc<RwLock<CryptoUtils>>,
) -> Result<(), P2PError> {
    tracing::info!(
        "Received manifest response: node has {} folders, {} resources",
        payload.folders_i_have.len(),
        payload.resources_i_have.len()
    );

    // 1. Prepare missing folders
    let folder_sync = services::folder_service::prepare_missing_folders(
        &payload,
        &shared_folder_ids,
        repo_ctx.clone(),
    )
    .await
    .map_err(|e| P2PError::Custom(e.to_string()))?;

    tracing::info!("Sending {} missing folders", folder_sync.folders.len());

    // 2. Send folders
    if !folder_sync.folders.is_empty() {
        send_folder_data(peer_conn.clone(), folder_sync).await?;
    }

    // 3. Prepare missing resources
    let resource_syncs = services::folder_service::prepare_missing_resources(
        &payload,
        &shared_folder_ids,
        &node_user,
        &current_user,
        repo_ctx,
        &crypto_utils,
    )
    .await
    .map_err(|e| P2PError::Custom(e.to_string()))?;

    tracing::info!("Sending {} missing resources", resource_syncs.len());

    // 4. Send resources (individual messages)
    for resource_sync in resource_syncs {
        send_resource_data(peer_conn.clone(), resource_sync).await?;
    }

    tracing::info!("Folder sync complete");

    Ok(())
}
```

---

#### 4. Send Folder Data

```rust
/// Send folder data sync message
pub async fn send_folder_data(
    peer_conn: Arc<PeerConnection>,
    folder_sync: FolderDataSync,
) -> Result<(), P2PError> {
    peer_conn
        .send_message(Message::FolderDataSync(folder_sync))
        .await
}
```

---

#### 5. Handle Folder Data Sync (Node Side)

```rust
/// Handle folder data sync from owner
pub async fn handle_folder_data_sync(
    payload: FolderDataSync,
    peer_conn: Arc<PeerConnection>,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: Arc<RwLock<CryptoUtils>>,
) -> Result<(), P2PError> {
    tracing::info!("Received {} folders from owner", payload.folders.len());

    // Process and save folders
    let received_count = services::folder_service::process_received_folders(
        &payload,
        repo_ctx,
        &crypto_utils,
    )
    .await
    .map_err(|e| P2PError::Custom(e.to_string()))?;

    tracing::info!("Successfully saved {} folders", received_count);

    // Send ack
    let ack = FolderDataAck {
        received_count,
        status: if received_count == payload.folders.len() {
            "success".to_string()
        } else {
            "partial".to_string()
        },
    };

    peer_conn
        .send_message(Message::FolderDataAck(ack))
        .await
}
```

---

#### 6. Send Resource Data

```rust
/// Send single resource data sync message
pub async fn send_resource_data(
    peer_conn: Arc<PeerConnection>,
    resource_sync: ResourceDataSync,
) -> Result<(), P2PError> {
    peer_conn
        .send_message(Message::ResourceDataSync(resource_sync))
        .await
}
```

---

#### 7. Handle Resource Data Sync (Node Side)

```rust
/// Handle single resource data sync from owner
pub async fn handle_resource_data_sync(
    payload: ResourceDataSync,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: Arc<RwLock<CryptoUtils>>,
) -> Result<(), P2PError> {
    tracing::info!("Received resource {} from owner", payload.resource.id);

    // Process and save resource
    services::folder_service::process_received_resource(
        &payload,
        repo_ctx,
        &crypto_utils,
    )
    .await
    .map_err(|e| P2PError::Custom(e.to_string()))?;

    // TODO: Send ack (not in POC)

    Ok(())
}
```

---

### Message Routing

**File**: `network/src/p2p/peer_connection.rs`

**Update `process_message()` function**:

```rust
async fn process_message(&self, message: &mut Message) -> P2PResult<()> {
    match message {
        // ... existing handlers ...

        Message::FolderManifestRequest(payload) => {
            folder_sync::handle_manifest_request(
                payload.clone(),
                Arc::new(self.clone()),
                self.repo_ctx.clone(),
                self.crypto_utils.clone(),
            )
            .await
        }

        Message::FolderManifestResponse(payload) => {
            // This needs to be handled at P2PService level
            // because we need shared_folder_ids context
            // For now, just log
            info!("Received FolderManifestResponse (handler TODO)");
            Ok(())
        }

        Message::FolderDataSync(payload) => {
            folder_sync::handle_folder_data_sync(
                payload.clone(),
                Arc::new(self.clone()),
                self.repo_ctx.clone(),
                self.crypto_utils.clone(),
            )
            .await
        }

        Message::FolderDataAck(payload) => {
            info!(
                "Received folder data ack: {} folders, status: {}",
                payload.received_count, payload.status
            );
            // TODO: Track acks for verification
            Ok(())
        }

        Message::ResourceDataSync(payload) => {
            folder_sync::handle_resource_data_sync(
                payload.clone(),
                self.repo_ctx.clone(),
                self.crypto_utils.clone(),
            )
            .await
        }

        // ... rest of handlers ...
    }
}
```

---

### Integration with shareFolder

**File**: `services/src/folder_service.rs`

**Update `share_folder()` signature and implementation**:

```rust
pub async fn share_folder(
    folder_id: &str,
    recipient_user_id: &str,
    _folder_permissions: Vec<(String, String)>,
    current_user: &User,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: &Arc<RwLock<CryptoUtils>>,
    domain: &str,
    p2p_service: &P2PService,  // NEW PARAMETER
) -> ServiceResult<()> {
    // ... existing shareFolder logic ...
    // (Create folder_share_record and share_records)

    // NEW: Auto-trigger sync
    initiate_folder_sync(
        folder_id,
        recipient_user_id,
        current_user,
        repo_ctx.clone(),
        crypto_utils.clone(),
        p2p_service,
    )
    .await?;

    Ok(())
}
```

---

### Sync Initiation Function

**File**: `services/src/folder_service.rs`

```rust
/// Initiate folder sync with node after sharing
async fn initiate_folder_sync(
    folder_id: &str,
    node_user_id: &str,
    current_user: &User,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: Arc<RwLock<CryptoUtils>>,
    p2p_service: &P2PService,
) -> ServiceResult<()> {
    // 1. Get node user
    let node_user = repo_ctx
        .user_repo
        .get_user_by_id(node_user_id)
        .await?;

    // 2. Get or create connection to node
    let node_device = repo_ctx
        .device_repo
        .get_devices_by_user_id(node_user_id)
        .await?
        .into_iter()
        .next()
        .ok_or_else(|| FolderServiceError::Validation(
            "Node has no devices".to_string()
        ))?;

    let peer_conn = p2p_service
        .connect_with_ticket(&node_device.device_pub_key)
        .await
        .map_err(|e| FolderServiceError::Validation(e.to_string()))?
        .ok_or_else(|| FolderServiceError::Validation(
            "Failed to connect to node".to_string()
        ))?;

    // 3. Build manifest request
    let manifest = get_folder_manifest_for_node(node_user_id, repo_ctx.clone()).await?;

    // 4. Send manifest request
    network::p2p::folder_sync::send_manifest_request(peer_conn, manifest)
        .await
        .map_err(|e| FolderServiceError::Validation(e.to_string()))?;

    tracing::info!(
        "Initiated folder sync for folder {} to node {}",
        folder_id,
        node_user_id
    );

    Ok(())
}
```

---

## File-by-File Changes

### 1. `core/src/models/p2p.rs`

**Changes**:
- Add `FolderManifestRequest`, `FolderManifestResponse`, `FolderDataSync`, `FolderDataAck`, `ResourceDataSync` to `Message` enum
- Define all new structs:
  - `FolderManifestRequest`
  - `FolderManifestItem`
  - `FolderManifestResponse`
  - `ResourceManifestItem`
  - `FolderDataSync`
  - `FolderWithShareRecord`
  - `FolderDataAck`
  - `ResourceDataSync`
  - `ResourceDataAck` (for future)

**Estimated lines**: ~150 new lines

---

### 2. `services/src/folder_service.rs`

**Changes**:
- Add `get_folder_manifest_for_node()`
- Add `prepare_missing_folders()`
- Add `prepare_missing_resources()`
- Add `prepare_single_resource()` (helper)
- Add `process_received_folders()`
- Add `process_received_resource()`
- Add `initiate_folder_sync()` (private)
- Update `share_folder()` signature to accept `p2p_service` parameter
- Update `share_folder()` to call `initiate_folder_sync()` at end

**Estimated lines**: ~400 new lines

---

### 3. `network/src/p2p/folder_sync.rs`

**Changes**:
- Complete rewrite (was deprecated stub)
- Add `send_manifest_request()`
- Add `handle_manifest_request()`
- Add `handle_manifest_response()`
- Add `send_folder_data()`
- Add `handle_folder_data_sync()`
- Add `send_resource_data()`
- Add `handle_resource_data_sync()`

**Estimated lines**: ~300 new lines (full rewrite)

---

### 4. `network/src/p2p/peer_connection.rs`

**Changes**:
- Update `process_message()` to handle new message types:
  - `FolderManifestRequest`
  - `FolderManifestResponse`
  - `FolderDataSync`
  - `FolderDataAck`
  - `ResourceDataSync`

**Estimated lines**: ~30 new lines

---

### 5. `core/src/repositories/mod.rs`

**Changes** (if needed):
- Add `save_resource_with_share_record()` to `ResourceRepository` trait
- Add `find_by_resource_and_user()` to `ShareRepository` trait

**Estimated lines**: ~10 new lines (trait methods)

---

### 6. `persistance/src/repositories/resource_repository.rs`

**Changes** (if needed):
- Implement `save_resource_with_share_record()`

**Estimated lines**: ~20 new lines

---

### 7. `persistance/src/repositories/share_repository.rs`

**Changes** (if needed):
- Implement `find_by_resource_and_user()`

**Estimated lines**: ~20 new lines

---

### 8. `crypto_utils/src/crypto_utils.rs`

**Changes** (if needed):
- Add `validate_ucan_full()` method if not exists
- Ensure `decrypt_data()` exists

**Estimated lines**: ~30 new lines (if validation function needed)

---

### 9. Tauri Handler (if needed for manual sync)

**File**: `tauri_handlers/src/handlers/folder.rs`

**Changes**:
- Add `sync_folder_to_node` command for manual sync trigger

**Estimated lines**: ~20 new lines

---

## Testing Strategy

### Unit Tests

#### 1. Diff Computation Tests
```rust
#[tokio::test]
async fn test_compute_missing_folders() {
    // Given: owner has f1, f2, f3
    // Node has: f1
    // Should send: f2, f3
}

#[tokio::test]
async fn test_compute_missing_resources() {
    // Given: folder f1 has r1, r2, r3
    // Node has: r1
    // Should send: r2, r3
}
```

#### 2. UCAN Validation Tests
```rust
#[tokio::test]
async fn test_validate_folder_ucan_success() {
    // Valid UCAN should pass
}

#[tokio::test]
async fn test_validate_folder_ucan_expired() {
    // Expired UCAN should fail
}

#[tokio::test]
async fn test_validate_folder_ucan_wrong_audience() {
    // Wrong aud should fail
}
```

#### 3. Re-encryption Tests
```rust
#[tokio::test]
async fn test_re_encrypt_resource() {
    // Owner encrypts resource
    // Re-encrypt for node
    // Node should be able to decrypt
}
```

---

### Integration Tests

#### 1. Full Sync Flow
```rust
#[tokio::test]
async fn test_full_folder_sync_flow() {
    // Setup: owner and node
    // 1. Owner shares folder
    // 2. Manifest exchange
    // 3. Folder sync
    // 4. Resource sync
    // 5. Verify node has all data
}
```

#### 2. Partial Sync
```rust
#[tokio::test]
async fn test_partial_sync_node_has_some_resources() {
    // Node already has some resources
    // Should only receive missing ones
}
```

#### 3. Empty Diff
```rust
#[tokio::test]
async fn test_sync_when_node_has_everything() {
    // Node already has all data
    // Should receive empty payloads
}
```

---

## Future TODOs

Document in `KNOWLEDGE_BASE.md`:

### 1. Ack Handling and Verification
- **Current**: Resources sent without waiting for acks
- **TODO**:
  - Track sent resources
  - Handle ResourceDataAck messages
  - Retry failed resources
  - Final verification: node confirms it has everything

### 2. Resume Interrupted Sync
- **Current**: No resume capability
- **TODO**:
  - Save sync state/progress
  - Detect interrupted syncs
  - Resume from last checkpoint
  - Handle partial failures gracefully

### 3. Sync State Tracking
- **Current**: No persistent tracking of what was synced
- **TODO**:
  - `sync_history` table
  - Track sync sessions, timestamps, status
  - Query: "When was folder last synced?"
  - Analytics: sync success rates

### 4. Manual Sync Trigger
- **Current**: Auto-sync only after shareFolder
- **TODO**:
  - Frontend "Sync Now" button
  - Tauri command: `syncFolderToNode`
  - Allow user to manually retry failed syncs

### 5. Automatic Sync on Connection
- **Current**: Only syncs when shareFolder called
- **TODO**:
  - After handshake, check for pending shares
  - Auto-sync any folders shared while node was offline
  - Background sync on reconnection

### 6. State Vector Sync (Phase 3)
- **Current**: Only initial data sync
- **TODO**:
  - For resources both owner and node have
  - Compare state vectors
  - 3-step CRDT merge (StateVectorRequest → UpdatesResponse → Apply)
  - Incremental updates instead of full snapshots

### 7. Asset Transfer
- **Current**: Only Loro documents synced
- **TODO**:
  - Detect asset IDs referenced in documents
  - Use AssetTransferMessage protocol
  - Send assets on-demand or with initial sync

### 8. Error Recovery
- **Current**: Errors logged, no retry
- **TODO**:
  - Exponential backoff retry
  - Circuit breaker for offline nodes
  - Dead letter queue for failed syncs

### 9. Progress Tracking
- **Current**: No progress indication
- **TODO**:
  - Emit progress events
  - Frontend progress bar
  - "Syncing: 5/10 resources..."

### 10. Bandwidth Optimization
- **Current**: Send full snapshots
- **TODO**:
  - Compression (gzip/brotli)
  - Chunked transfer for large resources
  - Rate limiting

---

## Logging Points

### Owner Side Logs

```rust
// Manifest request
info!("Sending manifest for {} folders to node {}", count, node_id);

// Manifest response received
info!("Node has {} folders, {} resources out of {} total",
      has_folders, has_resources, total);

// Diff computation
info!("Sending {} folders, {} resources to node {}",
      missing_folders, missing_resources, node_id);

// Folder sync sent
info!("Sent folder data with {} folders", count);

// Resource sync loop
info!("Sending resource {}/{}: {}", i, total, resource_id);

// Sync complete
info!("Folder sync complete for folder {} to node {}", folder_id, node_id);

// Errors
error!("Failed to re-encrypt resource {}: {}", resource_id, err);
error!("Failed to connect to node {}: {}", node_id, err);
```

---

### Node Side Logs

```rust
// Manifest request received
info!("Received manifest request with {} folders", count);

// Manifest processing
info!("I have {} folders, {} resources", has_folders, has_resources);

// UCAN validation
warn!("Invalid UCAN for folder {}, skipping", folder_id);

// Folder data received
info!("Received {} folders from owner {}", count, owner_id);

// Folder save result
info!("Saved folder {}", folder_id);
error!("Failed to save folder {}: {}", folder_id, err);

// Folder sync complete
info!("Successfully saved {} folders", success_count);

// Resource received
info!("Received resource {} from owner", resource_id);

// Resource save
info!("Saved resource {}", resource_id);
error!("Failed to save resource {}: {}", resource_id, err);

// Verification (future)
info!("Verification: expected {} resources, have {}", expected, actual);
```

---

## Summary

This implementation provides:
- ✅ Efficient manifest-based sync
- ✅ Independent diff calculation on both sides
- ✅ Full UCAN validation
- ✅ Secure re-encryption for node
- ✅ Individual folder/resource error handling
- ✅ Automatic sync after sharing
- ✅ Comprehensive logging for debugging
- ✅ Clear separation of concerns (service/network layers)
- ✅ Foundation for future enhancements (acks, resume, state vectors)

**Total estimated new code**: ~900 lines across 9 files

**Implementation time estimate**: 4-6 hours for core functionality + 2-3 hours for testing

---

**Next Step**: Review this plan, confirm all design decisions, then proceed with implementation.
