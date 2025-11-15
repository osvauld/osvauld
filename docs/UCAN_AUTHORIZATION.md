# UCAN Authorization Architecture - Complete Guide

**Last Updated:** 2025-11-15
**Status:** Production Ready (Phases 1-6+ Complete)
**Comprehensive Scope:** Architecture principles, implementation status, design decisions, and code cross-references

---

## Executive Summary

Osvauld uses a **three-tier UCAN token architecture** for authorization with **11 typed token wrappers** organized in a unified module:

### Core Achievements ✅
- **Unified UCAN module** - All code in `core/src/ucan/` with explicit traits
- **Data-driven design** - Frontend (permissions.ts) is source of truth, backend executes as data
- **Type safety** - 11 typed token wrappers prevent wrong token usage at compile time
- **Zero hardcoded templates** - All delegation templates extracted from tokens, never hardcoded
- **Self-documenting code** - Explicit URI parsing, clear naming (GenericUcan not ResourceUcan)
- **Pure functions** - SyncContext with deterministic permission logic

### Three-Tier Architecture
1. **Connection Tokens** - Device-to-device handshakes (OneTime, Owner, Node, User, ViewerAuth)
2. **Folder Tokens** - Folder access control (Owner, Share, Viewer)
3. **Resource Tokens** - Resource/document access control (Owner, Share, Viewer)

---

## Unified UCAN Module Structure

### Module Organization (core/src/ucan/)

**Code Location:** `/home/abe/osvauld/core/src/ucan/`

```
core/src/ucan/
├── mod.rs          # Public API & re-exports (lines 1-47)
├── types.rs        # Domain types & enums
├── parser.rs       # GenericUcan domain model & DelegationTemplate
├── token.rs        # 11 typed token wrappers + trait implementations
└── uri.rs          # URI building & parsing (explicit format extraction)
```

### Public API Surface

**File: `core/src/ucan/mod.rs` (lines 1-47)**
- Re-exports all public types for convenient importing
- Prelude module for `use osvauld_core::ucan::prelude::*`

**Types: `core/src/ucan/types.rs`**
- Domain enums: `Capability`, `Role`, `DocType`, `ResourceAction`
- Token type enums: `ConnectionTokenType`, `ResourceTokenType`
- Data structures: `SyncFacts`, `SyncDecision`, `DocMetadata`

**Parser: `core/src/ucan/parser.rs`**
- `GenericUcan` - Universal UCAN parser for any token type
- `DelegationTemplate` - Encodes delegation rules and token types
- `UcanTokenError`, `UcanTokenResult` - Error handling

**Tokens: `core/src/ucan/token.rs`**
- **Traits**: `UcanToken`, `HasId`, `CanDelegate`, `ResourceOps`, `FolderOps`
- **Connection tokens** (5 types):
  - `OneTimeConnectionToken` ✓ single-use handshake
  - `OwnerConnectionToken` ✓ owner device sync
  - `NodeConnectionToken` ✓ node device sync
  - `UserConnectionToken` ✓ P2P user collaboration
  - `ViewerAuthToken` ✓ viewer link authentication
- **Resource tokens** (3 types):
  - `ResourceOwnerToken` ✓ full resource control
  - `ResourceShareToken` ✓ node/user resource access
  - `ResourceViewerToken` ✓ viewer resource access
- **Folder tokens** (3 types):
  - `FolderOwnerToken` ✓ full folder control
  - `FolderShareToken` ✓ node/user folder access
  - `FolderViewerToken` ✓ viewer folder access

**URI Parsing: `core/src/ucan/uri.rs`**
- `ParsedCapabilityUri` enum with explicit format-specific extraction
- Formats (from `sthalam/frontend/desktop/src/config/permissions.ts`):
  - Folder: `domain:folder:id:operation`
  - Resource: `domain:resource:id:doc_name`
  - User: `domain:user:capability_type:user_id`

---

## Core Principles & Design Philosophy

### Principle 1: Frontend is Source of Truth

**Philosophy**: Authorization semantics belong in the frontend where business decisions are made

**Manifestation**:
- **Where**: `sthalam/frontend/desktop/src/config/permissions.ts`
- **What**: Defines token types, capabilities, delegation templates, and sync rules
- **Implementation**: Backend extracts templates from tokens and executes them as data

