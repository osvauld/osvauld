# UCAN Architecture Migration V2 - Implementation Plan

**Date:** 2025-01-15
**Status:** Planning
**Goal:** Redesign UCAN architecture with solid foundations for POC

---

## Executive Summary

Migrate UCAN implementation to:
- **Unified module** - All UCAN code in `core/src/ucan/`
- **Traits for reusability** - Generic functions via Rust traits
- **Explicit ID extraction** - Facts only, no URI parsing fallback
- **Clean facts structure** - Single source of truth
- **Updated frontend** - Match new backend structure

---

## Current State Analysis

### Current File Structure
```
core/src/models/
├── capability.rs          (313 lines) - Enums (Role, Capability, DocType)
├── ucan_domain.rs         (447 lines) - ResourceUcan parser, DelegationTemplate
├── ucan_token.rs          (239 lines) - Resource/Folder typed tokens
└── connection_token.rs    (267 lines) - Connection typed tokens
```

### Problems Identified

1. **Magic Indices in URI Parsing**
```rust
let parts: Vec<&str> = uri.split(':').collect();
let id = parts[2];  // ❌ What is parts[2]?
```

2. **Scattered Code**
- UCAN logic across 4 different files
- Hard to find related functionality

3. **Limited Reusability**
- No traits = duplicate code for each token type
- Can't write generic validation functions

4. **URI Parsing Fallback**
```rust
// Try facts first, then parse URI
if let Some(id) = facts.get("folder_id") { ... }
else { parse_from_uri() }  // ❌ Unnecessary complexity
```

5. **Unclear Facts Structure**
```json
{
  "cap": { "sthalam:folder:abc:add_resources": {...} },
  "fct": {
    "capabilities": {"add_resources": "add_resources"}  // Why?
  }
}
```

---

## Target State

### New Module Structure
```
core/src/ucan/
├── mod.rs              # Public API, re-exports
├── types.rs            # All enums (Role, Capability, DocType, TokenType)
├── parser.rs           # ResourceUcan, DelegationTemplate (parsing logic)
├── token.rs            # Traits + typed token wrappers
└── uri.rs              # URI builders (no parsing)
```

### Trait Hierarchy

```rust
UcanToken                  // Base trait - all tokens
├── HasId                  // Tokens with IDs (from facts)
│   ├── ResourceOps        // Resource-specific operations
│   └── FolderOps          // Folder-specific operations
└── CanDelegate            // Tokens that can delegate

// Token implementations:
ResourceOwnerToken: UcanToken + HasId + CanDelegate + ResourceOps
ResourceShareToken: UcanToken + HasId + CanDelegate + ResourceOps
ResourceViewerToken: UcanToken + HasId + ResourceOps (NO CanDelegate)

FolderOwnerToken: UcanToken + HasId + CanDelegate + FolderOps
FolderShareToken: UcanToken + HasId + CanDelegate + FolderOps
FolderViewerToken: UcanToken + HasId + FolderOps (NO CanDelegate)
```

### Simplified ID Extraction

```rust
// ONLY from facts - no fallback
pub fn resource_id(&self) -> Result<String, UcanError> {
    self.parsed.facts()
        .and_then(|f| f.get("resource_id"))
        .and_then(|v| v.as_str())
        .map(String::from)
        .ok_or(UcanError::MissingResourceId)
}
```

### Clean Facts Structure

```json
{
  "cap": { /* UCAN protocol capabilities */ },
  "fct": {
    "token_type": "folder_share",
    "role": "node",
    "folder_id": "abc123",        // ← Only source of truth
    "delegation": {                // ← Templates for child tokens
      "user": {
        "operations": { ... }
      }
    }
  }
}
```

---

## Migration Steps

### Phase 1: Setup (Day 1)

#### 1.1 Create New Module Structure
```bash
mkdir -p core/src/ucan
touch core/src/ucan/mod.rs
touch core/src/ucan/types.rs
touch core/src/ucan/parser.rs
touch core/src/ucan/token.rs
touch core/src/ucan/uri.rs
```

