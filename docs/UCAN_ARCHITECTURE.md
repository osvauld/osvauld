# UCAN Architecture - Osvauld Permission System

**Last Updated:** 2025-01-15
**Status:** Production Ready

---

## Overview

Osvauld uses a three-tier UCAN token architecture for authorization:
1. **Connection Tokens** - Device-to-device handshake and authentication
2. **Folder Tokens** - Folder-level access control
3. **Resource Tokens** - Resource-level access control

This separation provides compile-time safety, clear context boundaries, and explicit modeling of all token usage patterns.

---

## Core Principles

### 1. Frontend is Source of Truth
- ✅ Frontend (`permissions.ts`) defines ALL permission templates
- ✅ Frontend sends complete UCAN structure (including templates) to backend
- ✅ Backend stores UCAN exactly as received (no modification)
- ✅ Backend extracts templates when delegating
- ✅ Backend NEVER hardcodes permission structures
- ✅ To change permissions, update `permissions.ts` only

### 2. Data-Driven Backend
- ✅ Backend code is generic (works with any document names)
- ✅ Document names are data, not hardcoded
- ✅ Adding new documents = frontend-only change
- ✅ Backend derives behavior from UCAN content

### 3. Type Safety
- ✅ Typed token wrappers prevent wrong token usage
- ✅ Compile-time errors for type mismatches
- ✅ Self-documenting function signatures
- ✅ Cannot pass Connection token where Resource token expected

---

## Token Type Hierarchy

### Connection Tokens (Device Handshake)

**Purpose:** Authenticate devices and establish P2P connections

```rust
pub enum ConnectionTokenType {
    OneTimeConnection,   // Initial pairing (single-use)
    OwnerConnection,     // Owner device sync (long-lived)
    NodeConnection,      // Node device sync (long-lived)
    UserConnection,      // User P2P collaboration (long-lived)
    ViewerAuth,          // Viewer initial auth (short-lived)
}
```

**Typed Wrappers:**
```rust
pub struct OneTimeConnectionToken(ConnectionToken);
pub struct OwnerConnectionToken(ConnectionToken);
pub struct NodeConnectionToken(ConnectionToken);
pub struct UserConnectionToken(ConnectionToken);
pub struct ViewerAuthToken(ConnectionToken);
```

**Usage:**
```rust
// Parse connection token (validates type and role)
let owner_conn = OwnerConnectionToken::from_token(&token)?;

// Access inner domain model
let role = owner_conn.inner().role();  // Role::Owner

// Check delegation capability
if owner_conn.inner().can_delegate_to(Role::Node) {
    // Issue node token
}
```

**File:** `osvauld_core/src/models/connection_token.rs`

---

### Folder Tokens (Folder Access Control)

**Purpose:** Control access to folders and their contents

```rust
pub enum ResourceTokenType {
    FolderOwner,    // Full folder control
    FolderShare,    // Node/User folder access
    FolderViewer,   // Viewer folder access (read-only)
}
```

**Typed Wrappers:**
```rust
pub struct FolderOwnerToken {
    ucan: ResourceUcan,
}

impl FolderOwnerToken {
    pub fn from_token(token: &str) -> ResourceUcanResult<Self>;
    pub fn ucan(&self) -> &ResourceUcan;
    pub fn folder_id(&self) -> String;
    pub fn get_delegation_template(&self, role: &str) -> ResourceUcanResult<&DelegationTemplate>;
}
```

**Delegation Chain:**
```
FolderOwnerToken (Owner)
  ↓ delegate_to_node()
FolderShareToken (Node)
  ↓ delegate_to_user()
FolderShareToken (User)
  ↓ delegate_to_viewer()
FolderViewerToken (Viewer)
```

**UCAN Structure (FolderOwner):**
```json
{
  "aud": "<recipient_pub_key>",
  "iss": "<issuer_pub_key>",
  "cap": {
    "sthalam:folder:xyz": {
      "add_folder": [{}],
      "crud/read": [{}],
      "crud/update": [{}],
      "crud/delete": [{}],
      "share_folder": [{}]
    }
  },
  "fct": {
    "token_type": "folder_owner",
    "role": "owner",
    "delegation": {
      "node": {
        "capabilities": {
          "add_folder": "add_folder",
          "crud/read": "crud/read",
          "share_folder": "share_folder"
        }
      },
      "viewer": {
        "capabilities": {
          "crud/read": "crud/read",
          "request_resources": "request_resources"
        }
      }
    }
  }
}
```

