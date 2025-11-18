# Service Layer - Business Logic

**Last Updated:** 2025-11-18
**Status:** V3 Permits Architecture Implemented

---

## Overview

The service layer (`services/src/`) contains all business logic for Osvauld. It sits between the network layer (P2P orchestration) and the persistence layer (database repositories).

**Key Principle:** All authorization uses Permits - services delegate to Gurkha for Permit interpretation and validation.

**See Also:**
- [PERMITS_OVERVIEW.md](./PERMITS_OVERVIEW.md) - Core Permit architecture
- [DELEGATION.md](./DELEGATION.md) - Trust chain and delegation flows
- [SYNC_PROTOCOL.md](./SYNC_PROTOCOL.md) - Permit-driven sync mechanics

---

## Architecture

```
services/src/
├── resource_service/
│   ├── mod.rs              - Module organization
│   ├── core.rs             - Core patterns (filter_and_encrypt_for_peer)
│   ├── crud.rs             - CRUD operations
│   └── sync.rs             - Sync orchestration
├── folder_service.rs       - Folder business logic
├── user_service.rs         - User management
├── auth_service.rs         - Authentication
├── errors.rs               - Service error types
└── lib.rs                  - Module exports
```

**Note:** CRDT merge operations moved to `gurkha/src/merge.rs` in v3.

---

## Key Patterns

### 1. Gurkha Integration

Services use **Gurkha** (the domain logic layer) for all Permit operations.

**Example: Issue Folder Permit**
```rust
// Service calls Gurkha
let permit_token = ucan_service.issue_folder_owner_token(
    folder_id,
    template_json,  // From frontend permissions.ts
    &crypto_utils,
).await?;
```

**Gurkha responsibilities**:
- Parse template JSON from frontend
- Build Permit facts structure
- Sign with Ed25519 key
- Return UCAN token string

**Code:** `gurkha/src/service.rs` (UcanService public API)

---

### 2. Permit-Driven Filtering

**Core pattern:** `filter_and_encrypt_for_peer()` uses dual-Permit validation to filter documents.

**Location:** `services/src/resource_service/core.rs:244-310`

**Flow:**
```rust
pub async fn filter_and_encrypt_for_peer(
    resource_id: &str,
    our_permit: &str,
    peer_permit: &str,
    peer_pubkey: &str,
    crypto_utils: &Arc<RwLock<CryptoUtils>>,
) -> ServiceResult<FilteredResource> {
    // 1. Load and decrypt resource
    let resource = load_and_decrypt(resource_id, crypto_utils).await?;

    // 2. Create SyncContext (dual-Permit validation)
    let sync_context = SyncContext::new(our_permit, peer_permit)?;

    // 3. Filter documents using Gurkha
    let filtered_docs: HashMap<String, Vec<u8>> = resource.documents
        .into_iter()
        .filter_map(|(doc_name, doc_bytes)| {
            // Ask Gurkha: should we send this document?
            let decision = should_send_updates(&sync_context, &doc_name);
            match decision {
                SyncDecision::DontSend => {
                    info!("⊘ Filtered out {} (Permit-driven)", doc_name);
                    None
                }
                _ => Some((doc_name, doc_bytes))
            }
        })
        .collect();

    // 4. Re-encrypt for peer (forward secrecy)
    let encrypted = encrypt_for_peer(filtered_docs, peer_pubkey).await?;

    Ok(encrypted)
}
```

**Key concepts**:
- **SyncContext**: Holds both our Permit and peer's Permit
- **should_send_updates()**: Gurkha decision function (reads Permit facts)
- **Forward secrecy**: Re-encrypt with unique key for each recipient

**Code:**
- `services/src/resource_service/core.rs:244-310` - filter_and_encrypt_for_peer
- `gurkha/src/decision.rs:575-624` - should_send_updates logic
- `gurkha/src/decision.rs:19-45` - SyncContext struct

---

### 3. Template-Driven Delegation

**Pattern:** Extract delegation template from Permit, create delegated Permit.

