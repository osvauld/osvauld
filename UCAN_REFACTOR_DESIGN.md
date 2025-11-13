# UCAN Permission System Refactor - Design Document

## Overview

This document outlines the refactor of Osvauld's UCAN permission system to use typed token wrappers and pure functional logic for sync permission decisions.

## Problem Statement

### Current Issues
1. **Bug**: Viewer is sending updates for documents they have `crud/readonly` access to
2. **Implicit types**: Token types only exist in logic, not in code
3. **No type safety**: Can pass wrong token type to functions
4. **Scattered logic**: Permission checks spread across multiple files
5. **Hard to verify**: Difficult to prove correctness of permission logic
6. **Hardcoded templates**: Backend hardcodes viewer_template instead of extracting from owner UCAN

### Root Cause
The sync protocol only checks if peer has capability, but doesn't check:
- If sender has write permission (Viewer with readonly shouldn't send)
- If receiver explicitly rejects updates (no_incoming_updates fact)

## Design Goals

1. **Type Safety**: Use typed token wrappers (compile-time guarantees)
2. **Correctness**: Use pure functions (deterministic, testable, verifiable)
3. **Explicit Code**: Token types visible in function signatures
4. **Single Source of Truth**: Frontend defines all permissions, backend never hardcodes
5. **Data-Driven**: Backend is generic, works with any document names/types from frontend
6. **Maintainability**: Easy to understand, test, and extend

## Core Principles

### 1. Frontend is Source of Truth
- ✅ Frontend (`permissions.ts`) defines ALL permission templates
- ✅ Backend embeds templates in owner UCAN
- ✅ Backend extracts templates when delegating
- ✅ Backend NEVER hardcodes permission structures
- ✅ To change permissions, update `permissions.ts` only

### 2. Data-Driven Backend
- ✅ Backend code is generic (works with any document names)
- ✅ Document names are data, not hardcoded
- ✅ Adding new documents = frontend-only change
- ✅ Backend derives behavior from UCAN content

### 3. Domain vs Data Separation
- **Domain** (in code): Types with behavior (Capability, DocType, Role)
- **Data** (from UCAN): Document names, MIME types, constraints
- **Backend** validates domain types, passes through data

## Architecture

### High-Level Components

```
┌─────────────────────────────────────────────────────┐
│           osvauld_core::models                       │
│                                                       │
│  Capability (enum) - Collaborator, Viewer, Submitter│
│  Role (enum) - Owner, Node, User, Viewer            │
│  DocType (enum) - Crdt, Asset                       │
│  ResourceAction (enum) - GetShareLink, Delete, etc. │
│                                                       │
│  ResourceUcan (domain model)                        │
│    - Parse & extract UCAN data                      │
│    - Rich API for queries                           │
│                                                       │
│  UcanToken (typed wrappers)                         │
│    - ResourceOwnerToken                             │
│    - ResourceShareToken                             │
│    - FolderOwnerToken                               │
│    - FolderShareToken                               │
│                                                       │
│  SyncContext (dual-UCAN context)                    │
│    - Pure functions for sync decisions              │
└─────────────────────────────────────────────────────┘
                          ▲
                          │
┌─────────────────────────────────────────────────────┐
│              services/ucan_service.rs                │
│                                                       │
│  Token Generation & Management                       │
│    - Uses typed tokens in signatures                │
│    - Returns typed tokens                           │
│    - Extracts templates from delegator UCAN         │
└─────────────────────────────────────────────────────┘
                          ▲
                          │
┌─────────────────────────────────────────────────────┐
│            services/resource_service.rs              │
│                                                       │
│  Business Logic                                      │
│    - Uses SyncContext for permission checks         │
│    - Calls pure functions                           │
└─────────────────────────────────────────────────────┘
                          ▲
                          │
┌─────────────────────────────────────────────────────┐
│         network/p2p/resource_sync.rs                 │
│                                                       │
│  P2P Orchestration                                  │
│    - Parses tokens as typed wrappers                │
│    - Calls resource_service                         │
└─────────────────────────────────────────────────────┘
```

## Detailed Design

### 1. Domain Model (Backend Types with Behavior)

**File**: `osvauld_core/src/models/capability.rs`

```rust
/// Permission level for document access (domain concept)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Capability {
    /// Full bidirectional CRDT sync (was: crud/merge)
    Collaborator,

    /// Receive-only, no sending updates (was: crud/readonly)
    Viewer,

    /// Send full snapshots to isolated namespace (was: crud/submit)
    Submitter,
}

impl Capability {
    pub fn from_str(s: &str) -> Result<Self, String> {
        match s {
            "collaborator" => Ok(Capability::Collaborator),
            "viewer" => Ok(Capability::Viewer),
            "submitter" => Ok(Capability::Submitter),
            _ => Err(format!("Unknown capability: {}", s))
        }
    }

    pub fn can_write(&self) -> bool {
        matches!(self, Capability::Collaborator | Capability::Submitter)
    }

    pub fn can_sync_bidirectional(&self) -> bool {
        matches!(self, Capability::Collaborator)
    }
}

/// User role in UCAN token (domain concept)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Owner,   // Resource creator, full control
    Node,    // Sovereign node hosting resource
    User,    // Peer user (P2P collaboration)
    Viewer,  // Limited access (viewer mode)
}

/// Document type (domain concept with behavior)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocType {
    Crdt,   // CRDT document (Loro, state vectors, merge)
    Asset,  // Binary asset (blob storage, no merge)
}

impl DocType {
    pub fn from_str(s: &str) -> Result<Self, String> {
        match s {
            "crdt" => Ok(DocType::Crdt),
            "asset" => Ok(DocType::Asset),
            _ => Err(format!("Unknown doc type: {}", s))
        }
    }

    pub fn supports_merge(&self) -> bool {
        matches!(self, DocType::Crdt)
    }

    pub fn is_binary(&self) -> bool {
        matches!(self, DocType::Asset)
    }
}

/// Token type for validation (domain concept)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenType {
    ResourceOwner,
    ResourceShare,
    FolderOwner,
    FolderShare,
}

/// Sync decision for a document (domain concept)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncDecision {
    SendIncrementalUpdates,
    SendFullSnapshot,
    DontSend,
}

/// Resource-level actions (domain concept)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceAction {
    GetShareLink,
    RevokeAccess,
    UpdateMetadata,
    Delete,
    Export,
}

/// Sync behavior facts from UCAN (data structure)
#[derive(Debug, Clone, Default)]
pub struct SyncFacts {
    /// Documents that stay local (was: dont_send_to_node)
    pub local_only: Vec<String>,

    /// Documents that don't accept incoming updates (was: no_update_from_node)
    pub no_incoming_updates: Vec<String>,

    /// Documents that send full snapshots (was: full_doc_send)
    pub send_full_snapshot: Vec<String>,
}

/// Document metadata (combines domain types with data)
#[derive(Debug, Clone)]
pub struct DocMetadata {
    pub name: String,                      // Data: document name
    pub doc_type: DocType,                 // Domain: Crdt or Asset
    pub allowed_mimes: Option<Vec<String>>, // Data: MIME patterns
    pub max_size_mb: Option<u64>,          // Data: size limit
}
```

### 2. ResourceUcan Domain Model

**File**: `osvauld_core/src/models/resource_ucan.rs`

```rust
/// Parsed resource UCAN with domain logic
pub struct ResourceUcan {
    raw_token: String,
    parsed: ParsedUcan,
    role: Role,
    token_type: TokenType,
    capabilities: HashMap<String, Capability>,  // doc_name -> capability
    doc_metadata: HashMap<String, DocMetadata>,  // doc_name -> metadata
    sync_facts: SyncFacts,
    resource_actions: Option<Vec<ResourceAction>>,
    delegation_templates: HashMap<String, DelegationTemplate>,
}

impl ResourceUcan {
    /// Parse UCAN token and extract all domain information
    pub fn from_token(token: &str) -> ServiceResult<Self>;

    // Accessors
    pub fn raw_token(&self) -> &str;
    pub fn role(&self) -> Role;
    pub fn token_type(&self) -> TokenType;

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

    // Resource-level actions
    pub fn can_get_share_link(&self) -> bool;
    pub fn can_delete(&self) -> bool;

    // Resource/Folder ID extraction
    pub fn resource_id(&self) -> &str;
    pub fn folder_id(&self) -> &str;
}
```

### 3. Typed Token Wrappers

**File**: `osvauld_core/src/models/ucan_token.rs`

```rust
/// Resource owner token (full control)
pub struct ResourceOwnerToken {
    ucan: ResourceUcan,
}

impl ResourceOwnerToken {
    pub fn from_token(token: &str) -> ServiceResult<Self>;
    pub fn ucan(&self) -> &ResourceUcan;
    pub fn resource_id(&self) -> &str;
    pub fn get_delegation_template(&self, role: &str) -> ServiceResult<DelegationTemplate>;
}

/// Resource share token (node, user, or viewer)
pub struct ResourceShareToken {
    ucan: ResourceUcan,
}

impl ResourceShareToken {
    pub fn from_token(token: &str) -> ServiceResult<Self>;
    pub fn ucan(&self) -> &ResourceUcan;
    pub fn role(&self) -> Role;
    pub fn resource_id(&self) -> &str;
    pub fn get_delegation_template(&self, role: &str) -> ServiceResult<DelegationTemplate>;
}

// Similar for FolderOwnerToken, FolderShareToken
```

### 4. SyncContext with Pure Functions

**File**: `osvauld_core/src/models/sync_context.rs`

```rust
/// Context for dual-UCAN sync decisions
pub struct SyncContext {
    our_ucan: ResourceUcan,
    peer_ucan: ResourceUcan,
}

impl SyncContext {
    pub fn new(our_token: &str, peer_token: &str) -> ServiceResult<Self>;

    pub fn from_owner_and_share(
        our: &ResourceOwnerToken,
        peer: &ResourceShareToken,
    ) -> Self;

    pub fn from_share_tokens(
        our: &ResourceShareToken,
        peer: &ResourceShareToken,
    ) -> Self;
}

// Pure functions for sync logic

/// Determine if we should send updates for a document
pub fn should_send_updates(
    context: &SyncContext,
    doc_name: &str,
) -> SyncDecision;

/// Check if we can receive updates for a document
pub fn can_receive_updates(
    context: &SyncContext,
    doc_name: &str,
) -> bool;

/// Generate updates to send to peer
pub fn generate_updates(
    context: &SyncContext,
    resource: &Resource,
    peer_state_vectors: HashMap<String, VersionVector>,
) -> ServiceResult<HashMap<String, Vec<u8>>>;

/// Apply peer updates to resource
pub fn apply_updates(
    context: &SyncContext,
    resource: &mut Resource,
    peer_updates: HashMap<String, Vec<u8>>,
) -> ServiceResult<()>;
```

## Permission Logic

### Sync Decision Algorithm

**Function**: `should_send_updates(context, doc_name)`

```
1. Check our local_only facts
   IF doc in our_ucan.local_only:
       RETURN DontSend

2. Check our capability (can WE write?)
   MATCH our_ucan.get_capability(doc):
       Viewer → RETURN DontSend
       Collaborator → Continue
       Submitter → RETURN SendFullSnapshot
       None → RETURN DontSend

3. Check peer's no_incoming_updates
   IF doc in peer_ucan.no_incoming_updates:
       RETURN DontSend

4. Check peer capability (can THEY receive?)
   MATCH peer_ucan.get_capability(doc):
       Collaborator OR Viewer → RETURN SendIncrementalUpdates
       _ → RETURN DontSend
```

### Permission Matrix

| Document | Owner | Node | User | Viewer | Description |
|----------|-------|------|------|--------|-------------|
| collaborative_doc | collaborator | collaborator | collaborator | collaborator | Comments, annotations |
| content_doc | collaborator | collaborator | collaborator | viewer | Main content |
| template_doc | collaborator | collaborator | collaborator | viewer | Form structure |
| submissions_doc | collaborator | collaborator | collaborator | submitter | User submissions (isolated) |
| static_assets | collaborator | collaborator | viewer | viewer | Owner-controlled assets |
| shared_assets | collaborator | collaborator | collaborator | collaborator | Collaborative uploads |
| user_content_doc | collaborator (local) | collaborator (local) | collaborator (local) | collaborator (local) | Personal notes (never syncs) |

**Sync Behavior:**
- ✅ Owner ↔ Node ↔ User: Bidirectional for all docs (except local_only)
- ✅ Node/User → Viewer: One-way for viewer docs (content, template, static_assets)
- ❌ Viewer → Node/User: Blocked for viewer docs (readonly)
- ✅ Viewer → Node/User: Allowed for collaborative_doc, shared_assets
- ❌ Node/User → Viewer: Blocked for submissions_doc (isolated namespace)

## UCAN Structure

### Permission Template Flow

```
1. Frontend (permissions.ts)
   ↓ Defines permission templates

2. First Token Creation (Owner)
   ↓ Frontend sends templates to backend
   ↓ Backend embeds templates in owner UCAN
   ↓ Owner UCAN contains: node_template, user_template, viewer_template

3. Owner → Node/User Delegation
   ↓ Backend extracts node_template or user_template from owner UCAN
   ↓ Backend issues node/user token using extracted template
   ↓ Node/User UCAN contains: user_template, viewer_template (for further delegation)

4. Node/User → Viewer Delegation
   ↓ Backend extracts viewer_template from delegator's UCAN
   ↓ Backend issues viewer token using viewer_template
   ↓ Viewer UCAN contains: only their own capabilities

5. Sync Decisions
   ↓ Backend parses UCAN capabilities and facts
   ↓ Uses embedded data to make permission decisions
   ↓ All logic derived from UCAN content
```

### Token Types and Delegation Chain

```
┌──────────────────────────────────────────────────────┐
│ ResourceOwnerToken (token_type: resource_owner)      │
│   role: owner                                         │
│   Contains: node_template + user_template +          │
│             viewer_template                           │
│   Can delegate to: Node, User, Viewer                │
└──────────────────────────────────────────────────────┘
                          │
                          │ issue_resource_ucan_for_node()
                          │ (extracts node_template)
                          ↓
┌──────────────────────────────────────────────────────┐
│ ResourceShareToken (token_type: resource_share)      │
│   role: node                                          │
│   Contains: user_template + viewer_template          │
│   Can delegate to: User, Viewer                      │
└──────────────────────────────────────────────────────┘
                          │
                          │ issue_resource_ucan_for_user()
                          │ (extracts user_template)
                          ↓
┌──────────────────────────────────────────────────────┐
│ ResourceShareToken (token_type: resource_share)      │
│   role: user                                          │
│   Contains: viewer_template                           │
│   Can delegate to: Viewer                             │
└──────────────────────────────────────────────────────┘
                          │
                          │ create_viewer_resource_token()
                          │ (extracts viewer_template)
                          ↓
┌──────────────────────────────────────────────────────┐
│ ResourceShareToken (token_type: resource_share)      │
│   role: viewer                                        │
│   Contains: (no templates)                            │
│   Can delegate to: (none)                             │
└──────────────────────────────────────────────────────┘
```

### Resource Owner Token

```json
{
  "aud": "<recipient_pub_key>",
  "iss": "<issuer_pub_key>",

  "cap": [
    "sthalam:resource:{id}:collaborative_doc:collaborator",
    "sthalam:resource:{id}:content_doc:collaborator",
    "sthalam:resource:{id}:template_doc:collaborator",
    "sthalam:resource:{id}:submissions_doc:collaborator",
    "sthalam:resource:{id}:static_assets:collaborator",
    "sthalam:resource:{id}:shared_assets:collaborator",
    "sthalam:resource:{id}:user_content_doc:collaborator"
  ],

  "fct": {
    "token_type": "resource_owner",
    "role": "owner",

    "sync": {
      "local_only": ["user_content_doc"]
    },

    "doc_metadata": {
      "collaborative_doc": { "type": "crdt" },
      "content_doc": { "type": "crdt" },
      "template_doc": { "type": "crdt" },
      "submissions_doc": { "type": "crdt" },
      "user_content_doc": { "type": "crdt" },
      "static_assets": {
        "type": "asset",
        "allowed_mimes": ["image/*", "text/css", "font/*"],
        "max_size_mb": 5
      },
      "shared_assets": {
        "type": "asset",
        "allowed_mimes": ["image/*", "video/*", "audio/*"],
        "max_size_mb": 100
      }
    },

    "delegation": {
      "node": {
        "capabilities": {
          "collaborative_doc": "collaborator",
          "content_doc": "collaborator",
          "template_doc": "collaborator",
          "submissions_doc": "collaborator",
          "static_assets": "collaborator",
          "shared_assets": "collaborator",
          "user_content_doc": "collaborator"
        },
        "sync": {
          "local_only": ["user_content_doc"]
        }
      },
      "user": {
        "capabilities": {
          "collaborative_doc": "collaborator",
          "content_doc": "collaborator",
          "template_doc": "collaborator",
          "submissions_doc": "collaborator",
          "static_assets": "viewer",
          "shared_assets": "collaborator",
          "user_content_doc": "collaborator"
        },
        "sync": {
          "local_only": ["user_content_doc"]
        }
      },
      "viewer": {
        "capabilities": {
          "collaborative_doc": "collaborator",
          "content_doc": "viewer",
          "template_doc": "viewer",
          "submissions_doc": "submitter",
          "static_assets": "viewer",
          "shared_assets": "collaborator",
          "user_content_doc": "collaborator"
        },
        "sync": {
          "local_only": ["user_content_doc"],
          "no_incoming_updates": ["submissions_doc"],
          "send_full_snapshot": ["submissions_doc"]
        }
      }
    }
  }
}
```

### Resource Share Token (Node)

```json
{
  "aud": "<node_pub_key>",
  "iss": "<owner_pub_key>",

  "cap": [
    "sthalam:resource:{id}:collaborative_doc:collaborator",
    "sthalam:resource:{id}:content_doc:collaborator",
    "sthalam:resource:{id}:template_doc:collaborator",
    "sthalam:resource:{id}:submissions_doc:collaborator",
    "sthalam:resource:{id}:static_assets:collaborator",
    "sthalam:resource:{id}:shared_assets:collaborator",
    "sthalam:resource:{id}:user_content_doc:collaborator"
  ],

  "fct": {
    "token_type": "resource_share",
    "role": "node",

    "sync": {
      "local_only": ["user_content_doc"]
    },

    "delegation": {
      "user": { /* user_template */ },
      "viewer": { /* viewer_template */ }
    }
  }
}
```

### Resource Share Token (Viewer)

```json
{
  "aud": "<viewer_pub_key>",
  "iss": "<node_pub_key>",

  "cap": [
    "sthalam:resource:{id}:collaborative_doc:collaborator",
    "sthalam:resource:{id}:content_doc:viewer",
    "sthalam:resource:{id}:template_doc:viewer",
    "sthalam:resource:{id}:submissions_doc:submitter",
    "sthalam:resource:{id}:static_assets:viewer",
    "sthalam:resource:{id}:shared_assets:collaborator",
    "sthalam:resource:{id}:user_content_doc:collaborator"
  ],

  "fct": {
    "token_type": "resource_share",
    "role": "viewer",

    "sync": {
      "local_only": ["user_content_doc"],
      "no_incoming_updates": ["submissions_doc"],
      "send_full_snapshot": ["submissions_doc"]
    }
  }
}
```

## Implementation Plan

### Phase 1: Create Core Models
1. Create `capability.rs` with enums (Role, Capability, DocType, etc.)
2. Create `resource_ucan.rs` domain model
3. Create `ucan_token.rs` typed wrappers
4. Create `sync_context.rs` with pure functions
5. Add tests for each module

**Deliverable**: Core domain models with unit tests

### Phase 2: Update ucan_service.rs
1. **Remove hardcoded viewer_template** (lines 1264-1282 in current code)
2. Extract templates from delegator UCAN instead of hardcoding
3. Add `token_type` fact to all token generation functions
4. Update function signatures to use typed tokens (input and output)
5. Return typed tokens instead of (String, String)
6. Add backward compatibility if needed

**Critical**: Backend must NEVER hardcode permission templates. Always extract from delegator UCAN.

**Deliverable**: Token generation uses typed tokens and extracts templates

### Phase 3: Update resource_service.rs
1. Update `prepare_resource_sync_request()` to use SyncContext
2. Update `handle_resource_update()` to use SyncContext
3. Remove inline permission checks
4. Use pure functions from sync_context
5. Update function signatures to accept typed tokens

**Deliverable**: Resource service uses SyncContext for all permission logic

### Phase 4: Update resource_sync.rs
1. Parse incoming UCANs as typed tokens
2. Load our UCAN as typed token
3. Pass typed tokens to resource_service
4. Remove calls to merge_service (call resource_service directly)

**Deliverable**: P2P layer uses typed tokens

### Phase 5: Handle merge_service.rs
**Option A**: Delete entirely (recommended)
**Option B**: Keep minimal CRDT utilities

If keeping minimal utils:
```rust
// Only CRDT operations, no permission logic
pub fn export_snapshot(doc: &LoroDoc) -> Vec<u8>;
pub fn export_from(doc: &LoroDoc, from: &VersionVector) -> Vec<u8>;
pub fn import_updates(doc: &mut LoroDoc, updates: &[u8]) -> Result<()>;
```

**Deliverable**: Simplified or removed merge_service

### Phase 6: Update Frontend
1. Update `permissions.ts` capability names (collaborator, viewer, submitter)
2. Update fact names (local_only, no_incoming_updates, send_full_snapshot)
3. Add `token_type` field to all templates
4. Clean up structure: use `sync` and `delegation` sections
5. Add `doc_metadata` with type and constraints
6. Update any frontend UCAN parsing logic

**Deliverable**: Frontend uses new permission keywords

### Phase 7: Testing & Validation
**Test cases:**
1. Owner ↔ Node sync (all docs bidirectional)
2. Node ↔ Viewer sync (mixed permissions)
3. Viewer cannot send content_doc/template_doc updates ✓ (fixes current bug)
4. Viewer can send collaborative_doc updates ✓
5. Viewer can send shared_assets updates ✓
6. Viewer cannot send submissions_doc updates, Node cannot send to Viewer ✓
7. user_content_doc never syncs for anyone ✓
8. Type safety: Cannot pass wrong token type (compile-time)
9. Permission validation: Tokens validated at parse time

**Deliverable**: All tests passing, bug fixed

## Benefits

### Type Safety
✅ Can't pass wrong token type (compile-time error)
✅ Function signatures are self-documenting
✅ IDE autocomplete shows available methods per token type

### Correctness
✅ Pure functions are deterministic
✅ Easy to test and verify
✅ No side effects or hidden state
✅ Thread-safe by default

### Maintainability
✅ All permission logic in one place (SyncContext)
✅ Clear separation of concerns
✅ Easy to add new capabilities or roles
✅ Explicit, readable code

### Flexibility
✅ Frontend controls all permissions
✅ Backend never hardcodes document names
✅ Adding new documents = frontend-only change
✅ Permission changes don't require backend code changes

### Bug Fixes
✅ Fixes current bug (Viewer sending readonly doc updates)
✅ Prevents similar bugs in future
✅ Type system catches mistakes early

## Migration Strategy

1. **Non-breaking addition**: Create new modules alongside existing code
2. **Backward compatibility**: Support old API while migrating
3. **Gradual migration**: Update one service at a time
4. **Test continuously**: Run tests after each change
5. **Remove old code**: Once migration complete, remove deprecated code

## Risk Assessment

### Low Risk
- Creating new modules (doesn't break existing code)
- Adding new types (additive change)
- Pure functions (no side effects)

### Medium Risk
- Changing ucan_service signatures (many callers)
- Updating resource_service (core business logic)
- Frontend permission changes (need to sync with backend)

### Mitigation
- Add backward compatibility layer initially
- Extensive testing at each phase
- Incremental rollout with feature flags if needed
- Keep old code until new code proven stable

## Success Criteria

1. ✅ Viewer cannot send updates for readonly documents
2. ✅ All existing sync scenarios still work
3. ✅ All tests pass
4. ✅ No type errors (compile-time validation)
5. ✅ Code is more readable and maintainable
6. ✅ Performance is same or better
7. ✅ Backend has no hardcoded permission templates

## Timeline Estimate

- Phase 1 (Core models): 2-3 days
- Phase 2 (ucan_service): 1-2 days
- Phase 3 (resource_service): 1-2 days
- Phase 4 (resource_sync): 1 day
- Phase 5 (merge_service): 0.5 day
- Phase 6 (Frontend): 1 day
- Phase 7 (Testing): 2-3 days

**Total**: 8-12 days

## References

- Current bug logs: Viewer sending content_doc/template_doc updates
- SYNC_PROTOCOL_DESIGN.md: Sync protocol specification
- permissions.ts: Frontend permission definitions
- ucan_service.rs: Current token generation
- merge_service.rs: Current sync logic
