# Changelog - Services Layer

All notable changes to the services layer will be documented in this file.

Format:
```
## [Date] - [Agent Name]

### Function: `function_name()`
**File**: `path/to/file.rs:line`
**What Changed**: Description of the change
**Why**: Rationale for the change
```

---

## 2025-11-12 - Claude Code (Session)

### Function: `merge_service::extract_asset_ids()`
**File**: `services/src/merge_service.rs:407-422`
**What Changed**: Implemented function to extract asset IDs from resource.static_assets HashMap. Returns Vec<String> of all asset keys.
**Why**: Enable asset comparison during sync protocol. Needed for identifying which assets peer is missing in ResourceSyncRequest flow.

---

### Function: `merge_service::apply_submission()`
**File**: `services/src/merge_service.rs:438-488`
**What Changed**: Currently STUBBED. Needs Loro 1.x API implementation for nested maps to store viewer submissions in isolated namespaces.
**Why**: Required for viewer submission isolation in sync protocol. Viewers need to submit full documents that get merged without seeing other viewers' submissions.

---

### Function: `resource_service::get_asset_binary_data()`
**File**: `services/src/resource_service.rs:1193-1203`
**What Changed**: Currently STUBBED. Needs implementation to decrypt resource, extract asset from static_assets HashMap, and base64 decode.
**Why**: Required to retrieve asset binary data for AssetTransfer messages in sync protocol. Currently blocks asset sending to peers.

---

### Function: `resource_service::save_asset_binary_data()`
**File**: `services/src/resource_service.rs:1221-1235`
**What Changed**: Currently STUBBED. Needs implementation to load resource, add/update asset in static_assets HashMap, and save.
**Why**: Required to store received assets from peers. Currently blocks asset receiving in sync protocol.

---

### Function: `ucan_service::extract_folder_id()`
**File**: `services/src/ucan_service.rs` (added)
**What Changed**: Implemented function to parse folder_id from UCAN token capabilities. Extracts folder ID from resource format `{domain}:folder:{folder_id}`.
**Why**: P2P layer needed to validate folder access during sync operations.

---

### Function: `ucan_service::extract_user_id()`
**File**: `services/src/ucan_service.rs` (added)
**What Changed**: Implemented function to extract user_id from UCAN facts. Returns user_id string from token metadata.
**Why**: P2P layer needed to identify viewer for submission isolation.

---
