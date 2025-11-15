# Service Layer - Business Logic

**Last Updated:** 2025-01-15
**Status:** Folder Publishing Complete

---

## Overview

The service layer (`services/src/`) contains all business logic for Osvauld. It sits between the network layer (P2P orchestration) and the persistence layer (database repositories).

**Key Principle:** All permission checks use UCAN capabilities, not role-based logic.

---

## Architecture

```
services/src/
├── ucan_service.rs         - UCAN token generation & validation (756 lines)
├── folder_service.rs       - Folder business logic (430 lines)
├── resource_service.rs     - Resource business logic (1255 lines)
├── merge_service.rs        - CRDT merge operations (602 lines)
├── website_service.rs      - Viewer-specific operations (271 lines)
├── errors.rs               - Service error types
└── lib.rs                  - Module exports
```

---

## ucan_service.rs

**Purpose:** UCAN token generation, delegation, and validation

**Lines:** 756 (down from 1559 - 51% reduction)

**Status:** ✅ **Production Ready** (zero hardcoded templates)

### Module Structure

```rust
pub mod connection_tokens;  // Connection token generation
pub mod folder_tokens;      // Folder token generation
pub mod resource_tokens;    // Resource token generation
pub mod validation;         // UCAN validation functions
pub mod utilities;          // Helper functions (extract_*, get_*)
```

### Connection Tokens

#### issue_one_time()

```rust
/// Issue one-time connection token for initial pairing
pub async fn issue_one_time(
    delegatee_pub_key: &str,
    role: &str,
    domain: &str,
    crypto_utils: &Arc<RwLock<CryptoUtils>>,
    repo_ctx: Arc<RepositoryContext>,
) -> UcanServiceResult<OneTimeConnectionToken>
```

**Usage:** Initial device pairing (single-use, short-lived)

**Capabilities:**
- `{domain}:user:*:connect`
- `{domain}:user:*:share`

---

#### issue_owner_connection()

```rust
/// Issue owner connection token for device-to-device sync
pub async fn issue_owner_connection(
    delegatee_pub_key: &str,
    domain: &str,
    crypto_utils: &Arc<RwLock<CryptoUtils>>,
    repo_ctx: Arc<RepositoryContext>,
) -> UcanServiceResult<OwnerConnectionToken>
```

**Usage:** Owner device sync (long-lived)

**Capabilities:**
- `{domain}:user:*:connect`
- `{domain}:user:*:share`
- `{domain}:folder:*:add_folder`

---

#### issue_node_connection()

```rust
/// Issue node connection token
pub async fn issue_node_connection(
    delegatee_pub_key: &str,
    domain: &str,
    crypto_utils: &Arc<RwLock<CryptoUtils>>,
    repo_ctx: Arc<RepositoryContext>,
) -> UcanServiceResult<NodeConnectionToken>
```

**Usage:** Node device sync (long-lived)

**Capabilities:**
- `{domain}:user:*:connect`
- `{domain}:folder:*:add_folder`

---

### Folder Tokens

#### issue_folder_owner_token()

```rust
/// Issue self-signed folder owner token
///
/// Frontend provides complete folder_template_json with delegation templates
pub async fn issue_folder_owner_token(
    folder_id: &str,
    domain: &str,
    folder_template_json: &str,  // From frontend permissions.ts
    crypto_utils: &Arc<RwLock<CryptoUtils>>,
    repo_ctx: &Arc<RepositoryContext>,
) -> UcanServiceResult<(String, String)>  // (ucan_token, cid)
```

**Key Points:**
- ✅ Frontend provides complete template (including delegation templates)
- ✅ Backend never hardcodes folder capabilities
- ✅ Returns both token and CID (for share record)

**Delegation Templates Included:**
- `node` - Node folder capabilities
- `viewer` - Viewer folder capabilities

---

#### delegate_folder_to_node()

```rust
/// Delegate folder access to node
///
/// Extracts node template from owner's folder token (NEVER HARDCODE!)
pub async fn delegate_folder_to_node(
    owner_folder_token: &FolderOwnerToken,
    delegatee_pub_key: &str,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: Arc<RwLock<CryptoUtils>>,
) -> UcanServiceResult<FolderShareToken>
```