**Code Pattern** (services/src/ucan_service.rs:724):
```rust
let mut facts = template.to_facts();  // token_type comes from template!
facts.insert("role".to_string(), json!(peer_role));
```

### Principle 2: Data-Driven Backend

**Philosophy**: Backend code is generic, derives behavior from token content

**Manifestation**:
- No hardcoded document names
- No hardcoded capability mappings
- No hardcoded token types (removed in Phase 5+)
- All data flows from frontend templates

**Example**:
- Add a new document in permissions.ts → System automatically handles it
- No backend code changes required
- Deployment only needed for core logic fixes

### Principle 3: Type Safety at Compile Time

**Philosophy**: Wrong token type = compiler error, not runtime bug

**Code Example** (services/src/ucan_service.rs):
```rust
// Can't pass wrong token type - compiler error!
pub async fn delegate_to_node(
    owner_token: &FolderOwnerToken,  // Must be FolderOwnerToken
    ...
) -> ServiceResult<FolderShareToken>
```

### Principle 4: Explicit Over Implicit

**Philosophy**: Code should clearly show what it's doing

**URI Parsing** (core/src/ucan/uri.rs - Phase 6+ improvement):
```rust
// ✅ After: Explicit per-format extraction
match parts.get(1) {
    Some(&"folder") => {
        // Folder format: domain:folder:id:operation
        if let (Some(folder_id), Some(operation)) = (parts.get(2), parts.get(3)) {
            // Clear naming shows structure
        }
    }
}
```

**Naming** (core/src/models/p2p.rs - Phase 6+ improvement):
```rust
// ✅ After: Name describes content, not usage
pub struct Message {
    pub folder_ucan: String,  // Contains folder capabilities, used by any role
}
```

---

## Token Type Hierarchy

### Connection Tokens (Device Handshake)

**Purpose:** Authenticate devices and establish P2P connections

**Code Location**: `core/src/ucan/token.rs`

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

**Code Location**: `core/src/ucan/token.rs`

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

**Code Location**: `core/src/ucan/token.rs`

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

**Code Location**: `core/src/ucan/parser.rs`

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

**Code Location**: `core/src/ucan/parser.rs`

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

**Code Location**: `core/src/ucan/types.rs`

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
- sthalam:folder:xyz:add_resources  (can add resources to folder)
- sthalam:folder:xyz:share_folder   (can share folder)

Resource capabilities:
- sthalam:resource:abc:collaborative_doc:collaborator  (full access)
- sthalam:resource:abc:content_doc:viewer              (read-only)
- sthalam:resource:abc:submissions_doc:submitter       (submit-only)
```

### Validation Pattern

**Code Location**: `crypto_utils::ucan_utils` (called from services)

```rust
// Check if peer can add folders (services/src/ucan_service.rs)
let folder_resource = format!("{}:folder:*", domain);
crypto_utils::ucan_utils::check_capability(
    peer_token.parsed(),
    &folder_resource,
    "add_folder",
)?;

// Check if user has collaborator access
let doc_resource = format!("{}:resource:{}:collaborative_doc", domain, resource_id);
crypto_utils::ucan_utils::check_capability(
    user_token.parsed(),
    &doc_resource,
    "collaborator",
)?;
```

---

## Template Extraction Pattern

### Delegation Flow (NEVER HARDCODE!)

**Code Location**: `services/src/ucan_service.rs` (lines 680-750)

**Pattern used in all delegation functions**:

```rust
// 1. Extract template from delegator UCAN (NEVER HARDCODE!)
let template = delegator_token.get_delegation_template("viewer")
    .ok_or_else(|| ServiceError::MissingTemplate("viewer"))?;

// 2. Build capabilities (DATA-DRIVEN)
let capabilities = template.build_capabilities(&resource_id, "resource");

// 3. Convert to facts (GENERIC)
let mut facts = template.to_facts();
facts.insert("role".to_string(), json!("viewer"));

// 4. Get delegatee's public key
let delegatee_pub_key = repo_ctx.user_repo.get_user_by_id(delegatee_user_id).await?.ucan_pub_key;

