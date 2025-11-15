# UCAN Authorization Architecture - Complete Guide

**Last Updated:** 2025-11-16
**Status:** Production Ready (Complete with Gurkha Migration)
**Comprehensive Scope:** Architecture principles, implementation status, design decisions, and code cross-references

---

## Executive Summary

Osvauld uses a **three-tier UCAN token architecture** for authorization with **11 typed token wrappers** organized in the standalone `gurkha` library:

### Core Achievements ✅
- **Standalone gurkha library** - Pure domain logic with zero infrastructure dependencies
- **Data-driven design** - Frontend (permissions.ts) is source of truth, backend executes as data
- **Type safety** - 11 typed token wrappers prevent wrong token usage at compile time
- **Zero hardcoded templates** - All delegation templates extracted from tokens, never hardcoded
- **Self-documenting code** - Explicit URI parsing, clear naming (GenericUcan not ResourceUcan)
- **Pure functions** - Separation of decision logic from crypto operations
- **Thread-safe** - Proper RwLock management for async contexts

### Three-Tier Architecture
1. **Connection Tokens** - Device-to-device handshakes (OneTime, Owner, Node, User, ViewerAuth)
2. **Folder Tokens** - Folder access control (Owner, Share, Viewer)
3. **Resource Tokens** - Resource/document access control (Owner, Share, Viewer)

---

## Gurkha Library Structure

### Module Organization (gurkha/src/)

**Code Location:** `/home/abe/osvauld/gurkha/src/`

```
gurkha/src/
├── lib.rs          # Public API & re-exports
├── types.rs        # Domain types & enums
├── parser.rs       # GenericUcan domain model & DelegationTemplate
├── token.rs        # 11 typed token wrappers + trait implementations
├── uri.rs          # URI building & parsing (explicit format extraction)
├── crypto.rs       # UCAN signing & verification (crypto operations)
├── decision.rs     # Token generation & delegation logic (pure logic)
├── extractors.rs   # ID extraction & capability validation
├── service.rs      # UcanService - high-level public API
└── errors.rs       # GurkhaError types
```

### Design Principles

**From `gurkha/src/lib.rs`:**
- **Pure domain logic** - No database, no infrastructure dependencies
- **Raw key input** - Receives Ed25519 SigningKey/VerifyingKey directly
- **OCaml-ready** - Can be replaced with OCaml implementation
- **Type-safe** - 11 typed token wrappers prevent misuse
- **Separation of concerns** - Decision logic separate from crypto operations

### Public API Surface

**File: `gurkha/src/lib.rs`**
```rust
pub mod types;       // Domain types
pub mod uri;         // URI parsing
pub mod token;       // Typed wrappers
pub mod parser;      // GenericUcan, DelegationTemplate
pub mod decision;    // Pure logic (no crypto)
pub mod crypto;      // Crypto operations
pub mod extractors;  // Extraction utilities
pub mod service;     // Public API
pub mod errors;      // Error types

// Re-exports
pub use types::*;
pub use token::*;
pub use parser::{GenericUcan, DelegationTemplate};
pub use decision::{TokenDecision, DelegationDecision, SyncContext,
                   should_send_updates, can_receive_updates};
pub use service::UcanService;
```

---

## UcanService Architecture

### Service Structure

**Code Location:** `gurkha/src/service.rs`

```rust
pub struct UcanService {
    signing_key: Option<SigningKey>,
    verifying_key: Option<VerifyingKey>,
}

impl UcanService {
    /// Create new service (keys not loaded yet)
    pub fn new() -> Self {
        Self {
            signing_key: None,
            verifying_key: None,
        }
    }

    /// Load keys after login (typically during auth flow)
    pub fn load_keys(&mut self, signing_key: SigningKey, verifying_key: VerifyingKey) {
        self.signing_key = Some(signing_key);
        self.verifying_key = Some(verifying_key);
    }

    /// Private validation - ensures keys loaded before any operation
    fn get_keys(&self) -> ServiceResult<(&SigningKey, &VerifyingKey)> {
        match (&self.signing_key, &self.verifying_key) {
            (Some(sk), Some(vk)) => Ok((sk, vk)),
            _ => Err(ServiceError::KeysNotLoaded),
        }
    }

    // Public API methods...
    pub async fn issue_folder_owner_token(&self, ...) -> ServiceResult<T>;
    pub async fn delegate_folder_to_node(&self, ...) -> ServiceResult<T>;
    // ... etc
}
```