#### 1.2 Move Enums to types.rs
- Copy from `capability.rs`:
  - `Role`, `Capability`, `DocType`
  - `ResourceTokenType`, `ConnectionTokenType`
  - `SyncFacts`, `DocMetadata`
  - `ResourceAction`

#### 1.3 Move Parser to parser.rs
- Copy from `ucan_domain.rs`:
  - `ResourceUcan` struct and impl
  - `DelegationTemplate` struct and impl
  - `ResourceUcanError`

#### 1.4 Move Token Types to token.rs
- Copy from `ucan_token.rs` and `connection_token.rs`:
  - All token type structs
  - Initial implementations (will add traits next)

---

### Phase 2: Define Traits (Day 2)

#### 2.1 Define Core Traits in token.rs

```rust
/// Base trait for all UCAN tokens
pub trait UcanToken {
    fn raw_token(&self) -> &str;
    fn role(&self) -> Role;
    fn token_type(&self) -> TokenType;
    fn parsed(&self) -> &Ucan;
}

/// Tokens with extractable IDs
pub trait HasId: UcanToken {
    fn id(&self) -> Result<String, UcanError>;
}

/// Tokens that can delegate to child tokens
pub trait CanDelegate: UcanToken {
    fn delegation_template(&self, role: Role)
        -> Result<&DelegationTemplate, UcanError>;

    fn can_delegate_to(&self, role: Role) -> bool {
        self.delegation_template(role).is_ok()
    }
}

/// Resource-specific operations
pub trait ResourceOps: HasId {
    fn resource_id(&self) -> Result<String, UcanError> {
        self.id()
    }

    fn doc_capabilities(&self) -> &HashMap<String, Capability>;

    fn has_capability(&self, doc: &str, cap: Capability) -> bool {
        self.doc_capabilities()
            .get(doc)
            .map(|c| c >= &cap)
            .unwrap_or(false)
    }
}

/// Folder-specific operations
pub trait FolderOps: HasId {
    fn folder_id(&self) -> Result<String, UcanError> {
        self.id()
    }

    fn folder_operations(&self) -> Vec<String>;

    fn has_operation(&self, op: &str) -> bool {
        self.folder_operations().contains(&op.to_string())
    }
}
```

#### 2.2 Implement Traits for All Token Types

For each token type (ResourceOwnerToken, ResourceShareToken, etc.):
- Implement `UcanToken` (all tokens)
- Implement `HasId` (tokens with IDs)
- Implement `CanDelegate` (Owner and Share tokens only)
- Implement `ResourceOps` or `FolderOps` (domain-specific)

---

### Phase 3: Simplify ID Extraction (Day 2)

#### 3.1 Update parser.rs

Remove all URI parsing code from `ResourceUcan`:

```rust
impl ResourceUcan {
    /// Get resource ID from facts ONLY
    pub fn resource_id(&self) -> Result<String, UcanError> {
        self.parsed.facts()
            .and_then(|f| f.get("resource_id"))
            .and_then(|v| v.as_str())
            .map(String::from)
            .ok_or(UcanError::MissingResourceId)
    }

    /// Get folder ID from facts ONLY
    pub fn folder_id(&self) -> Result<String, UcanError> {
        self.parsed.facts()
            .and_then(|f| f.get("folder_id"))
            .and_then(|v| v.as_str())
            .map(String::from)
            .ok_or(UcanError::MissingFolderId)
    }

    // DELETE: Old parsing methods
    // - No more parsing from URI
    // - No more fallback logic
}
```

#### 3.2 Create URI Builders in uri.rs

```rust
/// Build resource capability URI
pub fn resource_capability(domain: &str, resource_id: &str, doc_name: &str) -> String {
    format!("{}:resource:{}:doc:{}", domain, resource_id, doc_name)
}

/// Build folder operation URI
pub fn folder_operation(domain: &str, folder_id: &str, operation: &str) -> String {
    format!("{}:folder:{}:op:{}", domain, folder_id, operation)
}

/// Build resource wildcard URI (for folder-scoped resources)
pub fn resource_wildcard(domain: &str, folder_id: &str, doc_name: &str) -> String {
    format!("{}:resource:{}/*:doc:{}", domain, folder_id, doc_name)
}
```