**Delegation Pattern (All folder/resource delegation functions follow this):**
```rust
// 1. Extract template from delegator UCAN (NEVER HARDCODE!)
let template = owner_folder_token.ucan()
    .get_delegation_template("node")
    .ok_or_else(|| UcanServiceError::MissingTemplate("node"))?;

// 2. Build capabilities (DATA-DRIVEN)
let capabilities = template.build_capabilities(folder_id, "folder");

// 3. Convert to facts (GENERIC)
let mut facts = template.to_facts();
facts["role"] = json!("node");
facts["token_type"] = json!("folder_share");

// 4-6. Get key, generate token, return typed wrapper
```

---

#### delegate_folder_to_viewer()

```rust
/// Delegate folder access to viewer
///
/// Extracts viewer template from delegator's folder token
pub async fn delegate_folder_to_viewer(
    delegator_folder_token: &FolderShareToken,  // Node or User token
    viewer_ucan_pub_key: &str,
    folder_id: &str,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: Arc<RwLock<CryptoUtils>>,
) -> UcanServiceResult<FolderViewerToken>
```

**Usage:** Node → Viewer delegation during viewer handshake

---

### Resource Tokens

#### issue_resource_owner_token()

```rust
/// Issue self-signed resource owner token
///
/// Frontend provides complete resource_template_json with delegation templates
pub async fn issue_resource_owner_token(
    resource_id: &str,
    domain: &str,
    resource_template_json: &str,  // From frontend permissions.ts
    crypto_utils: &Arc<RwLock<CryptoUtils>>,
    repo_ctx: Arc<RepositoryContext>,
) -> UcanServiceResult<ResourceOwnerToken>
```

**Key Points:**
- ✅ Frontend provides complete template (including delegation templates for node/user/viewer)
- ✅ Backend never hardcodes resource capabilities or document names
- ✅ Returns typed ResourceOwnerToken

**Delegation Templates Included:**
- `node` - Node resource capabilities
- `user` - User resource capabilities
- `viewer` - Viewer resource capabilities

---

#### delegate_resource_to_node()

```rust
/// Delegate resource access to node
///
/// Extracts node template from owner's resource token (NEVER HARDCODE!)
pub async fn delegate_resource_to_node(
    owner_resource_token: &ResourceOwnerToken,
    node_user_id: &str,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: Arc<RwLock<CryptoUtils>>,
) -> UcanServiceResult<ResourceShareToken>
```

**Same delegation pattern as folder tokens** - extracts template, builds capabilities, returns typed token.

---

#### delegate_resource_to_user()

```rust
/// Delegate resource access to user (for P2P collaboration)
pub async fn delegate_resource_to_user(
    delegator_resource_token: &ResourceShareToken,  // Node token
    user_id: &str,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: Arc<RwLock<CryptoUtils>>,
) -> UcanServiceResult<ResourceShareToken>
```

---

#### delegate_resource_to_viewer()

```rust
/// Delegate resource access to viewer
///
/// Extracts viewer template from delegator's resource token
pub async fn delegate_resource_to_viewer(
    delegator_resource_token: &ResourceShareToken,  // Node or User token
    viewer_ucan_pub_key: &str,
    resource_id: &str,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: Arc<RwLock<CryptoUtils>>,
) -> UcanServiceResult<ResourceViewerToken>
```

**Usage:** Node → Viewer delegation during resource transfer

---

### Validation Functions

#### validate_peer_can_add_folder()

```rust
/// Validate peer has add_folder capability (UCAN-first)
pub async fn validate_peer_can_add_folder(
    peer_token: &ConnectionToken,
    domain: &str,
) -> ServiceResult<()>
```

**Capability Check:**
```rust
// CORRECT format (fixed in commit 8704779e)
let folder_resource = format!("{}:folder:*", domain);  // "sthalam:folder:*"
crypto_utils::ucan_utils::check_capability(
    peer_token.parsed(),
    &folder_resource,   // Resource pattern
    "add_folder",       // Ability
)?;
```

**Common Mistake (WRONG):**
```rust
// WRONG - Don't do this!
let add_folder_resource = format!("{}:add_folder", domain);  // "sthalam:add_folder"
crypto_utils::ucan_utils::check_capability(
    peer_token.parsed(),
    &add_folder_resource,  // Wrong resource
    "use",                 // Wrong ability
)?;
```

