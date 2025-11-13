# UCAN Permission System Refactor - Design Document

## Progress Status

**Overall Progress**: 50% Complete (Phase 1-2 complete, Phase 3 80% complete)

- ✅ **Phase 1 Complete**: Core domain models (capability.rs, connection_token.rs, ucan_domain.rs, ucan_token.rs, sync_context.rs)
- ✅ **Phase 2 Complete**: Rewritten ucan_service.rs (1559 → 756 lines, zero hardcoded templates)
- 🔄 **Phase 3 In Progress (80%)**: 4-file resource_service module implemented (1210 lines written)
  - ✅ core.rs (330 lines) - Reusable building blocks
  - ✅ crud.rs (427 lines) - 7 CRUD functions with typed tokens
  - ✅ sync.rs (403 lines) - 19 sync functions (mix of new + transitional)
  - ✅ mod.rs (50 lines) - Module structure
  - ✅ Extended ucan_service utilities (302 lines added)
  - ⏳ Fixing 76 compilation errors (down from 200+)
- ⏳ **Phase 4-6 Pending**: P2P layer, Frontend, Testing

**Key Achievements**:
- 11 typed token wrappers created ✅
- UCAN 0.4.0 compatibility fixed ✅
- Zero hardcoded templates ✅
- Type-safe delegation pattern established ✅
- 4-file module structure implemented ✅
- Reusable core layer extracts 4 major patterns ✅
- Network package: 0 compilation errors ✅
- Services package: 76 errors (down from 200+) 🔄

**Phase 3 Implementation Details**:
- **New code written**: 1,512 lines (resource_service module 1210 + ucan_service utilities 302)
- **Core patterns extracted**: Load-Decrypt-Parse, Update-Encrypt-Save, State-Vector-Generation, Filter-and-Re-encrypt
- **Transitional strategy**: Keep merge_service & website_service temporarily for gradual migration
- **Typed token integration**: All new CRUD functions use ResourceOwnerToken, ResourceShareToken
- **Design decision**: core.rs is private module (internal reusable building blocks only)

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
- ✅ Frontend sends complete UCAN structure (including templates) to backend
- ✅ Backend stores UCAN exactly as received (no modification)
- ✅ Backend extracts templates when delegating
- ✅ Backend NEVER hardcodes permission structures
- ✅ Backend NEVER modifies or generates UCAN structure
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
│  ConnectionTokenType (enum)                         │
│    - OneTimeConnection, OwnerConnection,            │
│      NodeConnection, UserConnection, ViewerAuth     │
│                                                       │
│  ResourceTokenType (enum)                           │
│    - ResourceOwner, ResourceShare, ResourceViewer,  │
│      FolderOwner, FolderShare, FolderViewer         │
│                                                       │
│  ConnectionToken (domain model)                     │
│    - Parse & extract connection UCAN data           │
│    - Validate connection-specific claims            │
│                                                       │
│  ConnectionUcanToken (typed wrappers)               │
│    - OneTimeConnectionToken                         │
│    - OwnerConnectionToken                           │
│    - NodeConnectionToken                            │
│    - UserConnectionToken                            │
│    - ViewerAuthToken                                │
│                                                       │
│  ResourceUcan (domain model)                        │
│    - Parse & extract resource UCAN data             │
│    - Rich API for capability queries                │
│                                                       │
│  ResourceUcanToken (typed wrappers)                 │
│    - ResourceOwnerToken                             │
│    - ResourceShareToken                             │
│    - ResourceViewerToken                            │
│    - FolderOwnerToken                               │
│    - FolderShareToken                               │
│    - FolderViewerToken                              │
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

### 1. Token Type Architecture

Osvauld uses **two separate token hierarchies** for maximum type safety and explicitness:

1. **Connection Tokens** (`ConnectionTokenType`): For device-to-device connections and handshakes
2. **Resource Tokens** (`ResourceTokenType`): For resource/folder access and sync operations

