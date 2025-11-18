# Implementation Status

**Last Updated**: 2025-11-18
**Status**: V3 Permits Architecture - IMPLEMENTED

---

## Executive Summary

Osvauld has successfully implemented the **V3 Permits Architecture** - a facts-only, data-driven authorization system built on UCAN tokens.

**Key achievements**:
- ✅ Permits (facts-only UCAN extension) implemented
- ✅ Document-level permissions working
- ✅ Dual-Permit validation (SyncContext) functional
- ✅ Owner → Node → Viewer delegation chain operational
- ✅ Bidirectional document filtering working
- ✅ CRDT-based sync with Loro
- ✅ CEL integration (foundation, not extensively used yet)
- ✅ Forward secrecy via re-encryption

---

## Table of Contents

1. [Current Architecture](#current-architecture)
2. [Gurkha Module Structure](#gurkha-module-structure)
3. [Service Layer Integration](#service-layer-integration)
4. [Network Layer](#network-layer)
5. [Frontend Integration](#frontend-integration)
6. [What's Complete](#whats-complete)
7. [What's In Progress](#whats-in-progress)
8. [Known Issues](#known-issues)

---

## Current Architecture

### V3 Permits (Facts-Only)

Osvauld uses **Permits** - our extension of UCAN tokens with:
- **Facts-only structure**: All authorization in `fct` field, no capability URIs
- **Single Permit type**: No typed wrappers (removed 11 typed tokens from v2)
- **Data-driven**: Gurkha interprets facts dynamically, no hardcoded roles
- **Document-level**: Per-document permissions, not user-level roles
- **CEL-extensible**: Dynamic rules via expressions

**See**: `docs/PERMITS_OVERVIEW.md` for complete architecture

### Core Principles

1. **Bearer tokens** - Possession = authorization
2. **Dual-Permit validation** - Both sides must agree for sync
3. **Forward secrecy** - Re-encrypt for each delegation
4. **Decentralized** - No backend, cryptographic verification

---

## Gurkha Module Structure

**Gurkha** is the pure domain logic layer that interprets Permits.

```
gurkha/
├── lib.rs           # Public API, module exports
├── parser.rs        # Parse UCAN → Permit structs (818 lines)
├── decision.rs      # Authorization decisions (645 lines)
├── cel.rs           # CEL expression evaluation (335 lines)
├── service.rs       # UcanService public API (500 lines)
├── crypto.rs        # Ed25519 signing (189 lines)
├── verification.rs  # Proof chain validation
├── merge.rs         # CRDT operations (401 lines)
├── types.rs         # Domain types (153 lines)
├── builder.rs       # UCAN builder utilities
└── errors.rs        # Error types
```

### Key Modules

**parser.rs**:
- `Permit` struct - Main authorization token
- `UcanCore` - Parsed UCAN with facts
- `extract_template_from_token()` - Extract delegation templates
- Template-to-facts conversion
- Capability/sync facts extraction

**Code**: `gurkha/src/parser.rs`

**decision.rs**:
- `SyncContext` - Dual-Permit validation
- `should_send_updates()` - Document filtering logic
- `decide_folder_owner_token()` - Issue folder Permits
- `decide_resource_owner_token()` - Issue resource Permits

**Code**: `gurkha/src/decision.rs`

**cel.rs**:
- `OperationValidator` - CEL expression evaluator
- Context building (request, token, user, document)
- Expression evaluation (not extensively used yet)

**Code**: `gurkha/src/cel.rs`

**service.rs**:
- `UcanService` - Public API for services
- `issue_folder_owner_token()` - Create folder Permits
- `issue_resource_owner_token()` - Create resource Permits
- `delegate_folder()` / `delegate_resource()` - Delegation methods

**Code**: `gurkha/src/service.rs`

**merge.rs**:
- `export_snapshot()` - Full CRDT history
- `export_shallow_snapshot()` - Current state only
- `import_snapshot()` - Load Loro documents

**Code**: `gurkha/src/merge.rs`

---

## Service Layer Integration

Services use Gurkha for all Permit operations.

### Resource Service

**Location**: `services/src/resource_service/`

**Structure**:
```
resource_service/
├── mod.rs         # Module organization
├── core.rs        # Core patterns (filter_and_encrypt_for_peer, etc.)
├── crud.rs        # CRUD operations
└── sync.rs        # Sync orchestration
```

**Key functions**:
- `filter_and_encrypt_for_peer()` - Dual-Permit document filtering
- `load_and_decrypt_by_share_token()` - Load resource via Permit
- `delegate_and_create_share_record()` - Delegation with database persistence
- `encrypt_and_save_resource()` - Save with key rotation

**Code**: `services/src/resource_service/core.rs:244-310` (filter_and_encrypt_for_peer)

### Folder Service

**Location**: `services/src/folder_service.rs`

**Integration**:
- Creates folder owner Permits via `ucan_service.issue_folder_owner_token()`
- Validates folder access using Permit operations
- Delegates folder Permits to nodes

**Code**: `services/src/folder_service.rs`

### Removed

**Deleted** `services/src/merge_service.rs` - Functionality moved to `gurkha/src/merge.rs`

---

## Network Layer

**Location**: `network/src/p2p/`

### Key Components

**handshake.rs**:
- One-time bearer token mutual authentication
- Permit-based identity verification
- No hardcoded role checks

**folder_sync.rs**:
- Folder publishing (owner → node)
- Permit validation before accepting resources
- sync_handler message orchestration

**resource_sync.rs**:
- Resource transfer with Permit validation
- Document filtering via SyncContext
- Re-encryption for forward secrecy

**Code**:
- `network/src/p2p/handshake.rs` (mutual auth)
- `network/src/p2p/folder_sync.rs` (folder sync)
- `network/src/p2p/resource_sync.rs` (resource sync)

---

## Frontend Integration

### Permission Templates

**Location**: `sthalam/frontend/desktop/src/config/permissions.ts`

**Defines**:
- `FOLDER_TEMPLATE` - Folder owner + delegation templates
- `RESOURCE_TEMPLATE` - Resource owner + per-document capabilities

**Frontend sends templates to backend** - Backend embeds them in Permit facts.

**Code**: `sthalam/frontend/desktop/src/config/permissions.ts`

### Document Handling

**Location**: `sthalam/frontend/desktop/src/state/data.svelte.ts`

**Fixed issues**:
- ✅ Missing documents now create valid empty Loro snapshots
- ✅ Handles permission-based filtering gracefully
- ✅ No more "Decode error" when documents filtered out

**Code**: `sthalam/frontend/desktop/src/state/data.svelte.ts:363-380`

---

## What's Complete

### Core Architecture ✅

- [x] Facts-only Permit structure
- [x] Single Permit type (no typed wrappers)
- [x] Document-level permissions
- [x] Sync facts (local_only, no_incoming_updates, send_full_snapshot)
- [x] Three capabilities (Viewer, Submitter, Collaborator)

### Gurkha Service ✅

- [x] Permit parsing and validation
- [x] Dual-Permit validation (SyncContext)
- [x] Document filtering logic (should_send_updates)
- [x] Folder/resource Permit issuance
- [x] Delegation with template extraction
- [x] CEL integration (foundation)
- [x] CRDT merge operations
- [x] Proof chain validation

### Service Layer ✅

- [x] Resource service integration
- [x] Folder service integration
- [x] filter_and_encrypt_for_peer (core pattern)
- [x] Forward secrecy via re-encryption
- [x] Database persistence

### Network Layer ✅

- [x] Handshake with bearer tokens
- [x] Folder sync (publish/fetch)
- [x] Resource sync with filtering
- [x] Permit validation in handlers
- [x] sync_handler orchestration

### Frontend ✅

- [x] Permission template definitions
- [x] Missing document handling
- [x] Loro document integration
- [x] Viewer handshake working

### Delegation Chain ✅

- [x] Owner → Node delegation
- [x] Node → Viewer delegation
- [x] Shareable links (aud:*)
- [x] Individual viewer Permits
- [x] Bidirectional filtering

---

## What's In Progress

### CEL Expressions 🔄

**Status**: Foundation implemented, not extensively used yet

**Current**:
- CEL parser integrated
- OperationValidator working
- Context building functional

**Next**:
- Add CEL expressions to permission templates
- Use CEL for complex conditional logic
- Time-based permissions
- User verification rules

**Code**: `gurkha/src/cel.rs`

### Document Sync Optimization 🔄

**Current**:
- Full and shallow snapshots working
- Initial sync functional
- CRDT merge operational

**Next**:
- Optimize shallow snapshot use cases
- Better state tracking
- Incremental update efficiency

---

## Known Issues

### None Critical ✅

The viewer handshake and resource send flow is working as of commit `3f52a683`:
- ✅ Parser bug fixed (delegation.documents field)
- ✅ Frontend missing document handling fixed
- ✅ Document filtering operational
- ✅ Owner → Node → Viewer flow working

---

## File Structure Summary

### Core Models

```
core/src/models/
├── mod.rs            # Model exports (SyncContext removed - now in gurkha)
├── resource.rs       # Resource struct with Loro documents
├── document.rs       # CRDT operations (create_doc, state_frontiers)
├── folder.rs         # Folder struct
├── user.rs           # User identity
└── share_record.rs   # Share persistence
```

### Gurkha (Domain Logic)

```
gurkha/src/
├── lib.rs           # Public API
├── parser.rs        # Permit parsing (818 lines)
├── decision.rs      # Authorization (645 lines)
├── cel.rs           # CEL evaluation (335 lines)
├── service.rs       # UcanService (500 lines)
├── crypto.rs        # Signing (189 lines)
├── verification.rs  # Proof chains
├── merge.rs         # CRDT (401 lines)
├── types.rs         # Domain types (153 lines)
└── builder.rs       # UCAN builder
```

**Deleted** (v2 architecture):
- ❌ `gurkha/src/extractors.rs` (old typed extractors)
- ❌ `gurkha/src/uri.rs` (capability URIs, not used in v3)

### Services

```
services/src/
├── resource_service/
│   ├── core.rs      # Core patterns (filter_and_encrypt_for_peer)
│   ├── crud.rs      # CRUD operations
│   └── sync.rs      # Sync orchestration
├── folder_service.rs
├── user_service.rs
└── auth_service.rs
```

**Deleted** (merged into gurkha):
- ❌ `services/src/merge_service.rs` → `gurkha/src/merge.rs`

### Network

```
network/src/p2p/
├── handshake.rs      # Mutual authentication
├── folder_sync.rs    # Folder publishing
├── resource_sync.rs  # Resource transfer
└── sync_handler.rs   # Message orchestration
```

---

## Metrics

### Code Size

**Gurkha** (pure domain logic):
- Total: ~4,000 lines
- Largest: parser.rs (818), decision.rs (645), service.rs (500), merge.rs (401)

**Services** (business logic):
- resource_service: ~900 lines across 3 files
- folder_service: ~300 lines

**Network** (P2P orchestration):
- ~2,000 lines across handlers

### Removed from V2 → V3

**Deleted**:
- 11 typed token wrappers (~800 lines)
- URI capability system (~400 lines)
- Hardcoded role logic (~300 lines)

**Net reduction**: ~1,500 lines of complex type-checking code replaced with ~600 lines of dynamic fact interpretation.

---

## Next Steps

### Short Term

1. Extensive CEL rule usage in templates
2. Performance optimization (snapshot caching)
3. Better error messages (Permit validation failures)

### Long Term

1. OCaml port of Gurkha (pure domain logic, zero dependencies)
2. Formal verification of Permit logic
3. Advanced CEL patterns documentation

---

**See also**:
- [PERMITS_OVERVIEW.md](./PERMITS_OVERVIEW.md) - Core architecture
- [DELEGATION.md](./DELEGATION.md) - Trust chain
- [SYNC_PROTOCOL.md](./SYNC_PROTOCOL.md) - Sync mechanics
