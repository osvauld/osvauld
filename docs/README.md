# Osvauld Documentation

**Last Updated**: 2025-11-18
**Current**: V3 Permits Architecture - IMPLEMENTED

---

## Start Here

**New to Osvauld?** Read in this order:

1. **[PERMITS_OVERVIEW.md](./PERMITS_OVERVIEW.md)** - Core architecture (START HERE!)
2. **[DELEGATION.md](./DELEGATION.md)** - How trust flows through the system
3. **[SYNC_PROTOCOL.md](./SYNC_PROTOCOL.md)** - How documents synchronize
4. **[IMPLEMENTATION_STATUS.md](./IMPLEMENTATION_STATUS.md)** - What's working now

---

## Core Documentation

### Architecture

**[PERMITS_OVERVIEW.md](./PERMITS_OVERVIEW.md)** - Complete Permit architecture
- What are Permits? (UCAN extension)
- Identity & keys (PGP, UCAN signing, device)
- Bearer token architecture
- Permit structure (facts-only)
- Permission templates
- Document-level permissions
- The three capabilities (Viewer, Submitter, Collaborator)
- Why data-driven? (no hardcoded roles)

### Trust Chain

**[DELEGATION.md](./DELEGATION.md)** - Delegation from owner → node → viewer
- Folder creation (root Permit)
- Resource creation (independent Permits)
- Adding hosting node (handshake mechanism)
- Shareable links (aud:* bearer tokens)
- Viewer access (ephemeral Permits)
- Forward secrecy (re-encryption per delegation)

### Synchronization

**[SYNC_PROTOCOL.md](./SYNC_PROTOCOL.md)** - Permit-driven document sync
- Sync is Permit-driven (all decisions from facts)
- Publishing resources (owner → node)
- Dual-Permit validation (SyncContext)
- Document filtering logic (should_send_updates)
- Sync facts (local_only, no_incoming_updates, send_full_snapshot)
- Document capabilities & sync behavior
- Bidirectional filtering
- CRDT merge operations
- CEL expressions (future extensibility)

### Implementation

**[IMPLEMENTATION_STATUS.md](./IMPLEMENTATION_STATUS.md)** - Current status
- What's complete (V3 architecture fully working)
- Gurkha module structure
- Service layer integration
- Network layer
- Frontend integration
- Code metrics

---

## Layer-Specific Docs

### Network Layer

**[NETWORK_LAYER.md](./NETWORK_LAYER.md)** - P2P orchestration
- Handshake protocol
- Folder sync handlers
- Resource sync handlers
- Message routing
- Permit validation in handlers

### Service Layer

**[SERVICE_LAYER.md](./SERVICE_LAYER.md)** - Business logic
- Resource service patterns
- Folder service integration
- Gurkha usage examples
- Service-layer best practices

---

## Quick Reference

### Key Concepts

**Permits**: Our facts-only UCAN extension - bearer tokens that contain all authorization data

**Gurkha**: Pure domain logic layer that interprets Permits dynamically

**Document-level permissions**: Not user roles - each document has its own capability

**Dual-Permit validation**: Both sides' Permits must agree for sync (intersection logic)

**Forward secrecy**: Re-encrypt for each delegation with unique keys

### Three Capabilities

- **Viewer**: Read-only, receives updates
- **Submitter**: Append-only, isolated namespace
- **Collaborator**: Bidirectional CRDT sync

### Sync Facts

- `local_only`: Document stays private to device
- `no_incoming_updates`: Don't accept incoming changes
- `send_full_snapshot`: Send full history, not incremental

---

## Key Files

### Gurkha (Domain Logic)

```
gurkha/src/
├── parser.rs        # Permit parsing (818 lines)
├── decision.rs      # Authorization decisions (645 lines)
├── service.rs       # Public API (500 lines)
├── merge.rs         # CRDT operations (401 lines)
├── cel.rs           # CEL evaluation (335 lines)
├── crypto.rs        # Signing (189 lines)
└── types.rs         # Domain types (153 lines)
```

### Services (Business Logic)

```
services/src/
└── resource_service/
    ├── core.rs      # Core patterns (filter_and_encrypt_for_peer)
    ├── crud.rs      # CRUD operations
    └── sync.rs      # Sync orchestration
```

### Network (P2P)

```
network/src/p2p/
├── handshake.rs      # Mutual authentication
├── folder_sync.rs    # Folder publishing
├── resource_sync.rs  # Resource transfer
└── sync_handler.rs   # Message orchestration
```

### Frontend (Templates)

```
sthalam/frontend/desktop/src/
├── config/permissions.ts     # Permission templates
└── state/data.svelte.ts      # Document handling
```

---

## Architecture Principles

1. **Permits control everything** - All authorization from Permit facts
2. **No hardcoded roles** - Gurkha interprets dynamically
3. **Bearer tokens** - Possession = authorization
4. **Data-driven** - Frontend defines permissions, backend interprets
5. **Document-level** - Per-document capabilities, not user roles
6. **Forward secrecy** - Re-encrypt for each recipient
7. **Decentralized** - No backend, cryptographic verification

---

## For Developers

### Adding New Features

1. **Define permissions** in frontend `permissions.ts`
2. **Gurkha interprets** - no backend changes needed
3. **Use CEL expressions** for complex conditional logic
4. **Test with different Permits** to verify behavior

### Debugging

1. Check Permit facts (what does the token contain?)
2. Verify dual-Permit logic (do both sides agree?)
3. Check document capabilities (viewer/submitter/collaborator?)
4. Review sync facts (local_only, no_incoming_updates?)
5. Trace filtering decision (should_send_updates logic)

### Code References

All documentation includes code references with file paths and line numbers.

Example:
```
**Code**: services/src/resource_service/core.rs:244-310
```

---

## Status

✅ **V3 Permits Architecture IMPLEMENTED** (as of commit `3f52a683`)
- Owner → Node → Viewer delegation working
- Document filtering operational
- Bidirectional sync functional
- Frontend handles missing documents gracefully

---

**Questions?** Check the relevant documentation above or examine the code with references provided.