---

#### validate_folder_access_for_resource()

```rust
/// Validate peer's folder UCAN contains resource's folder_id
pub async fn validate_folder_access_for_resource(
    folder_ucan: &str,
    resource_folder_id: &str,
    domain: &str,
) -> ServiceResult<()>
```

**Usage:** Ensure peer can access folder before sending resource

---

#### validate_ucan_structure()

```rust
/// Generic UCAN validation (signature, expiration, structure)
pub async fn validate_ucan_structure(ucan_token: &str) -> ServiceResult<()>
```

---

### Utility Functions

#### extract_resource_id()

```rust
/// Extract resource ID from resource UCAN capabilities
pub async fn extract_resource_id(ucan_token: &str) -> ServiceResult<String>
```

**Pattern:** Parses capabilities like `sthalam:resource:abc:doc_name` → `abc`

---

#### extract_folder_id_with_add_resources()

```rust
/// Extract folder_id and validate add_resources capability
pub async fn extract_folder_id_with_add_resources(
    ucan: &str,
    domain: &str,
) -> ServiceResult<String>
```

**Usage:** Validate owner has permission to add resources to folder

---

#### extract_facts()

```rust
/// Extract raw UCAN facts JSON
pub async fn extract_facts(ucan: &str) -> ServiceResult<Option<serde_json::Value>>
```

---

## folder_service.rs

**Purpose:** Folder lifecycle management (create, share, accept, delete)

**Lines:** 430

**Status:** ✅ **Working**

### create_folder()

```rust
pub async fn create_folder(
    name: String,
    description: Option<String>,
    folder_template_json: String,  // From frontend
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: &Arc<RwLock<CryptoUtils>>,
    domain: &str,
    user: &User,
) -> ServiceResult<Folder>
```

**Flow:**
1. Validate input (non-empty name)
2. Create folder with temporary empty UCAN
3. Generate owner UCAN using frontend template
4. Update folder with generated UCAN
5. Create folder share record (owner is first recipient)
6. Save folder + share record atomically

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
    domain: &str,
) -> ServiceResult<()>
```

**Flow:**
1. Validate folder exists
2. Validate recipient exists
3. Check if already shared (prevent duplicates)
4. Get current user's folder share record (contains their folder UCAN)
5. Generate delegated folder UCAN for recipient (extracts template)
6. Create folder share record
7. Share all resources in folder with same role

**UCAN Delegation:**
```rust
let (folder_ucan_token, folder_ucan_cid) = crate::ucan_service::issue_delegated_folder_token(
    &current_user_folder_share.ucan_token,  // Delegator's folder token
    &recipient_user.ucan_pub_key,           // Recipient's pub key
    folder_id,
    recipient_role,  // "node" or "viewer"
    domain,
    repo_ctx.clone(),
    crypto_utils.clone(),
).await?;
```

---

### accept_folder_from_peer()

```rust
pub async fn accept_folder_from_peer(
    folder: &Folder,
    folder_share_record: &FolderShareRecord,
    peer_connection_token: &str,  // Peer's ConnectionToken
    domain: &str,
    repo_ctx: Arc<RepositoryContext>,
) -> ServiceResult<()>
```

**Flow:**
1. Parse peer connection token
2. Validate peer has `add_folder` capability (UCAN-first)
3. Validate folder share UCAN structure
4. Save folder + share record atomically

**Capability Validation (UCAN-first):**
```rust
let peer_token = ConnectionToken::from_token(peer_connection_token)
    .map_err(|e| FolderServiceError::Validation(format!("Invalid peer connection token: {}", e)))?;

