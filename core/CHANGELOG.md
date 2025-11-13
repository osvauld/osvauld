# Changelog - Core Models

All notable changes to the core models and data structures will be documented in this file.

Format:
```
## [Date] - [Agent Name]

### Function/Struct: `name()`
**File**: `path/to/file.rs:line`
**What Changed**: Description of the change
**Why**: Rationale for the change
```

---

## 2025-11-12 - Claude Code (Session)

### Struct: `Resource`
**File**: `core/src/models/resource.rs:70-82`
**What Changed**: Added `static_assets: HashMap<String, String>` field to Resource struct. Maps asset_id to base64-encoded binary data.
**Why**: Support asset storage in sync protocol. Previously assets were not part of Resource model, causing them to be lost during save/load cycles.

---

### Function: `Resource::from_decrypted_data()`
**File**: `core/src/models/resource.rs:118-174`
**What Changed**: Updated to parse `static_assets` as a HashMap<String, String> from JSON. Separates static_assets handling from Loro document parsing.
**Why**: Enable loading assets from encrypted resource blob. Previously only loaded Loro documents, ignoring static_assets field.

---

### Function: `Resource::to_json()`
**File**: `core/src/models/resource.rs:214-238`
**What Changed**: Updated to export `static_assets` as a JSON object/map alongside Loro document snapshots.
**Why**: Preserve assets during resource serialization for encryption and storage. Previously only exported Loro documents, losing asset data.

---

### Messages: Sync Protocol (Phase 1)
**File**: `core/src/models/p2p.rs`
**What Changed**: Added state_vectors and full_docs fields to ResourceSyncRequestMsg. Added FolderSyncRequestMsg, FolderSyncResponseMsg, AssetTransferMsg.
**Why**: Complete message protocol for bidirectional sync with CRDT state vectors, asset transfer, and folder discovery as per SYNC_PROTOCOL_DESIGN.md Phase 1.

---