**File:** `osvauld_core/src/models/ucan_token.rs`

---

### Resource Tokens (Resource Access Control)

**Purpose:** Control access to individual resources and their documents

```rust
pub enum ResourceTokenType {
    ResourceOwner,    // Full resource control
    ResourceShare,    // Node/User resource access
    ResourceViewer,   // Viewer resource access
}
```

**Typed Wrappers:**
```rust
pub struct ResourceOwnerToken {
    ucan: ResourceUcan,
}

impl ResourceOwnerToken {
    pub fn from_token(token: &str) -> ResourceUcanResult<Self>;
    pub fn ucan(&self) -> &ResourceUcan;
    pub fn resource_id(&self) -> String;
    pub fn get_delegation_template(&self, role: &str) -> ResourceUcanResult<&DelegationTemplate>;
}
```

**Delegation Chain:**
```
ResourceOwnerToken (Owner)
  ↓ delegate_to_node()
ResourceShareToken (Node)
  ↓ delegate_to_user()
ResourceShareToken (User)
  ↓ delegate_to_viewer()
ResourceViewerToken (Viewer)
```

**UCAN Structure (ResourceOwner):**
```json
{
  "aud": "<recipient_pub_key>",
  "iss": "<issuer_pub_key>",
  "cap": {
    "sthalam:resource:abc:collaborative_doc": {"collaborator": [{}]},
    "sthalam:resource:abc:content_doc": {"collaborator": [{}]},
    "sthalam:resource:abc:template_doc": {"collaborator": [{}]},
    "sthalam:resource:abc:submissions_doc": {"collaborator": [{}]},
    "sthalam:resource:abc:static_assets": {"collaborator": [{}]},
    "sthalam:resource:abc:user_content_doc": {"collaborator": [{}]}
  },
  "fct": {
    "token_type": "resource_owner",
    "role": "owner",
    "sync": {
      "local_only": ["user_content_doc"]
    },
    "doc_metadata": {
      "collaborative_doc": {"type": "crdt"},
      "content_doc": {"type": "crdt"},
      "template_doc": {"type": "crdt"},
      "submissions_doc": {"type": "crdt"},
      "user_content_doc": {"type": "crdt"},
      "static_assets": {"type": "asset"}
    },
    "delegation": {
      "node": {
        "capabilities": {
          "collaborative_doc": "collaborator",
          "content_doc": "collaborator",
          "template_doc": "collaborator",
          "submissions_doc": "collaborator",
          "static_assets": "collaborator",
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

---

## Domain Models

### ResourceUcan (Rich Query API)

**File:** `osvauld_core/src/models/ucan_domain.rs`

```rust
pub struct ResourceUcan {
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

impl ResourceUcan {
    // Parsing
    pub fn from_token(token: &str) -> ResourceUcanResult<Self>;

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

**Domain Types:**
```rust
pub enum Capability {
    Collaborator,  // Bidirectional CRDT sync (was: crud/merge)
    Viewer,        // Receive-only (was: crud/readonly)
    Submitter,     // Send full snapshots (was: crud/submit)
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
    pub local_only: Vec<String>,            // Don't send to peer
    pub no_incoming_updates: Vec<String>,   // Don't accept from peer
    pub send_full_snapshot: Vec<String>,    // Send full doc (not incremental)
}
```

---

## Capability Format

### Capability Resource Pattern

**Format:** `{domain}:{resource_type}:{id}` or `{domain}:{resource_type}:*`

**Examples:**
```
Connection capabilities:
- sthalam:user:*:connect      (can connect as user)
- sthalam:user:*:share         (can share with users)
- sthalam:folder:*:add_folder  (can add folders)

Folder capabilities:
- sthalam:folder:xyz:crud/read      (can read folder xyz)
- sthalam:folder:xyz:add_resources  (can add resources to folder)
- sthalam:folder:xyz:share_folder   (can share folder)

Resource capabilities:
- sthalam:resource:abc:collaborative_doc:collaborator  (full access)
- sthalam:resource:abc:content_doc:viewer             (read-only)
- sthalam:resource:abc:submissions_doc:submitter       (submit-only)
```

### Validation Pattern

**Function:** `crypto_utils::ucan_utils::check_capability(ucan, resource, ability)`

**Example:**
```rust
// Check if peer can add folders
let folder_resource = format!("{}:folder:*", domain);
crypto_utils::ucan_utils::check_capability(
    peer_token.parsed(),
    &folder_resource,   // "sthalam:folder:*"
    "add_folder",       // ability
)?;

// Check if user has collaborator access to collaborative_doc
let doc_resource = format!("{}:resource:{}:collaborative_doc", domain, resource_id);
crypto_utils::ucan_utils::check_capability(
    user_token.parsed(),
    &doc_resource,      // "sthalam:resource:abc:collaborative_doc"
    "collaborator",     // capability
)?;
```

**Wildcard Matching:**
- `sthalam:folder:*` matches `sthalam:folder:xyz`, `sthalam:folder:abc`, etc.
- `sthalam:resource:*:content_doc` matches all resources' content_doc
- Exact match: `sthalam:resource:abc:content_doc` matches only that specific doc

---

## Template Extraction Pattern

### Delegation Template Structure

**File:** `osvauld_core/src/models/ucan_domain.rs`

```rust
pub struct DelegationTemplate {
    pub capabilities: HashMap<String, String>,  // doc_name -> capability
    pub sync: Option<SyncTemplate>,
    pub doc_metadata: HashMap<String, DocMetadata>,
}

pub struct SyncTemplate {
    pub local_only: Vec<String>,
    pub no_incoming_updates: Vec<String>,
    pub send_full_snapshot: Vec<String>,
}

impl DelegationTemplate {
    /// Build capabilities list for UCAN creation
    /// Converts template + resource ID into Vec<(resource, ability)>
    pub fn build_capabilities(&self, id: &str, resource_type: &str) -> Vec<(String, String)> {
        self.capabilities.iter().map(|(doc_name, capability)| {
            let resource = format!("sthalam:{}:{}:{}", resource_type, id, doc_name);
            (resource, capability.clone())
        }).collect()
    }

    /// Convert template to UCAN facts JSON
    pub fn to_facts(&self) -> serde_json::Value {
        json!({
            "sync": {
                "local_only": self.sync.as_ref().map(|s| &s.local_only).unwrap_or(&vec![]),
                "no_incoming_updates": self.sync.as_ref().map(|s| &s.no_incoming_updates).unwrap_or(&vec![]),
                "send_full_snapshot": self.sync.as_ref().map(|s| &s.send_full_snapshot).unwrap_or(&vec![]),
            },
            "doc_metadata": self.doc_metadata,
        })
    }
}
```

### Delegation Flow (NEVER HARDCODE!)

**Pattern used in all delegation functions:**

```rust
// 1. Extract template from delegator UCAN (NEVER HARDCODE!)
let template = delegator_token.ucan().get_delegation_template("viewer")
    .ok_or_else(|| UcanServiceError::MissingTemplate("viewer"))?;

// 2. Build capabilities (DATA-DRIVEN)
let capabilities = template.build_capabilities(&resource_id, "resource");

// 3. Convert to facts (GENERIC)
let mut facts = template.to_facts();
facts["role"] = json!("viewer");
facts["token_type"] = json!("resource_share");

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

**Key Points:**
- ✅ Template extracted from delegator's UCAN (frontend-defined)
- ✅ No hardcoded document names or capabilities
- ✅ Generic code works with any document structure
- ✅ Frontend changes don't require backend code changes

---

## Usage Examples

### Example 1: Folder Publishing (Owner → Node)

```rust
// 1. Parse peer connection token (validates type)
let peer_token = ConnectionToken::from_token(peer_connection_token)?;

// 2. Check capability (UCAN-first)
let folder_resource = format!("{}:folder:*", domain);
crypto_utils::ucan_utils::check_capability(
    peer_token.parsed(),
    &folder_resource,
    "add_folder",
)?;

// 3. Validate folder UCAN structure
ucan_service::validate_ucan_structure(&folder_share_record.ucan_token).await?;

// 4. Save folder and share record
repo_ctx.folder_repo.save_folder_with_share_record(folder, folder_share_record).await?;
```

### Example 2: Resource Delegation (Owner → Node)

```rust
// 1. Parse owner token (validates type and role)
let owner_token = ResourceOwnerToken::from_token(&owner_ucan)?;

// 2. Extract template from owner's UCAN (NEVER HARDCODE!)
let template = owner_token.ucan().get_delegation_template("node")?;

// 3. Build capabilities (data-driven)
let capabilities = template.build_capabilities(&resource_id, "resource");

// 4. Create facts (generic)
let mut facts = template.to_facts();
facts["role"] = json!("node");
facts["token_type"] = json!("resource_share");

// 5. Get node's public key
let node_pub_key = repo_ctx.user_repo.get_user_by_id(node_user_id).await?.ucan_pub_key;

// 6. Generate UCAN
let ucan_token = crypto_utils.write().await.generate_ucan(
    &node_pub_key,
    capabilities,
    facts,
    None,
)?;

// 7. Return typed token
Ok(ResourceShareToken::from_token(&ucan_token)?)
```

### Example 3: Capability Check During Sync

```rust
// 1. Parse both tokens
let our_ucan = ResourceUcan::from_token(&our_token)?;
let peer_ucan = ResourceUcan::from_token(&peer_token)?;

// 2. Check our capability (can WE write?)
if our_ucan.get_capability("content_doc") != Some(Capability::Collaborator) {
    // We have viewer access, can't send updates
    return Ok(SyncDecision::DontSend);
}

// 3. Check peer's no_incoming_updates (do THEY accept?)
if peer_ucan.has_no_incoming_updates("content_doc") {
    // Peer explicitly rejects updates for this doc
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

### Permission Template (permissions.ts)

```typescript
export const RESOURCE_TEMPLATE = {
  owner_template: {
    capabilities: {
      "template_doc": "collaborator",
      "content_doc": "collaborator",
      "user_content_doc": "collaborator",
      "collaborative_doc": "collaborator",
      "submissions_doc": "collaborator",
      "static_assets": "collaborator",
    },
    doc_types: {
      "static_assets": "asset",
      "template_doc": "crdt",
      "content_doc": "crdt",
      "user_content_doc": "crdt",
      "collaborative_doc": "crdt",
      "submissions_doc": "crdt",
    },
    sync: {
      local_only: ["user_content_doc"],
    },
    delegation: {
      node: {
        capabilities: {
          "template_doc": "collaborator",
          "content_doc": "collaborator",
          "user_content_doc": "collaborator",
          "collaborative_doc": "collaborator",
          "submissions_doc": "collaborator",
          "static_assets": "collaborator",
        },
        sync: {
          local_only: ["user_content_doc"],
        },
      },
      viewer: {
        capabilities: {
          "template_doc": "viewer",
          "content_doc": "viewer",
          "collaborative_doc": "collaborator",
          "submissions_doc": "submitter",
          "static_assets": "viewer",
        },
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

## Summary

**UCAN Architecture Benefits:**
- ✅ Type safety (typed wrappers prevent wrong token usage)
- ✅ Frontend is source of truth (backend never hardcodes)
- ✅ Data-driven (document names are data, not code)
- ✅ Compile-time errors (can't pass wrong token type)
- ✅ Self-documenting (function signatures show token types)
- ✅ Flexible (add new documents without backend changes)

**Files:**
- `osvauld_core/src/models/capability.rs` - Domain types
- `osvauld_core/src/models/connection_token.rs` - Connection tokens
- `osvauld_core/src/models/ucan_domain.rs` - ResourceUcan domain model
- `osvauld_core/src/models/ucan_token.rs` - Typed wrappers (11 types)
- `services/src/ucan_service.rs` - Token generation (756 lines, zero hardcoded templates)
- `sthalam/frontend/desktop/src/config/permissions.ts` - Permission templates (frontend)

**Status:** ✅ **Production Ready**
