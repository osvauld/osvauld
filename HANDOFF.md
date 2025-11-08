# Migration Handoff Document

**Date**: 2025-11-07
**Current Status**: Phase 2.5 Complete - Tauri handler layer with AES key rotation
**Session**: Resource CRUD handlers complete (add, get, getAll, update)

---

## What's Been Completed

### 1. Dependencies Updated
- ✅ Added `loro = "1.5"` to `core/Cargo.toml`
- ✅ Added `loro = "1.5"` to `sthalam/src-tauri/Cargo.toml`
- ✅ Removed `yrs` from both files

### 2. document.rs Rewritten for Loro
**File**: `core/src/models/document.rs`

**Completed Functions**:
- `create_doc()` - Creates new LoroDoc
- `import_snapshot(bytes)` - Creates doc from snapshot
- `import_into(doc, bytes)` - Imports into existing doc
- `export_snapshot(doc)` - Full snapshot with history (owner/node)
- `export_shallow_snapshot(doc)` - Shallow snapshot without history (viewers)
- `export_updates(doc, from_version)` - Incremental updates from version vector
- `apply_updates(doc, updates)` - Apply incoming updates
- `oplog_vv(doc)` - Version vector with full history (owner/node use this)
- `state_frontiers(doc)` - Current state frontiers (viewers use this)

**Compilation**: ✅ Compiles successfully

### 3. Error Handling Updated
**File**: `services/src/errors.rs`

Added to `ResourceServiceError`:
```rust
#[error("Loro CRDT error: {0}")]
Loro(#[from] loro::LoroError),
```

**Compilation**: ✅ Compiles successfully

---

## Key Technical Decisions

### Shallow Snapshots for Viewers
- Loro supports `ExportMode::shallow_snapshot(&frontiers)` like Git shallow clone
- Viewers get documents WITHOUT full operation history
- Owner/Node keep full history for authoritative state
- Viewers can only sync updates from after their shallow snapshot point (acceptable)

### Version Tracking Strategy
**Owner/Node** use `oplog_vv()`:
- Returns VersionVector with complete operation history
- Used for authoritative storage
- Enables full history reconstruction

**Viewers** use `state_frontiers()`:
- Returns Frontiers representing current applied state
- Lighter weight, no history
- Sufficient for viewing and collaborative editing

### Export Modes
1. **ExportMode::Snapshot** - Full state + all history (owner/node storage)
2. **ExportMode::shallow_snapshot(&frontiers)** - State without history (send to viewers)
3. **ExportMode::updates(&version_vector)** - Incremental sync from a version

---

## Current Build Status

### What Compiles ✅
- ✅ `core/src/models/document.rs` - Loro wrappers (all type errors fixed)
- ✅ `core/src/models/resource.rs` - Resource model with Loro integration
- ✅ `crypto_utils/src/ucan_utils.rs` - UCAN extraction functions
- ✅ `services/src/errors.rs` - LoroError integration

### What Has Errors (Expected for Phase 2-3)
- ❌ `core/src/models/p2p.rs` - Imports legacy ResourceSyncData
- ❌ `core/src/models/sync.rs` - Imports legacy ResourceManifestData
- ❌ `core/src/repositories/mod.rs` - Imports legacy structs (ResourceKeyPair, ResourceManifestData, ResourceSyncData, ResourceWithKey)

**Expected Errors** (from cargo check):
```
error[E0432]: unresolved import `super::ResourceSyncData`
error[E0432]: unresolved import `super::resource::ResourceManifestData`
error[E0432]: unresolved imports `crate::models::ResourceKeyPair`, `ResourceManifestData`, `ResourceSyncData`, `ResourceWithKey`
```

These are normal - these files will be updated in Phase 2-3.

---

## Next Steps (Immediate)

### Step 1: Update resource.rs ✅ COMPLETED
**File**: `core/src/models/resource.rs`

**Completed**:
- Complete rewrite with two-struct architecture (EncryptedResource + Resource)
- ResourceType moved to metadata field
- All 7 core methods implemented
- Two-UCAN filtering logic
- Permission validation in apply_updates
- Basic UCAN parsing helpers (to be moved to ucan_utils.rs)
- All legacy code removed

### Step 2: Extend ucan_utils.rs ✅ COMPLETED
**File**: `crypto_utils/src/ucan_utils.rs`

**Completed**:
- ✅ Added 3 new error variants to UcanError
- ✅ Implemented `extract_doc_capabilities(token)` - Returns HashMap<doc_name, ability>
- ✅ Implemented `extract_template(token)` - Returns UcanTemplate from facts
- ✅ Implemented `extract_facts(token)` - General facts extractor
- ✅ Each function has internal helper taking `&Ucan` for efficiency
- ✅ Updated resource.rs to use new ucan_utils functions
- ✅ Added crypto_utils dependency to core/Cargo.toml
- ✅ Removed temporary UCAN parsing helpers from resource.rs

**UcanTemplate Struct**:
```rust
pub struct UcanTemplate {
    pub capabilities: HashMap<String, String>,
    pub no_update_from_node: Vec<String>,
    pub dont_send_to_node: Vec<String>,
}
```

### Step 3: Phase 2.5 - Tauri Handler Layer ✅ COMPLETED
**Files**: Multiple handler, type, and service files

**Completed**:
- ✅ Implemented 4 resource handlers (add, get, getAll, update)
- ✅ Added AES key rotation on resource updates
- ✅ Batch metadata API (replaced event-based loading)
- ✅ CamelCase serialization for TypeScript integration
- ✅ Updated repository layer for key rotation
- ✅ Function usage tracking in FUNCTION_USAGE_TRACKER.md

**Handlers Implemented**:
1. **handle_add_resource** - `tauri_handlers/src/handlers/resource.rs:18`
   - Creates resource with UCAN templates
   - Returns ResourceMetadata

2. **handle_get_resource** - `tauri_handlers/src/handlers/resource.rs:74`
   - Fetches and decrypts full resource
   - Returns ResourceResponse with Loro documents

