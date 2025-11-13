# Sync Protocol Design - IMPLEMENTATION READY

**Status:** READY FOR IMPLEMENTATION
**Date:** 2025-11-12
**Version:** 2.0
**Context:** Complete sync protocol specification with CRDT merge, folder sync, asset handling, and viewer isolation

---

## Table of Contents
1. [Current State & Requirements](#current-state--requirements)
2. [Core Decisions](#core-decisions)
3. [Message Protocol](#message-protocol)
4. [UCAN Structure](#ucan-structure)
5. [Sync Flows](#sync-flows)
6. [Asset Sync Protocol](#asset-sync-protocol)
7. [Folder Sync Protocol](#folder-sync-protocol)
8. [Viewer Submission Isolation](#viewer-submission-isolation)
9. [Implementation Plan](#implementation-plan)

---

## Current State & Requirements

### What We Found

1. **Owner ↔ Node sync is NOT bidirectional**
   - Current implementation uses `ResourceSyncRequest` protocol
   - Responder sends updates to initiator
   - Initiator generates response updates but **DISCARDS them** (see TODO at `network/src/p2p/resource_sync.rs:711-712`)
   - Result: **ONE-WAY sync only** (Responder → Initiator)

2. **Initial folder share uses simple push**
   - Uses `ResourceDataSync` message (not CRDT merge)
   - Owner → Node: Complete resource transfer
   - No state vectors, no incremental updates
   - Result: **ONE-WAY push**

3. **Viewer sync not yet implemented**
   - Viewer handshake and initial folder/resource transfer exists
   - No incremental sync protocol for viewers
   - No handling for special document behaviors (submissions, read-only, etc.)

4. **Asset handling is placeholder**
   - `missing_asset_ids` field exists but not used
   - No asset ID extraction or transfer protocol
   - `static_assets` treated like regular CRDT document

### Requirements

#### Owner ↔ Node Sync
- **Bidirectional CRDT merge** for ALL documents
- Both owner and node can edit simultaneously
- Changes merge automatically using Loro CRDT
- Incremental updates (state vectors)
- **Always 2 round trips** for proper convergence

#### Node ↔ Viewer Sync
- **Mixed behavior** depending on document type:
  - `collaborative_doc`: Bidirectional CRDT merge (viewer + node can both edit)
  - `content_doc`, `template_doc`: Read-only (node → viewer only)
  - `static_assets`: Read-only asset collection with ID-based sync
  - `submissions_doc`: **Special** - viewer sends full document, node never sends back
- **Always 2 round trips** for proper convergence (viewer may have collaborative_doc edits)

#### Viewer Submission Isolation
- Viewer creates submission work locally
- Viewer sends **FULL document** (not incremental CRDT updates)
- Node merges viewer's submission with submissions from other viewers
- Node **NEVER sends submissions back to viewer**
- Each viewer isolated in their own namespace: `viewer:{user_id}`
- Rationale: Viewers shouldn't see each other's submissions

---

## Core Decisions

### Decision 1: Submission Capability Name
**CHOSEN:** `crud/submit` (NOT `crud/appendonly` or `crud/full_send`)

**Rationale:**
- Semantic clarity: "submit" conveys one-way, immutable action
- Consistent naming: Matches `crud/merge`, `crud/readonly` pattern
- Security: Node validates submission schema before accepting
- Viewer isolation: Each submission namespaced by viewer ID

### Decision 2: UCAN Structure
**CHOSEN:** Remove `viewer_template` from viewer's own UCAN

**Rationale:**
- `viewer_template` is delegation metadata (belongs in owner/node UCANs only)
- Viewer's UCAN should only contain their concrete capabilities
- Reduces token size and prevents viewer from modifying delegation rules
- Architecturally cleaner separation

### Decision 3: Round Trips
**CHOSEN:** Always 2 rounds for everyone (owner/node/viewer)

**Rationale:**
- Viewers may have `collaborative_doc` edits to send back
- Viewer can't generate updates until they see node's state vectors
- Fixed rounds = predictable, testable behavior
- Simpler than dynamic round detection

### Decision 4: Folder Sync Strategy
**CHOSEN:** 3-step process (discovery → missing resources → incremental sync)

**Rationale:**
- Step 1: FolderSyncRequest identifies which resources are missing
- Step 2: Missing resources get full ResourceDataSync transfer
- Step 3: Existing resources sync via parallel ResourceSyncRequests
- Efficient: Only transfers missing resources in full
- Scalable: Parallel sync for existing resources

### Decision 5: Asset Handling
**CHOSEN:** Asset IDs included in state_vectors JSON, separate AssetTransfer messages

**Rationale:**
- `static_assets` is a **JSON map** (NOT a Loro CRDT) stored in encrypted resource data
- Format: `{"asset_id": "base64_binary_data", ...}`
- Asset IDs extracted by parsing JSON keys and included in state_vectors for comparison
- Set operations (HashSet difference) determine which assets are missing
- Binary asset data transferred via separate AssetTransfer messages
- Keeps CRDT documents small, assets transferred on-demand
- **NO state vectors for static_assets** - only asset_ids list

---

## Message Protocol

### ResourceSyncRequestMsg (Updated)

```rust
pub struct ResourceSyncRequestMsg {
    pub resource_ucan: String,      // Initiator's resource UCAN
    pub folder_ucan: String,        // Initiator's folder UCAN
    pub state_vectors: String,      // NEW: JSON of initiator's state vectors
    pub full_docs: String,          // NEW: JSON of full documents (for crud/submit)
}
```

#### state_vectors JSON Format

**For CRDT documents:**
```json
{
  "collaborative_doc": {
    "state_vector": [1, 150, 82, 223, ...]
  },
  "content_doc": {
    "state_vector": [0, 230, 15, ...]
  },
  "template_doc": {
    "state_vector": [1, 67, ...]
  },
  "submissions_doc": {
    "state_vector": [1, 89, ...]
  }
}
```

**For static_assets (special handling - NO CRDT):**
```json
{
  "static_assets": {
    "asset_ids": ["asset-123", "asset-456"]  // Asset IDs viewer currently has
    // NOTE: NO state_vector field - static_assets is JSON, not a CRDT!
  }
}
```

**Key Points:**
- Asset IDs are nested inside the static_assets entry, NOT a separate top-level field
- **NO state_vector for static_assets** - it's a JSON map, not a Loro CRDT document
- Asset IDs are extracted by parsing the JSON keys from the static_assets map

#### full_docs JSON Format

```json
{
  "submissions_doc": [1, 2, 3, 4, ...]  // Full Loro document snapshot bytes as array
}
```

Only contains documents where:
- Viewer has `crud/submit` capability, AND
- Document is in `full_doc_send` list in UCAN facts

---

### UpdatesResponse (Existing, with clarification)

```rust
pub struct ResourceUpdateMsg::UpdatesResponse {
    pub resource_id: String,
    pub updates: String,                    // JSON: {"doc_name": {"updates": [...], "state_vector": [...]}}
    pub state_vectors: String,              // Responder's current state
    pub missing_asset_ids: Vec<String>,     // Assets peer needs to download
    pub ucan_token: String,
}
```

#### missing_asset_ids Usage

Result of set operations comparing asset IDs:
```rust
// Extract peer's asset IDs from state_vectors JSON
let peer_asset_ids: HashSet<String> = extract_from_state_vectors(peer_state_vectors_json);

// Extract our asset IDs by parsing static_assets JSON map
let node_asset_ids: HashSet<String> = parse_static_assets_json(&resource.static_assets);

let missing_on_peer: Vec<String> = node_asset_ids
    .difference(&peer_asset_ids)
    .cloned()
    .collect();

// Include in response
missing_asset_ids: missing_on_peer
```

**Note:** `static_assets` is NOT a Loro document - it's parsed as JSON to get asset IDs.

---

### FolderSyncRequestMsg (NEW)

```rust
pub struct FolderSyncRequestMsg {
    pub folder_ucan: String,           // Folder-level UCAN
    pub resources: String,             // JSON array of resource metadata
}
```

#### resources JSON Format

```json
[
  {
    "resource_id": "abc123",
    "resource_ucan": "eyJ...",
    "last_modified": 1699564800
  },
  {
    "resource_id": "def456",
    "resource_ucan": "eyJ...",
    "last_modified": 1699564900
  }
]
```

---

### FolderSyncResponseMsg (NEW)

```rust
pub struct FolderSyncResponseMsg {
    pub folder_id: String,
    pub missing_resource_ids: Vec<String>,  // Resources responder doesn't have
    pub existing_resource_ids: Vec<String>, // Resources responder has
}
```

---

### AssetTransferMsg (NEW)

```rust
pub struct AssetTransferMsg {
    pub resource_id: String,
    pub asset_id: String,
    pub asset_data: Vec<u8>,           // Actual asset bytes
    pub metadata: String,              // JSON: {"mime_type": "image/png", "size": 12345, ...}
}
```

---

## UCAN Structure

### Owner/Node UCAN (contains viewer_template)

```json
{
  "cap": {
    "sthalam:folder:xyz": {"crud/merge": [{}]},
    "sthalam:resource:abc:collaborative_doc": {"crud/merge": [{}]},
    "sthalam:resource:abc:content_doc": {"crud/merge": [{}]},
    "sthalam:resource:abc:template_doc": {"crud/merge": [{}]},
    "sthalam:resource:abc:static_assets": {"crud/merge": [{}]},
    "sthalam:resource:abc:submissions_doc": {"crud/merge": [{}]},
    "sthalam:resource:abc:user_content_doc": {"crud/merge": [{}]}
  },
  "fct": {
    "role": "owner",
    "docs": ["collaborative_doc", "content_doc", "template_doc", "static_assets",
             "submissions_doc", "user_content_doc"],
    "doc_types": {
      "collaborative_doc": "crdt",
      "content_doc": "crdt",
      "template_doc": "crdt",
      "static_assets": "asset",
      "submissions_doc": "crdt",
      "user_content_doc": "crdt"
    },
    "viewer_template": {
      "capabilities": {
        "collaborative_doc": "crud/merge",
        "content_doc": "crud/readonly",
        "template_doc": "crud/readonly",
        "static_assets": "crud/readonly",
        "submissions_doc": "crud/submit"
      },
      "doc_types": {
        "collaborative_doc": "crdt",
        "content_doc": "crdt",
        "template_doc": "crdt",
        "static_assets": "asset",
        "submissions_doc": "crdt"
      },
      "no_update_from_node": ["submissions_doc"],
      "dont_send_to_node": ["user_content_doc"],
      "full_doc_send": ["submissions_doc"]
    }
  }
}
```

**Key Points:**
- `viewer_template` is ONLY in owner/node UCANs (for delegation)
- Used when creating viewer UCANs to derive viewer capabilities
- `crud/submit` for submissions_doc (NOT `crud/appendonly` or `crud/full_send`)
- `doc_types` includes `"static_assets": "asset"` for special handling
- `full_doc_send: ["submissions_doc"]` indicates full document protocol

---

### Viewer UCAN (NO viewer_template)

```json
{
  "cap": {
    "sthalam:folder:xyz": {"crud/readonly": [{}]},
    "sthalam:resource:abc:collaborative_doc": {"crud/merge": [{}]},
    "sthalam:resource:abc:content_doc": {"crud/readonly": [{}]},
    "sthalam:resource:abc:template_doc": {"crud/readonly": [{}]},
    "sthalam:resource:abc:static_assets": {"crud/readonly": [{}]},
    "sthalam:resource:abc:submissions_doc": {"crud/submit": [{}]}
  },
  "fct": {
    "role": "viewer",
    "docs": ["collaborative_doc", "content_doc", "template_doc", "static_assets", "submissions_doc"],
    "doc_types": {
      "collaborative_doc": "crdt",
      "content_doc": "crdt",
      "template_doc": "crdt",
      "static_assets": "asset",
      "submissions_doc": "crdt"
    },
    "no_update_from_node": ["submissions_doc"],
    "dont_send_to_node": ["user_content_doc"],
    "full_doc_send": ["submissions_doc"]
  }
}
```

**Key Points:**
- NO `viewer_template` in viewer's own UCAN
- Viewer reads its own `no_update_from_node` to ignore certain updates
- Viewer reads its own `capabilities` to determine what to send
- Viewer reads its own `full_doc_send` to know which docs to send as full snapshots

---

### UCAN Facts Explained

#### `no_update_from_node: ["submissions_doc"]`
- Viewer NEVER accepts updates for these docs from node
- Even if node sends updates, viewer ignores them
- Ensures viewer isolation (can't see other submissions)

#### `dont_send_to_node: ["user_content_doc"]`
- Viewer NEVER sends these docs to node
- These docs stay local to viewer
- Not included in state_vectors or full_docs

#### `full_doc_send: ["submissions_doc"]`
- Viewer sends FULL document (not incremental updates)
- Used with `crud/submit` capability
- Node merges full submission from viewer

#### `doc_types: {"static_assets": "asset"}`
- Identifies which documents are asset collections
- Asset-type docs include `asset_ids` in state_vectors
- Binary asset data transferred separately via AssetTransfer

---

## Sync Flows

### Flow 1: Owner ↔ Node Bidirectional Sync (2 Rounds)

#### Round 1: Initiator → Responder

**Initiator prepares and sends:**
```rust
ResourceSyncRequest {
    resource_ucan: "owner's UCAN",
    folder_ucan: "owner's folder UCAN",
    state_vectors: json!({
        "collaborative_doc": {"state_vector": [1, 200, 100, ...]},
        "content_doc": {"state_vector": [0, 300, 50, ...]},
        "template_doc": {"state_vector": [1, 80, ...]},
        "user_content_doc": {"state_vector": [2, 45, ...]},
        "submissions_doc": {"state_vector": [1, 120, ...]},
        "static_assets": {
            "state_vector": [0, 20, ...],
            "asset_ids": ["asset-1", "asset-2", "asset-3"]
        }
    }).to_string(),
    full_docs: json!({}).to_string()  // Empty - all docs use CRDT merge
}
```

**Responder processes:**
1. No full docs to apply (all use CRDT merge)
2. For each doc in state_vectors:
   - Compare initiator's state vs responder's state
   - Generate incremental updates: `export_updates(responder_doc, initiator_state_vector)`
3. For static_assets:
   - Extract initiator's asset_ids
   - Compare with responder's asset_ids using set operations
   - Determine missing_asset_ids

**Responder sends:**
```rust
UpdatesResponse {
    resource_id: "abc",
    updates: json!({
        "collaborative_doc": {"updates": [201, 202, ...], "state_vector": [1, 250, 120, ...]},
        "content_doc": {"updates": [301, 302, ...], "state_vector": [0, 350, 60, ...]},
        "template_doc": {"updates": [], "state_vector": [1, 80, ...]},
        "user_content_doc": {"updates": [46, 47, ...], "state_vector": [2, 50, ...]},
        "submissions_doc": {"updates": [121, 122, ...], "state_vector": [1, 125, ...]}
        // NOTE: static_assets NOT included - it's JSON, not a CRDT with updates
    }).to_string(),
    state_vectors: json!({...}).to_string(),
    missing_asset_ids: vec!["asset-99", "asset-100"],  // Assets initiator needs
    ucan_token: "owner's UCAN"
}
```

**Initiator applies responder's updates:**
1. Applies all incremental updates (CRDT merge)
2. Saves updated resource
3. **Continues to Round 2** (always, for proper convergence)

#### Round 2: Initiator → Responder (Convergence)

**Initiator processes:**
1. Generates response updates (initiator's changes responder doesn't have)
2. Compares new state vectors

**Initiator sends:**
```rust
UpdatesResponse {
    resource_id: "abc",
    updates: json!({
        // Only docs where initiator has changes responder doesn't have
        "collaborative_doc": {"updates": [210, 211, ...], "state_vector": [1, 255, 125, ...]},
        "user_content_doc": {"updates": [48, 49, ...], "state_vector": [2, 55, ...]}
        // Other docs omitted if no updates
    }).to_string(),
    state_vectors: json!({...}).to_string(),
    missing_asset_ids: vec![],  // Already handled in Round 1
    ucan_token: "owner's UCAN"
}
```

**Responder applies initiator's Round 2 updates:**
1. Applies incremental updates (CRDT merge)
2. Saves merged resource
3. **Sync complete** - both peers now converged

**Termination:** Always stop after 2 rounds (sufficient for CRDT convergence)

---

### Flow 2: Node ↔ Viewer Sync (2 Rounds, Mixed Mode)

#### Round 1: Viewer → Node

**Viewer analyzes its own UCAN:**
- `crud/merge` → Bidirectional CRDT merge (send state vector, apply updates)
- `crud/readonly` → Node → Viewer only (send state vector, apply updates, but don't send updates back)
- `crud/submit` → Viewer → Node only (send full document, ignore updates from node)
- `no_update_from_node` → Filter out when applying updates
- `dont_send_to_node` → Don't include in request at all

**Viewer prepares:**
1. State vectors for all docs (except those in `dont_send_to_node`)
2. Full document for `submissions_doc` (has `crud/submit` in `full_doc_send`)
3. Current asset_ids for `static_assets`

**Viewer sends:**
```rust
ResourceSyncRequest {
    resource_ucan: "viewer's UCAN",
    folder_ucan: "viewer's folder UCAN",
    state_vectors: json!({
        "collaborative_doc": {"state_vector": [1, 150, 82, ...]},
        "content_doc": {"state_vector": [0, 230, 15, ...]},
        "template_doc": {"state_vector": [1, 67, ...]},
        "static_assets": {
            "state_vector": [0, 12, ...],
            "asset_ids": ["asset-123", "asset-456"]
        },
        "submissions_doc": {"state_vector": [1, 89, ...]}
        // NO user_content_doc (in dont_send_to_node)
    }).to_string(),
    full_docs: json!({
        "submissions_doc": [1, 2, 3, 4, ...]  // Full document bytes
    }).to_string()
}
```

**Node processes:**
1. **Validate viewer's UCAN** (capabilities, folder access)
2. **Apply full documents:**
   - Extract viewer's `user_id` from UCAN
   - `submissions_doc`: Call `merge_service::apply_submission(resource, viewer_user_id, full_doc_bytes, viewer_ucan)`
   - Merges into isolated namespace: `viewer:{user_id}`
3. **Generate incremental updates for viewer:**
   - Check viewer's `no_update_from_node` in UCAN → Skip `submissions_doc`
   - For other docs: Generate updates based on state vectors
4. **Process assets:**
   - Extract viewer's asset_ids from state_vectors
   - Compare with node's asset_ids
   - Determine missing_asset_ids

**Node sends:**
```rust
UpdatesResponse {
    resource_id: "abc",
    updates: json!({
        "collaborative_doc": {
            "updates": [151, 152, ...],
            "state_vector": [1, 155, 85, ...]
        },
        "content_doc": {
            "updates": [231, 232, ...],
            "state_vector": [0, 235, 18, ...]
        },
        "template_doc": {
            "updates": [],
            "state_vector": [1, 67, ...]
        },
        "static_assets": {
            "updates": [13, 14, ...],
            "state_vector": [0, 15, ...]
        }
        // NO submissions_doc (filtered by no_update_from_node)
    }).to_string(),
    state_vectors: json!({...}).to_string(),
    missing_asset_ids: vec!["asset-789", "asset-999"],  // Assets viewer needs
    ucan_token: "viewer's UCAN"
}
```

#### Round 2: Viewer → Node (Convergence)

**Viewer processes node's updates:**
1. **Check `no_update_from_node`** from own UCAN
   - Skip `submissions_doc` even if received
2. **Apply updates by capability:**
   - `crud/merge` (collaborative_doc): CRDT merge, generate response updates
   - `crud/readonly` (content_doc, template_doc, static_assets): Apply updates only (don't generate response)
   - `crud/submit` (submissions_doc): Ignore (in no_update_from_node)
3. **Generate response updates** (only for `crud/merge` docs)

**Viewer sends:**
```rust
UpdatesResponse {
    resource_id: "abc",
    updates: json!({
        // Only collaborative_doc (has crud/merge)
        "collaborative_doc": {
            "updates": [160, 161, ...],
            "state_vector": [1, 165, 90, ...]
        }
        // NO content_doc, template_doc, static_assets (crud/readonly - one-way only)
        // NO submissions_doc (in no_update_from_node)
    }).to_string(),
    state_vectors: json!({...}).to_string(),
    missing_asset_ids: vec![],
    ucan_token: "viewer's UCAN"
}
```

**Node applies viewer's Round 2 updates:**
1. Applies collaborative_doc updates (CRDT merge)
2. Saves merged resource
3. **Sync complete** - viewer isolation maintained

**Termination:** Always stop after 2 rounds

---

## Asset Sync Protocol

### Overview

`static_assets` is a **JSON map** (NOT a Loro CRDT) containing binary asset data as base64 strings. Asset IDs are extracted from the JSON keys and included in state_vectors for comparison. Binary asset data is transferred separately via AssetTransfer messages when needed.

### static_assets Storage Structure

**IMPORTANT:** `static_assets` is **NOT a Loro CRDT document**. It's a JSON map stored in the encrypted resource blob.

```json
// JSON structure stored in EncryptedResource.encrypted_data:
{
  "template_doc": [1, 2, 3, ...],          // Loro CRDT snapshot
  "content_doc": [4, 5, 6, ...],           // Loro CRDT snapshot
  "user_content_doc": [7, 8, 9, ...],      // Loro CRDT snapshot
  "collaborative_doc": [10, 11, 12, ...],  // Loro CRDT snapshot
  "submissions_doc": [13, 14, 15, ...],    // Loro CRDT snapshot
  "static_assets": {                        // JSON map (NOT Loro)
    "asset_image_1762958880043_2ba91eae": "/9j/4AAQSkZJRgABAQAAAQABAAD...",
    "asset_file_1762958880044_3cd02cbf": "JVBERi0xLjQKJeLjz9MKMy...",
    "asset_image_1762958880045_4de13dc0": "iVBORw0KGgoAAAANSUhEUg..."
  }
}
```

**Asset metadata** (filename, mime_type, etc.) may be stored in a separate Loro CRDT document like `content_doc`, but the **binary data itself** is in the JSON map as base64 strings.

**Key Points:**
- Asset IDs are the keys of the static_assets JSON object
- Asset data is **base64-encoded string** (NOT raw binary array)
- NO Loro operations on static_assets - just JSON parse/stringify
- NO state vectors for static_assets
- Extract IDs via: `Object.keys(static_assets)` in JS or `static_assets.keys()` in Rust
- Asset ID format: `asset_{type}_{timestamp}_{random}`

### Asset ID Format
```
asset_{type}_{timestamp}_{random}
Examples:
  asset_image_1762958880043_2ba91eae
  asset_file_1762958880044_3cd02cbf
  asset_image_1762958880045_4de13dc0
```

### Asset Sync Flow

#### Step 1: Include Asset IDs in ResourceSyncRequest

**Viewer/Initiator:**
```rust
// Extract asset IDs from static_assets JSON map (NOT a Loro document)
let asset_ids = merge_service::extract_asset_ids(resource)?;

// Include in state_vectors JSON (nested under static_assets)
state_vectors: json!({
    "static_assets": {
        // NO state_vector field - static_assets is NOT a CRDT!
        "asset_ids": ["asset-123", "asset-456", "asset-789"]  // Asset IDs we have
    }
}).to_string()
```

#### Step 2: Node Compares Asset IDs (Set Operations)

**Node logic:**
```rust
// Extract viewer's asset IDs from state_vectors
let peer_state_vectors: HashMap<String, Value> = serde_json::from_str(&payload.state_vectors)?;
let static_assets_data = peer_state_vectors.get("static_assets")?;
let peer_asset_ids: Vec<String> = static_assets_data
    .get("asset_ids")
    .and_then(|v| v.as_array())
    .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
    .unwrap_or_default();

// Get node's asset IDs
let node_asset_ids = merge_service::extract_asset_ids(&resource)?;

// Set operations
let peer_set: HashSet<String> = peer_asset_ids.into_iter().collect();
let node_set: HashSet<String> = node_asset_ids.into_iter().collect();

let missing_on_peer: Vec<String> = node_set.difference(&peer_set).cloned().collect();
let missing_on_node: Vec<String> = peer_set.difference(&node_set).cloned().collect();
```

#### Step 3: Node Responds with Missing Asset IDs

**Node sends:**
```rust
UpdatesResponse {
    updates: json!({
        "static_assets": {
            "updates": [13, 14, ...],  // CRDT updates for metadata
            "state_vector": [0, 15, ...]
        }
    }).to_string(),
    missing_asset_ids: missing_on_peer,  // ["asset-999", "asset-1000"]
    // ...
}
```

#### Step 4: Asset Transfer (Separate Messages)

**Node sends assets peer needs:**
```rust
for asset_id in missing_asset_ids {
    // Load asset binary data from storage
    let asset = resource_service::retrieve_asset(&resource_id, &asset_id, repo_ctx)?;

    // Send AssetTransfer message
    AssetTransfer {
        resource_id: resource_id.clone(),
        asset_id: asset.asset_id,
        asset_data: asset.data,  // Binary bytes
        metadata: json!({
            "mime_type": asset.mime_type,
            "size": asset.data.len(),
            "filename": "logo.png"
        }).to_string()
    }
}
```

**Peer stores received assets:**
```rust
// 1. Save binary data to storage
resource_service::store_asset(&resource_id, &asset, repo_ctx)?;

// 2. Update metadata in static_assets document
let mut resource = resource_service::get_resource_by_id(&resource_id, repo_ctx)?;
merge_service::apply_asset_metadata(&mut resource, vec![asset])?;
resource_service::update_resource(&resource_id, resource.to_json()?, user, repo_ctx)?;
```

### Asset Storage Location

```
<data_dir>/resources/<resource_id>/assets/<asset_id>.<ext>
```

Example:
```
/home/user/.osvauld/resources/abc123/assets/asset-999.png
/home/user/.osvauld/resources/abc123/assets/asset-1000.jpg
/home/user/.osvauld/resources/abc123/assets/asset-1234.pdf
```

---

## Folder Sync Protocol

### Overview

Folder sync involves 3 steps:
1. **Resource Discovery:** Determine which resources are missing on peer
2. **Missing Resources Transfer:** Full transfer of new resources
3. **Incremental Sync:** Parallel sync of existing resources

### Step 1: Resource Discovery

**Initiator prepares:**
- List all resources in folder
- For each resource: resource_id, resource_ucan, last_modified

**Initiator sends:**
```rust
FolderSyncRequest {
    folder_ucan: "initiator's folder UCAN",
    resources: json!([
        {
            "resource_id": "abc123",
            "resource_ucan": "eyJ...",
            "last_modified": 1699564800
        },
        {
            "resource_id": "def456",
            "resource_ucan": "eyJ...",
            "last_modified": 1699564900
        }
    ]).to_string()
}
```

**Responder processes:**
1. Check which resources it has vs doesn't have
2. Categorize: missing (need full transfer), existing (need incremental sync)

**Responder sends:**
```rust
FolderSyncResponse {
    folder_id: "xyz",
    missing_resource_ids: vec!["ghi789"],  // Responder doesn't have these
    existing_resource_ids: vec!["abc123", "def456"]  // Responder has these
}
```

### Step 2: Missing Resources Transfer

**For each missing resource:**

Initiator sends complete resource data:
```rust
ResourceDataSync {
    resource_id: "ghi789",
    resource_ucan: "resource UCAN",
    folder_ucan: "folder UCAN",
    resource_data: json!({
        "collaborative_doc": [1, 2, 3, ...],  // Full Loro doc bytes
        "content_doc": [4, 5, 6, ...],
        "template_doc": [7, 8, 9, ...],
        "static_assets": [10, 11, 12, ...],
        "submissions_doc": [13, 14, 15, ...]
    }).to_string(),
    asset_data: json!({
        "asset-123": {
            "data": [/* base64 or bytes */],
            "metadata": {"mime_type": "image/png", "size": 12345}
        }
    }).to_string()
}
```

**Responder processes:**
1. Create new resource locally
2. Import all Loro documents
3. Save all assets
4. Create share record

### Step 3: Incremental Sync (Parallel)

**For each existing resource (sent in parallel):**

Use standard ResourceSyncRequest flow (2 rounds as described above).

**Initiator sends multiple ResourceSyncRequests in parallel:**
```rust
// For abc123:
ResourceSyncRequest {
    resource_ucan: "resource UCAN for abc123",
    folder_ucan: "folder UCAN",
    state_vectors: json!({...}).to_string(),
    full_docs: json!({...}).to_string()
}

// For def456 (sent at same time):
ResourceSyncRequest {
    resource_ucan: "resource UCAN for def456",
    folder_ucan: "folder UCAN",
    state_vectors: json!({...}).to_string(),
    full_docs: json!({...}).to_string()
}
```

Each resource syncs independently with 2 rounds each.

**Performance:** All existing resources sync in parallel = faster than sequential.

---

## Viewer Submission Isolation

### Namespace Strategy

**Key principle:** Each viewer gets isolated namespace using their `user_id`.

**Structure:**
```rust
submissions_doc (LoroDoc)
  └─ submissions (LoroMap)
     ├─ "viewer:alice_user_id" → (LoroMap) {
     │    "answers": [...],
     │    "submitted_at": 1699564800,
     │    "status": "submitted"
     │  }
     ├─ "viewer:bob_user_id" → (LoroMap) {
     │    "answers": [...],
     │    "submitted_at": 1699568400,
     │    "status": "submitted"
     │  }
     └─ "viewer:charlie_user_id" → (LoroMap) {
          "answers": [...],
          "submitted_at": 1699572000,
          "status": "draft"
       }
```

### Isolation Guarantees

1. **No cross-viewer visibility:** Viewers never receive submissions_doc updates from node
   - Enforced by `no_update_from_node: ["submissions_doc"]` in viewer UCAN
   - `generate_updates_for_peer()` filters out submissions_doc for viewers

2. **Per-viewer namespacing:** Each viewer's data isolated by `viewer:{user_id}` key
   - Node merges all submissions into single document
   - Prevents overwrites between viewers

3. **Owner visibility:** Owner/node can see all submissions
   - Owner syncs normally (no no_update_from_node restriction)
   - Can read all viewer namespaces for grading/review

### Submission Workflow

**Viewer side:**
```rust
// 1. Viewer creates submission locally
let submission_doc = create_doc();
let submission_map = submission_doc.get_or_create_map("data")?;
submission_map.insert("answer1", "Response to question 1")?;
submission_map.insert("submitted_at", chrono::Utc::now().timestamp())?;

// 2. During sync, send full snapshot (not incremental updates)
let snapshot = export_snapshot(&submission_doc);

// 3. P2P layer includes in ResourceSyncRequest
ResourceSyncRequestMsg {
    full_docs: json!({
        "submissions_doc": snapshot
    }).to_string(),
    ...
}
```

**Node side:**
```rust
// 1. Receive full submission
let full_docs: HashMap<String, Vec<u8>> = serde_json::from_str(&payload.full_docs)?;
let submission_bytes = full_docs.get("submissions_doc").unwrap();

// 2. Extract viewer user_id from UCAN
let viewer_user_id = ucan_service::extract_user_id(&payload.resource_ucan).await?;

// 3. Apply to isolated namespace
merge_service::apply_submission(
    &mut resource,
    &viewer_user_id,
    submission_bytes,
    &payload.resource_ucan,
).await?;

// 4. Node does NOT send submissions_doc back to viewer
// (filtered by no_update_from_node in generate_updates_for_peer)
```

---

## Implementation Plan

### Phase 1: Message Protocol Updates

**File:** `core/src/models/p2p.rs`

**Changes:**
1. Update `ResourceSyncRequestMsg`:
   ```rust
   pub struct ResourceSyncRequestMsg {
       pub resource_ucan: String,
       pub folder_ucan: String,
       pub state_vectors: String,      // NEW
       pub full_docs: String,          // NEW
   }
   ```

2. Add new messages:
   ```rust
   pub struct FolderSyncRequestMsg {
       pub folder_ucan: String,
       pub resources: String,
   }

   pub struct FolderSyncResponseMsg {
       pub folder_id: String,
       pub missing_resource_ids: Vec<String>,
       pub existing_resource_ids: Vec<String>,
   }

   pub struct AssetTransferMsg {
       pub resource_id: String,
       pub asset_id: String,
       pub asset_data: Vec<u8>,
       pub metadata: String,
   }
   ```

3. Update message enums:
   ```rust
   pub enum FolderMessage {
       FolderDataSync(FolderDataSync),
       FolderTokenRequest(FolderTokenRequest),
       FolderTokenResponse(FolderTokenResponse),
       FolderSyncRequest(FolderSyncRequestMsg),      // NEW
       FolderSyncResponse(FolderSyncResponseMsg),    // NEW
   }

   pub enum ResourceMessage {
       // ... existing variants
       AssetTransfer(AssetTransferMsg),              // NEW
   }
   ```

### Phase 2: UCAN Template Updates

**File:** `sthalam/frontend/desktop/src/config/permissions.ts`

**Line 56-74 changes:**
```typescript
viewer_template: {
  capabilities: {
    "template_doc": "crud/readonly",
    "content_doc": "crud/readonly",
    "collaborative_doc": "crud/merge",
    "submissions_doc": "crud/submit",      // CHANGED from crud/appendonly
    "static_assets": "crud/readonly",
  },
  doc_types: {
    "static_assets": "asset",
    "template_doc": "crdt",
    "content_doc": "crdt",
    "collaborative_doc": "crdt",
    "submissions_doc": "crdt",
  },
  no_update_from_node: ["submissions_doc"],     // CHANGED: was ["user_content_doc"]
  dont_send_to_node: ["user_content_doc"],      // EXISTING
  full_doc_send: ["submissions_doc"]            // NEW
}
```

**File:** `services/src/ucan_service.rs`

**Changes:**

1. **Line 1109-1126:** REMOVE entire `viewer_template` block from `create_viewer_resource_token()`:
   ```rust
   // DELETE this entire block - viewer's own UCAN should NOT have viewer_template
   ```

2. **Line 1275 area:** Update `viewer_template` in `issue_resource_ucan_for_node()`:
   ```rust
   "viewer_template": {
       "capabilities": {
           "template_doc": "crud/readonly",
           "content_doc": "crud/readonly",
           "collaborative_doc": "crud/merge",
           "submissions_doc": "crud/submit",  // Changed
           "static_assets": "crud/readonly",
       },
       "doc_types": {
           "static_assets": "asset",
           "template_doc": "crdt",
           "content_doc": "crdt",
           "collaborative_doc": "crdt",
           "submissions_doc": "crdt"
       },
       "no_update_from_node": ["submissions_doc"],
       "dont_send_to_node": ["user_content_doc"],
       "full_doc_send": ["submissions_doc"]
   }
   ```

### Phase 3: Service Layer - Merge Service

**File:** `services/src/merge_service.rs`

**New functions:**

1. **`apply_submission()`** (insert after line 395):
   ```rust
   /// Apply viewer submission with per-viewer isolation
   pub async fn apply_submission(
       resource: &mut Resource,
       viewer_user_id: &str,
       submission_doc_bytes: &[u8],
       viewer_ucan: &str,
   ) -> ServiceResult<()> {
       // Validate crud/submit capability
       // Import viewer's submission document
       // Get or create submissions_doc
       // Store in isolated namespace: viewer:{user_id}
   }
   ```

2. **`extract_asset_ids()`** (insert after apply_submission):
   ```rust
   /// Extract asset IDs from static_assets HashMap
   ///
   /// NOTE: static_assets is NOT a Loro document!
   /// It's a HashMap<String, String> where keys are asset IDs and values are base64 strings.
   pub async fn extract_asset_ids(
       resource: &Resource,
   ) -> ServiceResult<Vec<String>> {
       // Simply extract keys from resource.static_assets HashMap
       Ok(resource.static_assets.keys().cloned().collect())
   }
   ```

3. **`prepare_sync_data()`** (insert after extract_asset_ids):
   ```rust
   /// Prepare sync data, separating CRDTs from assets
   pub async fn prepare_sync_data(
       resource: &Resource,
       ucan_token: &str,
   ) -> ServiceResult<PreparedSyncData> {
       // Extract doc_types, capabilities, full_doc_send from UCAN
       // For CRDT docs: export state vectors
       // For asset docs: export state vector + asset_ids
       // For full_doc_send: export full snapshots
   }
   ```

**Updated functions:**

1. **`generate_updates_for_peer()`** (update around line 164-177):
   ```rust
   // ADD: Extract no_update_from_node
   let no_update_from_node: Vec<String> = peer_facts
       .get("no_update_from_node")
       .and_then(|v| v.as_array())
       .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
       .unwrap_or_default();

   // In loop: ADD check
   if no_update_from_node.contains(&doc_name) {
       continue;  // Skip docs in no_update_from_node
   }
   ```

### Phase 4: Service Layer - Resource Service

**File:** `services/src/resource_service.rs`

**New functions:**

1. **`prepare_resource_sync_request()`**:
   ```rust
   /// Prepare all data needed for ResourceSyncRequest
   pub async fn prepare_resource_sync_request(
       resource_id: &str,
       user_id: &str,
       repo_ctx: Arc<RepositoryContext>,
       crypto_utils: &Arc<RwLock<CryptoUtils>>,
   ) -> ServiceResult<ResourceSyncRequestData> {
       // Get UCANs
       // Load resource
       // Call prepare_sync_data
       // Return structured data
   }
   ```

2. **`get_resource_list_for_folder()`**:
   ```rust
   /// Get list of resources in folder for folder sync
   pub async fn get_resource_list_for_folder(
       folder_id: &str,
       user_id: &str,
       repo_ctx: Arc<RepositoryContext>,
   ) -> ServiceResult<Vec<ResourceSyncInfo>> {
       // Get resource IDs in folder
       // Get share records
       // Return minimal sync info
   }
   ```

3. **`compare_asset_ids()`**:
   ```rust
   /// Compare asset IDs using set operations
   pub fn compare_asset_ids(
       local_asset_ids: &[String],
       peer_asset_ids: &[String],
   ) -> AssetComparison {
       // HashSet difference operations
       // Return missing on each side
   }
   ```

### Phase 5: P2P Orchestration - Resource Sync

**File:** `network/src/p2p/resource_sync.rs`

**Changes:**

1. **`handle_resource_sync_request()` (line 323)** - UPDATE:
   ```rust
   async fn handle_resource_sync_request(...) -> Result<()> {
       // 1. Load resource
       // 2. Parse state_vectors, full_docs from payload
       // 3. Apply full_docs (call apply_submission for submissions_doc)
       // 4. Generate updates (call generate_updates_for_peer)
       // 5. Compare asset IDs (call compare_asset_ids)
       // 6. Send UpdatesResponse directly (no StateVectorRequest)
   }
   ```

2. **`handle_updates_response()` (line 711-712)** - COMPLETE TODO:
   ```rust
   async fn handle_updates_response(...) -> Result<()> {
       // 1. Apply peer's updates
       // 2. Generate our response updates
       // 3. Send second UpdatesResponse (always, for 2 rounds)
   }
   ```

3. **`handle_asset_transfer()` (NEW)**:
   ```rust
   async fn handle_asset_transfer(
       payload: AssetTransferMsg,
       ...
   ) -> Result<()> {
       // 1. Save asset binary data
       // 2. Update metadata in static_assets doc
   }
   ```

### Phase 6: P2P Orchestration - Folder Sync

**File:** `network/src/p2p/folder_sync.rs`

**Changes:**

1. **`sync_folder_handler()` (line 280)** - IMPLEMENT:
   ```rust
   pub async fn sync_folder_handler(...) -> Result<()> {
       // 1. Get resource list
       // 2. Send FolderSyncRequest
       // 3. Wait for FolderSyncResponse
       // 4. Parallel sync all resources
   }
   ```

2. **`handle_folder_sync_request()` (NEW)**:
   ```rust
   async fn handle_folder_sync_request(...) -> Result<()> {
       // 1. Categorize missing vs existing
       // 2. Send FolderSyncResponse
   }
   ```

3. **`handle_folder_sync_response()` (NEW)**:
   ```rust
   async fn handle_folder_sync_response(...) -> Result<()> {
       // 1. Send ResourceDataSync for missing resources
       // 2. Parallel ResourceSyncRequest for existing resources
   }
   ```

### Phase 7: Testing

**Unit Tests:**
- `apply_submission()` - Viewer isolation
- `extract_asset_ids()` - Asset extraction
- `prepare_sync_data()` - CRDT vs asset separation
- `compare_asset_ids()` - Set operations
- `generate_updates_for_peer()` - no_update_from_node filtering

**Integration Tests:**
- Owner ↔ Node 2-round sync
- Node ↔ Viewer 2-round sync with mixed modes
- Folder sync 3-step flow
- Asset transfer protocol
- Parallel resource sync

### Phase 8: Deployment

1. Deploy message protocol changes (backward compatible)
2. Deploy UCAN updates (regenerate viewer UCANs)
3. Deploy service layer (with new merge logic)
4. Deploy orchestration layer (enable bidirectional sync)
5. Monitor and verify convergence behavior

---

## Summary

This design provides complete specifications for:

1. **Bidirectional CRDT sync** with 2-round convergence for all roles
2. **Viewer isolation** using `crud/submit` and `no_update_from_node`
3. **Asset handling** with IDs in state_vectors and separate binary transfer
4. **3-step folder sync** with parallel resource syncing
5. **Clean orchestration** with all logic in service layers

**Key Architectural Decisions:**
- Always 2 rounds for everyone (owner/node/viewer)
- `crud/submit` for submissions (NOT `crud/appendonly` or `crud/full_send`)
- `viewer_template` only in owner/node UCANs, NOT in viewer's own UCAN
- Asset IDs nested in state_vectors under static_assets entry
- Binary assets transferred separately via AssetTransfer messages
- All business logic in services, orchestration stays pure

**Ready for implementation:** Follow phase-by-phase plan for systematic rollout.