---

### Phase 4: Update Frontend (Day 3)

#### 4.1 Update permissions.ts

**Folder Template:**
```typescript
export const FOLDER_TEMPLATE = {
  owner_template: {
    operations: {                    // Renamed from "capabilities"
      "add_resources": "allow",
      "get_share_link": "allow",
      "share_folder": "allow",
      // REMOVED: "crud/read" (doesn't apply to folders)
    },
    delegation: {
      node: {
        operations: {
          "add_resources": "allow",
          "get_share_link": "allow",
          "share_folder": "allow",
        }
      },
      viewer: {
        operations: {
          "get_share_link": "allow",
          "request_resources": "allow",
        }
      }
    }
  }
};
```

**Resource Template:** (Keep as-is, already good)

---

### Phase 5: Update Service Layer (Day 4)

#### 5.1 Update ucan_service.rs imports

```rust
// Old imports
use osvauld_core::models::{
    Capability, Role, ResourceUcan, ResourceOwnerToken
};

// New imports
use osvauld_core::ucan::{
    types::{Role, Capability},
    parser::{ResourceUcan, DelegationTemplate},
    token::{ResourceOwnerToken, FolderShareToken},
    uri,
};

// Or use prelude for traits
use osvauld_core::ucan::prelude::*;
```

#### 5.2 Use Traits in Functions

```rust
/// Generic delegation using traits
pub async fn delegate<T>(
    parent: &T,
    role: Role,
    child_pub_key: &str,
    id: &str,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: Arc<RwLock<CryptoUtils>>,
) -> ServiceResult<String>
where
    T: CanDelegate + HasId,
{
    // Extract template using trait
    let template = parent.delegation_template(role)?;

    // Build capabilities using URI builder
    let capabilities = template.capabilities
        .iter()
        .map(|(doc, cap)| {
            let uri = uri::resource_capability("sthalam", id, doc);
            (uri, cap.clone())
        })
        .collect();

    // Generate token...
}
```

#### 5.3 Update All Service Files

Files to update:
- `services/src/ucan_service.rs`
- `services/src/folder_service.rs`
- `services/src/resource_service/`
- `services/src/website_service.rs`

#### 5.4 Update Network Layer

Files to update:
- `network/src/p2p/folder_sync.rs`
- `network/src/p2p/resource_sync.rs`
- `network/src/p2p/handshake.rs`

---

### Phase 5+: Data-Driven Token Type System (COMPLETED)

#### 5+.1 Problem: Hardcoded Token Type Logic

**Before:** Token types were hardcoded in backend services with match statements:

```rust
// ❌ Hardcoded logic - makes backend changes required for new token types
let token_type = match peer_role {
    "node" | "user" => "resource_share",
    "viewer" => "resource_viewer",
    _ => "resource_share",
};
facts.insert("token_type".to_string(), json!(token_type));
```

This had several issues:
1. **Not extensible**: Adding new token types requires backend code changes
2. **Tight coupling**: Business logic (what token type is) in technical code
3. **Implicit semantics**: Relationship between role and token type not declared anywhere
4. **Duplication**: Same logic repeated in multiple delegation functions

#### 5+.2 Solution: Data-Driven Token Types via DelegationTemplate

**Architecture Change:**
- **Frontend** (permissions.ts) defines token types in templates - source of truth for authorization semantics
- **Backend** executes templates as data - no business logic, just data flow
- **Benefits**: New token types without code changes, explicit authorization model

**Implementation:**

1. **DelegationTemplate Structure Update** (core/src/ucan/parser.rs):
```rust
pub struct DelegationTemplate {
    pub token_type: String,                      // ← NEW: Data-driven token type
    pub capabilities: HashMap<String, String>,  // doc_name -> capability
    pub sync: Option<SyncFacts>,
}

impl DelegationTemplate {
    pub fn to_facts(&self) -> serde_json::Map<String, serde_json::Value> {
        let mut facts = serde_json::Map::new();

        // ← NEW: Include token_type from template (data-driven)
        facts.insert("token_type".to_string(), serde_json::Value::String(self.token_type.clone()));

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
        token_type: "resource_share",  // ← Explicitly defined - source of truth
        capabilities: { ... }
      },
      viewer: {
        token_type: "resource_viewer",  // ← Different token type = different behavior
        capabilities: { ... }
      }
    }
  }
};

export const FOLDER_TEMPLATE = {
  owner_template: {
    delegation: {
      node: {
        token_type: "folder_share",    // ← Also defined here
        capabilities: { ... }
      },
      viewer: {
        token_type: "folder_viewer",   // ← Explicit mapping
        capabilities: { ... }
      }
    }
  }
};
```