**Example: Delegate Folder to Node**
```rust
// Service layer
pub async fn delegate_folder_to_node(
    owner_folder_permit: &str,
    node_pubkey: &str,
    folder_id: &str,
    crypto_utils: &Arc<RwLock<CryptoUtils>>,
) -> ServiceResult<String> {
    // Call Gurkha to delegate
    let node_permit = ucan_service.delegate_folder(
        owner_folder_permit,
        node_pubkey,
        "node",  // Template name
        folder_id,
    ).await?;

    Ok(node_permit)
}
```

**Gurkha extracts template**:
1. Parse owner's Permit
2. Extract `delegation.node` template from Permit facts
3. Create new Permit with template facts
4. Sign with delegator's key
5. Return delegated Permit

**Code:**
- `gurkha/src/parser.rs:400-500` - extract_template_from_token
- `gurkha/src/service.rs:301-326` - issue_folder_owner_token
- `gurkha/src/service.rs:361-388` - delegate_folder

---

### 4. Forward Secrecy via Re-encryption

**Pattern:** Re-encrypt resource data for each recipient with unique keys.

**Why?** If one recipient is compromised, others remain secure.

**Example:**
```rust
// Encrypt for peer
pub async fn encrypt_and_save_resource(
    resource: &Resource,
    recipient_pubkey: &str,
    crypto_utils: &Arc<RwLock<CryptoUtils>>,
) -> ServiceResult<EncryptedResource> {
    // Generate unique symmetric key for this delegation
    let symmetric_key = generate_random_key();

    // Encrypt resource with symmetric key
    let encrypted_data = encrypt_with_key(&resource.documents, &symmetric_key)?;

    // Encrypt symmetric key with recipient's public key
    let encrypted_key = encrypt_key_for_recipient(&symmetric_key, recipient_pubkey)?;

    Ok(EncryptedResource {
        encrypted_data,
        encrypted_key,
    })
}
```

**Result:** Each delegation creates a unique encryption - no shared keys.

**Code:** `services/src/resource_service/core.rs` (encryption functions)

---

## Folder Service

**Location:** `services/src/folder_service.rs`

### create_folder()

```rust
pub async fn create_folder(
    name: String,
    folder_template_json: String,  // From frontend permissions.ts
    user: &User,
    domain: &str,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: &Arc<RwLock<CryptoUtils>>,
) -> ServiceResult<Folder>
```

**Flow:**
1. Validate input (non-empty name)
2. Call Gurkha to issue folder owner Permit
3. Create folder with Permit
4. Create folder share record (owner is first recipient)
5. Save folder + share record atomically

**Key point:** Frontend provides template, backend never hardcodes capabilities.

---

### share_folder()

```rust
pub async fn share_folder(
    folder_id: &str,
    recipient_user_id: &str,
    recipient_role: &str,  // "node" or "viewer"
    current_user: &User,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: &Arc<RwLock<CryptoUtils>>,
) -> ServiceResult<()>
```

**Flow:**
1. Get current user's folder Permit
2. Call Gurkha to delegate folder Permit to recipient (extracts template)
3. Create folder share record
4. Share all resources in folder with same role

---

### accept_folder_from_peer()

```rust
pub async fn accept_folder_from_peer(
    folder: &Folder,
    folder_share_record: &FolderShareRecord,
    peer_connection_permit: &str,
    domain: &str,
    repo_ctx: Arc<RepositoryContext>,
) -> ServiceResult<()>
```

**Flow:**
1. Parse peer's connection Permit
2. Validate peer has `add_folder` operation (Gurkha checks Permit facts)
3. Validate folder Permit structure
4. Save folder + share record atomically

**Permit validation:**
```rust
// Parse Permit
let peer_permit = Permit::from_token(peer_connection_permit)?;

// Ask Gurkha: does this Permit have add_folder operation?
if !peer_permit.has_operation("add_folder") {
    return Err("Peer lacks add_folder permission");
}
```

---

## Resource Service

**Location:** `services/src/resource_service/`

### Core Module (`core.rs`)