// 5. Generate UCAN token
let ucan_token = crypto_utils.write().await.generate_ucan(
    &delegatee_pub_key,
    capabilities,
    facts,
    None,  // No expiration
)?;

// 6. Return typed wrapper
Ok(ResourceViewerToken::from_token(&ucan_token)?)
```

**Key Points**:
- ✅ Template extracted from delegator's UCAN (frontend-defined)
- ✅ No hardcoded document names or capabilities
- ✅ Generic code works with any document structure
- ✅ Frontend changes don't require backend code changes

---

## Data-Driven Token Type System

### Problem: Hardcoded Token Type Logic

**Before** (services/src/ucan_service.rs - historical):
```rust
// ❌ Hardcoded logic - requires backend code changes for new types
let token_type = match peer_role {
    "node" | "user" => "resource_share",
    "viewer" => "resource_viewer",
    _ => "resource_share",
};
facts.insert("token_type".to_string(), json!(token_type));
```

**Issues**:
1. Not extensible - Adding new token types requires backend code changes
2. Tight coupling - Business logic in technical code
3. Implicit semantics - Relationship between role and token type not declared
4. Duplication - Same logic repeated in multiple functions

### Solution: Data-Driven Token Types

**Architecture**:
- **Frontend** (`sthalam/frontend/desktop/src/config/permissions.ts`) defines token types
- **Backend** (`services/src/ucan_service.rs`) executes templates as data

**Implementation**:

1. **DelegationTemplate** (core/src/ucan/parser.rs):
```rust
pub struct DelegationTemplate {
    pub token_type: String,                      // ← Data-driven!
    pub capabilities: HashMap<String, String>,
    pub sync: Option<SyncFacts>,
}

impl DelegationTemplate {
    pub fn to_facts(&self) -> serde_json::Map<String, serde_json::Value> {
        let mut facts = serde_json::Map::new();
        facts.insert("token_type".to_string(),
                     serde_json::Value::String(self.token_type.clone()));
        // Rest of facts...
    }
}
```

2. **Frontend Templates** (sthalam/frontend/desktop/src/config/permissions.ts):
```typescript
export const RESOURCE_TEMPLATE = {
  owner_template: {
    delegation: {
      node: {
        token_type: "resource_share",        // ← Explicitly defined
        capabilities: { ... }
      },
      viewer: {
        token_type: "resource_viewer",       // ← Different behavior
        capabilities: { ... }
      }
    }
  }
};
```

3. **Backend Service** (services/src/ucan_service.rs:724):
```rust
// ✅ Now: Using template.to_facts() which includes token_type
let mut facts = template.to_facts();  // token_type already included!
facts.insert("role".to_string(), json!(peer_role));
```

### Benefits

1. **Extensibility Without Code Changes**
   - New token type? Update frontend template, done
   - No backend deployment needed

2. **Explicit Authorization Model**
   - What token types exist is declared in frontend
   - Single source of truth

3. **Separation of Concerns**
   - Frontend owns authorization policy
   - Backend executes policy as data

---

## Usage Examples

### Example 1: Folder Publishing (Owner → Node)

**Code Location**: `services/src/ucan_service.rs` (lines 115-195)

```rust
// 1. Parse owner token (validates type)
let owner_token = FolderOwnerToken::from_token(&owner_folder_ucan)?;

// 2. Extract template from owner's UCAN (DATA-DRIVEN)
let template = owner_token.ucan().get_delegation_template("node")?;

// 3. Build capabilities (generic, no hardcoding)
let capabilities = template.build_capabilities(&folder_id, "folder");

// 4. Create facts (includes token_type from template)
let mut facts = template.to_facts();
facts.insert("role".to_string(), json!("node"));

// 5. Generate UCAN
let node_ucan = crypto_utils.write().await.generate_ucan(
    &node_pub_key,
    capabilities,
    facts,
    None,
)?;

// 6. Return typed token
Ok(FolderShareToken::from_token(&node_ucan)?)
```

### Example 2: Resource Delegation (Owner → Node)

**Code Location**: `services/src/ucan_service.rs` (lines 679-750)

```rust
// 1. Parse owner token (compile-time type safety)
let owner_token = ResourceOwnerToken::from_token(&owner_ucan)?;

// 2. Extract template from owner (data-driven)
let template = owner_token.ucan().get_delegation_template("node")?;