### Thread-Safe Wrapping

**Pattern used throughout:**
```rust
Arc<RwLock<gurkha::UcanService>>
```

**IMPORTANT:** Use `tokio::sync::RwLock`, NOT `std::sync::RwLock`

**Reason:** Tauri commands and async contexts require types to be `Send + Sync`. The `std::sync::RwLockReadGuard` is NOT `Send`, causing compilation errors.

**Example Error (if using std::sync::RwLock):**
```
error[E0277]: `std::sync::RwLockReadGuard<'_, UcanService>` cannot be sent
              between threads safely
```

**Correct Import:**
```rust
use tokio::sync::RwLock;  // ✅ Correct
// NOT: use std::sync::RwLock;  // ❌ Wrong in async contexts
```

---

## Integration Patterns

### 1. Application Initialization

**sthalam (Tauri App):**
```rust
// sthalam/src-tauri/src/lib.rs
use tokio::sync::RwLock;  // IMPORTANT: tokio, not std!

let crypto_utils = Arc::new(RwLock::new(CryptoUtils::new()));
let ucan_service = Arc::new(RwLock::new(gurkha::UcanService::new()));

// Pass to P2P service
let (p2p_service, p2p_receiver) = P2PService::new(
    repo_ctx.clone(),
    crypto_utils.clone(),
    ucan_service.clone(),  // Add this
    domain,
);

// Manage as Tauri state
app.manage(crypto_utils);
app.manage(ucan_service);
```

**kunki (CLI App):**
```rust
// kunki/src/main.rs
let crypto_utils = Arc::new(RwLock::new(CryptoUtils::new()));
let ucan_service = Arc::new(RwLock::new(gurkha::UcanService::new()));