3. **Backend Service Layer** (services/src/ucan_service.rs):
```rust
// ✅ Before: Removed hardcoded match statements
// ✅ Now: Using template.to_facts() which includes token_type
let mut facts = template.to_facts();  // token_type already included!
facts.insert("role".to_string(), json!(peer_role));

// In delegate_to_node(), delegate_to_user(), delegate_to_viewer(), delegate_by_role():
// - NO token_type insertion (it comes from template)
// - NO match logic on role
// - Just use template.to_facts()
```

4. **Parser with Backward Compatibility** (core/src/ucan/parser.rs):
```rust
// Extract token_type from delegation template
let token_type = template_obj
    .get("token_type")
    .and_then(|v| v.as_str())
    .map(String::from)
    .unwrap_or_else(|| format!("resource_{}", role_key)); // Fallback for old tokens
```

#### 5+.3 Design Philosophy: Resource-Driven Authorization

This implements the **resource-driven design pattern**:

```
┌─────────────────────────────────────────────────────────────┐
│ FRONTEND: Source of Truth                                   │
│                                                             │
│ permissions.ts defines:                                     │
│ • What operations exist (add_resources, share_folder, etc) │
│ • What capabilities grant access (collaborator, viewer)    │
│ • What token types are used (resource_share, folder_share) │
│ • How to delegate to different roles                       │
└─────────────────────────────────────────────────────────────┘
                        ↓
                   [API call]
                        ↓
┌─────────────────────────────────────────────────────────────┐
│ BACKEND: Dumb Executor                                      │
│                                                             │
│ ucan_service executes templates:                           │
│ • Read template from parsed token                          │
│ • Extract token_type from template (NOT from code logic)  │
│ • Build new token with template data                       │
│ • Sign and encrypt                                         │
│                                                             │
│ No business logic - just data flow                         │
└─────────────────────────────────────────────────────────────┘
```

#### 5+.4 Benefits

1. **Extensibility Without Code Changes**
   - New token type? Update frontend template, done
   - No backend deployment needed
   - System automatically handles new types

2. **Explicit Authorization Model**
   - What token types exist is declared in frontend
   - Relationship between role and token type is visible
   - Single source of truth for authorization semantics

3. **Separation of Concerns**
   - Frontend owns authorization policy
   - Backend executes policy as data
   - Decoupled, easier to understand and maintain

4. **Backward Compatibility**
   - Old tokens without token_type work via fallback
   - Graceful degradation to old behavior
   - No breaking changes

#### 5+.5 Files Modified

- **core/src/ucan/parser.rs**
  - Added `token_type: String` to DelegationTemplate
  - Updated `to_facts()` to include token_type
  - Updated `from_token()` to extract token_type with fallback

- **services/src/ucan_service.rs**
  - Removed hardcoded token_type in delegate_to_node()
  - Removed hardcoded token_type in delegate_to_user()
  - Removed hardcoded token_type in delegate_to_viewer()
  - Updated delegate_by_role() to use template.to_facts()
  - Removed unused extract_folder_id_with_add_resources()

- **sthalam/frontend/desktop/src/config/permissions.ts**
  - Added token_type to RESOURCE_TEMPLATE.delegation.node: "resource_share"
  - Added token_type to RESOURCE_TEMPLATE.delegation.viewer: "resource_viewer"
  - Added token_type to FOLDER_TEMPLATE.delegation.node: "folder_share"
  - Added token_type to FOLDER_TEMPLATE.delegation.viewer: "folder_viewer"

#### 5+.6 Testing

