# Delegation - The Trust Chain

**Status**: Current (reflects implemented system)
**Last Updated**: 2025-11-18

---

## Table of Contents

1. [Overview](#overview)
2. [Folder Creation - Root Permit](#folder-creation---root-permit)
3. [Resource Creation - Independent Permits](#resource-creation---independent-permits)
4. [Adding Hosting Node](#adding-hosting-node)
5. [Handshake Mechanism](#handshake-mechanism)
6. [Shareable Links](#shareable-links)
7. [Viewer Access](#viewer-access)
8. [Forward Secrecy - Why Re-encrypt](#forward-secrecy---why-re-encrypt)
9. [Delegation Flow Summary](#delegation-flow-summary)

---

## Overview

**Delegation** is how Permits flow through the trust chain: Owner → Node → Viewer.

Each delegation creates a **new Permit** based on templates embedded in the parent Permit.

### Key Principles

1. **Templates drive delegation** - Parent Permit contains templates for child Permits
2. **Each delegation = new Permit** - Not a reference, a fresh cryptographic token
3. **Forward secrecy** - Each Permit gets unique encryption keys
4. **Bearer tokens** - Anyone holding the Permit is authorized
5. **Decentralized** - No central authority needed for delegation

**Folder Permits** group resources for discovery. **Resource Permits** control actual data access.

---

## Folder Creation - Root Permit

When a user creates a folder, they establish the **root of a delegation chain**.

### The Flow

1. **User provides folder name** (frontend)
2. **Frontend sends template JSON** to backend with delegation templates
3. **Backend calls Gurkha** to issue owner Permit
4. **Gurkha embeds delegation templates** in Permit facts
5. **Folder stored with owner Permit**

### Folder Owner Permit Structure

```json
{
  "iss": "did:key:z6Mkf...",  // Owner's DID (self-signed)
  "aud": "did:key:z6Mkf...",  // Self (owner issues to themselves)
  "exp": 1234567890,
  "fct": {
    "token_type": "folder_owner",
    "relationship": "owner",
    "folder_id": "550e8400-...",
    "user_id": "base64_pubkey...",
    "operations": {
      "own": "allow",
      "get_share_link": "allow",
      "add_resources": "allow",
      "share_folder": "allow"
    },
    "delegation": {
      "node": {
        // Template for delegating to hosting node
        "operations": {
          "get_share_link": "allow",
          "add_resources": "allow",  // Node can persist resources!
          "share_folder": "allow"     // Node can delegate to viewers!
        },
        "auth_capabilities": {
          "can_connect": true,
          "persist_share": true,  // Node shares are persistent
          "can_delegate": false,
          "sync_enabled": true
        },
        "relationship": "node"
      },
      "viewer": {
        // Template for delegating to viewers
        "operations": {
          "request_resources": "allow",
          "get_share_link": "allow"
        },
        "auth_capabilities": {
          "can_connect": true,
          "persist_share": false,  // Viewer shares are ephemeral!
          "can_delegate": false,
          "sync_enabled": false
        },
        "relationship": "viewer"
      }
    }
  },
  "prf": []  // Empty - root token has no parent
}
```

### Why Node Needs These Permissions

**`add_resources`**: Node needs this to **persist new resources** created by owner. When owner publishes a resource, node stores it. Node validates using this permission.

**`share_folder`**: Node needs this to **delegate to viewers independently**. Owner doesn't need to be online for viewer onboarding. Node uses embedded `delegation.viewer` template.

**`persist_share: true`**: Node shares are **persistent** (stored in database) because owner and node have **bidirectional sync** (push + pull).

**Code**: Folder Permit creation
**See**:
- `tauri_handlers/src/handlers/folder.rs:28-35` (create_folder handler)
- `services/src/folder_service.rs:44-48` (calls UcanService)
- `gurkha/src/service.rs:301-326` (issue_folder_owner_token)
- `gurkha/src/decision.rs:459-500` (decide_folder_owner_token - embeds template)

---

## Resource Creation - Independent Permits

Resources get **their own Permits** - they are **NOT chained to folder Permits**.

### Why Independent?

**Folders are for grouping/discovery**, not authorization chains.

When a user has folder access, they can **request resources** from that folder. But actual resource access is controlled by **resource Permits**.

### Resource Owner Permit Structure

```json
{
  "iss": "did:key:z6Mkf...",
  "aud": "did:key:z6Mkf...",
  "exp": 1234567890,
  "fct": {
    "token_type": "resource_owner",
    "relationship": "owner",
    "resource_id": "abc-123-...",
    "folder_id": "550e8400-...",  // Associated folder (for discovery)
    "operations": {
      "own": "allow",
      "share": "allow",
      "create": "allow"
    },
    "documents": {
      // Per-document capabilities!
      "template_doc": { "capability": "collaborator", "type": "loro" },
      "content_doc": { "capability": "collaborator", "type": "loro" },
      "collaborative_doc": { "capability": "collaborator", "type": "loro" },
      "submissions_doc": { "capability": "collaborator", "type": "loro" },
      "user_content_doc": { "capability": "collaborator", "type": "loro" }
    },
    "delegation": {
      "node": {
        "documents": {
          "template_doc": { "capability": "collaborator" },
          "content_doc": { "capability": "collaborator" },
          "collaborative_doc": { "capability": "collaborator" },
          "submissions_doc": { "capability": "collaborator" },
          "user_content_doc": { "capability": "collaborator" }
        },
        "sync": {
          // Sync behavior per document
          "local_only": [],
          "no_incoming_updates": [],
          "send_full_snapshot": []
        }
      },
      "viewer": {
        "documents": {
          "template_doc": { "capability": "viewer" },      // Read-only
          "content_doc": { "capability": "viewer" },       // Read-only
          "collaborative_doc": { "capability": "collaborator" }  // Can edit!
          // submissions_doc and user_content_doc NOT included for viewers!
        },
        "sync": {
          "local_only": ["user_content_doc"],  // Private to viewer
          "no_incoming_updates": [],
          "send_full_snapshot": ["submissions_doc"]  // Full snapshots for submissions
        }
      }
    }
  },
  "prf": []  // Independent - not chained to folder Permit
}
```

### Document Filtering in Templates

Notice `delegation.viewer.documents` **does not include** `submissions_doc` or `user_content_doc`:
- **`submissions_doc`**: Viewer can submit, but shouldn't see others' submissions
- **`user_content_doc`**: Private to each user

This filtering happens **in the template** - Gurkha extracts it when delegating.

**Code**: Resource Permit creation
**See**:
- `services/src/resource_service/crud.rs` (create_resource)
- `gurkha/src/service.rs` (issue_resource_owner_token)
- `gurkha/src/parser.rs:400-500` (extract_template_from_token)

---

## Adding Hosting Node

Owner creates folder/resources locally → wants to publish → adds hosting node.

### The Flow

1. **Owner explicitly adds node** (via UI)
2. **Owner provides node's public key** for identification
3. **Mutual authentication** via one-time bearer tokens
4. **Handshake establishes connection**
5. **Both sides persist user data** in database
6. **Owner delegates folder Permit** to node
7. **sync_handler orchestrates** resource publishing

### One-Time Bearer Tokens

Both sides create **temporary bearer tokens** to prove identity:

**Node creates**:
```json
{
  "operations": {
    "own": "allow"
  }
}
```

**Important**: **Anyone holding this token is considered the owner** for that handshake. This is **bearer authentication** - possession = identity.

**Owner creates similar token** and presents to node.

Both sides:
1. Verify signature (cryptographic proof)
2. Check `operations.own == true`
3. Extract public key from DID
4. Establish mutual trust

**Code**: One-time token creation
**See**: `kunki/src/main.rs` (node's one-time token generation)

### After Handshake

After successful mutual authentication:
1. **Both can connect and authenticate** with each other
2. **Device IDs exchanged** for P2P connections (Iroh layer)
3. **Osvauld handshake complete** - identity-aware connection
4. **sync_handler triggers messages** for resource sync
5. **Owner can now publish** resources to node

### Device ID vs Permits

**Separate concerns**:
- **Device ID** = Iroh P2P layer (how to connect)
- **Permits** = Authorization layer (what you can do)

Device ID resolves to device public key for P2P connection, but **authorization happens via Permits**.

**Code**: Handshake flow
**See**:
- `network/src/p2p/handshake.rs` (mutual authentication)
- `network/src/p2p/p2p_init.rs` (Osvauld handshake)

---

## Handshake Mechanism

The handshake is **NOT hardcoded** - it's **data-driven** using Permit facts.

### Traditional Approach (What We DON'T Do)

```rust
// ❌ Hardcoded handshake
if user.role == "owner" && peer.role == "node" {
    establish_connection();
}
```

### Osvauld Approach (Data-Driven)

```rust
// ✅ Data-driven handshake
let owner_permit = Permit::from_token(&owner_token)?;
let node_permit = Permit::from_token(&node_token)?;

// Check facts dynamically
if owner_permit.has_operation("own") && node_permit.has_operation("own") {
    // Both proved ownership via bearer tokens
    establish_connection();
}
```

### No Backend Hardcoding

**Backend does NOT have** hardcoded logic like:
```rust
if role == "owner" { /* ... */ }
```

Instead, backend:
1. **Receives Permits** from both sides
2. **Reads facts** dynamically
3. **Validates cryptographic signatures**
4. **Issues new Permit** based on `delegation` template from owner's Permit
5. **Both sides persist** connection info

**Everything driven by Permit facts**, not backend code.

**Code**: Backend dynamic validation
**See**: `network/src/p2p/auth.rs` (Permit-based authentication)

---

## Shareable Links

When node wants to allow viewers to access a folder, it creates a **shareable link**.

### What is a Shareable Link?

A shareable link is a **Permit with `aud: *`** (audience = anyone).

```json
{
  "iss": "did:key:z6Mkf...",  // Node's DID
  "aud": "*",                  // ANYONE can use this!
  "exp": 1234567890,
  "fct": {
    "token_type": "folder_share_link",
    "folder_id": "550e8400-...",
    "operations": {
      "request_resources": "allow",
      "connect": "allow"
    }
  },
  "prf": ["bafyrei..."]  // Proof: delegated from node's folder Permit
}
```

### Trust Establishment

**Anyone holding this Permit token** can:
1. Connect to the node
2. Request resources from this folder
3. Prove authorization (Permit is cryptographically valid)

Node validates:
1. **Signature is valid** (cryptographic proof)
2. **Audience is wildcard** (`aud: *`)
3. **Operations include** `"request_resources"`
4. **Proof chain valid** (links back to owner's folder Permit)

If valid → **trust established** → node issues **individual viewer Permit**.

### Why `aud: *`?

**Shareable links are public** - anyone with the link can access. The wildcard audience makes this explicit.

After connecting with shareable link, node issues **individual Permit** for that specific viewer (with their DID as `aud`).

**Code**: Shareable link generation
**See**: `network/src/p2p/auth.rs` (create_shareable_link)

---

## Viewer Access

Viewer flow: Shareable link → Connection → Individual Permit → Resource access.

### Step-by-Step Flow

1. **Viewer obtains shareable link** (URL contains Permit token)
2. **Viewer connects to node** presenting shareable link Permit
3. **Node validates shareable link** (signature, proof chain, operations)
4. **Node extracts `delegation.viewer` template** from owner's resource Permit
5. **Node issues individual viewer Permit** using template
6. **Viewer requests resource** with their individual Permit
7. **Node filters documents** based on viewer's Permit
8. **Node encrypts for viewer's public key** (re-encryption!)
9. **Viewer receives filtered resource**

### Viewer's Individual Permit

```json
{
  "iss": "did:key:z6Mkf...",  // Node's DID (issuer)
  "aud": "did:key:z6Mkw...",  // Viewer's DID (audience)
  "exp": 1234567890,
  "fct": {
    "token_type": "resource_viewer",
    "resource_id": "abc-123-...",
    "operations": {
      "request_resources": "allow",
      "get_share_link": "allow"
    },
    "documents": {
      "template_doc": { "capability": "viewer" },
      "content_doc": { "capability": "viewer" },
      "collaborative_doc": { "capability": "collaborator" }
      // submissions_doc and user_content_doc FILTERED OUT
    },
    "auth_capabilities": {
      "can_connect": true,
      "persist_share": false,  // EPHEMERAL!
      "can_delegate": false,
      "sync_enabled": false
    }
  },
  "prf": ["bafyrei..."]  // Proof: delegated from node's resource Permit
}
```

### Why `persist_share: false`?

**Viewers are ephemeral** - their Permit is **NOT stored in database**.

**Why?**
- Viewers **pull data** (one-way)
- No need for persistent state
- Node doesn't need to track viewers long-term
- Share record contains **node info** for viewer to connect

**Owner/node shares** have `persist_share: true` because they need **bidirectional sync** (push + pull).

### Bidirectional Document Filtering

**Node → Viewer**:
- Node filters out `submissions_doc` and `user_content_doc`
- Only sends `template_doc`, `content_doc`, `collaborative_doc`

**Viewer → Node**:
- Viewer sends updates to `collaborative_doc` (append-only merge)
- Viewer submits to `submissions_doc` (isolated namespace)
- Viewer's `user_content_doc` stays private (not sent to node)

**Both sides filter** based on their Permits!

**Code**: Document filtering
**See**: `services/src/resource_service/core.rs:244-310` (filter_and_encrypt_for_peer)

---

## Forward Secrecy - Why Re-encrypt

Every delegation **re-encrypts data** for the recipient's public key.

### Why Re-encrypt?

**Option 1 (What we DON'T do)**: Use same encryption key for everyone
```
Owner encrypts → Node decrypts → Node sends same encrypted blob to viewer
```
**Problem**: If viewer's key is compromised, they can decrypt **everything** owner shared with node, even data not meant for viewers.

**Option 2 (What we DO)**: Re-encrypt for each delegation
```
Owner encrypts with key_owner → Node decrypts → Node re-encrypts with key_node → Viewer decrypts → Node re-encrypts with key_viewer
```
**Benefit**: **Forward secrecy** - Each recipient can only decrypt what's explicitly shared with them.

### The Flow

1. **Owner encrypts resource** with AES key
2. **Owner encrypts AES key** with owner's PGP public key
3. **Owner sends encrypted data + encrypted key** to node
4. **Node decrypts** using node's PGP private key
5. **Node filters documents** based on viewer's Permit
6. **Node generates NEW AES key** for viewer
7. **Node encrypts filtered data** with new AES key
8. **Node encrypts new AES key** with viewer's PGP public key
9. **Viewer receives filtered, re-encrypted resource**

### Key Rotation

Each delegation = **new encryption keys**.

**Benefits**:
1. **Forward secrecy** - Past keys can't decrypt future data
2. **Permission-based filtering** - Recipient only gets what they're authorized for
3. **Isolation** - Compromised viewer key doesn't expose owner data

**Code**: Re-encryption logic
**See**:
- `crypto_utils/src/lib.rs` (encrypt_data_for_user function)
- `services/src/resource_service/core.rs:244-310` (filter_and_encrypt_for_peer)

---

## Delegation Flow Summary

### Complete Trust Chain

```
Owner (root)
  │
  ├─ Creates folder → Folder Owner Permit (self-signed)
  │                   ├─ delegation.node template embedded
  │                   └─ delegation.viewer template embedded
  │
  ├─ Creates resource → Resource Owner Permit (independent)
  │                     ├─ delegation.node template embedded
  │                     └─ delegation.viewer template embedded
  │
  ├─ Adds node → One-time bearer tokens (mutual auth)
  │            → Handshake (Osvauld identity-aware)
  │            → Delegates folder Permit to node
  │
  └─ Publishes resource → Node receives encrypted resource + Permit
                       → Node stores (validates with folder Permit)

Node
  │
  ├─ Creates shareable link → Permit with aud:*
  │
  └─ Viewer connects → Validates shareable link
                    → Extracts delegation.viewer template
                    → Issues individual viewer Permit
                    → Filters documents
                    → Re-encrypts for viewer
                    → Sends filtered resource

Viewer
  │
  └─ Receives filtered resource → Only sees permitted documents
                                 → Can collaborate on collaborative_doc
                                 → Cannot see submissions_doc or user_content_doc
```

### Key Takeaways

1. **Templates drive delegation** - Embedded in parent Permits
2. **Each delegation = new Permit** - Fresh cryptographic token
3. **Bearer tokens** - Possession = authorization
4. **Forward secrecy** - Re-encrypt for each recipient
5. **Data-driven** - No hardcoded roles, Gurkha interprets facts
6. **Document filtering** - Bidirectional, permission-based
7. **Decentralized** - No central authority needed

---

**Next**: Read [SYNC_PROTOCOL.md](./SYNC_PROTOCOL.md) to understand how Permits drive document synchronization.