// Pass to service functions
handle_start(&pass, repo_ctx, crypto_utils, ucan_service, domain).await?;
```

### 2. Key Loading (Login Flow)

```rust
// In auth/login service
pub async fn login(
    passphrase: &str,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: &Arc<RwLock<CryptoUtils>>,
    ucan_service: &Arc<RwLock<gurkha::UcanService>>,
) -> ServiceResult<User> {
    // Load certificate and decrypt keys
    let (user, device) = load_certificate(passphrase, repo_ctx, crypto_utils).await?;

    // Load UCAN keys into service
    let encrypted_ucan_key = repo_ctx.store_repo.get_ucan_key().await?;
    let (signing_key, verifying_key) = {
        let crypto = crypto_utils.read().await;
        crypto.decrypt_ucan_key(&encrypted_ucan_key)?
    };

    // Load keys into UcanService
    {
        let mut ucan_guard = ucan_service.write().await;
        ucan_guard.load_keys(signing_key, verifying_key);
    }

    Ok(user)
}
```

### 3. Tauri Command Handlers

```rust
#[tauri::command]
pub async fn handle_add_folder(
    input: AddFolderInput,
    config: State<'_, HandlerConfig>,
    repo_ctx: State<'_, Arc<RepositoryContext>>,
    crypto_utils: State<'_, Arc<RwLock<CryptoUtils>>>,
    ucan_service: State<'_, Arc<RwLock<gurkha::UcanService>>>,  // Add this
    user_state: State<'_, UserState>,
) -> Result<BaseCryptoResponse, String> {
    let user = user_state.get_user().await?;

    let folder = create_folder(
        input.name,
        Some(input.description),
        input.folder_template_json,
        repo_ctx.inner().clone(),
        &crypto_utils,
        &config.domain,
        &user,
        &ucan_service,  // Pass it
    )
    .await
    .map_err(|e| e.to_string())?;

    Ok(BaseCryptoResponse::FolderCreated(folder))
}
```

### 4. Service Layer Functions

```rust
pub async fn create_folder(
    name: String,
    description: Option<String>,
    folder_template_json: String,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: &Arc<RwLock<CryptoUtils>>,
    domain: &str,
    user: &User,
    ucan_service: &Arc<RwLock<gurkha::UcanService>>,  // Add this parameter
) -> ServiceResult<Folder> {
    let mut folder = Folder::new(name, description, false, String::new());

    // Lock, use, and release
    let ucan_service_guard = ucan_service.read().await;
    let (folder_root_ucan_key, ucan_cid) = ucan_service_guard
        .issue_folder_owner_token(&folder.id, domain, &folder_template_json)
        .await?;
    drop(ucan_service_guard);  // Explicit drop (optional but clear)

    folder.ucan = folder_root_ucan_key.clone();
    // ... rest of function

    Ok(folder)
}
```

### 5. P2P Message Handlers

```rust
pub async fn handle_resource_data_sync(
    payload: &ResourceDataSync,
    peer_conn: Arc<PeerConnection>,
    repo_ctx: Arc<RepositoryContext>,
    _crypto_utils: Arc<RwLock<CryptoUtils>>,
) -> P2PResult<()> {
    let domain = &peer_conn.domain;

    services::accept_resource_from_peer(
        &payload.resource,
        &payload.share_records,
        &payload.folder_ucan,
        domain,
        repo_ctx,
        &peer_conn.ucan_service,  // Use from peer_conn
    )
    .await
    .map_err(|e| {
        error!("Failed to accept resource from peer: {}", e);
        P2PError::InvalidState(format!("Failed to accept resource: {}", e))
    })?;

    Ok(())
}
```

---

## Core Principles & Design Philosophy

### Principle 1: Frontend is Source of Truth

**Philosophy**: Authorization semantics belong in the frontend where business decisions are made

**Manifestation**:
- **Where**: `sthalam/frontend/desktop/src/config/permissions.ts`
- **What**: Defines token types, capabilities, delegation templates, and sync rules
- **Implementation**: Backend extracts templates from tokens and executes them as data

**Code Pattern** (gurkha/src/decision.rs):
```rust
let mut facts = template.to_facts();  // token_type comes from template!
facts.insert("role".to_string(), json!(peer_role));
```

### Principle 2: Data-Driven Backend

**Philosophy**: Backend code is generic, derives behavior from token content

**Manifestation**:
- No hardcoded document names
- No hardcoded capability mappings
- No hardcoded token types
- All data flows from frontend templates

**Example**:
- Add a new document in permissions.ts → System automatically handles it
- No backend code changes required
- Deployment only needed for core logic fixes

### Principle 3: Type Safety at Compile Time

**Philosophy**: Wrong token type = compiler error, not runtime bug

**Code Example** (gurkha/src/service.rs):
```rust
// Can't pass wrong token type - compiler error!
pub async fn delegate_to_node(
    owner_token: &FolderOwnerToken,  // Must be FolderOwnerToken
    ...
) -> ServiceResult<FolderShareToken>
```

### Principle 4: Explicit Over Implicit

**Philosophy**: Code should clearly show what it's doing

**URI Parsing** (gurkha/src/uri.rs):
```rust
// ✅ Explicit per-format extraction
match parts.get(1) {
    Some(&"folder") => {
        // Folder format: domain:folder:id:operation
        if let (Some(folder_id), Some(operation)) = (parts.get(2), parts.get(3)) {
            // Clear naming shows structure
        }
    }
}
```

---

## Token Type Hierarchy

### Connection Tokens (Device Handshake)

**Purpose:** Authenticate devices and establish P2P connections

**Code Location**: `gurkha/src/token.rs`

```rust
pub enum ConnectionTokenType {
    OneTimeConnection,   // Initial pairing (single-use)
    OwnerConnection,     // Owner device sync (long-lived)
    NodeConnection,      // Node device sync (long-lived)
    UserConnection,      // User P2P collaboration (long-lived)
    ViewerAuth,          // Viewer initial auth (short-lived)
}
```

**Typed Wrappers**:
```rust
pub struct OneTimeConnectionToken(ConnectionToken);
pub struct OwnerConnectionToken(ConnectionToken);
pub struct NodeConnectionToken(ConnectionToken);
pub struct UserConnectionToken(ConnectionToken);
pub struct ViewerAuthToken(ConnectionToken);
```

### Folder Tokens (Folder Access Control)

**Purpose:** Control access to folders and their contents

**Code Location**: `gurkha/src/token.rs`

```rust
pub struct FolderOwnerToken {
    ucan: GenericUcan,
}

pub struct FolderShareToken {
    ucan: GenericUcan,
}

pub struct FolderViewerToken {
    ucan: GenericUcan,
}
```

**Delegation Chain**:
```
FolderOwnerToken (Owner)
  ↓ delegate_folder_to_node()
FolderShareToken (Node)
  ↓ delegate_folder_to_user()
FolderShareToken (User)
  ↓ delegate_folder_to_viewer()