// 3. Build capabilities using template
let capabilities = template.build_capabilities(&resource_id, "resource");

// 4. Create facts with token_type from template
let mut facts = template.to_facts();
facts.insert("role".to_string(), json!("node"));

// 5. Generate and return typed token
let node_ucan = crypto_utils.write().await.generate_ucan(...)?;
Ok(ResourceShareToken::from_token(&node_ucan)?)
```

### Example 3: Capability Check During Sync

**Code Location**: `services/src/merge_service.rs`

```rust
// 1. Parse both tokens
let our_ucan = GenericUcan::from_token(&our_token)?;
let peer_ucan = GenericUcan::from_token(&peer_token)?;

// 2. Check our capability (can WE write?)
if our_ucan.get_capability("content_doc") != Some(Capability::Collaborator) {
    return Ok(SyncDecision::DontSend);
}

// 3. Check peer's no_incoming_updates (do THEY accept?)
if peer_ucan.has_no_incoming_updates("content_doc") {
    return Ok(SyncDecision::DontSend);
}

// 4. Check peer capability (can THEY receive?)
match peer_ucan.get_capability("content_doc") {
    Some(Capability::Collaborator) | Some(Capability::Viewer) => {
        Ok(SyncDecision::SendIncrementalUpdates)
    }
    _ => Ok(SyncDecision::DontSend),
}
```

---

## Frontend Integration

### Permission Templates (permissions.ts)

**Code Location**: `sthalam/frontend/desktop/src/config/permissions.ts`

```typescript
export const RESOURCE_TEMPLATE = {
  owner_template: {
    token_type: "resource_owner",
    capabilities: {
      "template_doc": "collaborator",
      "content_doc": "collaborator",
      "collaborative_doc": "collaborator",
      "submissions_doc": "collaborator",
      "static_assets": "collaborator",
    },
    doc_metadata: {
      "static_assets": { "type": "asset", "allowed_mimes": ["image/*"] },
      "template_doc": { "type": "crdt" },
      // ... rest of metadata
    },
    sync: {
      local_only: ["user_content_doc"],
    },
    delegation: {
      node: {
        token_type: "resource_share",
        capabilities: { /* node template */ },
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
│ BACKEND: Dumb Executor                                      │
│ services/src/ucan_service.rs                               │
│ • Extract template from delegator UCAN                     │
│ • Build capabilities from template                         │
│ • Generate new token with template data                    │
│ • No business logic - just data flow                       │
└─────────────────────────────────────────────────────────────┘
                        ↓
┌─────────────────────────────────────────────────────────────┐
│ TOKEN VALIDATION: Core Module                               │
│ core/src/ucan/                                              │
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

**Phases Completed:**
- ✅ Phase 1: Core domain models (capability.rs, types.rs, etc)
- ✅ Phase 2: Rewritten ucan_service.rs (zero hardcoded templates)
- ✅ Phase 3: Consolidated services (resource_service module)
- ✅ Phase 5+: Data-driven token type system
- ✅ Phase 6+: URI parsing architecture & naming clarity

**Build Status:**
- ✅ All crates compile successfully
- ✅ No type errors in UCAN module
- ✅ All services layer integrated

**Testing:**
- ✅ Folder sync with UCAN-first publishing
- ✅ Resources delegated with correct token types
- ✅ All permission validations working
- ✅ End-to-end sync flows verified

---

## References

**Core Implementation Files:**
- `/home/abe/osvauld/core/src/ucan/` - UCAN module (mod.rs, types.rs, parser.rs, token.rs, uri.rs)
- `/home/abe/osvauld/services/src/ucan_service.rs` - Token generation (756 lines, zero hardcoded)
- `/home/abe/osvauld/sthalam/frontend/desktop/src/config/permissions.ts` - Frontend templates
- `/home/abe/osvauld/core/src/models/sync_context.rs` - Pure sync decision logic

**Related Documentation:**
- `SYNC_PROTOCOL.md` - Complete P2P sync protocol specification
- `NETWORK_LAYER.md` - P2P orchestration and message routing
- `SERVICE_LAYER.md` - Business logic layer architecture

---

**Status:** Production Ready
**Owner:** Core Authorization System
**Last Verified:** 2025-11-15
