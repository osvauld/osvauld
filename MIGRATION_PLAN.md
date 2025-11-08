# Osvauld Protocol Migration: Yrs → Loro

## Overview

Migrating the Osvauld protocol from Yrs CRDTs to Loro CRDTs. Focus on Sthalam app initially, with generic protocol design that works for all Osvauld apps.

**Status**: Implementation Phase - Phase 2.5 Complete (65% done)
**Started**: 2025-11-06
**Phase 2 Completed**: 2025-11-07
**Phase 2.5 Completed**: 2025-11-07 (Tauri Handler Layer)
**Target**: TBD

---

## Architecture Decisions

### 1. Generic Protocol Design
**Decision**: All protocol-level code (core, crypto_utils) is app-agnostic. Sthalam-specific logic lives in sthalam/.

**Rationale**:
- Osvauld is a protocol, Sthalam is one app using it
- Future apps (Livnote, others) will use same protocol
- Merge logic, permissions, doc structure all defined in UCAN tokens

### 2. UCAN-Driven Everything
**Decision**: UCAN tokens contain doc structure, permissions, and merge behavior. No separate config.

**Rationale**:
- Single source of truth
- Tokens are self-describing
- No server-side permission checks needed
- Capabilities define sync behavior directly

**Two-Template Architecture** (✅ Implemented Phase 2):
- Owner's UCAN contains two templates in facts: `owner_template` and `viewer_template`
- Frontend sends complete template JSON when creating resources
- Role-based delegation: automatically selects template based on recipient role

**Owner UCAN Format**:
```json
{
  "cap": {
    "domain:resource:abc123:main_doc": {"crud/merge": [{}]},
    "domain:resource:abc123:comments": {"crud/merge": [{}]}
  },
  "fct": {
    "owner_template": {
      "capabilities": {
        "main_doc": "crud/merge",
        "comments": "crud/merge"
      }
    },
    "viewer_template": {
      "capabilities": {
        "main_doc": "crud/readonly",
        "comments": "crud/merge"
      },
      "no_update_from_node": ["cart_state"],
      "dont_send_to_node": ["cart_state"]
    }
  }
}
```