FolderViewerToken (Viewer)
```

### Resource Tokens (Resource Access Control)

**Purpose:** Control access to individual resources and their documents

**Code Location**: `gurkha/src/token.rs`

```rust
pub struct ResourceOwnerToken {
    ucan: GenericUcan,
}

pub struct ResourceShareToken {
    ucan: GenericUcan,
}

pub struct ResourceViewerToken {
    ucan: GenericUcan,
}
```

**Delegation Chain**:
```
ResourceOwnerToken (Owner)
  ↓ delegate_resource_to_node()
ResourceShareToken (Node)
  ↓ delegate_resource_to_user()
ResourceShareToken (User)
  ↓ delegate_resource_to_viewer()
ResourceViewerToken (Viewer)
```

---

## Domain Models

### GenericUcan (Universal UCAN Parser)

**Code Location**: `gurkha/src/parser.rs`

```rust
pub struct GenericUcan {
    raw_token: String,
    parsed: Ucan,
    role: Role,
    token_type: ResourceTokenType,
    capabilities: HashMap<String, Capability>,
    doc_metadata: HashMap<String, DocMetadata>,
    sync_facts: SyncFacts,
    delegation_templates: HashMap<String, DelegationTemplate>,
    proof_chain: Vec<String>,
}

impl GenericUcan {
    // Parsing
    pub fn from_token(token: &str) -> UcanTokenResult<Self>;

    // Capability queries
    pub fn has_capability(&self, doc: &str) -> bool;
    pub fn get_capability(&self, doc: &str) -> Option<Capability>;

    // Sync behavior queries
    pub fn is_local_only(&self, doc: &str) -> bool;
    pub fn has_no_incoming_updates(&self, doc: &str) -> bool;
    pub fn should_send_full_snapshot(&self, doc: &str) -> bool;

    // Document type queries
    pub fn get_doc_type(&self, doc: &str) -> Option<DocType>;
    pub fn supports_merge(&self, doc: &str) -> bool;

    // Template access (for delegation)
    pub fn get_delegation_template(&self, role: &str) -> Option<&DelegationTemplate>;

    // Resource/Folder ID extraction
    pub fn resource_id(&self) -> Option<String>;
    pub fn folder_id(&self) -> Option<String>;
}
```

### DelegationTemplate

**Code Location**: `gurkha/src/parser.rs`

```rust
pub struct DelegationTemplate {
    pub token_type: String,                      // Data-driven token type
    pub capabilities: HashMap<String, String>,   // doc_name -> capability
    pub sync: Option<SyncFacts>,
    pub doc_metadata: HashMap<String, DocMetadata>,
}

impl DelegationTemplate {
    pub fn build_capabilities(&self, id: &str, resource_type: &str) -> Vec<(String, String)>;
    pub fn to_facts(&self) -> serde_json::Map<String, serde_json::Value>;
}
```

### Domain Types

**Code Location**: `gurkha/src/types.rs`

```rust
pub enum Capability {
    Collaborator,  // Bidirectional CRDT sync
    Viewer,        // Receive-only
    Submitter,     // Send full snapshots
}

pub enum Role {
    Owner,   // Resource creator
    Node,    // Sovereign node
    User,    // Peer user
    Viewer,  // Limited access
}

pub enum DocType {
    Crdt,   // CRDT document (Loro)
    Asset,  // Binary asset (blob)
}

pub struct SyncFacts {
    pub local_only: Vec<String>,
    pub no_incoming_updates: Vec<String>,
    pub send_full_snapshot: Vec<String>,
}

pub struct DocMetadata {
    pub name: String,
    pub doc_type: DocType,
    pub allowed_mimes: Option<Vec<String>>,
    pub max_size_mb: Option<u64>,
}
```

---

## Capability Format

### Capability Resource Pattern

**From**: `sthalam/frontend/desktop/src/config/permissions.ts`

**Format:** `{domain}:{resource_type}:{id}:{component}`

**Examples**:
```
Connection capabilities:
- sthalam:user:*:connect      (can connect as user)
- sthalam:user:*:share         (can share with users)

Folder capabilities:
- sthalam:folder:xyz:own               (owns folder)
- sthalam:folder:xyz:add_resources     (can add resources to folder)
- sthalam:folder:xyz:share_folder      (can share folder)
- sthalam:folder:xyz:get_share_link    (can get share link)