3. **handle_get_all_resources_metadata** - `tauri_handlers/src/handlers/resource.rs:114`
   - Batch API for all resources
   - No decryption (metadata is unencrypted)
   - Replaces resource-added events

4. **handle_update_resource** - `tauri_handlers/src/handlers/resource.rs:168`
   - **AES Key Rotation**: Generates NEW AES key on every update
   - Updates encrypted_data AND encrypted_key in database
   - Forward secrecy benefit

**Key Rotation Implementation**:
```rust
// services/src/resource_service.rs:313
let (encrypted_data, encrypted_key) = encrypt_data_for_user(&data, &user.public_key)?;
// Generates NEW random AES key on every call

// persistance/src/repositories/resource_repository.rs:127-131
diesel::update(resources::table.find(resource_id))
    .set((
        resources::encrypted_data.eq(data),
        resources::encrypted_key.eq(encrypted_key),  // NEW KEY
        resources::updated_at.eq(now),
    ))
```

**Repository Updates**:
- Updated `ResourceRepository` trait: `update_resource(data, encrypted_key, resource_id)`
- Updated `SqliteResourceRepository` implementation
- Updates 3 fields per update: encrypted_data, encrypted_key, updated_at

**Types Added**:
- `UpdateResourceInput` - Request type with id and data
- `ResourceMetadata` - Response type for metadata operations
- `ResourceResponse` - Full resource data response
- `ResourceUpdated` variant in `BaseCryptoResponse`

**Security Benefits**:
- Forward secrecy: Old encrypted data unreadable if old key compromised
- Fresh encryption envelope on every update
- Simple single-user implementation (Phase 2.5)
- P2P key distribution consideration for Phase 3 multi-user

---

## Architecture Reference

### Device Topology
```
Owner's Desktop ←→ Owner's Node ←→ Viewers
```

- **Owner's Desktop**: Separate device (laptop), creates content
- **Owner's Node**: Separate device (Raspberry Pi), serves viewers
- **Viewers**: Request content from node

### Data Flow

**Owner Publishes**:
```
Desktop → ResourceAdd(full snapshot) → Node stores
```

**Viewer Requests**:
```
Viewer → StateVectorRequest(state_frontiers) → Node
Node → filter docs → export_shallow_snapshot → Viewer
```

**Viewer Updates (Bidirectional)**:
```
Viewer → StateVectorRequest(with updates) → Node
Node → merge → relay to Owner
```

### UCAN Sync Behaviors

| Ability | Behavior | Owner→Viewer | Viewer→Node |
|---------|----------|--------------|-------------|
| `crud/readonly` | Pull only | ✅ Send updates | ❌ Reject |
| `crud/merge` | Bidirectional | ✅ Send updates | ✅ Accept & merge |
| `crud/appendonly` | Push only | ❌ Don't send | ✅ Accept appends |

---

## Important References

### Documentation Files
1. **MIGRATION_PLAN.md** - Phase breakdown, tasks, progress tracking
2. **KNOWLEDGE_BASE.md** - Technical details, algorithms, learnings
3. **This file (HANDOFF.md)** - Current status and next steps

### Key Loro API Docs
- Shallow snapshots: https://loro.dev/docs/advanced/shallow_snapshot
- Export modes: https://loro.dev/docs/tutorial/encoding
- API reference: https://docs.rs/loro/latest/loro/

### Code Files Modified
- `core/Cargo.toml` - Added loro dependency
- `sthalam/src-tauri/Cargo.toml` - Added loro dependency
- `core/src/models/document.rs` - Completely rewritten
- `services/src/errors.rs` - Added LoroError variant

### Code Files To Modify Next
- `core/src/models/resource.rs` - Update struct + methods
- `crypto_utils/src/ucan_utils.rs` - Add UCAN extractors

---

## Questions Answered This Session

### Q: Should viewers have full oplog history?
**A**: No. Viewers use `state_frontiers()` for current state only. Owner/Node use `oplog_vv()` for full history.

### Q: What's the difference between oplog_vv and state_frontiers?
**A**:
- `oplog_vv()` - All recorded operations, includes history
- `state_frontiers()` - Current applied state, no history
- For sync: Use oplog for owner/node, state_frontiers for viewers

### Q: How to send docs to viewers without history?
**A**: Use `export_shallow_snapshot()` which calls `ExportMode::shallow_snapshot(&frontiers)`

### Q: Can viewers collaborate on merge docs without history?
**A**: Yes - Loro's CRDT works with state_frontiers. They don't need full oplog for collaboration.

---

## Open Questions for Next Session

### Resource Model Design
1. Should docs be `HashMap<String, LoroDoc>` or different structure?
2. How to handle encrypted_data field - JSON format?
3. Should cached_state_vector be per-doc HashMap?
4. What metadata format for unencrypted data?

### UCAN Parsing
1. Exact format of capability strings from frontend?
2. How to handle multiple doc capabilities in one token?
3. Where to store merge logic - in facts or derive from ability?

### Service Layer
1. When to decrypt - in service or in resource?
2. How to handle viewer re-encryption?
3. State vector caching strategy?

---

## How to Continue

### 1. Review Documentation
Read in order:
1. This file (HANDOFF.md) - Current status
2. MIGRATION_PLAN.md - Overall plan
3. KNOWLEDGE_BASE.md - Technical details

### 2. Check Current State
```bash
cd /home/abe/osvauld/core
cargo check  # Should show resource.rs errors
```

### 3. Start resource.rs
Before writing code, ask questions about:
- Resource struct design
- Which fields to keep/remove/add
- Method signatures
- Encryption handling

### 4. Follow Planning Mode Pattern
- Ask detailed questions before implementing
- Update KNOWLEDGE_BASE.md with learnings
- Update MIGRATION_PLAN.md with progress
- Document decisions and rationale

---

## Files to Reference

### Current Implementation
- `core/src/models/document.rs` - See Loro wrapper pattern
- `services/src/errors.rs` - See error handling pattern

### Old Implementation (for reference)
- `core/src/models/resource.rs` - Current Yrs version (needs migration)
- Any file using `YjsDocExt` trait (being removed)