crate::ucan_service::validation::validate_peer_can_add_folder(&peer_token, domain).await?;
```

---

### get_responder_folder_ucan_for_folder()

```rust
pub async fn get_responder_folder_ucan_for_folder(
    initiator_folder_ucan: &str,
    local_user_id: &str,
    domain: &str,
    repo_ctx: Arc<RepositoryContext>,
) -> ServiceResult<String>
```

**Purpose:** Resource sync protocol - responder looks up their folder UCAN when receiving sync request

**Flow:**
1. Extract folder_id from initiator's folder UCAN
2. Get responder's folder share record (contains their folder UCAN)
3. Return responder's folder UCAN

---

### prepare_viewer_folder_data()

```rust
pub async fn prepare_viewer_folder_data(
    folder_id: &str,
    viewer_user_id: &str,
    node_user_id: &str,
    viewer_ucan_pub_key: &str,
    crypto_utils: &Arc<RwLock<CryptoUtils>>,
    repo_ctx: &Arc<RepositoryContext>,
    domain: &str,
) -> ServiceResult<()>
```

**Purpose:** Prepare folder for viewer's first connection (issue FolderViewer token, save share record)

**Flow:**
1. Load folder from database
2. Get node's folder share token (for delegation)
3. Delegate FolderViewer token from node's token
4. Create and save FolderShareRecord (so viewer can reconnect)

---

## resource_service.rs

**Purpose:** Resource lifecycle management and sync operations

**Lines:** 1255

**Status:** ✅ **Working** (folder publishing complete)

### CRUD Operations

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
2. Generate owner UCAN using frontend template
3. Create resource share record (owner is first recipient)
4. Save resource + share record atomically

---

#### get_resource_by_id_direct()

```rust
pub async fn get_resource_by_id_direct(
    resource_id: &str,
    user_id: &str,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: &Arc<RwLock<CryptoUtils>>,
) -> ServiceResult<Resource>
```

**Flow:**
1. Load encrypted resource from database
2. Get user's public key
3. Decrypt resource data
4. Parse as Resource

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
2. Encrypt resource data (key rotation for forward secrecy)
3. Save to database

---

#### share_resource()

```rust
pub async fn share_resource(
    resource_id: &str,
    recipient_user_id: &str,
    recipient_role: &str,  // "node", "user", or "viewer"
    current_user: &User,
    domain: &str,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: &Arc<RwLock<CryptoUtils>>,
) -> ServiceResult<()>
```

**Flow:**
1. Validate resource exists
2. Validate recipient exists
3. Check if already shared
4. Get current user's resource share record (contains their resource UCAN)
5. Generate delegated resource UCAN for recipient (extracts template)
6. Create resource share record
7. Save share record

**UCAN Delegation (type-safe):**
```rust
// Parse owner token (type-safe)
let owner_token = ResourceOwnerToken::from_token(&current_user_share.ucan_token)?;

// Delegate to node (extracts node template from owner UCAN)
let node_token = crate::ucan_service::resource_tokens::delegate_to_node(
    &owner_token,
    recipient_user_id,
    repo_ctx.clone(),
    crypto_utils.clone(),
).await?;

// node_token is ResourceShareToken (compile-time type safety)
```

---

### Sync Operations

#### prepare_resource_transfer()

```rust
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

**Purpose:** Prepare resource for transfer to peer (UCAN-first: validates, filters, encrypts)

**Flow:**
1. Validate folder access (peer's folder UCAN contains resource's folder_id)
2. Get peer's resource share record (contains their resource UCAN)
3. Load and decrypt resource
4. Filter documents based on peer's UCAN capabilities
5. Re-encrypt for peer using their public key
6. Return encrypted resource

**Document Filtering (UCAN-driven):**
```rust
// Parse peer's resource UCAN
let peer_resource_ucan = ResourceUcan::from_token(&peer_share.ucan_token)?;

// Filter documents based on capabilities
let filtered_docs: HashMap<String, Vec<u8>> = resource.documents.iter()
    .filter_map(|(doc_name, doc_bytes)| {
        // Check if peer has capability for this document
        if peer_resource_ucan.has_capability(doc_name) {
            Some((doc_name.clone(), doc_bytes.clone()))
        } else {
            tracing::info!("  ⊘ Filtered out {} (peer lacks capability)", doc_name);
            None
        }
    })
    .collect();
```

---

#### accept_resource_from_peer()

```rust
pub async fn accept_resource_from_peer(
    resource: &EncryptedResource,
    share_records: &[ShareRecord],
    owner_folder_ucan: &str,
    domain: &str,
    repo_ctx: Arc<RepositoryContext>,
) -> ServiceResult<()>
```

**Purpose:** Accept and save a resource from peer (UCAN-first validation)

**Flow:**
1. Extract folder_id from owner's folder UCAN
2. Verify owner has `add_resources` capability in folder UCAN
3. Validate resource UCAN structure
4. Save resource to database
5. Save ALL share records (enables node to forward to viewers)