This separation provides:
- ✅ Clear compile-time distinction between connection and resource contexts
- ✅ Easier testing (connection logic separate from sync logic)
- ✅ Better security (connection tokens can't be misused for resource operations)
- ✅ Explicit modeling of all token usage patterns

#### Token Usage Table

| Token Type | Issuer | Audience | Usage Context | Lifespan | Can Delegate To |
|------------|--------|----------|---------------|----------|-----------------|
| **Connection Tokens** |
| OneTimeConnection | Any role | Peer device | Initial P2P handshake (FirstConnectRequest.one_time_ucan) | Single use | N/A (consumed immediately) |
| OwnerConnection | Owner device | Peer device | Owner device-to-device sync (FirstConnectRequest.issued_ucan) | Long-lived | Node, User, Viewer |
| NodeConnection | Node device | Peer device | Node device-to-device sync (FirstConnectRequest.issued_ucan) | Long-lived | User, Viewer |
| UserConnection | User device | Peer device | User P2P collaboration (FirstConnectRequest.issued_ucan) | Long-lived | Viewer |
| ViewerAuth | Node (via link) | Viewer | Initial viewer authentication (ViewerHandshakeRequest.viewer_auth_token) | Single use / Short-lived | None |
| **Resource/Folder Tokens** |
| ResourceOwner | Owner | Self | Full resource control, contains all delegation templates | Permanent | Node, User, Viewer |
| ResourceShare | Owner/Node | Node/User | Resource access for Node or User role | Long-lived | User (if Node), Viewer |
| ResourceViewer | Node | Viewer | Viewer's resource access token (UpdateUcanMessage.new_ucan_token) | Session-based | None |
| FolderOwner | Owner | Self | Full folder control, contains all delegation templates | Permanent | Node, User, Viewer |
| FolderShare | Owner/Node | Node/User | Folder access for Node or User role | Long-lived | User (if Node), Viewer |
| FolderViewer | Node | Viewer | Viewer's folder access token (sent from Node to Viewer) | Session-based | None |

#### Delegation Chains

**P2P Connection Chain (Owner/Node/User)**:
```
OneTimeConnection (initial handshake)
  ↓
OwnerConnection / NodeConnection / UserConnection (ongoing sync)
  ↓
ResourceOwner / ResourceShare / FolderOwner / FolderShare (resource access)
```

**Viewer Connection Chain**:
```
ViewerAuth (from shareable link - connection token)
  ↓
ResourceViewer / FolderViewer (issued by Node via UpdateUcanMessage - resource tokens)
```

**Proof Chain Tracking**:
- All delegated tokens include `prf: [<parent_ucan_cid>]` in facts
- No recursive validation (just store parent CID for auditability)

### 2. Domain Model (Backend Types with Behavior)

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

/// Connection/Handshake token types (separate hierarchy from resource tokens)
/// These tokens are used for device-to-device connections and authentication
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectionTokenType {
    /// One-time use token for initial device handshake (any role)
    /// Used in: FirstConnectRequest.one_time_ucan
    /// Lifespan: Single use
    /// Purpose: Prove device pairing authorization
    OneTimeConnection,

    /// Owner device-to-device connection token
    /// Used in: FirstConnectRequest.issued_ucan, UcanAndUserExchange.ucan_token
    /// Lifespan: Long-lived
    /// Purpose: Owner device sync and full control operations
    OwnerConnection,

    /// Node device-to-device connection token
    /// Used in: FirstConnectRequest.issued_ucan, UcanAndUserExchange.ucan_token
    /// Lifespan: Long-lived
    /// Purpose: Node device sync and hosting operations
    NodeConnection,

    /// User device-to-device connection token (P2P collaboration)
    /// Used in: FirstConnectRequest.issued_ucan, UcanAndUserExchange.ucan_token
    /// Lifespan: Long-lived
    /// Purpose: User device sync and collaboration operations
    UserConnection,

    /// Viewer authentication token (from shareable link)
    /// Used in: ViewerHandshakeRequest.viewer_auth_token, WebsiteRequest.ucan_token
    /// Lifespan: Single use or short-lived
    /// Purpose: Initial viewer authentication
    ViewerAuth,
}

/// Resource/Folder token types (for sync and resource operations)
/// These tokens control access to specific resources and folders
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResourceTokenType {
    /// Resource owner token (full control)
    /// Role: Owner
    /// Contains: Delegation templates for Node/User/Viewer
    ResourceOwner,

    /// Resource share token (for Node or User role)
    /// Role: Node or User
    /// Contains: Delegated capabilities, may contain further delegation templates
    ResourceShare,

    /// Resource viewer token (for Viewer role)
    /// Role: Viewer
    /// Issued by Node after ViewerAuth validation
    /// Used in: UpdateUcanMessage.new_ucan_token (sent from Node to Viewer)
    /// Purpose: Viewer access to specific resource
    ResourceViewer,

    /// Folder owner token (full control)
    /// Role: Owner
    /// Contains: Delegation templates for Node/User/Viewer
    FolderOwner,

    /// Folder share token (for Node or User role)
    /// Role: Node or User
    /// Contains: Delegated capabilities, may contain further delegation templates
    FolderShare,

    /// Folder viewer token (for Viewer role)
    /// Role: Viewer
    /// Issued by Node after ViewerAuth validation
    /// Used in: Folder access for viewer (sent from Node to Viewer)
    /// Purpose: Viewer access to specific folder
    FolderViewer,
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

### 2. ConnectionToken Domain Model

**File**: `osvauld_core/src/models/connection_token.rs`

```rust
/// Parsed connection UCAN with domain logic
pub struct ConnectionToken {
    raw_token: String,
    parsed: Ucan,
    token_type: ConnectionTokenType,
    role: Role,
}

impl ConnectionToken {
    /// Parse connection UCAN token
    pub fn from_token(token: &str) -> ConnectionTokenResult<Self>;

    // Accessors
    pub fn raw_token(&self) -> &str;
    pub fn token_type(&self) -> ConnectionTokenType;
    pub fn role(&self) -> Role;
    pub fn parsed(&self) -> &Ucan;

    // Validation
    pub fn is_one_time(&self) -> bool;
    pub fn can_delegate_to(&self, target_role: Role) -> bool;
}
```

**Typed Wrappers**:
```rust
pub struct OneTimeConnectionToken(ConnectionToken);
pub struct OwnerConnectionToken(ConnectionToken);
pub struct NodeConnectionToken(ConnectionToken);
pub struct UserConnectionToken(ConnectionToken);
pub struct ViewerAuthToken(ConnectionToken);

// Each wrapper validates token_type AND role in constructor
impl OneTimeConnectionToken {
    pub fn from_token(token: &str) -> ConnectionTokenResult<Self> {
        let conn = ConnectionToken::from_token(token)?;
        if conn.token_type() != ConnectionTokenType::OneTimeConnection {
            return Err(ConnectionTokenError::InvalidTokenType(...));
        }
        Ok(Self(conn))
    }
    pub fn inner(&self) -> &ConnectionToken { &self.0 }
}

impl OwnerConnectionToken {
    pub fn from_token(token: &str) -> ConnectionTokenResult<Self> {
        let conn = ConnectionToken::from_token(token)?;
        if conn.token_type() != ConnectionTokenType::OwnerConnection {
            return Err(ConnectionTokenError::InvalidTokenType(...));
        }
        if conn.role() != Role::Owner {
            return Err(ConnectionTokenError::ValidationFailed(...));
        }
        Ok(Self(conn))
    }
    pub fn inner(&self) -> &ConnectionToken { &self.0 }
}
// Similar for NodeConnection, UserConnection, ViewerAuth...
```

### 3. ResourceUcan Domain Model

**File**: `osvauld_core/src/models/ucan_domain.rs`

```rust
/// Parsed resource UCAN with domain logic
pub struct ResourceUcan {
    raw_token: String,
    parsed: Ucan,
    role: Role,
    token_type: ResourceTokenType,
    capabilities: HashMap<String, Capability>,  // doc_name -> capability
    doc_metadata: HashMap<String, DocMetadata>,  // doc_name -> metadata
    sync_facts: SyncFacts,
    resource_actions: Option<Vec<ResourceAction>>,
    delegation_templates: HashMap<String, DelegationTemplate>,
    proof_chain: Vec<String>,  // Parent UCAN CIDs
}

impl ResourceUcan {
    /// Parse UCAN token and extract all domain information
    pub fn from_token(token: &str) -> ResourceUcanResult<Self>;

    // Accessors
    pub fn raw_token(&self) -> &str;
    pub fn role(&self) -> Role;
    pub fn token_type(&self) -> ResourceTokenType;
    pub fn proof_chain(&self) -> &[String];
    pub fn parsed(&self) -> &Ucan;

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

    // Resource/Folder ID extraction (returns Option since ID might not be present)
    pub fn resource_id(&self) -> Option<String>;
    pub fn folder_id(&self) -> Option<String>;
}
```

### 4. ResourceUcan Typed Wrappers

**File**: `osvauld_core/src/models/ucan_token.rs`

```rust
/// Resource owner token (full control)
pub struct ResourceOwnerToken {
    ucan: ResourceUcan,
}

impl ResourceOwnerToken {
    pub fn from_token(token: &str) -> ResourceUcanResult<Self>;
    pub fn ucan(&self) -> &ResourceUcan;
    pub fn resource_id(&self) -> String;
    pub fn get_delegation_template(&self, role: &str) -> ResourceUcanResult<&DelegationTemplate>;
}

/// Resource share token (for Node or User role)
pub struct ResourceShareToken {
    ucan: ResourceUcan,
}

impl ResourceShareToken {
    pub fn from_token(token: &str) -> ResourceUcanResult<Self>;
    pub fn ucan(&self) -> &ResourceUcan;
    pub fn role(&self) -> Role;
    pub fn resource_id(&self) -> String;
    pub fn get_delegation_template(&self, role: &str) -> ResourceUcanResult<&DelegationTemplate>;
}

/// Resource viewer token (for Viewer role)
pub struct ResourceViewerToken {
    ucan: ResourceUcan,
}

impl ResourceViewerToken {
    pub fn from_token(token: &str) -> ResourceUcanResult<Self>;
    pub fn ucan(&self) -> &ResourceUcan;
    pub fn resource_id(&self) -> String;
}

/// Folder owner token (full control)
pub struct FolderOwnerToken {
    ucan: ResourceUcan,
}

impl FolderOwnerToken {
    pub fn from_token(token: &str) -> ResourceUcanResult<Self>;
    pub fn ucan(&self) -> &ResourceUcan;
    pub fn folder_id(&self) -> String;
    pub fn get_delegation_template(&self, role: &str) -> ResourceUcanResult<&DelegationTemplate>;
}

/// Folder share token (for Node or User role)
pub struct FolderShareToken {
    ucan: ResourceUcan,
}

impl FolderShareToken {
    pub fn from_token(token: &str) -> ResourceUcanResult<Self>;
    pub fn ucan(&self) -> &ResourceUcan;
    pub fn role(&self) -> Role;
    pub fn folder_id(&self) -> String;
    pub fn get_delegation_template(&self, role: &str) -> ResourceUcanResult<&DelegationTemplate>;
}

/// Folder viewer token (for Viewer role)
pub struct FolderViewerToken {
    ucan: ResourceUcan,
}

impl FolderViewerToken {
    pub fn from_token(token: &str) -> ResourceUcanResult<Self>;
    pub fn ucan(&self) -> &ResourceUcan;
    pub fn folder_id(&self) -> String;
}
```

### 5. SyncContext with Pure Functions

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
   ↓ Frontend generates complete owner UCAN with all templates
   ↓ Frontend signs UCAN token
   ↓ Frontend sends signed UCAN to backend
   ↓ Backend stores UCAN as-is (no modification)
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
1. Create `capability.rs` with enums (Role, Capability, DocType, ConnectionTokenType, ResourceTokenType, etc.)
2. Create `connection_token.rs` domain model and typed wrappers
3. Create `ucan_domain.rs` domain model (handles both resource and folder UCANs)
4. Create `ucan_token.rs` typed wrappers (for all resource/folder token types)
5. Create `sync_context.rs` with pure functions
6. Update `mod.rs` to export all new modules

**Deliverable**: Core domain models for both connection and resource token hierarchies

**Status**: ✅ Phase 1 Complete

### Phase 2: Rewrite ucan_service.rs & Extend Domain Models
1. **Extend DelegationTemplate** in `ucan_domain.rs`:
   - Add `build_capabilities(id, resource_type)` method - Convert template to Vec<(String, String)>
   - Add `to_facts()` method - Convert template to JSON facts
2. **Complete rewrite of ucan_service.rs** (1559 → 756 lines, 51% reduction):
   - Organize into 4 modules: connection_tokens, resource_tokens, folder_tokens, validation
   - **Remove ALL hardcoded templates** - Extract from delegator UCANs only
   - Use typed tokens for all inputs/outputs
   - Follow consistent 6-step delegation pattern
3. **Fix UCAN 0.4.0 API compatibility** in domain models:
   - Update core/Cargo.toml: ucan = "0.4.0" (match crypto_utils)
   - Fix `.capabilities()` iteration (use `.iter()`)
   - Fix `.facts()` access (use `.as_ref()`)
   - Fix `.proofs()` handling (returns `Option<Vec<String>>`)

**Critical Pattern (All Delegation Functions)**:
```rust
// 1. Extract template (NEVER HARDCODE!)
let template = delegator_token.get_delegation_template("role")?;
// 2. Build capabilities (DATA-DRIVEN)
let capabilities = template.build_capabilities(id, "resource_type");
// 3. Convert to facts (GENERIC)
let mut facts = template.to_facts();
// 4-6. Get key, generate token, return typed wrapper
```

**Result**:
- Zero hardcoded templates ✅
- All typed tokens ✅
- Data-driven architecture ✅
- 48 compilation errors in services (expected for Phase 3) ✅

**Deliverable**: Rewritten ucan_service with typed tokens and template extraction

**Status**: ✅ Phase 2 Complete

### Phase 3: Consolidate Services - Delete Ductape
**Goal**: Consolidate all resource/folder/viewer operations into resource_service module with reusable core

**Status**: 🔄 **In Progress** (80% complete - module structure implemented, fixing compilation errors)

**Architecture Decision**: Split resource_service.rs (1255 lines) into 4-file module with reusable building blocks

**Module Structure** (Implemented):
```
services/src/resource_service/
├── mod.rs       (50 lines)    - Public API surface + re-exports ✅
├── core.rs      (330 lines)   - Reusable building blocks (NEW) ✅
├── crud.rs      (427 lines)   - Resource lifecycle operations ✅
└── sync.rs      (403 lines)   - CRDT sync + consolidated functions ✅
```

**Total Implemented**: 1,210 lines of new code

**Part A: Delete merge_service.rs** (602 lines)
- **Why**: Permission logic now in SyncContext, CRDT ops are simple 1-line calls
- **Move to resource_service/sync.rs**:
  - Direct CRDT operations (LoroDoc method calls)
  - State vector management
  - Asset handling
  - Use SyncContext for permission filtering
  - Refactor using core.rs helpers

**Part B: Delete website_service.rs** (271 lines)
- **Why**: Ductape that calls deleted ucan_service functions, no domain models
- **Move to resource_service/sync.rs**:
  - `prepare_folder_for_viewer()` - Use FolderViewerToken
  - `prepare_resource_for_viewer()` - Use SyncContext for filtering
  - `check_folder_and_user_status()` - Use typed tokens

**Part C: Refactor resource_service.rs into Module** ✅ **IMPLEMENTED**

**Implementation Decision**: Instead of generic trait, created concrete functions for each token type to avoid cross-crate trait implementation issues.

1. **Created resource_service/core.rs** (330 lines - NEW Reusable building blocks):

   **Pattern 1: Load-Decrypt-Parse** (eliminates 5+ duplications):
   ```rust
   /// Load resource by ResourceOwnerToken
   pub async fn load_and_decrypt_by_owner_token(...)

   /// Load resource by ResourceShareToken
   pub async fn load_and_decrypt_by_share_token(...)

   /// Load resource by ResourceViewerToken
   pub async fn load_and_decrypt_by_viewer_token(...)

   /// Load resource directly by ID (internal use)
   pub async fn load_and_decrypt_resource(resource_id, ...)

   /// Batch decrypt resources (for folder listings)
   pub async fn decrypt_resources(encrypted_resources, ...)
   ```

   **Pattern 2: Update-Encrypt-Save** (eliminates 3+ duplications):
   ```rust
   /// Encrypt resource data and save to database
   /// Uses key rotation for forward secrecy
   pub async fn encrypt_and_save_resource(
       resource: &Resource,
       user_public_key: &str,
       repo_ctx: Arc<RepositoryContext>,
   ) -> ServiceResult<()>
   ```

   **Pattern 3: State-Vector-Generation** (eliminates 4+ duplications):
   ```rust
   /// Build state vectors JSON for documents matching filter
   /// Generic filter function allows flexible document selection
   pub fn build_state_vectors_json(
       resource: &Resource,
       doc_filter: impl Fn(&str) -> bool,
   ) -> ServiceResult<String>
   ```

   **Pattern 4: Filter-and-Re-encrypt** (eliminates 3+ duplications):
   ```rust
   /// Filter resource documents and encrypt for peer
   /// Uses SyncContext for permission-based filtering
   pub async fn filter_and_encrypt_for_peer(
       resource: &Resource,
       sync_context: &SyncContext,
       peer_public_key: &str,
   ) -> ServiceResult<(Vec<u8>, Vec<u8>)>
   ```

   **Helper**:
   ```rust
   /// Create SyncContext from two raw UCAN tokens
   pub async fn create_sync_context(our_token, peer_token) -> ServiceResult<SyncContext>
   ```

2. **Created resource_service/crud.rs** (427 lines) ✅:

   **7 CRUD functions implemented**:
   - `create_resource()` - Uses `ucan_service::resource_tokens::issue_owner_token()`
   - `get_resource_by_id_direct()` - Uses `core::load_and_decrypt_resource()`
   - `get_resource_by_token()` - Uses `core::load_and_decrypt_by_share_token()`
   - `get_all_resources_metadata()` - Batch metadata fetching (no decryption)
   - `update_resource()` - Direct encryption/save (key rotation for forward secrecy)
   - `delete_resource()` - Soft delete
   - `share_resource()` - Uses `ucan_service::resource_tokens::delegate_to_node()` with typed tokens

   **Implementation Highlights**:
   ```rust
   // BEFORE (resource_service.rs lines 301-317):
   pub async fn get_resource_by_id_direct(resource_id: &str, ...) {
       let encrypted_resource = repo_ctx.resource_repo.find_by_id(resource_id).await?;
       let resource = decrypt_encrypted_resource(&encrypted_resource, crypto_utils).await?;
       Ok(resource)
   }

   // AFTER (crud.rs - uses core helper):
   pub async fn get_resource_by_id_direct(resource_id: &str, ...) {
       core::load_and_decrypt_resource(resource_id, repo_ctx, crypto_utils).await
   }
   ```

   **Typed Token Integration**:
   - `share_resource()` now parses owner token as `ResourceOwnerToken`
   - Delegates using typed `delegate_to_node()` returning `ResourceShareToken`
   - Eliminates string-based UCAN manipulation

3. **Created resource_service/sync.rs** (403 lines) ✅:

   **19 functions implemented** (mix of new implementations and delegations):

   **State Vector Operations** (3 functions):
   - `get_resource_state_vectors_by_ucan()` - Delegates to merge_service (transitional)
   - `generate_updates_for_peer()` - Delegates to merge_service (transitional)
   - `apply_peer_updates()` - Loads, applies via merge_service, saves with core

   **Resource Transfer** (6 functions):
   - `get_resource_ucans_for_sync()` - Get UCANs for folder sync
   - `prepare_resource_for_peer()` - Uses `core::filter_and_encrypt_for_peer()`
   - `prepare_resource_transfer()` - Orchestrates UCAN + data preparation
   - `save_resource_transfer()` - Stub (TODO)
   - `accept_resource_from_peer()` - Stub (TODO)
   - `prepare_resource_sync_request()` - Uses `core::build_state_vectors_json()`

   **Folder Sync Helpers** (3 functions):
   - `compare_asset_ids()` - Stub (TODO)
   - `get_resource_list_for_folder()` - Database query
   - Asset operations (2 stubs)

   **Website/Viewer Operations** (3 functions - delegates to website_service):
   - `check_user_and_folder_status()` - Transitional delegation
   - `get_folder_to_send()` - Transitional delegation
   - `prepare_resource_for_viewer()` - Transitional delegation

   **Re-exports from merge_service** (5 functions - transitional):
   - Uses `pub use crate::merge_service::...` for backwards compatibility

   **Implementation Strategy**:
   - Core functions use new patterns (SyncContext, core.rs helpers)
   - Transitional functions delegate to old services (merge_service, website_service)
   - Allows incremental refactoring without breaking existing code

4. **Created resource_service/mod.rs** (50 lines) ✅:
   ```rust
   mod core;  // Private - internal use only
   mod crud;  // Public - re-exported
   mod sync;  // Public - re-exported

   pub use crud::*;  // Export all CRUD functions
   pub use sync::*;  // Export all sync functions
   ```

   **Design Decision**: core.rs is private module - provides reusable building blocks for crud.rs and sync.rs but not exposed to external callers.

**Part D: Extended ucan_service.rs** ✅ **IMPLEMENTED**

Added backwards-compatible utility functions needed by old code:

1. **utilities module** (157 lines):
   - `extract_resource_id(ucan_token)` - Extract resource ID from UCAN capabilities
   - `extract_folder_id_from_viewer_token(viewer_ucan, domain)` - Extract folder ID for viewers
   - `extract_folder_id_with_add_resources(ucan, domain)` - Validate add_resources capability
   - `extract_doc_capabilities(ucan)` - Get document name → capability map
   - `extract_facts(ucan)` - Get raw UCAN facts JSON
   - `extract_folder_capabilities(ucan)` - Get folder capability list

2. **connection_tokens additions** (44 lines):
   - `issue_one_time_connection_token()` - Wrapper for issue_one_time()
   - `generate_public_folder_view_token()` - Wrapper for issue_viewer_auth()

3. **folder_tokens additions** (47 lines):
   - `issue_folder_owner_token()` - Create self-signed folder owner token

4. **validation additions** (54 lines):
   - `validate_folder_access_for_resource()` - Verify folder token matches folder_id
   - `validate_ucan_structure()` - Generic UCAN validation

5. **Re-exports** (added at top level):
   ```rust
   pub use connection_tokens::{issue_one_time_connection_token, generate_public_folder_view_token};
   pub use folder_tokens::issue_folder_owner_token;
   pub use utilities::{extract_resource_id, extract_folder_id_from_viewer_token, ...};
   pub use validation::{validate_folder_access_for_resource, validate_ucan_structure};
   ```

**Design Decision**: Keep old extraction functions as wrappers around new typed tokens. Allows gradual migration without breaking existing code.

**Part E: Update Callers** ⏳ **PENDING**
- Update folder_service.rs to use new signatures
- Update share_service.rs to use typed tokens
- Update website_handler.rs to call resource_service (not website_service)
- Update resource_sync.rs to use typed tokens
- Import changes:
  ```rust
  // OLD:
  use crate::merge_service::filter_documents_to_send;
  use crate::website_service::prepare_resource_for_viewer;

  // NEW:
  use crate::resource_service::{filter_and_encrypt_for_peer, prepare_resource_for_viewer};
  ```

**Files Status**:
- ✅ services/src/resource_service.rs → RENAMED to resource_service_OLD.rs (backup)
- ⏳ services/src/merge_service.rs (602 lines) - KEPT for transitional delegations
- ⏳ services/src/website_service.rs (271 lines) - KEPT for transitional delegations

**Files Created**:
- ✅ services/src/resource_service/mod.rs (50 lines)
- ✅ services/src/resource_service/core.rs (330 lines) - NEW REUSABLE LAYER
- ✅ services/src/resource_service/crud.rs (427 lines)
- ✅ services/src/resource_service/sync.rs (403 lines)

**Code Metrics (Actual)**:
- **Before**: 2128 lines (resource_service 1255 + merge_service 602 + website_service 271)
- **After**: 1210 lines (mod 50 + core 330 + crud 427 + sync 403)
- **New code written**: 1210 lines (all new, refactored implementations)
- **Old code retained**: merge_service (602) + website_service (271) = 873 lines (transitional)
- **Net reduction target**: ~500 lines once transitional code removed

**Implementation Status**:
- All business logic in resource_service module ✅
- Reusable core functions (DRY) ✅
- Transitional delegations to old services ⏳ (allows incremental migration)
- Typed tokens used in new code ✅
- Clean architecture established ✅
- Testable building blocks ✅

**Remaining Work**:
- Fix 76 compilation errors (missing functions, type mismatches, error variants)
- Remove transitional delegations (replace with full implementations)
- Delete merge_service.rs and website_service.rs
- Update lib.rs exports
- Verify build with 0 errors

**Current Compilation Status**:
- services package: 76 errors (down from 200+ at start)
- network package: 0 errors ✅

**Deliverable**: 4-file resource_service module with reusable core. Transitional delegation strategy allows gradual migration without breaking changes.

### Phase 4: Update P2P Orchestration Layer
**Goal**: Update P2P message handlers to use typed tokens and resource_service

**Part A: Update resource_sync.rs**
1. Parse incoming UCANs as typed tokens:
   ```rust
   // OLD: String tokens
   let peer_ucan_token = &request.ucan_token;

   // NEW: Typed tokens
   let peer_token = ResourceShareToken::from_token(&request.ucan_token)?;
   let our_token = ResourceShareToken::from_token(&our_ucan)?;
   ```

2. Create SyncContext for permission checks:
   ```rust
   let sync_context = SyncContext::from_ucans(
       our_token.ucan().clone(),
       peer_token.ucan().clone(),
   );
   ```

3. Pass typed tokens to resource_service (no more merge_service calls)

**Part B: Update folder_sync.rs**
- Use FolderShareToken, FolderOwnerToken
- Pass typed tokens to folder_service
- Remove any merge_service calls

**Part C: Update website_handler.rs**
- Parse ViewerAuth tokens as typed ViewerAuthToken
- Use FolderViewerToken, ResourceViewerToken
- Call resource_service (not website_service)
- Update handshake flow to use typed tokens

**Result**:
- All P2P handlers use typed tokens ✅
- No merge_service calls ✅
- No website_service calls ✅
- Clean orchestration → service layer calls ✅

**Deliverable**: P2P layer uses typed tokens and updated service calls

### Phase 5: Update Frontend
1. Update `permissions.ts` capability names (collaborator, viewer, submitter)
2. Update fact names (local_only, no_incoming_updates, send_full_snapshot)
3. Add `token_type` field to all templates
4. Clean up structure: use `sync` and `delegation` sections
5. Add `doc_metadata` with type and constraints
6. Update any frontend UCAN parsing logic

**Deliverable**: Frontend uses new permission keywords

### Phase 6: Integration Testing & Validation
**Manual test cases:**
1. Owner ↔ Node sync (all docs bidirectional)
2. Node ↔ Viewer sync (mixed permissions)
3. Viewer cannot send content_doc/template_doc updates ✓ (fixes current bug)
4. Viewer can send collaborative_doc updates ✓
5. Viewer can send shared_assets updates ✓
6. Viewer cannot send submissions_doc updates, Node cannot send to Viewer ✓
7. user_content_doc never syncs for anyone ✓
8. Type safety: Cannot pass wrong token type (compile-time)
9. Permission validation: Tokens validated at parse time

**Deliverable**: Integration tests passing, bug fixed

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

**This is a BREAKING CHANGE - No backward compatibility**

1. **Create new modules**: Add domain models alongside existing code
2. **Update backend**: Remove all hardcoded templates and structure generation
3. **Update frontend**: Frontend now generates complete UCAN tokens
4. **Breaking API changes**: Function signatures change to use typed tokens
5. **All-at-once migration**: Frontend and backend must be updated together
6. **Remove old code**: Delete deprecated UCAN generation code immediately

**Why breaking?**
- Fundamental shift: Backend no longer generates UCAN structure
- Old tokens (with hardcoded templates) incompatible with new system
- Clean break is simpler than maintaining dual systems

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

- Phase 1 (Core models): ✅ Complete (2 days actual)
- Phase 2 (Rewrite ucan_service + fix UCAN 0.4.0): ✅ Complete (1 day actual)
- Phase 3 (Consolidate services, delete merge/website): 2-3 days
- Phase 4 (Update P2P layer): 1-2 days
- Phase 5 (Frontend): 1 day
- Phase 6 (Testing): 2-3 days

**Total**: 9-13 days (3 days complete, 6-10 days remaining)

## References

- Current bug logs: Viewer sending content_doc/template_doc updates
- SYNC_PROTOCOL_DESIGN.md: Sync protocol specification
- permissions.ts: Frontend permission definitions
- ucan_service.rs: Current token generation
- merge_service.rs: Current sync logic
