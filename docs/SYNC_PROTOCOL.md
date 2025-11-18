# Sync Protocol - Permit-Driven Synchronization

**Status**: Implemented (reflects current system)
**Last Updated**: 2025-11-18

---

## Table of Contents

1. [Overview](#overview)
2. [Sync is Permit-Driven](#sync-is-permit-driven)
3. [Publishing Resources](#publishing-resources)
4. [Dual-Permit Validation (SyncContext)](#dual-permit-validation-synccontext)
5. [Document Filtering Logic](#document-filtering-logic)
6. [Sync Facts](#sync-facts)
7. [Document Capabilities & Sync Behavior](#document-capabilities--sync-behavior)
8. [Bidirectional Filtering](#bidirectional-filtering)
9. [CRDT Merge Operations](#crdt-merge-operations)
10. [CEL Expressions](#cel-expressions)
11. [Forward Secrecy](#forward-secrecy)

---

## Overview

**Osvauld's sync protocol is entirely driven by Permits.** Every sync decision - what to send, who can receive, how to merge - comes from Permit facts, not hardcoded logic.

### Key Principles

1. **Permits control everything** - All sync decisions read from Permit facts
2. **Dual-Permit validation** - Both sides' Permits must agree
3. **Document-level filtering** - Per-document sync behavior
4. **Bidirectional filtering** - Both sender and receiver filter
5. **CRDT-based** - Loro documents merge automatically
6. **CEL-extensible** - Complex rules via expressions (future)

### Terminology

**Publish vs Fetch**: Describes sync direction with node as center of star topology:
- **Publish** = toward node (owner → node)
- **Fetch** = from node (node → viewer)

Both are the same underlying sync protocol, just different directions.

---

## Sync is Permit-Driven

Traditional sync protocols have hardcoded rules:

```rust
// ❌ Hardcoded sync logic
if user.role == "owner" {
    send_all_documents();
} else if user.role == "viewer" {
    send_only_public_documents();
}
```

**Osvauld reads Permit facts dynamically:**

```rust
// ✅ Permit-driven sync
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

**Benefits**:
- No backend changes for new permission models
- Per-document sync behavior
- CEL expressions for complex rules
- Application-agnostic

**Code**: Permit-driven sync
**See**: `services/src/resource_service/core.rs:244-310` (filter_and_encrypt_for_peer)

---

## Publishing Resources

When owner publishes a resource to node:

### The Flow

1. **Owner initiates** sync_folder (publish)
2. **Owner's service** calls `filter_and_encrypt_for_peer()`
3. **Dual-Permit validation** (owner's + node's Permits)
4. **For each document** in resource:
   - Check: Should we send this document?
   - If yes: Export snapshot (full or shallow)
   - Clone document via import/export
   - Add to filtered resource
5. **Serialize** filtered resource to JSON
6. **Re-encrypt** for node's PGP public key
7. **Send** encrypted resource + Permit to node
8. **Node validates** using folder Permit operations
9. **Node stores** encrypted resource

### Why Send Permit with Resource?

Each resource transfer includes the **resource Permit** because:
1. Node needs to **validate** authorization
2. Node uses Permit to **delegate** to viewers later
3. Node reads **document capabilities** from Permit
4. Node checks **sync facts** from Permit

**No Permit = no authorization = no sync.**

**Code**: Publishing flow
**See**:
- `network/src/p2p/folder_sync.rs` (folder sync handler)
- `services/src/resource_service/sync.rs` (prepare_resource_transfer)

---

## Dual-Permit Validation (SyncContext)

**Key insight**: Sync requires **TWO Permits** - ours and theirs.

### Why Two Permits?

We need to answer:
1. **Can WE send?** (check our Permit)
2. **Can THEY receive?** (check their Permit)

If either says "no" → don't send.

### SyncContext

`SyncContext` holds both Permits and provides dual-validation:

```rust
pub struct SyncContext {
    pub our_ucan: UcanCore,   // Our Permit
    pub peer_ucan: UcanCore,  // Peer's Permit
}

impl SyncContext {
    pub fn new(our_permit: Permit, peer_permit: Permit) -> Result<Self> {
        Ok(Self {
            our_ucan: our_permit.into_inner(),
            peer_ucan: peer_permit.into_inner(),
        })
    }
}
```

**Code**: SyncContext implementation
**See**: `gurkha/src/decision.rs:20-50` (SyncContext struct)

### Intersection Logic

Sync is an **intersection** of permissions:

```
Owner's Permit says: "I can send template_doc"
Node's Permit says: "I can receive template_doc"
→ Send template_doc ✅

Owner's Permit says: "I can send user_content_doc"
Node's Permit says: "I cannot receive user_content_doc"
→ Don't send user_content_doc ❌

Owner's Permit says: "user_content_doc is local_only"
Node's Permit says: "I can receive user_content_doc"
→ Don't send user_content_doc ❌ (our restriction wins)
```

**Both must agree** for sync to happen.

**Code**: Dual-validation logic
**See**: `gurkha/src/decision.rs:575-624` (should_send_updates)

---

## Document Filtering Logic

The heart of sync is `should_send_updates()` - **pure function** that reads Permit facts and decides.

### Decision Tree

```
should_send_updates(sync_context, doc_name) → SyncDecision

Step 1: Check local_only
├─ our_ucan.is_local_only(doc) OR peer_ucan.is_local_only(doc)?
│  └─ YES → DontSend ❌
│
Step 2: Check our capability
├─ our_ucan.get_capability(doc)?
│  ├─ None → DontSend ❌
│  ├─ Viewer → DontSend ❌ (can't write)
│  └─ Collaborator/Submitter → Continue
│
Step 3: Check send_full_snapshot flag
├─ our_ucan.should_send_full_snapshot(doc)?
│  └─ YES → SendFullSnapshot ✅ (for submitters)
│
Step 4: Check peer's no_incoming_updates
├─ peer_ucan.has_no_incoming_updates(doc)?
│  └─ YES → DontSend ❌
│
Step 5: Check peer's capability
└─ peer_ucan.get_capability(doc)?
   ├─ None → DontSend ❌
   ├─ Collaborator → SendIncrementalUpdates ✅ (bidirectional)
   ├─ Viewer → SendIncrementalUpdates ✅ (read-only)
   └─ Submitter → DontSend ❌
```

### SyncDecision

```rust
pub enum SyncDecision {
    SendIncrementalUpdates,  // Send shallow snapshot (current state)
    SendFullSnapshot,         // Send full snapshot (complete history)
    DontSend,                 // Don't send anything
}
```

**SendIncrementalUpdates**: For most syncs - viewer/collaborator receives current state

**SendFullSnapshot**: For submitters - send complete history for proper merge

**DontSend**: Document is filtered out (permissions don't allow)

**Code**: Decision logic
**See**: `gurkha/src/decision.rs:575-624` (should_send_updates implementation)

---

## Sync Facts

Sync facts are **per-document flags** in Permit that control sync behavior.

### Structure

```json
{
  "fct": {
    "sync": {
      "local_only": ["user_content_doc"],
      "no_incoming_updates": ["template_doc"],
      "send_full_snapshot": ["submissions_doc"]
    }
  }
}
```

### local_only

```json
"local_only": ["user_content_doc"]
```

**Meaning**: This document is **private to this device**.
- Don't send to anyone
- Don't receive from anyone
- Stays completely local

**Use case**: User's private workspace, drafts, personal notes

**Check**: If **either our OR peer's** Permit has `local_only` for a document → DontSend

**Code**: `gurkha/src/parser.rs:615-617` (is_local_only check)

### no_incoming_updates

```json
"no_incoming_updates": ["template_doc"]
```

**Meaning**: This document is **one-way outgoing only**.
- We can send updates
- We don't accept incoming updates
- Protects from unwanted changes

**Use case**: Template definitions that shouldn't be modified by viewers

**Check**: If **peer's** Permit has `no_incoming_updates` → DontSend (they don't want it)

**Code**: `gurkha/src/parser.rs:619-621` (has_no_incoming_updates check)

### send_full_snapshot

```json
"send_full_snapshot": ["submissions_doc"]
```

**Meaning**: Send **full CRDT history**, not just current state.
- Used for submitters (append-only merge)
- Ensures proper isolated namespace merging
- Larger payload but correct semantics

**Use case**: Form submissions where viewer's writes go to isolated namespace

**Check**: If **our** Permit has `send_full_snapshot` → SendFullSnapshot

**Code**: `gurkha/src/parser.rs:623-625` (should_send_full_snapshot check)

---

## Document Capabilities & Sync Behavior

Each document has a **capability** that determines sync behavior.

### Viewer (Read-Only)

```json
{ "capability": "viewer" }
```

**Sync behavior**:
- **Receive** updates from source
- **Cannot send** updates back
- One-way: source → viewer

**Implementation**:
```rust
capability.can_write() == false
→ DontSend (step 2 of decision tree)
```

**Use case**: Website content, published documents

**Code**: `gurkha/src/types.rs:16-44` (Capability::Viewer, can_write returns false)

### Submitter (Append-Only)

```json
{ "capability": "submitter" }
```

**Sync behavior**:
- **Send** full snapshots (not incremental)
- Writes go to **isolated namespace**: `viewer:{user_id}`
- **Append-only merge** at destination
- One-way: submitter → host

**Implementation**:
```rust
capability.can_write() == true
→ Continue to step 3
should_send_full_snapshot == true
→ SendFullSnapshot
```

**Use case**: Form submissions, user-generated content isolation

**Code**:
- `gurkha/src/types.rs:16-44` (Capability::Submitter)
- `gurkha/src/decision.rs:595-605` (send_full_snapshot check)

### Collaborator (Bidirectional CRDT)

```json
{ "capability": "collaborator" }
```

**Sync behavior**:
- **Send AND receive** updates
- **Bidirectional CRDT merge**
- Changes merge automatically via Loro
- Both sides: peer ↔ peer

**Implementation**:
```rust
capability.can_write() == true
→ Continue to step 3
should_send_full_snapshot == false
→ Continue to step 5
peer.capability.can_sync_bidirectional() == true
→ SendIncrementalUpdates
```

**Use case**: Collaborative editing, public comments, shared documents

**Code**: `gurkha/src/types.rs:16-44` (can_sync_bidirectional returns true)

---

## Bidirectional Filtering

**Critical insight**: **Both sides filter** what they send based on their Permits.

### Example: Owner ↔ Node

**Owner's Permit**:
```json
{
  "documents": {
    "template_doc": { "capability": "collaborator" },
    "user_content_doc": { "capability": "collaborator" }
  },
  "sync": {
    "local_only": ["user_content_doc"]
  }
}
```

**Node's Permit**:
```json
{
  "documents": {
    "template_doc": { "capability": "collaborator" },
    "user_content_doc": { "capability": "collaborator" }
  }
}
```

**Owner → Node sync**:
- `template_doc`: ✅ Both allow, owner can write, node can receive → Send
- `user_content_doc`: ❌ Owner has `local_only` → Don't send

**Node → Owner sync** (if initiated):
- `template_doc`: ✅ Both allow → Send
- `user_content_doc`: ❌ Owner has `local_only` → Don't send

**Both sides respect `local_only`** even though only owner's Permit has it.

### Example: Node ↔ Viewer

**Node's Permit**:
```json
{
  "documents": {
    "template_doc": { "capability": "collaborator" },
    "collaborative_doc": { "capability": "collaborator" },
    "submissions_doc": { "capability": "collaborator" }
  }
}
```

**Viewer's Permit**:
```json
{
  "documents": {
    "template_doc": { "capability": "viewer" },
    "collaborative_doc": { "capability": "collaborator" }
    // submissions_doc NOT included!
  }
}
```

**Node → Viewer sync**:
- `template_doc`: ✅ Node can send, viewer can receive (read-only) → Send
- `collaborative_doc`: ✅ Both collaborators → Send
- `submissions_doc`: ❌ Viewer's Permit doesn't include it → Don't send

**Viewer → Node sync**:
- `template_doc`: ❌ Viewer is read-only (can't write) → Don't send
- `collaborative_doc`: ✅ Both collaborators → Send

**Each side filters independently** based on its own Permit!

**Code**: Bidirectional filtering implementation
**See**: `services/src/resource_service/core.rs:244-310` (both sides call filter_and_encrypt_for_peer)

---

## CRDT Merge Operations

Osvauld uses **Loro CRDT** for document merging.

### Export Operations

**Full Snapshot**:
```rust
pub fn export_snapshot(doc: &LoroDoc) -> Vec<u8> {
    doc.export(ExportMode::Snapshot)
}
```

- Exports **complete CRDT history**
- All operations from document creation
- Can be imported into empty LoroDoc
- Larger size (includes operation log)

**Shallow Snapshot**:
```rust
pub fn export_shallow_snapshot(doc: &LoroDoc) -> Vec<u8> {
    let frontiers = doc.state_frontiers();
    doc.export(ExportMode::shallow_snapshot(&frontiers))
}
```

- Exports **current state only** at specific frontier
- No operation history
- Smaller size
- **Requires receiver to have prior state** (for incremental updates)

**Code**: Export operations
**See**: `gurkha/src/merge.rs:69-95` (export functions)

### Import Operation

```rust
pub fn import_snapshot(snapshot_bytes: &[u8]) -> Result<LoroDoc> {
    let doc = LoroDoc::new();
    doc.import(snapshot_bytes)?;
    Ok(doc)
}
```

- Creates new LoroDoc
- Imports snapshot (full or shallow)
- **Note**: Shallow snapshots require context (receiver must have base state)

**Code**: Import operation
**See**: `gurkha/src/merge.rs:33-41` (import_snapshot)

### Merge Behavior

**Automatic merge**: When both sides have changes, Loro merges automatically:

```
Owner's doc:         Node's doc:
  Content: "A"         Content: "B"
  State: [1,0]         State: [0,1]
         ↓                    ↓
    Export snapshot    Export snapshot
         ↓                    ↓
       Send →          ← Send
         ↓                    ↓
     Import            Import
         ↓                    ↓
      Merge!           Merge!
         ↓                    ↓
  Content: "AB"      Content: "AB"
  State: [1,1]       State: [1,1]
```

**Eventual consistency**: After all updates exchanged, both sides converge to same state.

**Code**: Loro library handles merging internally

---

## CEL Expressions

CEL (Common Expression Language) enables **dynamic permission evaluation** without code changes.

### Structure

```json
{
  "fct": {
    "cel_rules": {
      "moderate": "request.time < token.exp && user.verified == true",
      "delete": "user.role == 'admin' || resource.owner == user.id"
    }
  }
}
```

### How CEL Works

1. **Permit contains CEL expressions** in facts
2. **Gurkha evaluates expressions** at decision time
3. **Context provided**: request, token, user, resource, document
4. **Result**: true (allow) or false (deny)

### Example Use Case

**Time-based permissions**:
```json
"cel_rules": {
  "edit_template": "request.time < token.exp - 86400"
}
```
(Allow editing template only if token expires in more than 24 hours)

**Verified user requirement**:
```json
"cel_rules": {
  "collaborate": "user.verified == true && user.reputation > 10"
}
```

### Current Status

**CEL is implemented but not extensively used yet.** It's an **important future feature** for:
- Complex conditional permissions
- Time-based rules
- User reputation/verification checks
- Dynamic document access patterns

**Code**: CEL integration
**See**: `gurkha/src/cel.rs` (OperationValidator, expression evaluation)

---

## Forward Secrecy

Every sync operation **re-encrypts** data for the recipient.

### Why Re-encrypt?

**Goal**: Each recipient should **only decrypt what's explicitly shared** with them.

**Bad approach** (what we DON'T do):
```
Owner encrypts once → Send same ciphertext to everyone
```
Problem: Compromised viewer key could decrypt owner's full dataset.

**Good approach** (what we DO):
```
Owner encrypts → Node decrypts → Node filters → Node re-encrypts → Viewer decrypts
```
Benefit: Viewer only gets filtered subset, encrypted with unique keys.

### The Flow

1. **Owner** encrypts resource with AES key
2. **Owner** encrypts AES key with owner's PGP public key
3. **Send** to node
4. **Node** decrypts using node's PGP private key
5. **Node** filters documents based on viewer's Permit
6. **Node** generates NEW AES key
7. **Node** encrypts filtered resource with new AES key
8. **Node** encrypts new AES key with viewer's PGP public key
9. **Send** to viewer
10. **Viewer** decrypts using viewer's PGP private key

### Key Rotation

**Each delegation = new encryption keys.**

Benefits:
1. **Forward secrecy** - Old keys can't decrypt new data
2. **Permission-based filtering** - Only authorized data accessible
3. **Isolation** - Compromised viewer doesn't expose owner data

**Code**: Re-encryption
**See**:
- `crypto_utils/src/lib.rs` (encrypt_data_for_user)
- `services/src/resource_service/core.rs:149-167` (encrypt_and_save_resource)

---

## Summary

**Osvauld's sync is entirely Permit-driven:**

1. **Dual-Permit validation** - Both sides' Permits must agree
2. **Document-level filtering** - Per-document sync decisions
3. **Dynamic interpretation** - Gurkha reads facts, no hardcoded logic
4. **Bidirectional filtering** - Both sides filter independently
5. **Three capabilities** - Viewer (read), Submitter (append), Collaborator (sync)
6. **Sync facts** - local_only, no_incoming_updates, send_full_snapshot
7. **CRDT merge** - Loro handles automatic conflict resolution
8. **CEL-extensible** - Complex rules without code changes
9. **Forward secrecy** - Re-encryption per delegation

**Everything is driven by Permit facts** - change the Permit, change the sync behavior. No backend modifications required.

---

**Previous**: Read [DELEGATION.md](./DELEGATION.md) to understand the trust chain.
**See also**: [PERMITS_OVERVIEW.md](./PERMITS_OVERVIEW.md) for core Permit architecture.