Resource capabilities:
- sthalam:resource:abc:share_resource  (can share this resource)
- sthalam:resource:abc:collaborative_doc:collaborator  (full access)
- sthalam:resource:abc:content_doc:viewer              (read-only)
- sthalam:resource:abc:submissions_doc:submitter       (submit-only)
```

### Validation Pattern

**Code Location**: `gurkha/src/extractors.rs`

```rust
// Extract folder ID from folder token
let folder_id = gurkha::extractors::extract_id_from_resource_type(
    ucan.parsed(),
    domain,
    "folder",
)?;

// Validate folder token has add_resources capability
gurkha::extractors::validate_has_capability(
    ucan.parsed(),
    domain,
    "folder",
    "add_resources",
)?;

// Extract resource ID and validate share capability
let resource_id = gurkha::extractors::extract_id_from_resource_type(
    ucan.parsed(),
    domain,
    "resource",
)?;

gurkha::extractors::validate_has_capability(
    ucan.parsed(),
    domain,
    "resource",
    "share_resource",
)?;
```

---

## Template Extraction Pattern

### Delegation Flow (NEVER HARDCODE!)

**Code Location**: `gurkha/src/service.rs`

**Pattern used in all delegation functions**:

```rust
// 1. Parse owner token (validates type)
let owner_token = FolderOwnerToken::from_token(&owner_folder_ucan)?;

// 2. Extract template from owner's UCAN (DATA-DRIVEN)
let template = owner_token.ucan().get_delegation_template("node")
    .ok_or_else(|| ServiceError::MissingTemplate("node"))?;

// 3. Build capabilities (generic, no hardcoding)
let capabilities = template.build_capabilities(&folder_id, "folder");

// 4. Create facts (includes token_type from template)
let mut facts = template.to_facts();
facts.insert("role".to_string(), json!("node"));

// 5. Get delegatee's public key
let delegatee_pub_key = /* get from database */;

// 6. Generate UCAN token (using internal crypto)
let node_ucan = self.generate_ucan_internal(
    &delegatee_pub_key,
    capabilities,
    facts,
    None,  // No expiration
)?;

// 7. Return typed wrapper
Ok(FolderShareToken::from_token(&node_ucan)?)
```

**Key Points**:
- ✅ Template extracted from delegator's UCAN (frontend-defined)
- ✅ No hardcoded document names or capabilities
- ✅ Generic code works with any document structure
- ✅ Frontend changes don't require backend code changes

---

## Frontend Integration

### Permission Templates (permissions.ts)

**Code Location**: `sthalam/frontend/desktop/src/config/permissions.ts`

```typescript
export const RESOURCE_TEMPLATE = {
  owner_template: {
    capabilities: {
      "share_resource": "allow",
      "template_doc": "collaborator",
      "content_doc": "collaborator",
      "collaborative_doc": "collaborator",
      "submissions_doc": "collaborator",
      "static_assets": "collaborator",
    },
    doc_types: {
      "static_assets": "asset",
      "template_doc": "crdt",
      // ... rest
    },
    sync: {
      local_only: ["user_content_doc"],
    },
    delegation: {
      node: {
        token_type: "resource_share",
        capabilities: {
          "share_resource": "allow",
          "template_doc": "collaborator",
          // ...
        },
        sync: { local_only: ["user_content_doc"] },
      },
      viewer: {
        token_type: "resource_viewer",
        capabilities: { /* viewer template */ },
        sync: {
          local_only: ["user_content_doc"],
          no_incoming_updates: ["submissions_doc"],
          send_full_snapshot: ["submissions_doc"],
        },
      },
    },
  },
};
```

**Backend extracts this structure verbatim from owner UCAN and uses it for delegation.**

---

## Common Pitfalls & Solutions

### ❌ Problem: Threading Error

```
error[E0277]: `std::sync::RwLockReadGuard<'_, UcanService>` cannot be sent
              between threads safely
```

**Cause:** Using `std::sync::RwLock` instead of `tokio::sync::RwLock`

**Solution:**
```rust
// ❌ WRONG
use std::sync::RwLock;