**Sync Behaviors**:
- `crud/readonly` - Pull only (viewer can't push)
- `crud/merge` - Bidirectional (collaborative)
- `crud/appendonly` - Push only (append to sovereign)

**Role-Based Delegation**:
- `owner` role → uses owner_template (full access)
- `node` role → uses owner_template (full access)
- `viewer` role → uses viewer_template (restricted access)

### 3. Service Layer = Encryption Boundary
**Decision**: Resource struct contains decrypted data. resource_service.rs handles encryption/decryption.

**Rationale**:
- Clean separation of concerns
- Business logic works with plain data
- Encryption details isolated in service layer

**Flow**:
```
DB (encrypted) ←→ resource_service ←→ Resource (decrypted) ←→ Business Logic
```

### 4. Resource.rs Has Business Logic
**Decision**: Resource model has methods for filtering, state vectors, updates. Service just coordinates.

**Rationale**:
- Resource knows its own structure
- Generic filtering logic (uses UCAN parsing)
- Service stays thin - just encrypt/decrypt/persist

### 5. No Over-Engineering
**Decision**: Start minimal. Only implement what's needed. Add features during testing.

**Rationale**:
- Unknown unknowns will emerge during implementation
- Simpler to refactor minimal code
- Faster to get to working prototype

### 6. Breaking Changes Acceptable
**Decision**: No backward compatibility with Yrs. Clean slate.

**Rationale**:
- Better architecture without legacy constraints
- Yrs → Loro is fundamentally different
- No production users yet

### 7. Metadata for Unencrypted Data
**Decision**: Resource.metadata field contains unencrypted JSON (title, search config).

**Rationale**:
- Search indexing without decryption
- Quick metadata access
- Title display without loading docs

---

## Implementation Phases

### Phase 1: Core Protocol Foundation
**Status**: ✅ Complete (100%)

**Files Changed**:
- `core/src/models/document.rs` - Rewritten for Loro ✅
- `core/src/models/resource.rs` - Complete rewrite with new methods ✅
- `crypto_utils/src/ucan_utils.rs` - UCAN extractors and flexible generation ✅
- `crypto_utils/src/crypto_utils.rs` - Flexible UCAN wrappers ✅
- `services/src/errors.rs` - Added LoroError ✅

**Tasks Completed**:
- [x] Rewrite document.rs with Loro wrappers ✅
  - [x] create_doc()
  - [x] import_snapshot()
  - [x] import_into()
  - [x] export_snapshot()
  - [x] export_shallow_snapshot()
  - [x] export_updates()
  - [x] apply_updates()
  - [x] oplog_vv()
  - [x] state_frontiers()
- [x] Add LoroError to ServiceError ✅
- [x] Update Resource struct ✅
  - [x] Two-struct pattern: EncryptedResource (DB) + Resource (runtime)
  - [x] Resource has: id, folder_id, ucan_token, metadata, docs HashMap
  - [x] No cached state vectors (computed on-demand)
  - [x] ResourceType moved to metadata field
- [x] Add Resource methods ✅
  - [x] from_decrypted_data() - Load from decrypted JSON
  - [x] filter_to_send() - Filter docs based on two UCANs
  - [x] get_state_vectors() - Export state vectors as JSON
  - [x] generate_updates() - Generate updates from peer's state
  - [x] apply_updates() - Apply with permission validation
  - [x] apply_updates_filtered() - Apply with viewer filtering
  - [x] to_json() - Export for encryption
- [x] Extend ucan_utils ✅
  - [x] extract_resource_id_from_ucan() (already existed)
  - [x] extract_folder_id_from_ucan() (already existed)
  - [x] extract_doc_capabilities() - Extract doc permissions
  - [x] extract_facts() - Extract UCAN facts
  - [x] extract_owner_template() - Extract owner template
  - [x] extract_viewer_template() - Extract viewer template
  - [x] extract_ucan_facts() - Extract UcanFacts struct
  - [x] generate_flexible_resource_owner_ucan() - Flexible UCAN generation

**Dependencies**: None

**Actual Time**: 3 sessions

---

### Phase 2: Service Layer
**Status**: ✅ Complete (100%)

**Files Changed**:
- `services/src/resource_service.rs` - Complete rewrite (1200+ lines → 724 lines, 40% reduction) ✅
- `crypto_utils/src/crypto_utils.rs` - Added flexible UCAN delegation ✅

**Key Achievements**:
- 40% code reduction (34 methods → 14 methods)
- Removed all sync methods (deferred to Phase 3)
- Removed vector clock methods
- Removed resource keys methods
- Unified decrypt_resources helper
- Role-based sharing with template selection

**Tasks Completed**:
- [x] Complete rewrite of resource_service.rs ✅
  - [x] decrypt_resources() - Unified helper for batch decryption
  - [x] create_resource() - With flexible UCAN template JSON
  - [x] get_resource_by_id_direct() - Fetch single resource
  - [x] update_resource() - Replace Loro snapshots
  - [x] delete_resource() - Soft delete
  - [x] get_resources_for_folder() - List folder resources
  - [x] get_all_resources() - List all user resources
  - [x] share_resource() - Role-based sharing with auto template selection
  - [x] get_share_records_for_resource() - List shares
  - [x] get_resource_ucan_key() - Get UCAN keypair
  - [x] validate_authority_for_update() - Permission validation
  - [x] resolve_proof() - UCAN proof resolution
- [x] Add flexible delegation to crypto_utils ✅
  - [x] issue_flexible_delegated_resource_ucan() - Role-based delegation
  - [x] Automatic template selection (owner/node vs viewer)
- [x] Integration with existing repository layer ✅

**Methods Removed** (20 total):
- 9 sync methods (move_resource_in, resource_update_out, etc.)
- 3 vector clock methods
- 2 resource keys methods
- 6 helper methods (toggle_fav, update_last_accessed, auto_share, etc.)

**Dependencies**: Phase 1

**Actual Time**: 1 session (with 15 design questions)

---

### Phase 2.5: Tauri Handler Layer (Sthalam Integration)
**Status**: ✅ Complete (100%)

**Files Changed**:
- `tauri_handlers/src/handlers/resource.rs` - Resource handlers ✅
- `tauri_handlers/src/types/common.rs` - Input/Output types ✅
- `sthalam/src-tauri/src/lib.rs` - Handler registration ✅
- `sthalam/frontend/desktop/src/utils/helper.ts` - TypeScript types ✅
- `core/src/repositories/mod.rs` - Updated update_resource trait ✅
- `persistance/src/repositories/resource_repository.rs` - Updated implementation ✅
- `services/src/resource_service.rs` - Updated update_resource with AES key rotation ✅

**Key Achievements**:
- Batch metadata API (replaced event-based loading)
- AES key rotation on every update (forward secrecy)
- Complete resource CRUD handlers
- CamelCase serialization for TypeScript
- Usage tracking for cleanup

**Tasks Completed**:
- [x] Implement handle_add_resource ✅
  - Input: AddResourceInput (resourcePayload, folderId, resourceType, ucanTemplateJson, metadataJson)
  - Output: BaseCryptoResponse::ResourceCreated(ResourceMetadata)
  - Creates resource with owner UCAN template
- [x] Implement handle_get_resource ✅
  - Input: GetResource (resourceId)
  - Output: BaseCryptoResponse::SelectedResourceResponse(ResourceResponse)
  - Fetches and decrypts full resource data
- [x] Implement handle_get_all_resources_metadata ✅
  - Input: None
  - Output: BaseCryptoResponse::ResourcesMetadata(Vec<ResourceMetadata>)
  - Batch API replaces resource-added events
  - No decryption (metadata is unencrypted)
- [x] Implement handle_update_resource ✅
  - Input: UpdateResourceInput (id, data)
  - Output: BaseCryptoResponse::ResourceUpdated(ResourceMetadata)
  - **AES Key Rotation**: Generates NEW AES key on every update
  - Updates both encrypted_data AND encrypted_key in database
  - Forward secrecy: Old encrypted data can't be decrypted with old key
- [x] Update repository layer ✅
  - Updated ResourceRepository trait: `update_resource(data, encrypted_key, resource_id)`
  - Updated SqliteResourceRepository implementation
  - Updates 3 fields: encrypted_data, encrypted_key, updated_at
- [x] Add TypeScript types ✅
  - UpdateResourceInput, ResourceMetadata, ResourceResponse
  - CamelCase serialization with serde
- [x] Function usage tracking ✅
  - Marked handlers as ✅ ACTIVE, ⏳ TODO, or ❌ REMOVED
  - Documented complete flow in FUNCTION_USAGE_TRACKER.md
  - Tracked crypto_utils and repository functions used

**Security Implementation - AES Key Rotation**:
```rust
// services/src/resource_service.rs:313
let (encrypted_data, encrypted_key) = encrypt_data_for_user(&data, &user.public_key)?;
// Generates NEW AES key on every call
```

**Repository Update**:
```rust
// persistance/src/repositories/resource_repository.rs:127
diesel::update(resources::table.find(resource_id))
    .set((
        resources::encrypted_data.eq(data),
        resources::encrypted_key.eq(encrypted_key),  // NEW KEY
        resources::updated_at.eq(now),
    ))
```

**Phase 3 Consideration**:
- Multi-user sharing will need to distribute new AES keys via P2P
- Owner updates resource → generates new key → notifies shared users
- Users update their local encrypted copies independently

**Dependencies**: Phase 2

**Actual Time**: 1 session

---

### Phase 3: Network Protocol
**Status**: Not Started

**Files to Change**:
- `core/src/models/p2p.rs` - Update message types
- `sthalam/src-tauri/src/listeners/resource_handler.rs` - NEW file

**Tasks**:
- [ ] Update p2p.rs messages
  - [ ] ResourceAdd
  - [ ] StateVectorRequest
  - [ ] StateVectorResponse
  - [ ] AssetTransfer
- [ ] Create resource_handler.rs
  - [ ] handle_resource_add()
  - [ ] handle_state_vector_request()
  - [ ] handle_state_vector_response()
  - [ ] handle_asset_transfer()
- [ ] Wire up to network listeners

**Dependencies**: Phase 2

**Estimated Time**: 1 week

---

### Phase 4: Search & Indexing
**Status**: Not Started

**Files to Change**:
- `search_indexer/src/extractor.rs` - Rewrite for Loro

**Tasks**:
- [ ] Rewrite extractor.rs
  - [ ] extract_content() - generic Loro text extraction
  - [ ] extract_text_from_doc() - traverse Loro structure
  - [ ] Read search config from Resource.metadata
  - [ ] Extract title from metadata (unencrypted)
- [ ] Integrate with resource_handler (trigger on ResourceAdd)

**Dependencies**: Phase 3

**Estimated Time**: 3 days

---

### Phase 5: Database Migration
**Status**: Not Started

**Files to Change**:
- Database schema
- Repository implementations

**Tasks**:
- [ ] Create migration script
  - [ ] Add new columns (encrypted_data, encrypted_key, cached_state_vector, ucan_token, metadata)
  - [ ] Drop old columns (data, signature, favourite, deleted, etc.)
  - [ ] Drop resource_vectors table
  - [ ] Drop resource_keys table
- [ ] Update repository code for new schema
- [ ] Test migration on dev database

**Dependencies**: Phase 4

**Estimated Time**: 2 days

---

### Phase 6: Cleanup
**Status**: Not Started

**Files to Delete**:
- `core/src/repositories/resource_vector.rs`
- `core/src/repositories/resource_key.rs`

**Tasks**:
- [ ] Delete resource_vector.rs
- [ ] Delete resource_key.rs
- [ ] Remove imports/references to deleted files
- [ ] Clean up dead code in other files

**Dependencies**: Phase 5

**Estimated Time**: 1 day

---

### Phase 7: Integration & Testing
**Status**: Not Started

**Tasks**:
- [ ] Unit tests
  - [ ] document.rs Loro operations
  - [ ] Resource filtering
  - [ ] UCAN extraction
- [ ] Integration tests
  - [ ] Owner publishes resource
  - [ ] Viewer requests with limited UCAN
  - [ ] Bidirectional sync (merge)
  - [ ] Append-only sync
- [ ] E2E manual testing
  - [ ] Full owner → node → viewer flow
  - [ ] Search indexing
  - [ ] Asset sync

**Dependencies**: Phase 6

**Estimated Time**: 1 week

---

## File-Level Changes

### Files to Create
```
services/src/resource_service.rs
sthalam/src-tauri/src/listeners/resource_handler.rs
```

### Files to Rewrite
```
core/src/models/document.rs         - Yrs → Loro, same interface
search_indexer/src/extractor.rs     - Loro-based text extraction
```

### Files to Update
```
core/src/models/resource.rs         - New struct + methods
core/src/models/p2p.rs              - New message types
crypto_utils/src/ucan_utils.rs      - UCAN extractors
```

### Files to Delete
```
core/src/repositories/resource_vector.rs
core/src/repositories/resource_key.rs
```

### Files to Ignore (Not Migrating)
```
livnote/**/*                        - Focus on Sthalam only
```

---

## Database Schema Changes

### New Columns
```sql
ALTER TABLE resources ADD COLUMN encrypted_data TEXT;
ALTER TABLE resources ADD COLUMN encrypted_key TEXT;
ALTER TABLE resources ADD COLUMN cached_state_vector TEXT;
ALTER TABLE resources ADD COLUMN ucan_token TEXT;
ALTER TABLE resources ADD COLUMN metadata TEXT;
```

### Columns to Keep
```
id, resource_type, folder_id, created_at, updated_at
```

### Columns to Drop
```sql
ALTER TABLE resources DROP COLUMN data;
ALTER TABLE resources DROP COLUMN signature;
ALTER TABLE resources DROP COLUMN favourite;
ALTER TABLE resources DROP COLUMN deleted;
ALTER TABLE resources DROP COLUMN deleted_at;
ALTER TABLE resources DROP COLUMN created_folder_id;
ALTER TABLE resources DROP COLUMN created_by;
ALTER TABLE resources DROP COLUMN last_accessed;
```

### Tables to Drop
```sql
DROP TABLE resource_vectors;
DROP TABLE resource_keys;
```

---

## Risk Assessment

### High Risk
1. **UCAN token format** - FE and BE must agree on structure
   - Mitigation: Document format clearly, validate early

2. **Encryption key management** - Key rotation, viewer-specific keys
   - Mitigation: Test thoroughly, document key lifecycle

### Medium Risk
1. **Loro library maturity** - Less battle-tested than Yrs
   - Mitigation: Extensive testing, follow Loro best practices

2. **Performance** - Loro snapshot sizes, state vector computation
   - Mitigation: Profile, optimize hot paths

### Low Risk
1. **Database migration** - Schema changes
   - Mitigation: Test on dev, backup production

2. **Search indexing** - New text extraction logic
   - Mitigation: Compare results with old indexer

---

## Success Criteria

- [ ] Owner can publish resource with multiple docs
- [ ] Node stores encrypted resource
- [ ] Viewer can request with UCAN
- [ ] Node filters docs based on UCAN capabilities
- [ ] Viewer can update merge docs
- [ ] Viewer can append to appendonly docs
- [ ] Viewer cannot update readonly docs
- [ ] Search indexing works
- [ ] Title displays without decrypting
- [ ] Assets sync separately

---

## Notes & Open Questions

### Questions
- Q: How large can assets get? Should we stream?
  - A: TBD during testing

- Q: Should we cache decrypted resources in memory?
  - A: TBD based on performance testing

- Q: Rate limiting for viewer updates?
  - A: Not implementing initially, add if needed

### Design Decisions to Revisit
- Metadata format (might need versioning)
- Asset sync protocol (might need chunking)
- State vector caching strategy

---

## Progress Log

### 2025-11-06 - Session 1
**Planning & document.rs Implementation**

**Completed**:
- ✅ Architecture planning and design decisions
- ✅ Created MIGRATION_PLAN.md
- ✅ Created KNOWLEDGE_BASE.md
- ✅ Created HANDOFF.md
- ✅ Added loro = "1.5" to core/Cargo.toml
- ✅ Added loro = "1.5" to sthalam/src-tauri/Cargo.toml
- ✅ Removed yrs dependencies
- ✅ Completely rewrote core/src/models/document.rs for Loro
  - All 9 functions implemented and documented
  - Shallow snapshot support for viewers
  - Version vector handling (oplog_vv vs state_frontiers)
- ✅ Added LoroError to services/src/errors.rs

**Key Decisions Made**:
1. Viewers use shallow snapshots (no history) via `export_shallow_snapshot()`
2. Owner/Node use full snapshots via `export_snapshot()`
3. Viewers track state with `state_frontiers()`, Owner/Node use `oplog_vv()`
4. Three export modes: Snapshot, Shallow Snapshot, Updates
5. UCAN-driven doc structure and merge logic
6. Service layer = encryption boundary
7. Breaking changes acceptable (no Yrs backward compatibility)

**Next Session Should Start With**:
- resource.rs design questions
- Ask about struct fields, methods, encryption handling
- Follow same planning-before-coding pattern

**Status**: ~15% complete (Phase 1.1 done, 1.2 and 1.3 remaining)

---

### 2025-11-07 - Session 2
**Resource.rs Implementation**

**Completed**:
- ✅ Designed two-struct architecture (EncryptedResource vs Resource)
- ✅ Removed ResourceType enum, moved to metadata field
- ✅ Implemented Resource struct with docs HashMap
- ✅ Implemented all 7 core Resource methods:
  - from_decrypted_data() - Load Loro docs from decrypted JSON
  - filter_to_send() - Two-UCAN filtering with shallow snapshots
  - get_state_vectors() - Export state vectors as JSON
  - generate_updates() - Generate incremental updates
  - apply_updates() - Apply with permission validation
  - apply_updates_filtered() - Apply with viewer filtering
  - to_json() - Export for encryption
- ✅ Compilation successful

**Key Decisions Made**:
1. Two-struct pattern: EncryptedResource (DB) + Resource (runtime)
2. ResourceType becomes metadata field, not enum
3. No cached state vectors (computed on-demand from LoroDoc)
4. Two-UCAN filtering (our UCAN + peer UCAN)
5. Always use shallow snapshots when sending to peers
6. Viewer filtering respects no_update_from_node rules

**Status**: ~40% complete (Phase 1 complete)

---

### 2025-11-07 - Session 3
**UCAN Utils Implementation**

**Completed**:
- ✅ Implemented UcanFacts struct
- ✅ Implemented UcanTemplate struct (capabilities + sync rules)
- ✅ Added 5 extraction functions to ucan_utils.rs:
  - extract_doc_capabilities() - Extract doc permissions from UCAN
  - extract_facts() - Extract facts section
  - extract_owner_template() - Extract owner template
  - extract_viewer_template() - Extract viewer template
  - extract_ucan_facts() - Extract complete UcanFacts struct
- ✅ Implemented generate_flexible_resource_owner_ucan():
  - Accepts ucan_template_json from frontend
  - Generates owner UCAN with both templates in facts
  - Returns (ucan_token, encrypted_private_key)
- ✅ Added wrapper in crypto_utils.rs
- ✅ Compilation successful

**Key Decisions Made**:
1. Two-template structure (owner_template + viewer_template)
2. Frontend controls UCAN structure via template JSON
3. UcanTemplate includes capabilities + no_update_from_node + dont_send_to_node
4. Flexible generation accepts complete template, not individual fields
5. Both templates stored in facts section of owner UCAN

**Status**: ~50% complete (Phase 1 complete)

---

### 2025-11-07 - Session 4
**Resource Service Complete Rewrite**

**Completed**:
- ✅ Asked 15 design questions before implementation
- ✅ Complete rewrite of resource_service.rs (1200+ → 724 lines)
- ✅ Implemented 14 core methods (removed 20 legacy methods)
- ✅ Added issue_flexible_delegated_resource_ucan() to crypto_utils
- ✅ Role-based delegation (owner/node/viewer)
- ✅ Automatic template selection based on recipient role
- ✅ All code compiles successfully
- ✅ Updated HANDOFF.md with session log

**14 Methods Implemented**:
1. decrypt_resources() - Unified batch decryption helper
2. create_resource() - With flexible UCAN template JSON
3. get_resource_by_id_direct() - Fetch single resource
4. update_resource() - Replace Loro snapshots
5. delete_resource() - Soft delete
6. get_resources_for_folder() - List folder resources
7. get_all_resources() - List all user resources
8. share_resource() - Role-based sharing
9. get_share_records_for_resource() - List shares
10. get_resource_ucan_key() - Get UCAN keypair
11. validate_authority_for_update() - Permission validation
12. resolve_proof() - UCAN proof resolution
13-14. Internal helpers for encryption/decryption

**20 Methods Removed**:
- 9 sync methods (deferred to Phase 3)
- 3 vector clock methods
- 2 resource keys methods
- 6 UI/helper methods (toggle_fav, auto_share, etc.)

**Key Decisions Made**:
1. 40% code reduction strategy
2. Frontend sends complete Loro snapshots in update_resource()
3. Role-based delegation extracts role from recipient token
4. Owner and node roles use owner_template
5. Viewer role uses viewer_template
6. No auto-sharing (removed for now)
7. Unified decrypt helper handles both single and batch
8. Service returns Vec<Resource> for consistency

**Status**: 60% complete (Phase 2 complete)

---

### 2025-11-07 - Session 8
**Resource Update Bug Fix**

**Completed**:
- ✅ Fixed data corruption bug in resource update flow
- ✅ Removed metadata fields from frontend update payload
- ✅ Updated documentation with bug fix details

**Bug Details**:
- Frontend was sending metadata fields (client_id, last_modified, title) mixed with Loro documents
- Service encrypted entire payload including metadata
- Parser expected only document arrays, failed on string fields
- Error: "expected a sequence, got string '3708909928'"

**Fix**:
- Removed client_id, last_modified, title from loroContent in saveCurrentResource()
- Now only sends document snapshots: template_doc, content_doc, user_content_doc, collaborative_doc, submissions_doc, static_assets
- Matches add_resource pattern (documents vs metadata separation)

**Key Learning**:
- Resource::from_decrypted_data() expects HashMap<String, Vec<Value>>
- All fields in encrypted_data must be arrays (Loro document bytes)
- Metadata should remain separate (unencrypted in metadata column)
- Update flow is "save document state" not "update metadata"

**Files Modified**:
- `sthalam/frontend/desktop/src/state/data.svelte.ts` - Removed metadata from loroContent

---

**Last Updated**: 2025-11-07
