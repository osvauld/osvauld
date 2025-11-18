# Permits - Core Architecture

**Status**: Current (reflects implemented system)
**Last Updated**: 2025-11-18

---

## Table of Contents

1. [What are Permits?](#what-are-permits)
2. [Identity & Keys](#identity--keys)
3. [Bearer Token Architecture](#bearer-token-architecture)
4. [Permit Structure](#permit-structure)
5. [Permission Templates](#permission-templates)
6. [Document-Level Permissions](#document-level-permissions)
7. [The Three Capabilities](#the-three-capabilities)
8. [How Gurkha Interprets Permits](#how-gurkha-interprets-permits)
9. [Why Data-Driven?](#why-data-driven)

---

## What are Permits?

**Permits are Osvauld's extension of UCAN tokens** - a facts-only, data-driven authorization system where everything revolves around cryptographic bearer tokens.

### Key Principles

1. **Bearer tokens**: Possession = identity and authorization
2. **Facts-only**: No capability URIs, pure JSON structure
3. **Data-driven**: No hardcoded roles, Gurkha interprets dynamically
4. **Document-level**: Permissions per-document, not per-user
5. **Decentralized**: No backend, all cryptographic verification

### Why "Permits"?

We use "Permits" to distinguish our implementation from generic UCAN:
- **Permits = UCAN + facts-only architecture + CEL integration**
- UCAN is the standard, Permits is our opinionated implementation
- All authorization flows through Permits - they're the center of the system

---

## Identity & Keys

Osvauld uses **three separate keys** for different purposes:

### 1. PGP Key (Main Identity)
- **Purpose**: Encryption and decryption of data
- **Storage**: User-controlled, never leaves device
- **Identity**: This IS the user

**Code**: Frontend generates at signup
**See**: `sthalam/frontend/desktop/src/lib/crypto/pgp.ts`

### 2. UCAN Signing Key (Permit Signatures)
- **Purpose**: Signs and verifies Permits
- **Derivation**: Deterministically derived from PGP key
- **Format**: Ed25519 keypair → DID (Decentralized Identifier)
- **DID format**: `did:key:z{base58_encoded_pubkey}`

**Code**: Derived during user initialization
**See**: `gurkha/src/crypto.rs:20-28` (Ed25519KeyMaterial)

### 3. Device Key (P2P Connections)
- **Purpose**: Iroh P2P network connections
- **Usage**: Device discovery and routing
- **Separation**: P2P layer (Iroh) is separate from authorization layer (Permits)

**device_id** resolves to device public key for P2P connections, but authorization happens via Permits.

### No Backend Required

All identity verification is **cryptographic**:
- DIDs are derived from public keys
- Permits are self-signed or chain back to root identity
- Proof chains enable verification without central authority

---

## Bearer Token Architecture

### What is a Bearer Token?

**Anyone holding a Permit token is authorized to use it.**

There is **no central database** checking "does this user have permission?" Instead:
1. User presents Permit
2. System verifies cryptographic signature
3. System reads facts from Permit
4. System grants access based on facts

### Example: One-Time Tokens

When adding a hosting node, the node creates a **one-time bearer token** with `operations.own = true`:

```json
{
  "operations": {
    "own": "allow"
  }
}
```

**Anyone holding this token is considered the owner** for that handshake session.

**Code**: See node handshake flow
**See**: `kunki/src/main.rs` (node's one-time token generation)

### Trust Establishment

Trust is established through:
1. **Cryptographic signatures** (did this key sign this Permit?)
2. **Proof chains** (was this Permit delegated from a trusted root?)
3. **Facts validation** (does this Permit grant the requested capability?)

No server checks, no role tables - **pure cryptography**.

---

## Permit Structure

Permits use a **facts-only** architecture. All authorization data lives in the `fct` (facts) field of the UCAN token.

### Example: Folder Owner Permit

```json
{
  "iss": "did:key:z6Mkf...",  // Issuer (self-signed)
  "aud": "did:key:z6Mkf...",  // Audience (same = self-issued)
  "exp": 1234567890,          // Expiry
  "fct": {
    "token_type": "folder_owner",
    "relationship": "owner",
    "folder_id": "550e8400-e29b-41d4-a716-446655440000",
    "user_id": "base64_pubkey...",
    "operations": {
      "own": "allow",
      "get_share_link": "allow",
      "add_resources": "allow",
      "share_folder": "allow"
    },
    "delegation": {
      "node": { /* template for delegating to node */ },
      "viewer": { /* template for delegating to viewer */ }
    }
  },
  "prf": []  // Proof chain (empty for root tokens)
}
```

### Key Fields

**Standard UCAN fields**:
- `iss`: Issuer DID
- `aud`: Audience DID (who this Permit is for)
- `exp`: Expiration timestamp
- `prf`: Proof chain (array of parent Permit CIDs)

**Facts field** (`fct`):
- `token_type`: Descriptive label (not used for logic!)
- `relationship`: Descriptive relationship (not used for logic!)
- `operations`: What operations are allowed
- `documents`: Per-document capabilities (for resources)
- `delegation`: Templates for further delegation
- `sync`: Sync behavior (local_only, no_incoming_updates, etc.)
- `cel_rules`: CEL expressions for dynamic validation (future)

**IMPORTANT**: `token_type` and `relationship` are **descriptive only** - Gurkha does NOT use these for authorization logic. All decisions come from `operations`, `documents`, and other facts.

**Code**: Permit structure and parsing
**See**: `gurkha/src/parser.rs:48-100` (UcanCore and Permit structs)

---

## Permission Templates

Permission templates define **what can be delegated** from a Permit.

### Where Templates Live

**Frontend defines templates**: All permission templates are defined in frontend TypeScript, not backend code.

**See**: `sthalam/frontend/desktop/src/config/permissions.ts`

### Folder Template Example

```typescript
export const FOLDER_TEMPLATE = {
  owner_template: {
    operations: {
      "own": "allow",
      "get_share_link": "allow",
      "add_resources": "allow",
      "share_folder": "allow",
    },
    delegation: {
      node: {
        token_type: "folder_share",
        operations: {
          "get_share_link": "allow",
          "add_resources": "allow",
          "share_folder": "allow",
        },
        auth_capabilities: {
          can_connect: true,
          persist_share: true,  // Node shares are persistent
          can_delegate: false,
          sync_enabled: true,
        },
        relationship: "node",
      },
      viewer: {
        token_type: "folder_viewer",
        operations: {
          "request_resources": "allow",
          "get_share_link": "allow",
        },
        auth_capabilities: {
          can_connect: true,
          persist_share: false,  // Viewer shares are ephemeral!
          can_delegate: false,
          sync_enabled: false,
        },
        relationship: "viewer",
      },
    },
  },
};
```

### How Templates Work

1. **Owner creates folder** → Frontend sends template JSON to backend
2. **Backend issues Permit** → Template is embedded in `fct.delegation` field
3. **Owner delegates to node** → Gurkha extracts `delegation.node` template
4. **Gurkha creates new Permit** → Template becomes the facts of node's Permit
5. **Node delegates to viewer** → Gurkha extracts `delegation.viewer` template
6. **Gurkha creates viewer Permit** → Viewer gets filtered capabilities

**Code**: Template processing
**See**:
- `gurkha/src/decision.rs:459-500` (decide_folder_owner_token - embeds template)
- `gurkha/src/parser.rs:400-500` (extract_template_from_token - extracts for delegation)
- `gurkha/src/service.rs:301-326` (issue_folder_owner_token - orchestrates)

### Why Templates in Permits?

**Templates enable autonomous delegation** - the hosting node can delegate to viewers without owner involvement:

1. Owner gives node a Permit with `delegation.viewer` template
2. Node can create viewer Permits using that template
3. **No owner needed** for viewer onboarding
4. Decentralized, scalable authorization

---

## Document-Level Permissions

Osvauld uses **document-level permissions**, not user-level roles.

### Why Not User Roles?

**Problem**: Same user needs different permissions on different documents.

**Example** (Sthalam website/app):
```
Resource has 5 documents:
- template_doc: Owner edits, viewer reads
- content_doc: Owner edits, viewer reads
- collaborative_doc: EVERYONE can edit (public comments!)
- submissions_doc: Viewer submits, owner reads submissions
- user_content_doc: Private to each user
```

**Same viewer** has:
- Read-only on `template_doc`
- Append-only on `collaborative_doc`
- Write to own namespace in `submissions_doc`
- Full control of `user_content_doc`

**User roles can't express this** - you need per-document capabilities.

### Resource Permit with Document Permissions

```json
{
  "fct": {
    "token_type": "resource_owner",
    "resource_id": "...",
    "operations": {
      "own": "allow",
      "share": "allow",
      "create": "allow"
    },
    "documents": {
      "template_doc": {
        "capability": "collaborator",
        "type": "loro"
      },
      "content_doc": {
        "capability": "collaborator",
        "type": "loro"
      },
      "collaborative_doc": {
        "capability": "collaborator",
        "type": "loro"
      },
      "submissions_doc": {
        "capability": "collaborator",
        "type": "loro"
      },
      "user_content_doc": {
        "capability": "collaborator",
        "type": "loro"
      }
    },
    "delegation": {
      "node": {
        "documents": {
          "template_doc": { "capability": "collaborator" },
          "content_doc": { "capability": "collaborator" },
          "collaborative_doc": { "capability": "collaborator" },
          "submissions_doc": { "capability": "collaborator" },
          "user_content_doc": { "capability": "collaborator" }
        }
      },
      "viewer": {
        "documents": {
          "template_doc": { "capability": "viewer" },
          "content_doc": { "capability": "viewer" },
          "collaborative_doc": { "capability": "collaborator" },
          // submissions_doc and user_content_doc FILTERED OUT for viewers
        }
      }
    }
  }
}
```

### Document Names Don't Matter

**CRITICAL**: Document names are **arbitrary** and **data-driven**.

You can name documents anything: `"foo"`, `"bar"`, `"my_custom_doc_123"`.

Gurkha interprets permissions **dynamically** based on the `documents` facts structure, not hardcoded document names.

**Code**: See how Gurkha reads document permissions
**See**: `gurkha/src/parser.rs:615-625` (capability extraction from facts.documents)

---

## The Three Capabilities

There are **three document capabilities** in Osvauld:

### 1. Viewer (Read-Only)
```json
{ "capability": "viewer" }
```

**Behavior**:
- Can **receive** updates via CRDT sync
- **Cannot** send updates back
- One-way: source → viewer

**Use case**: Viewing website content, reading published data

**Sync**: Receives shallow snapshots or incremental updates

**Code**: `gurkha/src/types.rs:16-44` (Capability enum and can_write() method)

### 2. Submitter (Append-Only)
```json
{ "capability": "submitter" }
```

**Behavior**:
- Can **write** to isolated namespace
- Writes go to separate document section
- **Append-only merge** - can't modify others' submissions
- One-way: submitter → host

**Use case**: Form submissions, user-generated content isolation

**Sync**: Sends full snapshots of isolated writes

**Code**:
- `gurkha/src/types.rs:16-44` (Capability::Submitter)
- `gurkha/src/decision.rs:595-605` (send_full_snapshot check for submitters)

### 3. Collaborator (Bidirectional CRDT)
```json
{ "capability": "collaborator" }
```

**Behavior**:
- **Full CRDT synchronization** (read + write)
- Can send and receive updates
- Changes merge automatically via Loro CRDT
- Bidirectional: peer ↔ peer

**Use case**: Collaborative editing, public comments, shared documents

**Sync**: Bidirectional shallow snapshot or incremental updates

**Code**: `gurkha/src/types.rs:16-44` (can_sync_bidirectional() returns true)

### Capability Decision Tree

When determining sync behavior, Gurkha checks:

```
can_write() ?
├─ No  → Viewer (read-only)
└─ Yes → can_sync_bidirectional() ?
         ├─ No  → Submitter (append-only)
         └─ Yes → Collaborator (bidirectional)
```

**Code**: Sync decision logic
**See**: `gurkha/src/decision.rs:575-624` (should_send_updates function)

---

## How Gurkha Interprets Permits

**Gurkha is the pure domain logic layer** that interprets Permits and makes authorization decisions.

### Gurkha's Role

1. **Parse Permits** - Extract facts from UCAN tokens
2. **Validate** - Check cryptographic signatures and proof chains
3. **Decide** - Determine what operations are allowed
4. **Delegate** - Create new Permits from templates
5. **Evaluate CEL** - Run dynamic permission expressions (future)

**Gurkha has ZERO infrastructure dependencies** - no database, no HTTP, no I/O. Pure domain logic.

### Key Modules

```
gurkha/
├── parser.rs       # Parse UCAN tokens → Permit structs
├── decision.rs     # Authorization decisions (should_send_updates, etc.)
├── cel.rs          # CEL expression evaluation
├── service.rs      # Public API (UcanService)
├── crypto.rs       # Ed25519 signing
├── verification.rs # Proof chain validation
├── merge.rs        # CRDT operations
└── types.rs        # Domain types (Capability, SyncFacts, etc.)
```

### How Services Use Gurkha

Services call Gurkha with Permits and ask questions:

```rust
// Service layer
let permit = Permit::from_token(&token)?;

// Ask Gurkha: Can this user perform this operation?
if permit.has_operation("add_resources") {
    // Proceed with operation
}

// Ask Gurkha: Which documents should we send to this peer?
let sync_context = SyncContext::new(our_permit, peer_permit)?;
for doc_name in resource.doc_names() {
    let decision = should_send_updates(&sync_context, doc_name);
    match decision {
        SyncDecision::DontSend => continue,
        SyncDecision::SendFullSnapshot => send_full(doc),
        SyncDecision::SendIncrementalUpdates => send_shallow(doc),
    }
}
```

**Code**: Service integration with Gurkha
**See**:
- `services/src/resource_service/core.rs:244-310` (filter_and_encrypt_for_peer)
- `services/src/folder_service.rs` (folder operations using Permits)

### Dynamic Interpretation

Gurkha does **NOT** have hardcoded role checks like:

```rust
// ❌ WRONG - Hardcoded role logic
if permit.token_type == "folder_owner" {
    // Allow operation
}
```

Instead, it reads facts dynamically:

```rust
// ✅ CORRECT - Data-driven logic
if permit.has_operation("add_resources") {
    // Allow operation
}
```

This enables:
- **Custom permission models** per application
- **CEL expressions** for complex rules
- **No backend changes** for new permission schemes

---

## Why Data-Driven?

### The Problem with Hardcoded Roles

**Traditional approach**: Backend has hardcoded role checks.

```rust
// ❌ Hardcoded
match user.role {
    Role::Owner => allow(),
    Role::Admin => allow(),
    Role::Viewer => deny(),
}
```

**Problems**:
1. Adding new roles requires backend changes
2. Same role means same permissions everywhere
3. Can't customize per-document or per-operation
4. Tight coupling between authorization and application logic

### The Data-Driven Approach

**Osvauld approach**: Backend interprets Permit facts dynamically.

```rust
// ✅ Data-driven
if permit.get_capability(doc_name).can_write() {
    allow()
} else {
    deny()
}
```

**Benefits**:
1. **Frontend defines permissions** - No backend changes needed
2. **Per-document granularity** - Different capabilities per document
3. **Decoupled** - Authorization logic separate from app logic
4. **CEL-extensible** - Complex rules via expressions, not code
5. **Application-agnostic** - Same backend for any permission model

### Example: Adding New Permission Type

**Want to add "moderator" role with custom capabilities?**

**Hardcoded approach**:
1. Update backend enum
2. Add role checks throughout code
3. Deploy backend
4. Update frontend

**Data-driven approach**:
1. Update `permissions.ts` template:
```typescript
moderator: {
  operations: {
    "moderate": "allow",
    "delete_comments": "allow"
  },
  cel_rules: {
    "moderate": "request.time < token.exp && user.verified == true"
  }
}
```
2. Done! Backend interprets it dynamically.

---

## Summary

**Permits are the center of Osvauld's architecture**:

1. **Bearer tokens** - Possession = authorization
2. **Facts-only** - No capability URIs, pure JSON
3. **Data-driven** - Gurkha interprets dynamically, no hardcoded roles
4. **Document-level** - Permissions per-document, not per-user
5. **Three capabilities** - Viewer (read), Submitter (append), Collaborator (sync)
6. **Decentralized** - No backend, cryptographic verification
7. **Templates** - Enable autonomous delegation
8. **CEL-extensible** - Complex rules without code changes

**Everything revolves around Permits** - sync, delegation, filtering, authorization - all driven by Permit facts.

---

**Next**: Read [DELEGATION.md](./DELEGATION.md) to understand how Permits are delegated through the trust chain.