// ✅ CORRECT
use tokio::sync::RwLock;
```

### ❌ Problem: Keys Not Loaded Error

```
ServiceError::KeysNotLoaded
```

**Cause:** Trying to use UcanService before calling `load_keys()`

**Solution:** Ensure keys are loaded during login:
```rust
// In auth/login flow
let (signing_key, verifying_key) = crypto.decrypt_ucan_key(&certificate.private_key)?;
let mut ucan_guard = ucan_service.write().await;
ucan_guard.load_keys(signing_key, verifying_key);
```

### ❌ Problem: Missing Parameter in Function Call

```
error[E0061]: this function takes 8 arguments but 7 arguments were supplied
```

**Cause:** Forgot to add `&ucan_service` parameter after refactoring

**Solution:** Add `&ucan_service` (or `&peer_conn.ucan_service` in P2P context)

---

## Architecture Diagram

```
┌─────────────────────────────────────────────────────────────┐
│ FRONTEND: Source of Truth                                   │
│ permissions.ts                                              │
│ • Token types (resource_share, folder_owner, etc)          │
│ • Capabilities per role (collaborator, viewer, submitter)   │
│ • Sync behavior (local_only, no_incoming_updates)          │
└─────────────────────────────────────────────────────────────┘
                        ↓
              [Token Generation]
                        ↓
┌─────────────────────────────────────────────────────────────┐
│ GURKHA: Pure Domain Logic                                   │
│ gurkha/src/                                                  │
│ • decision.rs - Token generation logic (pure)               │
│ • crypto.rs - UCAN signing/verification                     │
│ • service.rs - Public API orchestration                     │
│ • extractors.rs - ID extraction & capability validation     │
│ • No infrastructure dependencies                            │
└─────────────────────────────────────────────────────────────┘
                        ↓
┌─────────────────────────────────────────────────────────────┐
│ SERVICES LAYER: Business Logic                              │
│ services/src/                                                │
│ • Accept &Arc<RwLock<UcanService>>                          │
│ • Coordinate between repo, crypto, and UCAN                 │
│ • No business logic - just orchestration                    │
└─────────────────────────────────────────────────────────────┘
                        ↓
┌─────────────────────────────────────────────────────────────┐
│ TOKEN VALIDATION: Type-Safe Wrappers                        │
│ gurkha/src/token.rs, parser.rs                               │
│ • Parse token with GenericUcan                             │
│ • Type-safe wrappers (ResourceOwnerToken, etc)             │
│ • Trait-based queries (get_capability, is_local_only, etc) │
└─────────────────────────────────────────────────────────────┘
```

---

## Security Properties

### Type Safety
- ✅ Can't pass `ConnectionToken` where `ResourceToken` expected (compile-time)
- ✅ Can't pass `ViewerToken` to function expecting `OwnerToken` (compile-time)
- ✅ Wrong token type = compiler error, not runtime bug

### Authorization Enforcement
- ✅ Capabilities validated via trait methods
- ✅ Sync behavior enforced via SyncFacts checking
- ✅ Role-based access control via typed tokens

### Audit Trail
- ✅ All delegated tokens include proof chain (parent UCAN CIDs)
- ✅ Frontend source of truth makes authorization decisions traceable
- ✅ Template-based delegation is verifiable and reproducible

---

## Implementation Status

**Migration Complete:**
- ✅ Gurkha library created with pure domain logic
- ✅ UcanService with optional key loading
- ✅ Thread-safe integration across all layers
- ✅ Zero infrastructure dependencies in gurkha
- ✅ All services updated to use Arc<RwLock<UcanService>>

**Build Status:**
- ✅ All crates compile successfully
- ✅ No type errors in UCAN module
- ✅ All services layer integrated
- ✅ P2P layer updated

**Testing:**
- ✅ Folder sync with UCAN-first publishing
- ✅ Resources delegated with correct token types
- ✅ All permission validations working
- ✅ End-to-end sync flows verified

---

## References

**Core Implementation Files:**
- `/home/abe/osvauld/gurkha/src/` - UCAN library (lib.rs, types.rs, parser.rs, token.rs, uri.rs, crypto.rs, decision.rs, extractors.rs, service.rs, errors.rs)
- `/home/abe/osvauld/sthalam/frontend/desktop/src/config/permissions.ts` - Frontend templates
- `/home/abe/osvauld/services/src/*` - Service layer using UcanService

**Related Documentation:**
- `SYNC_PROTOCOL.md` - Complete P2P sync protocol specification
- `NETWORK_LAYER.md` - P2P orchestration and message routing
- `SERVICE_LAYER.md` - Business logic layer architecture

---

**Status:** Production Ready
**Owner:** Core Authorization System
**Last Verified:** 2025-11-16