### Frontend Examples
- `sthalam/frontend/desktop/src/shared/loro/loroCoordinator.ts` - How FE uses Loro
- Shows doc structure, snapshot handling, version vectors

---

## Critical Path

```
✅ Phase 1.1: document.rs (DONE)
✅ Phase 1.2: resource.rs (DONE)
✅ Phase 1.3: ucan_utils.rs (DONE)
✅ Phase 2: resource_service.rs + UCAN templates (DONE - 40% code reduction)
🔄 Phase 2.5: Handler layer implementation (IN PROGRESS - 4/14 handlers done)
→ Phase 3: Network protocol (NEXT)
→ Phase 4: Search indexer
→ Phase 5: Database migration
```

**Estimated Progress**: 65% complete (Phases 1-2 complete, Phase 2.5 in progress)

---

## Session Log

### Session 2: 2025-11-06 - Resource.rs Design & Implementation

**Approach**: Interactive planning mode with granular questions

**Design Questions Answered** (19 total):
1. Struct architecture - Separate EncryptedResource and Resource
2. Document storage - LoroDoc HashMap in memory
3. Version vector caching - No cache, compute on-demand
4. Role awareness - No role field, extract from UCAN dynamically
5. Method structure - Separate methods (not combined)
6. JSON parsing - Resource.rs handles it internally
7. Filtering return type - HashMap of doc snapshots
8. UCAN structure - Template in facts with capabilities and sync rules
9. ResourceType handling - Moved to metadata field
10. Loro merge capabilities - Regular merge for now (no append-only)
11. Method signatures - Structured types with JSON parsing
12. Sync methods - Separate apply_updates and generate_updates
13. Two-UCAN filtering - Both our_ucan and peer_ucan required
14. Permission validation - Check crud/* capabilities before applying
15. Filtered updates - apply_updates_filtered for no_update_from_node
16. Always shallow snapshots when sending to peers
17. UCAN facts structure - Template with capabilities and sync rules
18. Legacy code - All removed for clean implementation
19. Method count - 7 core methods implemented

**Key Architectural Decisions**:
1. **Two-Struct Pattern**: EncryptedResource (storage) vs Resource (runtime)
2. **UCAN-Driven**: Roles and permissions extracted from tokens dynamically
3. **No Caching**: Version vectors computed on-demand from LoroDoc
4. **ResourceType in Metadata**: No longer a struct field, just UI hint
5. **Two-UCAN Filtering**: Filter docs based on both sender and receiver UCANs
6. **Always Shallow**: Send shallow snapshots to all peers
7. **Regular Merge**: Use standard CRDT merge for all capabilities (append-only deferred)

**UCAN Template Structure Defined**:
```json
{
  "cap": {
    "domain:resource:id:doc_name": {"crud/ability": [{}]}
  },
  "fct": {
    "template": {
      "capabilities": {"doc_name": "crud/ability"},
      "no_update_from_node": ["uiState"],
      "dont_send_to_node": ["uiState"]
    }
  }
}
```

**Implementation Completed**:
- ✅ EncryptedResource struct (6 fields)
- ✅ Resource struct (5 fields with docs HashMap)
- ✅ DocUpdate helper struct
- ✅ from_decrypted_data() - Load from JSON
- ✅ filter_to_send() - Two-UCAN filtering + shallow snapshots
- ✅ get_state_vectors() - Return all version vectors
- ✅ generate_updates() - Incremental sync with filtering
- ✅ apply_updates() - With permission validation
- ✅ apply_updates_filtered() - With no_update_from_node check
- ✅ to_json() - Export for storage
- ✅ parse_ucan_facts() helper
- ✅ parse_ucan_capabilities() helper
- ✅ All legacy code removed

**Documentation Updated**:
- ✅ KNOWLEDGE_BASE.md - Complete resource model section with all method flows
- ✅ KNOWLEDGE_BASE.md - Added append-only signature verification note for future
- ✅ HANDOFF.md - Updated with session progress

**Files Modified**:
- `core/src/models/resource.rs` - Complete rewrite (515 lines)
- `KNOWLEDGE_BASE.md` - Resource Model section expanded
- `HANDOFF.md` - Progress tracking updated

**Compilation Status**: Resource.rs complete, rest of codebase has expected errors (will fix in Phase 2-3)

**Next Session Should Start With**:
1. Implement ucan_utils.rs functions (Phase 1.3)
2. Move UCAN parsing from resource.rs to ucan_utils.rs
3. Add extract_resource_id, extract_folder_id, extract_doc_capabilities
4. Then proceed to resource_service.rs (Phase 2)

**Estimated Progress**: Phase 1 is 66% complete (2/3 tasks done)

---

### Session 3: 2025-11-07 - UCAN Utils Implementation

**Approach**: Interactive planning mode with granular questions before implementation

**Design Questions Answered** (11 total):
1. Function pattern - Public string API + internal Ucan helpers (parse once, use many)
2. Caveat structure - Discussed potential uses (sync_mode, quotas, rate limiting) - deferred to later
3. Append-only enforcement - Regular merge for now, defer signature verification
4. Capability return type - Flat HashMap (one UCAN = one resource with sub-docs)
5. Template structure - Parse once with UcanTemplate struct
6. Function list confirmed - extract_doc_capabilities, extract_template, extract_facts
7. Error handling strategy - Return errors for strict validation
8. Error types - Add specific error variants (TemplateNotFound, TemplateInvalid, NoDocumentCapabilities)
9. Edge cases - Deferred discussion, focus on making it functional first
10. One UCAN = one resource confirmed
11. Implementation approval - Proceed with implementation

**Key Technical Decisions**:
1. **Parse-Once Pattern**: Public functions take `&str`, internal helpers take `&Ucan` for efficiency
2. **Strict Validation**: Return errors instead of defaulting to empty values
3. **Defer Complexity**: Edge cases, sync mode caveats, and append-only deferred to later
4. **Flat Structure**: extract_doc_capabilities returns HashMap<String, String> directly
5. **BTreeMap Conversion**: UCAN facts() returns BTreeMap, needs conversion to JSON Map
6. **Remove Temporary Code**: Clean up resource.rs temporary UCAN parsing helpers

**UCAN Extraction Functions Implemented**:
```rust
// Public API (takes &str)
pub fn extract_facts(token: &str) -> Result<Option<Map<String, Value>>, UcanError>
pub fn extract_template(token: &str) -> Result<Option<UcanTemplate>, UcanError>
pub fn extract_doc_capabilities(token: &str) -> Result<HashMap<String, String>, UcanError>

// Internal helpers (takes &Ucan for efficiency)
fn extract_facts_from_ucan(ucan: &Ucan) -> Option<Map<String, Value>>
fn extract_template_from_ucan(ucan: &Ucan) -> Result<Option<UcanTemplate>, UcanError>
fn extract_doc_capabilities_from_ucan(ucan: &Ucan) -> Result<HashMap<String, String>, UcanError>
```

**UcanTemplate Struct**:
```rust
pub struct UcanTemplate {
    pub capabilities: HashMap<String, String>,
    pub no_update_from_node: Vec<String>,
    pub dont_send_to_node: Vec<String>,
}
```

**Implementation Completed**:
- ✅ Added 3 new error variants to UcanError enum
- ✅ Implemented extract_facts() with BTreeMap→Map conversion
- ✅ Implemented extract_template() with struct parsing
- ✅ Implemented extract_doc_capabilities() with capability parsing
- ✅ All 3 internal helper functions implemented
- ✅ crypto_utils compiles successfully
- ✅ Updated resource.rs to use new functions (4 method updates)
- ✅ Removed temporary UCAN parsing helpers from resource.rs (~55 lines)
- ✅ Added crypto_utils dependency to core/Cargo.toml
- ✅ Fixed export_shallow_snapshot() Result type mismatches

**Documentation Updated**:
- ✅ KNOWLEDGE_BASE.md - UCAN Extraction Algorithm section (lines 82-133)
- ✅ HANDOFF.md - Step 2 marked complete, Critical Path updated

**Files Modified**:
- `crypto_utils/src/errors.rs` - Added 3 error variants
- `crypto_utils/src/ucan_utils.rs` - Added ~185 lines (3 public functions + 3 helpers + struct)
- `core/src/models/resource.rs` - Removed ~55 lines, updated 4 methods to use new functions
- `core/Cargo.toml` - Added crypto_utils dependency
- `KNOWLEDGE_BASE.md` - Updated UCAN Extraction Algorithm section
- `HANDOFF.md` - Progress tracking updated

**Compilation Status**:
- ✅ crypto_utils compiles successfully
- ✅ resource.rs uses new ucan_utils functions
- ⏳ Remaining errors are expected (document.rs type mismatches from Phase 1.1, legacy imports for Phase 2-3)

**Type Error Fixes (Phase 1.1 completion)**:
After implementing Phase 1.3, fixed all document.rs type mismatches from Loro API changes:
- ✅ `import_into()` - Map Result<ImportStatus> to Result<()>
- ✅ `export_snapshot()` - Unwrap Result<Vec<u8>, LoroEncodeError> with expect
- ✅ `export_shallow_snapshot()` - Unwrap Result with expect
- ✅ `export_updates()` - Unwrap inner Result with expect
- ✅ `apply_updates()` - Map Result<ImportStatus> to Result<()>
- ✅ Removed unused imports from resource.rs

**Final Compilation Status**:
- ✅ All Phase 1 files compile successfully
- ✅ document.rs, resource.rs, ucan_utils.rs all working
- ⏳ Only legacy struct import errors remain (expected for Phase 2-3)

**Estimated Progress**: Phase 1 is 100% complete (3/3 tasks done) - Overall 40% complete

---

### Session 4: 2025-11-07 - Phase 2: resource_service.rs & UCAN Templates

**Approach**: Interactive planning mode with detailed questions, then complete rewrite

**Design Questions Answered** (15 total):
1. DecryptedResource location - Use domain Resource struct directly (it IS DecryptedResource)
2. Missing service fields - Keep timestamps/favourite in EncryptedResource only
3. ResourceType enum - Extract from UCAN when needed, no separate field
4. Code reduction strategy - Remove 20 methods, keep 14 core methods
5. Sync methods - All removed, deferred to Phase 3
6. UI helpers - Removed toggle_fav and update_last_accessed
7. Vector clock methods - All removed
8. Resource keys methods - Removed
9. Data storage format - Option A (simple Loro snapshots)
10. create_resource signature - Accept ucan_template_json from frontend
11. UCAN template structure - Two templates (owner_template, viewer_template)
12. User roles - Owner/Node/Viewer with template selection
13. Sharing flow - Role-based, auto-select template
14. Decryption helpers - One unified decrypt_resources for single & batch
15. Update flow - Option A (frontend sends complete snapshots)

**Key Architectural Decisions**:
1. **Two-Template UCAN Facts**: owner_template (full access), viewer_template (restricted)
2. **Role-Based Delegation**: Recipient role determines which template to use
3. **40% Code Reduction**: From 1200+ lines to 724 lines in resource_service.rs
4. **No Sync Methods**: Deferred to Phase 3 network protocol redesign
5. **UCAN-Driven**: No ResourceType field, extract from UCAN when needed
6. **Simplified Flow**: Frontend sends complete snapshots, no diff computation

**UCAN Facts Structure**:
```json
{
  "owner_template": {
    "capabilities": {"main_doc": "crud/merge", ...},
    "no_update_from_node": [],
    "dont_send_to_node": []
  },
  "viewer_template": {
    "capabilities": {"main_doc": "crud/readonly", ...},
    "no_update_from_node": ["main_doc"],
    "dont_send_to_node": ["main_doc"]
  }
}
```

**Implementation Completed**:

**crypto_utils/src/ucan_utils.rs** (~150 lines added):
- ✅ `UcanFacts` struct with owner_template and viewer_template
- ✅ `extract_ucan_facts()` - Extract both templates
- ✅ `extract_owner_template()` - Extract owner template only
- ✅ `extract_viewer_template()` - Extract viewer template only
- ✅ `extract_template_from_facts()` - Helper for parsing templates
- ✅ `generate_flexible_resource_owner_ucan()` - Accept templates from frontend

**crypto_utils/src/crypto_utils.rs** (~90 lines added):
- ✅ `generate_flexible_resource_owner_ucan()` wrapper
- ✅ `issue_flexible_delegated_resource_ucan()` - Role-based delegation with template selection

**services/src/resource_service.rs** (Complete rewrite - 724 lines):
- ✅ `decrypt_resources()` - Unified helper (single & batch)
- ✅ `create_resource()` - With flexible UCAN from frontend
- ✅ `get_resource_by_id_direct()` - Fetch single resource
- ✅ `update_resource()` - Replace snapshots
- ✅ `delete_resource()` - Soft delete
- ✅ `get_resources_for_folder()` - List folder resources
- ✅ `get_all_resources()` - List all user resources
- ✅ `share_resource()` - Role-based sharing with template selection
- ✅ `get_share_records_for_resource()` - Get shares
- ✅ `get_resource_ucan_key()` - Get UCAN token
- ✅ `validate_authority_for_update()` - UCAN validation
- ✅ `resolve_proof()` - UCAN proof resolution

**Methods Removed** (20 total):
- All 9 sync methods (Phase 3)
- 3 vector clock methods
- 2 resource keys methods
- UI helpers: toggle_fav, update_last_accessed
- Share helpers: prepare_share_resource, auto_share_resource_with_folder_users
- Legacy helpers: get_resource, decrypt_single_resource, etc.

**Compilation Status**:
- ✅ crypto_utils compiles successfully
- ✅ resource_service.rs compiles successfully
- ⏳ Only legacy struct import errors remain (expected from Phase 1)

**Files Modified**:
- `crypto_utils/src/ucan_utils.rs` - Added UcanFacts and flexible UCAN generation
- `crypto_utils/src/crypto_utils.rs` - Added flexible wrappers
- `services/src/resource_service.rs` - Complete rewrite (1200+ → 724 lines)

**Next Session Should Start With**:
1. Update KNOWLEDGE_BASE.md with Phase 2 architecture
2. Then proceed to Phase 3: Network protocol redesign

**Estimated Progress**: Phase 2 is 100% complete - Overall 60% complete (Phases 1-2 done)

---

**Ready to Continue**: Update documentation, then start Phase 3 (network protocol).

---

### Session 5: 2025-11-07 - Phase 3: Network Protocol Migration (In Progress)

**Goal**: Migrate network protocol from Yrs to Loro with unified UCAN-based sync

**Approach**: Complete protocol simplification - unified sync for owner/viewer, remove all device/user sync

**Key Simplifications Decided**:
1. **No User/Device Sync**: Remove all user and device manifest sync for now (add later if needed)
2. **Unified Protocol**: Same sync flow for owner↔node and viewer↔node (UCAN-based)
3. **No State Tracking**: Viewers always do full sync (no state vector caching)
4. **No Live Edit**: Remove all real-time collaborative editing features (add later)
5. **Single Message Enum**: Merge Website messages into main Message enum
6. **No Connection Types**: Everything determined from UCAN tokens
7. **3-Step Protocol**: StateVectorRequest → UpdatesResponse → Merge (bidirectional)
8. **Asset Sync**: Separate phase after CRDT sync (AssetTransfer messages)

**Implementation Completed**:

**1. core/src/models/p2p.rs** (Complete rewrite - simplified):
- ✅ Removed `LiveEdit` variant and `LiveEditMessage` enum (~36 lines removed)
- ✅ Removed `Website` wrapper, merged all WebsiteMessage variants into main Message
- ✅ Removed `ConnectionType`, `ConnectionAction`, `DisconnectStatus` enums
- ✅ Updated `ResourceUpdateMsg`:
  - StateVectorRequest: Added `asset_ids: Vec<String>` field
  - UpdatesResponse: Added `state_vectors`, `missing_asset_ids` fields
  - Removed `FinalUpdateMerge` and `VectorClockResponse` (deprecated 4-step protocol)
- ✅ Added `AssetTransferMessage` enum for static asset sync:
  - AssetRequest { resource_id, asset_ids, ucan_token }
  - AssetResponse { resource_id, assets: Vec<Asset> }
- ✅ Changed `ResourceAdditionRequest` from ResourceSyncData → EncryptedResource
- ✅ All messages now use EncryptedResource instead of ResourceSyncData

**New 3-Step Sync Protocol**:
```
Step 1: Peer A sends StateVectorRequest {
  resource_id,
  state_vectors: JSON {"doc": {"state_vector": [1,2,3,...]}},
  asset_ids: ["asset1", "asset2"],
  ucan_token
}

Step 2: Peer B sends UpdatesResponse {
  resource_id,
  updates: JSON {"doc": {"updates": [...], "state_vector": [...]}},
  state_vectors: Peer B's current state,
  missing_asset_ids: ["asset3"],  // Assets B needs from A
  ucan_token
}

Step 3: Both peers apply updates and merge
After: Asset sync via AssetTransfer messages if needed
```

**2. Deprecated Files Deleted**:
- ✅ `core/src/models/sync.rs` - Device/User manifest types (no longer needed)
- ✅ `core/src/models/resource_key.rs` - Deprecated key management
- ✅ `core/src/models/vector_clock.rs` - Deprecated vector clocks

**3. core/src/models/mod.rs** (Updated):
- ✅ Removed sync, resource_key, vector_clock module declarations
- ✅ Removed re-exports of deprecated types

**4. core/src/repositories/mod.rs** (Cleaned up):
- ✅ Replaced `ResourceWithKey` → `EncryptedResource` (5 occurrences)
- ✅ Removed `ResourceKeyRepository` trait entirely
- ✅ Removed `VectorClockRepository` trait entirely
- ✅ Removed deprecated methods:
  - find_resource_with_key()
  - save_resource_with_key()
  - save_resource_with_dependencies()
  - share_resource_transaction()
  - add_device_with_vector_clocks()
  - get_resource_manifest_data()
  - get_resource_sync_data()
  - save_resource_sync_data()
  - share_folder_transaction()
  - save_resource_and_folder_sharing_data()
  - save_bulk_sharing_data()

**Compilation Status**:
- ✅ **core crate compiles successfully!**
- ✅ All deprecated type errors resolved
- ✅ ~550 lines of deprecated code removed

**Code Reduction Summary**:
- p2p.rs: ~80 lines removed (LiveEdit + deprecated messages)
- Deleted files: ~400 lines (sync.rs, resource_key.rs, vector_clock.rs)
- Repository traits: ~150 lines of deprecated methods removed
- **Total: ~630 lines removed**

**Files Modified**:
- `core/src/models/p2p.rs` - Complete protocol simplification
- `core/src/models/mod.rs` - Removed deprecated module exports
- `core/src/repositories/mod.rs` - Removed deprecated traits and methods

**Files Deleted**:
- `core/src/models/sync.rs`
- `core/src/models/resource_key.rs`
- `core/src/models/vector_clock.rs`

---

## What's Next (Phase 3 Continuation)

### Immediate Tasks:

**Step 2: Add Network Sync Methods to resource_service.rs** (⏳ Next)
Add 4 wrapper methods for network layer:
1. `get_resource_state_vectors()` - Wrap Resource.get_state_vectors()
2. `generate_updates_for_peer()` - Wrap Resource.generate_updates()
3. `apply_peer_updates()` - Wrap Resource.apply_updates()
4. `prepare_resource_for_peer()` - Wrap Resource.filter_to_send() + re-encrypt

**Step 3: Rewrite network/src/p2p/website_handler.rs**
- Remove all Yrs-specific service calls
- Implement new 3-step protocol
- Use EncryptedResource instead of ResourceSyncData
- Add asset sync after CRDT sync

**Step 4: Clean up network/src/p2p/incoming_handler.rs**
- Remove all LiveEdit handlers (~450 lines)
- Update ResourceUpdateMsg handlers for 3-step protocol
- Remove Website message wrapper logic

**Step 5: Rewrite network/src/p2p/p2p_service.rs**
- Remove live edit connection logic (~100 lines)
- Simplify around new protocol
- Update send_resource() and sync_resource()

**Step 6: Delete network/src/p2p/resource_sync.rs**
- Entire file deprecated (713 lines)

**Step 7: Testing**
- Test owner→node publish flow
- Test viewer→node sync flow
- Test bidirectional updates
- Test asset discovery and transfer

### Estimated Remaining Work:
- Step 2: 1-2 hours (add 4 sync methods)
- Step 3: 3-4 hours (rewrite website_handler.rs)
- Step 4: 2 hours (clean up incoming_handler.rs)
- Step 5: 3 hours (rewrite p2p_service.rs)
- Step 6: 5 minutes (delete file)
- Step 7: 2-3 hours (testing and fixes)

**Total Phase 3 Remaining**: ~12-14 hours

**Overall Progress**: Phase 1-2 complete (60%), Phase 3 in progress (~10% done)

---

### Session 6: 2025-11-07 - Persistence Layer Migration & Cleanup

**Goal**: Update persistence layer to use EncryptedResource, remove deprecated code, make everything compile

**Approach**: Direct implementation - fix database schema, update repositories, remove deprecated code

**Implementation Completed**:

**1. Database Schema Migration**:
- ✅ Updated `persistance/migrations/2024-10-18-052005_create_initial_schema/up.sql`:
  - Removed `resource_keys` table (deprecated)
  - Removed `resource_vector_clocks` table (deprecated)
  - Updated `resources` table to new schema:
    ```sql
    CREATE TABLE resources (
      id TEXT PRIMARY KEY NOT NULL,
      folder_id TEXT NOT NULL,
      encrypted_data TEXT NOT NULL,
      encrypted_key TEXT NOT NULL,
      ucan_token TEXT NOT NULL,
      metadata TEXT,
      created_at BIGINT NOT NULL,
      updated_at BIGINT NOT NULL,
      FOREIGN KEY (folder_id) REFERENCES folders (id)
    );
    ```

**2. Schema.rs Updated**:
- ✅ Removed `resource_keys` and `resource_vector_clocks` Diesel table definitions
- ✅ Updated `resources` table schema to match new structure
- ✅ Removed deprecated joinable! declarations
- ✅ Cleaned up allow_tables_to_appear_in_same_query!

**3. Models.rs Updated** (`persistance/src/models.rs`):
- ✅ Removed deprecated imports (ResourceKey, ResourceVectorClock, ResourceType)
- ✅ Updated ResourceModel to match new schema (8 fields)
- ✅ Removed ResourceKeyModel (entire struct + conversions, ~45 lines)
- ✅ Removed ResourceVectorClockModel (entire struct + conversions, ~90 lines)
- ✅ Updated ResourceModel conversions to work with EncryptedResource
- ✅ Added serde_json for metadata serialization (Value ↔ String)
- ✅ Metadata handling: Serialize Value to JSON string in DB, deserialize back

**4. Repository Files Deleted**:
- ✅ `persistance/src/repositories/resource_key_repository.rs` (entire file)
- ✅ `persistance/src/repositories/vector_clock_repository.rs` (entire file)

**5. Repositories/mod.rs Updated**:
- ✅ Removed `resource_key_repository` and `vector_clock_repository` module declarations
- ✅ Removed SqliteResourceKeyRepository and SqliteVectorClockRepository exports

**6. Database/mod.rs Updated**:
- ✅ Removed ResourceKeyRepository and VectorClockRepository imports
- ✅ Removed resource_key_repo and vector_clock_repo from RepositoryContext struct
- ✅ Removed repository initialization in initialize_repositories()

**7. ResourceRepository Trait Cleaned** (`core/src/repositories/mod.rs`):

**Removed deprecated methods**:
- ❌ `save(&self, resource: &Resource)` - Was taking decrypted Resource
- ❌ `soft_delete_resource()` - No soft deletes in new schema
- ❌ `toggle_fav()` - No favourite field
- ❌ `update_last_accessed()` - No last_accessed field
- ❌ `get_favourites()` - No favourite field
- ❌ `find_by_id_raw()` - Returns decrypted Resource (service layer concern)
- ❌ `find_owner_by_resource_id()` - No owner field

**Added new method**:
- ✅ `save_encrypted(&self, resource: &EncryptedResource)` - Replaces save()

**Kept essential methods** (9 total):
- find_by_folder, find_all_by_folder, find_by_id
- delete_resource, get_all_resources, update_resource
- get_all_resource_ids, get_folder_ids_for_resources, get_resource_ids_by_folder_id

**8. SqliteResourceRepository Complete Rewrite** (`persistance/src/repositories/resource_repository.rs`):
- ✅ Reduced from 1096 lines → 206 lines (81% reduction!)
- ✅ All methods now work with EncryptedResource instead of Resource
- ✅ Removed all deprecated method implementations
- ✅ Removed all resource_keys and vector_clock queries
- ✅ Simplified folder deletion (hard delete resources instead of soft delete)
- ✅ Clean, focused implementation matching new trait

**9. FolderShareRepository Updated**:
- ✅ Removed share_folder_transaction() method (deprecated)
- ✅ Cleaned up imports (removed ResourceKey, ResourceVectorClock, resource_keys, resource_vector_clocks)
- ✅ No more resource key or vector clock handling

**10. FolderRepository Updated**:
- ✅ Fixed soft_delete() to hard delete resources (no more deleted/deleted_at fields)

**11. Services Layer Cleanup**:
- ✅ Deleted `services/src/sync_service.rs` (773 lines removed)
- ✅ Deleted `services/src/node_service.rs` (315 lines removed)
- ✅ Removed SyncServiceError enum from errors.rs (40 lines removed)
- ✅ Removed Sync(#[from] SyncServiceError) variant from ServiceError
- ✅ Updated lib.rs to remove sync_service and node_service modules

**Compilation Status**:
- ✅ **Persistence layer compiles with 0 errors, 0 warnings!**
- ✅ **Core crate compiles successfully!**
- ✅ **Services layer will need updates (expected)**

**Code Reduction Summary**:
- ResourceRepository: 1096 → 206 lines (890 lines removed, 81% reduction)
- Models.rs: Removed ResourceKeyModel + ResourceVectorClockModel (~135 lines)
- Deleted repository files: ~400 lines
- Deleted sync_service.rs: 773 lines
- Deleted node_service.rs: 315 lines
- Deprecated methods removed from traits: ~150 lines
- **Total Session 6: ~2,665 lines removed**

**Database Schema Changes**:
- resources table: 13 fields → 8 fields (5 fields removed)
- Removed 2 entire tables (resource_keys, resource_vector_clocks)
- Cleaner, simpler schema aligned with EncryptedResource model

**Files Modified**:
- `persistance/migrations/2024-10-18-052005_create_initial_schema/up.sql`
- `persistance/src/database/schema.rs`
- `persistance/src/database/mod.rs`
- `persistance/src/models.rs`
- `persistance/src/repositories/mod.rs`
- `persistance/src/repositories/resource_repository.rs` (complete rewrite)
- `persistance/src/repositories/folder_repository.rs`
- `persistance/src/repositories/folder_share_repository.rs`
- `core/src/repositories/mod.rs` (ResourceRepository trait)
- `services/src/lib.rs`
- `services/src/errors.rs`

**Files Deleted**:
- `persistance/src/repositories/resource_key_repository.rs`
- `persistance/src/repositories/vector_clock_repository.rs`
- `services/src/sync_service.rs`
- `services/src/node_service.rs`

**Next Session Should Start With**:
1. Update services layer to use new repository methods (many errors expected)
2. Add network sync wrapper methods to resource_service.rs (Step 2 from Phase 3)
3. Then continue with network layer updates (Steps 3-7)

**Estimated Progress**: Phase 3 is ~15% complete (core + persistence done, services + network remain)

---

**Ready to Continue**: Fix services layer compilation errors, then add network sync methods to resource_service.rs

---

### Session 7: 2025-11-07 - Handler Implementation: getCredential

**Goal**: Implement `handle_get_resource` handler to fetch and decrypt a single resource

**Approach**: Remove unnecessary user_id parameter, implement service function, add handler, update types

**Key Simplifications Decided**:
1. **No User Lookup**: encrypted_key is stored in resource record, no separate user lookup needed
2. **Remove user_id Parameter**: Updated ResourceRepository.find_by_id() to not require user_id
3. **Unified Decryption**: Reuse existing decrypt_resources() helper for single resource

**Implementation Completed**:

**1. Repository Layer** - Removed unnecessary parameter:
- ✅ Updated `ResourceRepository` trait: `find_by_id(&self, id: &str)` (removed user_id)
- ✅ Updated `SqliteResourceRepository` implementation to match

**2. Service Layer** - Implemented fetch and decrypt:
- ✅ Implemented `get_resource_by_id_direct()` in `services/src/resource_service.rs:234`
  - Fetches EncryptedResource from database
  - Decrypts using existing `decrypt_resources()` helper
  - Returns Resource with loaded Loro documents

**3. Handler Layer** - Implemented Tauri command:
- ✅ Implemented `handle_get_resource()` in `tauri_handlers/src/handlers/resource.rs:68`
  - No user lookup needed
  - Converts Resource to JSON format (doc snapshots)
  - Returns ResourceResponse with id, data, favourite, lastAccessed, folderId

**4. Registration**:
- ✅ Registered `handle_get_resource` in `sthalam/src-tauri/src/lib.rs:164`

**5. TypeScript Types** - Frontend interfaces:
- ✅ `GetResourceInput` interface (`helper.ts:35`)
- ✅ `ResourceResponse` interface (`helper.ts:39`) with data structure

**6. Documentation**:
- ✅ Updated `FUNCTION_USAGE_TRACKER.md`:
  - Marked `getCredential` as ✅ ACTIVE (line 42)
  - Added complete flow documentation with crypto_utils functions
  - Documented repository and service layer interactions

**Flow Summary**:
```
Frontend (data.svelte.ts:326)
  ↓ sendMessage("getCredential", { resourceId })
Handler (resource.rs:68)
  ↓ handle_get_resource
Service (resource_service.rs:234)
  ↓ get_resource_by_id_direct
Repository (resource_repository.rs:71)
  ↓ find_by_id(resource_id)
Crypto Utils (crypto_utils.rs:119)
  ↓ decrypt_resource (AES + PGP)
  ↓ Resource::from_decrypted_data
Frontend
  ← ResourceResponse { id, data, folder_id }
```

**Compilation Status**:
- ✅ All crates compile successfully
- ✅ Services layer compiles with warnings only
- ✅ Tauri handlers compiles with warnings only
- ✅ Main application compiles successfully

**Files Modified**:
- `core/src/repositories/mod.rs` - Updated ResourceRepository trait
- `persistance/src/repositories/resource_repository.rs` - Updated find_by_id signature
- `services/src/resource_service.rs` - Implemented get_resource_by_id_direct
- `tauri_handlers/src/handlers/resource.rs` - Implemented handle_get_resource
- `sthalam/src-tauri/src/lib.rs` - Registered handler
- `sthalam/frontend/desktop/src/utils/helper.ts` - Added TypeScript types
- `sthalam/FUNCTION_USAGE_TRACKER.md` - Updated with flow documentation

**Handler Progress**:
- ✅ handle_add_folder (Session 5)
- ✅ handle_get_folders (Session 5)
- ✅ handle_add_resource (Previous session)
- ✅ handle_get_resource (This session)
- ⏳ handle_update_resource (Next)
- ⏳ handle_get_resources_for_folder (Next)
- ⏳ handle_get_all_resources (Next)

**Next Session Should Start With**:
1. Implement remaining resource handlers as needed by frontend
2. Continue with network layer updates (Phase 3 continuation)
3. Test resource creation and fetching flow end-to-end

**Estimated Progress**: Handler layer ~30% complete (4/14 handlers implemented)

---

### Session 8: 2025-11-07 - Resource Update Bug Fix

**Goal**: Fix data corruption bug in resource update flow

**Issue**: Resource updates were failing with parser error:
```
Failed to parse resource: Failed to parse decrypted data JSON: invalid type: string "3708909928", expected a sequence at line 1 column 14950
```

**Root Cause Analysis**:

**The Bug**:
- Frontend `saveCurrentResource()` was sending BOTH Loro documents AND metadata fields (client_id, last_modified, title) in one `data` field
- Service `update_resource()` encrypted the ENTIRE payload (documents + metadata) into `encrypted_data`
- When fetching, parser expected ONLY document arrays but found metadata string fields
- Parser failed trying to convert string `"3708909928"` (the client_id) to Vec<Value>

**Add Flow (Working Correctly)**:
```typescript
// Frontend sends:
resourcePayload: JSON.stringify(loroContent),  // Just Loro documents
metadataJson: JSON.stringify(metadata)         // Separate metadata
```

**Update Flow (Buggy)**:
```typescript
// Frontend was sending:
data: JSON.stringify({
  template_doc: [...],
  content_doc: [...],
  ...
  client_id: "3708909928",  // ← Metadata mixed in!
  last_modified: 1234567890,
  title: "My Document"
})
```

**Implementation Completed**:

**1. Frontend Fix** (`sthalam/frontend/desktop/src/state/data.svelte.ts:415-423`):
- ✅ Removed `client_id`, `last_modified`, and `title` from loroContent object
- ✅ Now only sends Loro document snapshots (template_doc, content_doc, user_content_doc, collaborative_doc, submissions_doc, static_assets)
- ✅ Matches add_resource pattern

**Before**:
```typescript
const loroContent = {
  template_doc: Array.from(snapshots.template),
  content_doc: Array.from(snapshots.content),
  ...
  static_assets: staticAssetsArray,
  client_id: this.clientId.toString(),  // ← Removed
  last_modified: Date.now(),            // ← Removed
  title                                 // ← Removed
};
```

**After**:
```typescript
const loroContent = {
  template_doc: Array.from(snapshots.template),
  content_doc: Array.from(snapshots.content),
  user_content_doc: Array.from(snapshots.userContent),
  collaborative_doc: Array.from(snapshots.collaborative),
  submissions_doc: Array.from(snapshots.submissions),
  static_assets: staticAssetsArray
  // No metadata fields - just documents
};
```

**Why This Fix Works**:
- `encrypted_data` now contains only Loro documents (all arrays)
- Parser successfully parses all fields as `Vec<Value>`
- No string fields to cause type mismatch
- Consistent with add_resource pattern

**Alternative Considered (Rejected)**:
- Could have updated handler/service/repository to accept separate metadata parameter
- Decided simpler to just remove metadata from update payload (metadata doesn't change on save)
- Matches the principle: updates only modify document content, not metadata

**Compilation Status**:
- ✅ Frontend change compiles successfully
- ✅ No backend changes needed (just removed problematic data)

**Files Modified**:
- `sthalam/frontend/desktop/src/state/data.svelte.ts` - Removed metadata fields from loroContent (line 415-423)

**Documentation Updated**:
- ✅ HANDOFF.md - Bug fix session documented
- ✅ FUNCTION_USAGE_TRACKER.md - Updated with bug fix notes
- ✅ KNOWLEDGE_BASE.md - Added gotcha about metadata in updates
- ✅ MIGRATION_PLAN.md - Progress log entry

**Key Learnings**:

**Metadata Separation**:
- Add flow correctly separates: resourcePayload (encrypted) vs metadataJson (unencrypted)
- Update flow should follow same pattern
- Metadata (title, client_id, last_modified) shouldn't be in encrypted document data

**Parser Expectations**:
- `Resource::from_decrypted_data()` expects: `HashMap<String, Vec<Value>>`
- Every field must be an array (document snapshot bytes)
- String or number fields will cause parser failure

**Update Semantics**:
- Resource updates only modify document content (Loro snapshots)
- Metadata updates (title changes) would need separate endpoint/logic
- Current update is "save current document state" not "update resource metadata"

**Next Session Should Start With**:
1. Test the bug fix: update a resource and verify fetch works
2. Consider if metadata updates need separate handler
3. Continue with remaining handlers or network layer

**Estimated Progress**: Handler layer ~30% complete, bug fixed