**Capability Validation:**
```rust
// Extract folder_id from owner's folder UCAN
let folder_id = crate::ucan_service::extract_folder_id_with_add_resources(
    owner_folder_ucan,
    domain
)
.await?;

// This validates owner has add_resources capability
```

---

#### get_resource_ucans_for_sync()

```rust
pub async fn get_resource_ucans_for_sync(
    resource_id: &str,
    current_user_id: &str,
    peer_user_id: &str,
    repo_ctx: Arc<RepositoryContext>,
) -> ServiceResult<(String, String, String)>  // (our_ucan, peer_ucan, cid)
```

**Purpose:** Get UCANs for resource sync protocol

**Returns:**
- Our resource UCAN (from our share record)
- Peer's resource UCAN (from their share record)
- CID (for proof chain)

---

### Helper Functions

#### get_resource_share_records_for_folder()

```rust
pub async fn get_resource_share_records_for_folder(
    folder_id: &str,
    user_id: &str,
    repo_ctx: Arc<RepositoryContext>,
) -> ServiceResult<Vec<ShareRecord>>
```

**Purpose:** Get all resources shared with a user in a folder

---

#### get_all_share_records_for_resource()

```rust
pub async fn get_all_share_records_for_resource(
    resource_id: &str,
    repo_ctx: Arc<RepositoryContext>,
) -> ServiceResult<Vec<ShareRecord>>
```

**Purpose:** Get ALL share records for a resource (for viewer forwarding)

---

## merge_service.rs

**Purpose:** CRDT merge operations (Loro document manipulation)

**Lines:** 602

**Status:** ✅ **Working** (used for resource filtering during folder publishing)

### Key Functions

#### filter_documents_to_send()

```rust
/// Filter resource documents based on peer's UCAN capabilities
pub async fn filter_documents_to_send(
    resource: &Resource,
    peer_ucan: &str,
) -> ServiceResult<HashMap<String, Vec<u8>>>
```

**Flow:**
1. Parse peer's resource UCAN
2. Iterate over resource documents
3. Check if peer has capability for each document
4. Return filtered documents

**Usage:** Used by `prepare_resource_transfer()` to filter documents before re-encryption

---

## Error Handling

**File:** `services/src/errors.rs`

### Service Error Types

```rust
pub enum UcanServiceError {
    MissingTemplate(String),
    InvalidTokenType(String),
    ValidationFailed(String),
    CryptoError(String),
    DatabaseError(String),
}

pub enum FolderServiceError {
    FolderNotFound { folder_id: String },
    EmptyFolderName,
    Validation(String),
    UcanError(String),
    DatabaseError(String),
}

pub enum ResourceServiceError {
    ResourceNotFound { resource_id: String },
    Validation(String),
    UcanError(String),
    EncryptionError(String),
    DatabaseError(String),
}
```

### Error Conversion

All service errors convert to generic `ServiceResult<T>`:

```rust
pub type ServiceResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;
```

---

## Summary

**Service Layer Principles:**
- ✅ UCAN-first (all permission checks use UCAN capabilities)
- ✅ Type-safe (uses typed tokens from ucan_service)
- ✅ Transactional (database operations are atomic)
- ✅ Data-driven (no hardcoded document names or capabilities)
- ✅ Frontend is source of truth (templates extracted from delegator UCANs)

**Current Implementation:**
- ✅ ucan_service - Token generation with template extraction (756 lines)
- ✅ folder_service - Folder lifecycle and sharing (430 lines)
- ✅ resource_service - Resource lifecycle and sync (1255 lines)
- ✅ merge_service - CRDT operations and filtering (602 lines)

**What Works:**
- ✅ Connection token generation (Owner, Node, User, Viewer)
- ✅ Folder token generation and delegation
- ✅ Resource token generation and delegation
- ✅ Folder publishing (create, share, accept from peer)
- ✅ Resource publishing (create, share, prepare transfer, accept from peer)
- ✅ UCAN validation (capability checks, structure validation)

**Next Steps:**
- ⏳ State vector generation (for bidirectional sync)
- ⏳ CRDT update generation and application
- ⏳ Submission isolation (viewer namespaces)
- ⏳ Asset ID extraction and transfer

**Status:** ✅ **Folder Publishing Working**