Tested resource delegation flow:
- ✅ Folder sync with UCAN-first publishing
- ✅ Resources delegated with correct token types
- ✅ No more "invalid token type" errors
- ✅ All services build successfully

---

### Phase 6: Cleanup (Day 5)

#### 6.1 Delete Old Files

```bash
rm core/src/models/capability.rs
rm core/src/models/ucan_domain.rs
rm core/src/models/ucan_token.rs
rm core/src/models/connection_token.rs
```

#### 6.2 Update mod.rs Files

Remove old module declarations, add new ones:

```rust
// core/src/lib.rs
pub mod ucan;  // New unified module

// Remove from models/mod.rs:
// pub mod capability;
// pub mod ucan_domain;
// pub mod ucan_token;
// pub mod connection_token;
```

---

## Testing Strategy

### Unit Tests

For each module:
- **types.rs**: Test enum serialization/deserialization
- **parser.rs**: Test UCAN parsing with various token structures
- **token.rs**: Test trait implementations for all token types
- **uri.rs**: Test URI building functions

### Integration Tests

- Test token generation and delegation flow
- Test ID extraction from facts
- Test capability validation
- Test folder/resource sync with new tokens

### End-to-End Tests

- Owner publishes folder to Node
- Owner sends resources to Node
- Node receives and validates tokens
- Verify all UCANs have correct structure

---

## Rollout Plan

### Stage 1: Development (5 days)
- Complete all migration steps
- All tests passing
- No compilation errors

### Stage 2: Testing (1 day)
- Manual testing of folder/resource sync
- Verify FolderResourcesRequest works
- Check all UCAN validations

### Stage 3: Deployment (immediate)
- Single user (you), no coordination needed
- Deploy and test in production

---

## Rollback Plan

If issues arise:
- Git revert to pre-migration commit
- All old code preserved in git history
- Clean cutover, no hybrid state

---

## Success Criteria

✅ All UCAN code in `core/src/ucan/` module
✅ Traits implemented for all token types
✅ IDs extracted from facts only (no URI parsing)
✅ All service layer using traits
✅ Frontend permissions.ts updated
✅ All tests passing
✅ FolderResourcesRequest working end-to-end
✅ Zero compilation errors

---

## Risk Assessment

| Risk | Impact | Mitigation |
|------|--------|------------|
| Breaking existing functionality | High | Comprehensive testing before deployment |
| Import errors across codebase | Medium | Update all imports systematically |
| Frontend/backend mismatch | Medium | Update permissions.ts alongside backend |
| Missing token validations | High | Review all validation logic during migration |

---

## File Change Summary

### New Files
- `core/src/ucan/mod.rs`
- `core/src/ucan/types.rs`
- `core/src/ucan/parser.rs`
- `core/src/ucan/token.rs`
- `core/src/ucan/uri.rs`

### Modified Files
- `core/src/lib.rs`
- `core/src/models/mod.rs`
- `services/src/ucan_service.rs` (major)
- `services/src/folder_service.rs`
- `services/src/resource_service/*.rs`
- `network/src/p2p/*.rs`
- `sthalam/frontend/desktop/src/config/permissions.ts`

### Deleted Files
- `core/src/models/capability.rs`
- `core/src/models/ucan_domain.rs`
- `core/src/models/ucan_token.rs`
- `core/src/models/connection_token.rs`

---

## Timeline

| Phase | Duration | Dependencies |
|-------|----------|--------------|
| Setup + Move Code | 1 day | None |
| Define + Implement Traits | 1 day | Phase 1 |
| Simplify ID Extraction | 1 day | Phase 2 |
| Update Frontend | 1 day | None (parallel) |
| Update Services | 1 day | Phase 2, 3 |
| Testing + Cleanup | 1 day | Phase 5 |
| **Total** | **5-6 days** | |

---

## Next Steps

1. Review this plan
2. Get approval to proceed
3. Create feature branch: `feat/ucan-migration-v2`
4. Execute Phase 1 (Setup)
5. Commit after each phase
6. Final review and merge

---

**Status:** Ready to implement
**Owner:** Developer
**Reviewer:** N/A (solo project)