**Key function:** `filter_and_encrypt_for_peer()` (see Pattern #2 above)

**Code:** `services/src/resource_service/core.rs:244-310`

---

### CRUD Module (`crud.rs`)

#### create_resource()

```rust
pub async fn create_resource(
    folder_id: String,
    resource_type: String,
    metadata: serde_json::Value,
    resource_template_json: String,  // From frontend
    user: &User,
    domain: &str,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: &Arc<RwLock<CryptoUtils>>,
) -> ServiceResult<Resource>
```

**Flow:**
1. Create resource with default encrypted data
2. Call Gurkha to issue resource owner Permit (from template)
3. Create resource share record
4. Save resource + share record atomically

---

#### update_resource()

```rust
pub async fn update_resource(
    resource_id: &str,
    updated_resource_json: String,
    user: &User,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: &Arc<RwLock<CryptoUtils>>,
) -> ServiceResult<()>
```

**Flow:**
1. Parse updated resource JSON
2. **Re-encrypt** resource data (key rotation for forward secrecy)
3. Save to database

**Why re-encrypt?** Forward secrecy - each save uses new encryption key.

---

### Sync Module (`sync.rs`)

#### prepare_resource_transfer()

```rust
pub async fn prepare_resource_transfer(
    resource_id: &str,
    current_user: &User,
    peer_folder_permit: &str,
    peer_role: &str,
    peer_user: &User,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: &Arc<RwLock<CryptoUtils>>,
) -> ServiceResult<EncryptedResource>
```

**Flow:**
1. Validate peer's folder Permit contains resource's folder_id
2. Get peer's resource Permit
3. Load and decrypt resource
4. **Filter documents using dual-Permit validation** (calls filter_and_encrypt_for_peer)
5. Return encrypted, filtered resource

**This is the core sync function** - used by network layer when sending resources.

---

#### accept_resource_from_peer()

```rust
pub async fn accept_resource_from_peer(
    resource: &EncryptedResource,
    share_records: &[ShareRecord],
    owner_folder_permit: &str,
    domain: &str,
    repo_ctx: Arc<RepositoryContext>,
) -> ServiceResult<()>
```

**Flow:**
1. Parse owner's folder Permit
2. Validate owner has `add_resources` operation
3. Validate resource Permit structure
4. Save resource to database
5. Save ALL share records (enables node to forward to viewers)

---

## Error Handling

**File:** `services/src/errors.rs`

### Service Error Types

```rust
pub enum FolderServiceError {
    FolderNotFound { folder_id: String },
    EmptyFolderName,
    PermitValidation(String),
    DatabaseError(String),
}

pub enum ResourceServiceError {
    ResourceNotFound { resource_id: String },
    PermitValidation(String),
    EncryptionError(String),
    DatabaseError(String),
}
```

All service errors convert to `ServiceResult<T>`:

```rust
pub type ServiceResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;
```

---

## Summary

**Service Layer Principles (V3):**
- ✅ **Permit-driven** - All authorization via Permits
- ✅ **Gurkha integration** - Delegate to Gurkha for Permit operations
- ✅ **Dual-Permit validation** - Both sides' Permits must agree (SyncContext)
- ✅ **Template extraction** - Frontend defines permissions, backend interprets
- ✅ **Forward secrecy** - Re-encrypt for each recipient
- ✅ **Data-driven** - No hardcoded document names or capabilities
- ✅ **Transactional** - Database operations are atomic

**Key Patterns:**
1. **filter_and_encrypt_for_peer** - Core filtering pattern using SyncContext
2. **Template-driven delegation** - Extract templates from Permits
3. **Forward secrecy** - Re-encryption with unique keys
4. **Permit validation** - Check operations using Gurkha

**Files:**
- `services/src/resource_service/core.rs` - Core filtering pattern (~300 lines)
- `services/src/resource_service/crud.rs` - CRUD operations
- `services/src/resource_service/sync.rs` - Sync orchestration
- `services/src/folder_service.rs` - Folder management (~300 lines)

**Status:** ✅ **V3 Service Layer Working**

**See Also:**
- [PERMITS_OVERVIEW.md](./PERMITS_OVERVIEW.md) - Permit architecture
- [NETWORK_LAYER.md](./NETWORK_LAYER.md) - P2P orchestration
- [SYNC_PROTOCOL.md](./SYNC_PROTOCOL.md) - Sync mechanics
- [IMPLEMENTATION_STATUS.md](./IMPLEMENTATION_STATUS.md) - Current status
