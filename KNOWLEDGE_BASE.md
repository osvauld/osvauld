# Osvauld Protocol Technical Documentation

**Status**: Living Document - Updated as implementation progresses
**Last Updated**: 2025-11-10
**Phase 3 Complete**: UCAN Service Architecture implemented (code deduplication)

This document captures implementation details, algorithms, and technical decisions as we migrate from Yrs to Loro.

---

## Table of Contents

1. [Architecture Overview](#architecture-overview)
2. [Sovereign Node Connection Protocol](#sovereign-node-connection-protocol)
3. [Viewer Connection String Generation](#viewer-connection-string-generation)
4. [UCAN Token Structure](#ucan-token-structure)
5. [Resource Model](#resource-model)
6. [Encryption & Key Management](#encryption--key-management)
7. [Folder Sharing](#folder-sharing)
8. [Sync Protocol & Algorithms](#sync-protocol--algorithms)
9. [Search Indexing](#search-indexing)
10. [Code Patterns](#code-patterns)
11. [Implementation Notes](#implementation-notes)

---

## Architecture Overview

### System Topology
```
Owner's Desktop ←→ Owner's Node ←→ Viewers
```

**Roles**:
- **Owner's Desktop**: Content creation (separate device, e.g., laptop)
- **Owner's Node**: Server (separate device, e.g., Raspberry Pi in local network)
- **Viewers**: Request content from node

### Data Flow

**Publishing**:
```
Owner Desktop → (ResourceAdd) → Node stores encrypted
```

**Viewer Request**:
```
Viewer → (StateVectorRequest) → Node → filter & re-encrypt → Viewer
```

**Viewer Update**:
```
Viewer → (StateVectorRequest with updates) → Node → merge → relay to Owner
```

---

## Sovereign Node Connection Protocol

**Status**: ✅ Implemented (Phase 2.5 Complete)
**Last Updated**: 2025-11-09

### Overview

The sovereign node connection protocol establishes a secure P2P connection between the owner's desktop/mobile device and their sovereign node (e.g., Kunki running on a Raspberry Pi). The protocol uses reciprocal role-based UCAN tokens to identify each party.

**Connection Tokens Include**:
- Base capabilities: `user-connect` and `user-share`
- Role-based additional capabilities:
  - `role="node"` tokens include `add_folder` capability
  - `role="viewer"` tokens have no additional capabilities

### Core Concept: Reciprocal Roles

**Key Principle**: Each party stores a token describing WHO THEY ARE CONNECTING TO, not who they are.

- **Owner stores**: Token with `role='owner'` (identifies the node as owner's node)
- **Node stores**: Token with `role='node'` (identifies the owner as the node operator)

### First Connection Flow

#### 1. Generate Connection String (Kunki)

**Command**: `kunki token` or `kunki start --print-token`
**Location**: `kunki/src/main.rs:handle_token()` (line 367)

```rust
// Generate one-time UCAN with role='owner'
let (token, pub_key) = generate_one_time_ucan_token(
    domain,
    &UserRole::Owner.to_string(),  // Role: "owner"
    &crypto_utils,
    repo_ctx,
).await?;

// Create connection string (base64-encoded JSON)
let connection_details = json!({
    "user_public_key": user.public_key,
    "device_public_key": device.device_key,
    "username": user.username,
    "ucan_token": token,           // One-time token with role='owner'
    "ucan_pub_key": pub_key,
});
```

**Output**: Base64-encoded connection string containing one-time token with `role='owner'`

#### 2. Add Sovereign Node (Owner App)

**Frontend**: User pastes connection string into app
**Handler**: `handle_add_sovereign_node()` (tauri_handlers/src/handlers/p2p.rs:58-104)

```rust
// 1. Decode connection string
let details: UserDetails = decode_and_parse(input)?;

// 2. Save to database (first_sync=false)
let (_user, device) = add_known_user(
    details.username,
    details.user_public_key,
    details.device_public_key,
    details.ucan_token,  // One-time token with role='owner'
    details.ucan_pub_key,
    repo_ctx,
    crypto_utils,
).await?;

// 3. Connect immediately (happy path)
p2p_service.connect_with_ticket(&device.id).await?;
```

#### 3. Initiate Handshake (Owner)

**Function**: `initiate_handshake()` (network/src/p2p/handshake.rs:37-101)

```rust
// Extract peer role from stored one-time token
let peer_role_from_token = crypto_utils::get_role_from_ucan_token(&peer_user.ucan_token).await?;
let peer_role = PeerRole::from_string(&peer_role_from_token);  // PeerRole::Owner

debug!("Extracted peer role from UCAN token: {:?}", peer_role);  // "owner"

// Determine what role to issue them
// Reciprocal relationship: owner gets 'node' token, node gets 'owner' token
let issued_token_role = match peer_role {
    PeerRole::Owner => "node",  // If peer is owner, issue them 'node' token
    PeerRole::Node => "owner",  // If peer is node, issue them 'owner' token
    _ => "user",
};

// Issue persistent token
let new_ucan_token = issue_connect_ucan_token(
    repo_ctx,
    crypto_utils,
    domain,
    &peer_user.ucan_pub_key,
    issued_token_role,  // "node"
).await?;

// Send FirstConnectRequest
FirstConnectRequest {
    issued_ucan: new_ucan_token,      // Persistent token with role='node' for node to store
    one_time_ucan: peer_user.ucan_token,  // Node's one-time token with role='owner'
    ...
}
```

#### 4. Process Request (Node)

**Function**: `process_first_user_connection_request()` (handshake.rs:279-362)

```rust
// Extract peer role from one-time token
let peer_role_from_token = crypto_utils::get_role_from_ucan_token(&payload.one_time_ucan).await?;
let peer_role = PeerRole::from_string(&peer_role_from_token);  // PeerRole::Owner

info!("Extracted peer role from one-time UCAN token: {:?}", peer_role);  // "owner"

// Determine what role to issue them
// Reciprocal: owner gets 'node' token, node gets 'owner' token
let issued_token_role = match peer_role {
    PeerRole::Owner => "node",  // If peer is owner, issue them 'node' token
    PeerRole::Node => "owner",  // If peer is node, issue them 'owner' token
    _ => "user",
};

// Issue persistent token
let peer_issued_ucan_token = issue_connect_ucan_token(
    repo_ctx,
    crypto_utils,
    domain,
    &peer_ucan_pub,
    issued_token_role,  // "node"
).await?;

// Save owner to database (first_sync=true)
user.ucan_token = payload.issued_ucan.clone();  // Store token owner gave us (role='node')

// Set connection type
let connection_type = ConnectionType::from_peer_role(&peer_role);
self.set_connection_type(connection_type).await;

// Send FirstConnectResponse
FirstConnectResponse {
    issued_ucan: peer_issued_ucan_token,  // Persistent token with role='owner'
    ...
}

// Emit role-specific event
P2PEvent::UserConnected { peer_id }  // Based on peer_role extraction
```

#### 5. Handshake Complete

**Owner**: `process_first_user_connection_handshake_response()` (handshake.rs:405-451)

```rust
// Validate and extract role from issued token
let peer_role = PeerRole::from_string(&peer_role_from_token);  // PeerRole::Owner (from node's response)

// Save node to database (first_sync=true)
user.ucan_token = payload.issued_ucan.clone();  // Store token node gave us (role='owner')

// Set connection type
let connection_type = ConnectionType::from_peer_role(&peer_role);
self.set_connection_type(connection_type).await;

// Emit role-specific event
P2PEvent::UserConnected { peer_id }
```

**Result**:
- Owner's DB: Stores node with token (role='owner'), first_sync=true
- Node's DB: Stores owner with token (role='node'), first_sync=true
- Both sides: Connection established with appropriate ConnectionType

### Reconnection Flow (first_sync=true)

**When**: App restart, manual reconnection

**Flow**: `initiate_handshake()` (handshake.rs:87-101)

```rust
// Extract peer role from stored token
let peer_role = PeerRole::from_string(&peer_role_from_token);

// Send UcanAndUserExchange with stored token
HandshakeExchange {
    ucan_token: peer_user.ucan_token,  // Persistent token from database
    ...
}

// Set connection type from peer role
let connection_type = ConnectionType::from_peer_role(&peer_role);
self.set_connection_type(connection_type).await;
```

**Processing**: `process_exchange_message()` (handshake.rs:173-230)

```rust
// Extract peer role from token
let peer_role = PeerRole::from_string(&peer_role_from_token);

// Set connection type
let connection_type = ConnectionType::from_peer_role(&peer_role);
self.set_connection_type(connection_type).await;

// Emit Connected event
P2PEvent::Connected { peer_id }
```

### P2P Events

**New Events** (network/src/p2p/emitter.rs:17-27):
```rust
pub enum P2PEvent {
    NodeConnected { peer_id: String },      // Sovereign node connected
    UserConnected { peer_id: String },      // User/owner connected
    ViewerConnected { peer_id: String },    // Viewer connected
    FirstConnection { peer_id: String },    // First-time handshake complete
    Connected { peer_id: String },          // Reconnection complete
    ...
}
```

**Emission**: During handshake, events are emitted based on extracted `peer_role`:
```rust
let event = match peer_role {
    PeerRole::Node => P2PEvent::NodeConnected { peer_id },
    PeerRole::Viewer => P2PEvent::ViewerConnected { peer_id },
    _ => P2PEvent::UserConnected { peer_id },
};
```

### Key Implementation Details

**File Locations**:
- PeerRole enum: `core/src/models/p2p.rs:10-37`
- ConnectionType enum: `network/src/p2p/peer_connection.rs:16-32`
- Handshake logic: `network/src/p2p/handshake.rs`
- Handler: `tauri_handlers/src/handlers/p2p.rs:58-104`
- Frontend: `sthalam/frontend/desktop/src/utils/helper.ts:103`
- Kunki: `kunki/src/main.rs` (p2p_init integration, password masking)

**Security Features**:
- Password prompting with `rpassword` (invisible input)
- One-time tokens replaced with persistent tokens after first handshake
- Role-based connection type enforcement
- Immediate connection (no background spawn for simplicity)

**No Auto-Retry** (Happy Path Only):
- Connection failures are not automatically retried
- Manual reconnection required
- Future enhancement: Add auto-retry on startup for nodes with first_sync=false

### Connection Token Capabilities

**Status**: ✅ Implemented (2025-11-09)

#### add_folder Capability for Node Connections

**Purpose**: Grants permission to add/share folders between owner and node

**Why It's Needed**:
- When owner shares a folder with node, node must validate the request
- Owner's token (issued by node) must prove owner has `add_folder` permission
- Without this capability, folder sharing would fail validation

**Implementation**: Role-based capability determination at service layer

**File Locations**:
- Service logic: `services/src/user_service.rs:141-152` (issue_connect_ucan_token)
- Crypto wrapper: `crypto_utils/src/crypto_utils.rs:298-323` (issue_connect_and_share_user_token)
- Core UCAN generation: `crypto_utils/src/ucan_utils.rs:252-294` (generate_delegation_and_connection_token)

**Service Layer Logic** (services/src/user_service.rs:141-152):
```rust
// Determine additional capabilities based on role
let additional_capabilities = match role {
    "owner" => {
        // Owners can add folders (for owner ↔ node connections)
        vec![(format!("{}:add_folder", domain), "use".to_string())]
    }
    "node" => {
        // Nodes can add folders (for future node ↔ node connections)
        vec![(format!("{}:add_folder", domain), "use".to_string())]
    }
    "viewer" => {
        // Viewers have no additional capabilities beyond connect and share
        vec![]
    }
    _ => vec![],
};
```

**Token Structure** (role="owner"):
```json
{
  "cap": {
    "{domain}:user-connect:{user_id}": {"use": [{}]},
    "{domain}:user-share:{user_id}": {"use": [{}]},
    "{domain}:add_folder": {"use": [{}]}
  },
  "fct": {
    "role": "owner"
  }
}
```

**Note**: Both "owner" and "node" roles receive `add_folder` capability to enable folder sharing in both directions.

**Handshake Flow** (Updated 2025-11-09):
1. **Owner → Node** (FirstConnectRequest):
   - Owner extracts peer_role = "owner" from node's one-time token
   - Owner uses same role: issued_token_role = "owner"
   - Owner issues token to node with role="owner" → includes `add_folder`

2. **Node → Owner** (FirstConnectResponse):
   - Node extracts peer_role = "owner" from owner's one-time token
   - Node uses same role: issued_token_role = "owner"
   - Node issues token to owner with role="owner" → includes `add_folder`

3. **Result**: Both sides receive tokens with role="owner" and `add_folder` capability

**Role Assignment Logic** (network/src/p2p/handshake.rs):
```rust
// Use the same role from the peer's token
// This ensures role consistency: owner gets 'owner', node gets 'node'
let issued_token_role = peer_role.as_str();
```

**Important**: The role in the token's facts field now **matches** the role from the peer's one-time token, ensuring consistency across connections.

**Validation During Folder Sharing**:
```rust
// When owner shares folder with node:
// 1. Node receives folder share request
// 2. Node looks up owner's token (the token node issued to owner)
// 3. Node parses capabilities from owner's token
// 4. Node validates: owner has "{domain}:add_folder" with "use" ability
// 5. If valid, accept folder; otherwise reject
```

**Architecture Pattern**:
- **Generic crypto function**: Accepts `additional_capabilities` parameter
- **Service layer determines capabilities**: Based on role being issued
- **Follows folder sharing pattern**: Handler/service determines business logic, crypto executes

**Design Benefits**:
- Reusable for different connection types (node, viewer, future roles)
- Role-based capability grants are centralized in service layer
- Easy to extend with new capabilities per role
- Consistent with overall UCAN architecture

### Resource Sync Validation

**Status**: ✅ Implemented (2025-11-09)

#### Validation Architecture

**Design**: Folder UCAN validates resource sync permissions, not peer connection token

**Critical Understanding**:
- **Peer connection tokens** grant connection-level capabilities:
  - `{domain}:add_folder` - can share folders
  - `{domain}:user-connect:{user_id}` - can connect to user
  - `{domain}:user-share:{user_id}` - can share with user
- **Folder UCANs** grant folder-level capabilities:
  - `{domain}:folder:{folder_id}:add_resources` - can add resources to folder
  - `{domain}:folder:{folder_id}:crud/read` - can read folder
  - `{domain}:folder:{folder_id}:share_folder` - can share folder

**Why Folder UCAN for Resource Validation**:
```
❌ Wrong: Check peer connection token for add_resources capability
   - Connection tokens don't have folder-specific capabilities
   - add_resources is a folder-level permission, not connection-level

✓ Correct: Check owner's folder UCAN for add_resources capability
   - Owner sends their folder UCAN proving they can add resources to this folder
   - Folder UCAN specifies which folder_id it applies to
   - Node validates folder_id in UCAN matches resource.folder_id
```

#### Validation Flow

**File**: `services/src/ucan_service.rs` (formerly `validation_service.rs`)

**Function**: `validate_peer_can_add_resources()`

**Algorithm**:
1. **Validate Folder UCAN Structure**:
   - Parse `owner_folder_ucan` token
   - Verify it's a valid UCAN token
   - Extract capabilities

2. **Extract Folder ID with add_resources Capability**:
   - Call `extract_folder_id_with_add_resources_capability()`
   - Looks for capability: `{domain}:folder:{folder_id}` with ability `add_resources`
   - Returns folder_id if found

3. **Verify Folder ID Match**:
   - Compare folder_id from UCAN with `expected_folder_id` (resource.folder_id)
   - Reject if mismatch - UCAN is for wrong folder

**Code Location**: `services/src/ucan_service.rs:110-147`

```rust
pub async fn validate_peer_can_add_resources(
    owner_folder_ucan: &str,
    expected_folder_id: &str,
    domain: &str,
) -> ServiceResult<()> {
    // 1. Validate owner's folder UCAN structure
    let folder_ucan = crypto_utils::ucan_utils::validate_structure(owner_folder_ucan)
        .await?;

    // 2. Extract folder_id and verify add_resources capability
    let folder_id_from_ucan =
        crypto_utils::ucan_utils::extract_folder_id_with_add_resources_capability(
            &folder_ucan, domain,
        )?;

    // 3. Verify folder_id matches expected folder_id
    if folder_id_from_ucan != expected_folder_id {
        return Err(ResourceServiceError::UcanError(format!(
            "Folder ID mismatch: UCAN has {}, expected {}",
            folder_id_from_ucan, expected_folder_id
        )));
    }

    Ok(())
}
```

#### UCAN Utility Function

**File**: `crypto_utils/src/ucan_utils.rs:466-497`

**Function**: `extract_folder_id_with_add_resources_capability()`

**Purpose**: Extract folder_id from UCAN while verifying add_resources permission

**Algorithm**:
```rust
pub fn extract_folder_id_with_add_resources_capability(
    ucan: &Ucan,
    domain: &str,
) -> Result<String, UcanError> {
    let folder_pattern = format!("{}:folder:", domain);

    for capability in ucan.capabilities().iter() {
        let cap_resource = capability.resource;

        // Match: {domain}:folder:{folder_id}
        if cap_resource.starts_with(&folder_pattern)
           && capability.ability == "add_resources" {
            // Extract folder_id from capability resource
            if let Some(folder_id) = cap_resource.strip_prefix(&folder_pattern) {
                if !folder_id.is_empty() && folder_id != "*" {
                    return Ok(folder_id.to_string());
                }
            }
        }
    }

    Err(UcanError::CapabilityNotFound)
}
```

#### Resource Sync Message Structure

**File**: `core/src/models/p2p.rs`

**Message**: `ResourceDataSync`

```rust
pub struct ResourceDataSync {
    pub resource: EncryptedResource,
    pub share_records: Vec<ShareRecord>,  // All share records for this resource
    pub owner_folder_ucan: String,         // Owner's folder UCAN (proves add_resources)
}
```

**Fields**:
- `resource`: Re-encrypted resource with filtered docs for recipient
- `share_records`: All ShareRecord entries for this resource (enables forwarding viewer updates to owner)
- `owner_folder_ucan`: Owner's folder UCAN token containing `add_resources` capability

#### Integration Points

**Sender** (`network/src/p2p/folder_sync.rs:27-33`):
```rust
// Get owner's folder to extract UCAN
let owner_folder = get_folder_by_id(folder_id, repo_ctx.clone()).await?;
let owner_folder_ucan = owner_folder.ucan.clone();

// Pass to resource sync
resource_sync::send_all_resources_for_folder(
    folder_id,
    recipient_user_id,
    current_user,
    owner_folder_ucan,  // Owner's UCAN proving add_resources
    peer_conn,
    repo_ctx,
    crypto_utils,
).await
```

**Receiver** (`services/src/resource_service.rs:748-763`):
```rust
pub async fn accept_resource_from_peer(
    resource: &EncryptedResource,
    share_records: &[ShareRecord],
    owner_folder_ucan: &str,  // Validate this
    domain: &str,
    repo_ctx: Arc<RepositoryContext>,
) -> ServiceResult<()> {
    // Validate owner's folder UCAN has add_resources for this folder
    crate::validate_peer_can_add_resources(
        owner_folder_ucan,
        &resource.folder_id,
        domain,
    ).await?;

    // Save resource with all share records in transaction
    repo_ctx
        .resource_repo
        .save_resource_with_share_records(resource, share_records)
        .await?;

    Ok(())
}
```

#### Handshake Token Storage Fix

**Status**: ✅ Fixed (2025-11-09)

**Problem**: During handshake, peer's connection token was being stored incorrectly
- `peer_conn.user.ucan_token` contained the token WE issued TO the peer
- We needed the token the peer sent TO us (proves their capabilities)

**Solution** (`network/src/p2p/handshake.rs:168-172`):
```rust
// Update peer user's ucan_token with the token they sent us (proves their capabilities)
let mut updated_peer_user = payload.peer_user.clone();
updated_peer_user.ucan_token = payload.ucan_token.clone();

self.set_peer_user_and_device(updated_peer_user, payload.peer_device.clone()).await;
```

**Impact**: Both folder_sync and resource_sync handlers now get correct peer connection token via `peer_conn.user.ucan_token`

### Testing Checklist

- [x] Kunki generates connection string correctly
- [x] handle_add_sovereign_node saves to DB
- [x] Auto-connection triggers after add
- [x] FirstConnectRequest with correct roles
- [x] Token validation succeeds
- [x] FirstConnectResponse received
- [x] Both sides save with first_sync=true
- [x] Role-specific events emitted (NodeConnected/UserConnected)
- [x] Reconnection works with stored tokens
- [x] Connection type set correctly based on peer_role
- [x] Password masking works in Kunki CLI
- [x] Connection tokens include add_folder capability for role="node"
- [x] Folder sharing validates add_folder capability from owner's token
- [x] Peer connection token correctly stored during handshake
- [x] Resource sync validates folder UCAN add_resources capability
- [x] Folder ID in UCAN matches resource.folder_id

---

## Viewer Connection String Generation

**Status**: ✅ Implemented (2025-11-10)
**Last Updated**: 2025-11-10

### Overview

The Viewer Connection String Generation feature allows owners to request shareable connection strings from their sovereign nodes. These connection strings contain viewer UCAN tokens that enable public viewers to connect to the node and request resources from specific folders.

**Flow**: Owner's Desktop → Request → Sovereign Node → Generate Viewer Token → Return Connection String → Owner Shares with Viewers

### Architecture

```
Owner's Desktop                 Sovereign Node
      |                               |
      |  1. Request Token             |
      |  (FolderTokenRequest)         |
      |------------------------------>|
      |                               |
      |                               | 2. Validate get_share_link
      |                               | 3. Generate viewer UCAN
      |                               | 4. Create connection string
      |                               |
      |  5. Return String             |
      |  (FolderTokenResponse)        |
      |<------------------------------|
      |                               |
      |  6. Display in UI             |
```

### Implementation Components

#### 1. UCAN Token Generation

**File**: `crypto_utils/src/ucan_utils.rs:926-974`

**Function**: `generate_viewer_connection_token()`

**Purpose**: Generic token generator that accepts capabilities and facts from service layer

**Signature**:
```rust
pub async fn generate_viewer_connection_token(
    owner_signing_key: &SigningKey,
    owner_verifying_key: &VerifyingKey,
    capabilities: Vec<(String, String)>,
    facts: Option<serde_json::Map<String, serde_json::Value>>,
    audience: &str,
    expiry_seconds: Option<u64>,
) -> Result<String, UcanError>
```

**Key Features**:
- Accepts capabilities as `Vec<(resource, ability)>` tuples
- Accepts optional facts as JSON map
- Flexible audience (typically `"*"` for viewers)
- Configurable expiry (defaults to 30 years if None)
- Service layer controls all business logic

**Wrapper**: `crypto_utils/src/crypto_utils.rs:439-462`
```rust
pub async fn generate_viewer_connection_token(
    &self,
    encrypted_ucan_private_key: &str,
    capabilities: Vec<(String, String)>,
    facts: Option<serde_json::Map<String, serde_json::Value>>,
    audience: &str,
    expiry_seconds: Option<u64>,
) -> Result<String, CryptoError>
```

#### 2. UCAN Validation

**File**: `services/src/ucan_service.rs:158-212`

**Function**: `validate_peer_can_request_link()`

**Purpose**: Validates requester has `get_share_link` capability for the folder

**Validation Flow**:
1. Parse peer's folder UCAN structure
2. Extract folder_id with `get_share_link` capability
3. Verify folder_id matches expected folder
4. Return error if validation fails

**Capability Format**: `{domain}:folder:{folder_id}` with ability `get_share_link`

**Pattern**: Same as `validate_peer_can_add_resources()` but checks for different ability

#### 3. Service Layer Token Generation

**File**: `services/src/ucan_service.rs:214-292`

**Function**: `generate_viewer_token_for_folder()`

**Purpose**: Orchestrates validation and token generation

**Flow**:
1. Validate requester has `get_share_link` capability
2. Get encrypted UCAN key from store
3. Build capabilities:
   - `{domain}:user-connect:*` with `use` (universal connection)
   - `{domain}:folder:{folder_id}` with `request_resources`
   - `{domain}:folder:{folder_id}` with `get_share_link`
   - `{domain}:resource:{folder_id}/*` with `request_resources`
   - `{domain}:resource:{folder_id}/*` with `get_share_link`
4. Build facts:
   - `role: "viewer"`
   - `folder_id: "{folder_id}"`
5. Generate viewer token (30 days expiry)

**Returns**: Viewer UCAN token string

#### 4. P2P Message Handlers

**File**: `network/src/p2p/folder_sync.rs:132-223`

##### handle_folder_token_request()

**Purpose**: Generate and send viewer connection string

**Flow**:
1. Call `ucan_service::generate_viewer_token_for_folder()` to validate and generate token
2. Get current user and device info from peer connection
3. Get UCAN public key from encrypted store
4. Create connection details JSON:
   ```json
   {
     "user_public_key": "<node_pgp_key>",
     "device_public_key": "<node_device_key>",
     "username": "<node_username>",
     "ucan_token": "<viewer_ucan_token>",
     "ucan_pub_key": "<node_ucan_pub_key>",
     "folder_id": "<folder_id>"
   }
   ```
5. Base64-encode connection string
6. Send `FolderTokenResponse` message back to requester

##### handle_folder_token_response()

**Purpose**: Emit event to frontend with connection string

**Flow**:
1. Receive connection string from node
2. Emit `P2PEvent::FolderTokenReceived` with folder_id and connection_string
3. Frontend listener catches event and displays

**File**: `network/src/p2p/peer_connection.rs:412-424`

**Routing**: peer_connection delegates to folder_sync handlers
```rust
Message::FolderTokenRequest(payload) => {
    folder_sync::handle_folder_token_request(
        payload.clone(),
        Arc::new(self.clone()),
    )
    .await
}
Message::FolderTokenResponse(payload) => {
    folder_sync::handle_folder_token_response(
        payload.clone(),
        Arc::new(self.clone()),
    )
    .await
}
```

#### 5. Request Initiation

**File**: `network/src/p2p/sync_handler.rs:258-344`

**Function**: `request_folder_token()`

**Purpose**: Initiate folder token request to node

**Flow**:
1. Get current user
2. Get folder to extract UCAN
3. Get recipient's devices
4. Get or establish peer connection
5. Create and send `FolderTokenRequest`:
   ```rust
   FolderTokenRequest {
       folder_id: folder.id,
       folder_ucan: folder.ucan,  // Owner's folder UCAN (has get_share_link)
   }
   ```

#### 6. Event Bridge Layer

**File**: `sthalam/src-tauri/src/event_manager/tauri_listener.rs:28-77`

**Purpose**: Listen for frontend `"request-folder-token"` event

**Flow**:
1. Parse payload: `{folderId, deviceId, domain}`
2. Call `sync_handler::request_folder_token()`
3. Async spawn (fire-and-forget)

**File**: `sthalam/src-tauri/src/event_manager/p2p_listener.rs:80-94`

**Purpose**: Bridge P2P events to frontend

**Flow**:
1. Catch `P2PEvent::FolderTokenReceived`
2. Emit to frontend: `"folder-token-received"` with `{folderId, connectionString}`

#### 7. Frontend Integration

**File**: `sthalam/frontend/desktop/src/components/PublishWebsiteModal.svelte`

**Request Flow** (lines 78-90):
```javascript
await emit("request-folder-token", {
    folderId: currentWebsite.id,
    deviceId: selectedUser.id,
    domain: "sthalam"
});
```

**Response Flow** (lines 152-164):
```javascript
unlisten = await listen("folder-token-received", (event: any) => {
    const { folderId, connectionString } = event.payload;
    if (folderId === dataState.currentWebsite?.id) {
        generatedConnectionString = connectionString;
        isGeneratingLink = false;
    }
});
```

**UI Elements**:
- Dropdown to select published sovereign node
- "Generate Shareable Link" button
- Connection string display with copy button
- Loading states during generation

### Viewer UCAN Token Structure

```json
{
  "aud": "*",
  "cap": {
    "sthalam:user-connect:*": {
      "use": [{}]
    },
    "sthalam:folder:{folder_id}": {
      "request_resources": [{}],
      "get_share_link": [{}]
    },
    "sthalam:resource:{folder_id}/*": {
      "request_resources": [{}],
      "get_share_link": [{}]
    }
  },
  "fct": {
    "role": "viewer",
    "folder_id": "{folder_id}"
  },
  "exp": <30 days from now>,
  "iss": "did:key:z6Mk...<node_ucan_pub_key>"
}
```

**Capabilities Breakdown**:
1. `user-connect:*` - Can connect to any node (viewers may use different nodes)
2. `folder:request_resources` - Can request resources from this specific folder
3. `folder:get_share_link` - Can view/request share links for this folder
4. `resource:*/request_resources` - Can request specific resources in the folder
5. `resource:*/get_share_link` - Can get share links for resources in the folder

**Key Properties**:
- **Audience**: `"*"` (wildcard - any viewer can use this)
- **Role**: `"viewer"` (identifies as viewer connection)
- **Expiry**: 30 days (balances security with usability)
- **No proof chain**: Root token issued directly by node

### Connection String Format

**Base64-encoded JSON**:
```json
{
  "user_public_key": "<node_pgp_public_key>",
  "device_public_key": "<node_device_key>",
  "username": "<node_username>",
  "ucan_token": "<viewer_ucan_token>",
  "ucan_pub_key": "<node_ucan_pub_key>",
  "folder_id": "<folder_id>"
}
```

**Usage**: Viewers decode this string to extract:
- Node connection info (public keys, username)
- Viewer UCAN token for authentication
- Folder ID to request resources from

### Security Considerations

**Validation**:
- Owner must have `get_share_link` capability in their folder UCAN
- Only nodes can generate viewer tokens (sovereign node requirement)
- Viewer tokens have limited 30-day lifetime
- Viewer tokens grant read-only capabilities (`request_resources`)

**Isolation**:
- Each folder gets unique viewer token
- Viewers cannot access other folders
- Viewers cannot modify resources (no write capabilities)
- Token expiry forces periodic refresh

### Message Protocol

**FolderTokenRequest**:
```rust
pub struct FolderTokenRequest {
    pub folder_id: String,
    pub folder_ucan: String,  // Owner's folder UCAN
}
```

**FolderTokenResponse**:
```rust
pub struct FolderTokenResponse {
    pub folder_id: String,
    pub connection_string: String,  // Base64-encoded JSON
}
```

### Complete Event Flow

```
1. User clicks "Generate Shareable Link" in UI
   ↓
2. Frontend emits "request-folder-token"
   ↓
3. Tauri listener catches event
   ↓
4. Calls sync_handler::request_folder_token()
   ↓
5. sync_handler sends FolderTokenRequest to node
   ↓
6. peer_connection routes to folder_sync::handle_folder_token_request()
   ↓
7. folder_sync validates and generates viewer token
   ↓
8. folder_sync creates connection string
   ↓
9. folder_sync sends FolderTokenResponse back
   ↓
10. peer_connection routes to folder_sync::handle_folder_token_response()
   ↓
11. folder_sync emits P2PEvent::FolderTokenReceived
   ↓
12. p2p_listener catches event
   ↓
13. p2p_listener emits "folder-token-received" to frontend
   ↓
14. Frontend listener catches event
   ↓
15. UI displays connection string with copy button
```

### Implementation Checklist

- [x] Core UCAN generation function with flexible capabilities/facts
- [x] CryptoUtils wrapper for key decryption
- [x] UCAN validation for get_share_link capability
- [x] Service layer token generation orchestration
- [x] folder_sync handlers for request/response
- [x] peer_connection message routing
- [x] sync_handler request initiation
- [x] Tauri event listeners (request and response)

---

## Website Viewer Connection Protocol

**Status**: 🚧 In Progress (Phase 1 Complete)
**Last Updated**: 2025-11-10

### Overview

The Website Viewer Connection Protocol enables public viewers to connect to sovereign nodes using shareable connection strings, validate folder access, and receive read-only resource data. This protocol is separate from the owner-node connection protocol and uses a different message structure.

**Flow**: Viewer receives connection string → Connects to node → Sends WebsiteRequest → Node validates → Node sends folder and resources

### Architecture

```
Viewer (Browser/Desktop)          Sovereign Node
      |                                |
      |  1. Parse connection string    |
      |     (add node to DB)           |
      |                                |
      |  2. Establish P2P connection   |
      |------------------------------>|
      |                                |
      |  3. Validate & Send            |
      |     WebsiteRequest             |
      |------------------------------>|
      |                                |
      |                                | 4. Validate folder access
      |                                | 5. Check first_sync status
      |                                | 6. Check folder exists
      |                                |
      |  7. Send folder & resources    |
      |<------------------------------|
      |                                |
      |  8. Display content            |
```

### Phase 1: Connection Establishment & Validation (✅ Complete)

#### Message Structure

**WebsiteRequest** (sent by viewer):
```rust
pub struct WebsiteRequest {
    pub ucan_token: String,        // Viewer UCAN from connection string
    pub viewer_user: User,          // Viewer's User struct
    pub viewer_device: Device,      // Viewer's Device struct
    pub first_sync: bool,           // Viewer's first_sync status
}
```

**Message Wrapper**:
```rust
pub enum WebsiteMessage {
    WebsiteRequest(WebsiteRequest),
    // Future: WebsiteResponse, WebsiteReconnectRequest, etc.
}

pub enum Message {
    // ...
    Website(WebsiteMessage),  // Routed to website_handler
}
```

#### Viewer Side Implementation

**File**: `tauri_handlers/src/handlers/p2p.rs:handle_connect_to_website()`

**Flow**:
1. Parse connection string (base64 JSON with node info + UCAN token)
2. Derive node user_id from public key
3. Check if node already exists in viewer's database
   - If exists: Reuse existing device
   - If not: Call `add_known_user()` to save node
4. Fire-and-forget: Spawn async task to connect and send request

**File**: `network/src/p2p/sync_handler.rs:connect_to_website()`

**Parameters**:
- `device_id`: Node's device ID
- `ucan_token`: Viewer UCAN from connection string
- `p2p_service`: P2P service reference

**Flow**:
1. Establish P2P connection with node via `connect_with_ticket()`
2. Delegate to `website_handler::initiate_website_request()`

**File**: `network/src/p2p/website_handler.rs:initiate_website_request()`

**Validation Steps**:
1. Get viewer's local user/device from P2PService state
2. Get node device to find user_id
3. **Call `services::check_user_and_folder_status()`**:
   - Extract folder_id from UCAN token (via `ucan_service`)
   - Check if folder exists in viewer's database
   - Check if node user has completed first_sync
4. Handle validation results:
   - ❌ `!first_sync_done`: Return error (node not ready)
   - ❌ `!folder_exists`: Return error (folder not found)
   - ✅ Both true: Proceed to send WebsiteRequest
5. Create and send WebsiteRequest message

#### Node Side Implementation

**File**: `network/src/p2p/website_handler.rs:process_website_request()`

**Flow**:
1. Receive WebsiteRequest from viewer
2. Get node's local user/device from peer_conn
3. Extract repo_ctx and domain from peer_conn
4. **Call `services::check_user_and_folder_status()`**:
   - Extract folder_id from viewer's UCAN
   - Check if folder exists in node's database
   - Check if node user has completed first_sync
5. Return status: `(first_sync_done, folder_exists)`

**File**: `services/src/website_service.rs:check_user_and_folder_status()`

**Purpose**: Validate folder access and connection readiness

**Flow**:
1. Extract folder_id from UCAN using `ucan_service::extract_folder_id_with_add_resources()`
2. Check folder existence: `folder_repo.find_by_id(folder_id)`
3. Check first_sync: `user_repo.get_user_by_id(node_user_id)` → `user.first_sync`
4. Return `(first_sync_done, folder_exists)`

**Note**: Folder validation happens on BOTH sides:
- Viewer validates before sending request
- Node validates when receiving request

#### Message Routing

**peer_connection.rs** routes Website messages to centralized handler:
```rust
Message::Website(website_msg) => {
    website_handler::process_message(
        Arc::new(self.clone()),
        website_msg.clone(),
    ).await
}
```

**website_handler::process_message()** delegates to specific handlers:
```rust
match message {
    WebsiteMessage::WebsiteRequest(payload) => {
        process_website_request(peer_conn, payload).await
    }
    // Future: Other variants
}
```

### Phase 2: Folder and Resource Sync (⏳ TODO)

After validation succeeds, node must send folder and resources to viewer.

#### TODO: Response Flow

**Scenarios to handle**:
1. ✅ **First connection (`first_sync=true`, folder exists)**:
   - [ ] Send full folder metadata
   - [ ] Send all resources in folder
   - [ ] Use existing `folder_sync::send_folder_with_resources()`?

2. ⏳ **Reconnection (`first_sync=true`, folder exists, viewer already has folder)**:
   - [ ] Check if viewer already has folder (how to detect?)
   - [ ] Send only new/updated resources
   - [ ] Use CRDT merge protocol or full sync?

3. ❌ **First sync not done (`first_sync=false`)**:
   - [ ] Wait for first_sync completion?
   - [ ] Return error to viewer?
   - [ ] Queue request for later?

4. ❌ **Folder not found (`folder_exists=false`)**:
   - [ ] Return error to viewer
   - [ ] How to communicate this to viewer UI?

#### TODO: WebsiteResponse Message

Need to define response structure:
```rust
// TODO: Define in core/src/models/p2p.rs
pub struct WebsiteResponse {
    pub status: ConnectionStatus,  // Success, FolderNotFound, NotReady, etc.
    pub node_user: User,
    pub node_device: Device,
    // Additional fields?
}
```

#### TODO: Persistent Viewer Tokens

**Current**: One-time UCAN token from connection string
**Needed**: Node should issue persistent connection token for reconnections

- [ ] Define viewer-specific token format
- [ ] Store viewer connections on node side (or keep ephemeral?)
- [ ] Implement token refresh mechanism
- [ ] Handle token expiry and renewal

### Phase 3: Viewer Updates & Sync (⏳ TODO)

#### TODO: Bidirectional Sync

**Scenarios**:
1. **Viewer modifies resource (if allowed)**:
   - [ ] Viewer sends CRDT updates to node
   - [ ] Node validates write permissions
   - [ ] Node merges and propagates to owner

2. **Owner updates resource**:
   - [ ] Node pushes updates to connected viewers
   - [ ] Use existing resource sync protocol?

3. **Collaborative editing**:
   - [ ] Multiple viewers editing same resource
   - [ ] CRDT conflict resolution
   - [ ] Real-time sync or periodic?

### Implementation Checklist

#### Phase 1: Connection & Validation (✅ Complete)
- [x] WebsiteMessage and WebsiteRequest types
- [x] Connection string parser (reuses sovereign node parser)
- [x] Viewer-side: parse connection string and add node to DB
- [x] Viewer-side: establish P2P connection
- [x] Viewer-side: validate folder access before sending request
- [x] Viewer-side: send WebsiteRequest message
- [x] Node-side: receive and route WebsiteRequest
- [x] Node-side: validate folder access and first_sync status
- [x] website_service: check_user_and_folder_status()
- [x] Centralized message routing (website_handler::process_message)
- [x] Error handling for validation failures

#### Phase 2: Folder & Resource Sync (⏳ TODO)
- [ ] Define WebsiteResponse message structure
- [ ] Implement send_folder_to_viewer()
- [ ] Implement send_resources_to_viewer()
- [ ] Handle first connection vs reconnection
- [ ] Detect if viewer already has folder
- [ ] Emit events to frontend for UI updates
- [ ] Handle errors gracefully (folder not found, etc.)
- [ ] Test with real connection strings

#### Phase 3: Bidirectional Sync (⏳ TODO)
- [ ] Define viewer write permissions
- [ ] Implement viewer → node update protocol
- [ ] Implement node → viewer update push
- [ ] Handle collaborative editing conflicts
- [ ] Persistent viewer connection tokens
- [ ] Token refresh mechanism
- [ ] Viewer session management

### Security Considerations

**Viewer Tokens**:
- ✅ Tokens have limited capabilities (`request_resources`)
- ✅ Tokens scoped to specific folder (folder_id in UCAN)
- ⏳ Token expiry enforcement (30 days - needs testing)
- ⏳ Token refresh mechanism (TODO)

**Validation**:
- ✅ Folder access validated on both sides
- ✅ first_sync status checked before allowing access
- ⏳ UCAN signature verification (basic structure check only)
- ⏳ Rate limiting for viewer requests (TODO)

**Isolation**:
- ✅ Viewers cannot access other folders
- ✅ Viewers stored in separate database (ephemeral)
- ⏳ Viewers cannot see other viewers (TODO: verify)
- ⏳ Resource filtering based on viewer permissions (TODO)

### Key Differences from Sovereign Node Protocol

**Sovereign Node Connection**:
- Reciprocal role-based UCANs (owner ↔ node)
- Persistent connection (first_sync protocol)
- Full read/write access
- Stored in user database

**Website Viewer Connection**:
- One-way UCAN (viewer has token for node)
- Ephemeral/session-based connection
- Read-only access (TODO: verify)
- NOT stored in node's user database (or is it?)

### Open Questions

1. **Viewer Storage**: Should viewers be stored in node's database?
   - Current: YES (added via `add_known_user()`)
   - Alternative: Ephemeral (not stored, session-only)
   - Decision: TBD

2. **Folder Detection**: How to detect if viewer already has folder?
   - Check folder in viewer's DB?
   - Use CRDT state vectors?
   - Always send full folder?

3. **Write Permissions**: Can viewers modify resources?
   - Current token: Only `request_resources` capability
   - Future: Add write capabilities for collaborative editing?

4. **Connection Lifecycle**: When to disconnect viewers?
   - On browser close?
   - After inactivity timeout?
   - Never (persistent connection)?

5. **Token Storage**: Where should viewer connection token be stored on viewer side?
   - In user.ucan_token field (current)?
   - Separate viewer_tokens table?
   - Not stored at all (ephemeral)?
- [x] Frontend event emission and listening
- [x] UI integration in PublishWebsiteModal

### Future Enhancements

**Token Refresh**:
- Implement automatic token refresh before expiry
- Notify owner when viewer tokens are expiring
- Allow manual token revocation

**Analytics**:
- Track viewer token usage
- Monitor connection attempts
- Alert on suspicious access patterns

**Advanced Capabilities**:
- Time-based access restrictions
- Resource-specific viewer tokens
- Multi-folder viewer access

---

## UCAN Token Structure

### Capability Format

```
{domain}:resource:{resource_id}:{doc_name} - {ability}
```

**Example Token**:
```json
{
  "cap": {
    "sthalam:resource:abc123:content": {"crud/readonly": [{}]},
    "sthalam:resource:abc123:comments": {"crud/merge": [{}]},
    "sthalam:resource:abc123:submissions": {"crud/appendonly": [{}]}
  }
}
```

### Sync Abilities

| Ability | Behavior | Use Case |
|---------|----------|----------|
| `crud/readonly` | Pull only | Public content |
| `crud/merge` | Bidirectional | Collaborative docs |
| `crud/appendonly` | Push only | Form submissions |

### Two-Template Architecture

**Status**: ✅ Implemented (Phase 2)

**Design**: Owner UCAN contains two templates in facts section:
- `owner_template` - Full access capabilities (for owner and node roles)
- `viewer_template` - Restricted capabilities (for viewer role)

**UcanFacts Structure**:
```rust
#[derive(Debug, Clone)]
pub struct UcanFacts {
    pub owner_template: UcanTemplate,
    pub viewer_template: UcanTemplate,
}

#[derive(Debug, Clone)]
pub struct UcanTemplate {
    pub capabilities: HashMap<String, String>,  // doc_name → ability
    pub no_update_from_node: Vec<String>,       // Local-only docs (don't accept)
    pub dont_send_to_node: Vec<String>,         // Local-only docs (don't send)
}
```

**Owner UCAN Example**:
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

### Extraction Algorithm

**Status**: ✅ Implemented (Phase 1.3 & Phase 2)

**File**: `crypto_utils/src/ucan_utils.rs`

**Functions Implemented**:

#### extract_doc_capabilities(token: &str)
**Purpose**: Extract document capabilities from UCAN token
**Returns**: `HashMap<String, String>` - Map of doc_name → ability
**Example**: `{"main_doc": "crud/readonly", "comments": "crud/merge"}`

**Algorithm**:
1. Parse UCAN token using `Ucan::try_from()`
2. Iterate through all capabilities
3. Filter capabilities matching pattern: `domain:resource:resource_id:doc_name`
4. Extract doc_name (4th part) and ability
5. Return HashMap or error if no doc capabilities found

#### extract_ucan_facts(token: &str)
**Purpose**: Extract complete UcanFacts with both templates
**Returns**: `Result<UcanFacts, UcanError>`
**Usage**: Get both owner_template and viewer_template from owner UCAN

**Algorithm**:
1. Parse UCAN and extract facts section
2. Extract owner_template using extract_owner_template()
3. Extract viewer_template using extract_viewer_template()
4. Return UcanFacts struct or error if templates missing

#### extract_owner_template(token: &str)
**Purpose**: Extract owner template from UCAN facts
**Returns**: `Result<UcanTemplate, UcanError>`

**Algorithm**:
1. Parse UCAN and extract facts section
2. Look for `owner_template` key in facts
3. Extract `capabilities` object (required)
4. Extract `no_update_from_node` array (optional, defaults to empty)
5. Extract `dont_send_to_node` array (optional, defaults to empty)
6. Return UcanTemplate or error if template missing

#### extract_viewer_template(token: &str)
**Purpose**: Extract viewer template from UCAN facts
**Returns**: `Result<UcanTemplate, UcanError>`

**Algorithm**: Same as extract_owner_template but looks for `viewer_template` key

#### extract_facts(token: &str)
**Purpose**: General facts extractor
**Returns**: `Option<Map<String, Value>>` - Raw facts map
**Usage**: For extracting custom facts beyond templates

#### Existing Functions (already implemented):
- `extract_resource_id_from_ucan(ucan: &Ucan)` - Extracts resource ID from capabilities
- `extract_folder_id_from_ucan(ucan: &Ucan)` - Extracts folder ID from capabilities

**Pattern**: All new functions follow the pattern:
- Public function takes `token: &str`, parses and validates
- Internal helper takes `ucan: &Ucan`, does the extraction
- Enables both convenience (parse once) and efficiency (reuse parsed UCAN)

### Flexible UCAN Generation

**Status**: ✅ Implemented (Phase 2)

**File**: `crypto_utils/src/ucan_utils.rs` (core logic) + `crypto_utils/src/crypto_utils.rs` (wrapper)

#### generate_flexible_resource_owner_ucan()
**Purpose**: Generate owner UCAN with custom templates from frontend
**Location**: `crypto_utils/src/ucan_utils.rs`

**Signature**:
```rust
pub async fn generate_flexible_resource_owner_ucan(
    owner_signing_key: &SigningKey,
    owner_verifying_key: &VerifyingKey,
    resource_id: &str,
    capability_prefix: &str,
    ucan_template_json: &str,  // Frontend provides complete template
    expiry_seconds: Option<u64>,
) -> Result<(String, String), UcanError>
```

**Returns**: `(ucan_token, encrypted_private_key)`

**Algorithm**:
1. Parse ucan_template_json to get UcanFacts (owner_template + viewer_template)
2. Build capabilities from owner_template
3. For each doc in owner_template.capabilities:
   - Add capability: `{prefix}:resource:{resource_id}:{doc_name}` with ability
4. Add both templates to facts section
5. Generate UCAN token with capabilities and facts
6. Encrypt private key with owner's public key
7. Return (token, encrypted_key)

**Frontend Control**: Frontend sends complete template structure, backend doesn't hardcode any doc names or abilities

#### CryptoUtils Wrapper
**Purpose**: Decrypt owner's UCAN private key and generate UCAN
**Location**: `crypto_utils/src/crypto_utils.rs`

**Signature**:
```rust
pub async fn generate_flexible_resource_owner_ucan(
    &self,
    encrypted_ucan_private_key: &str,
    resource_id: &str,
    capability_prefix: &str,
    ucan_template_json: &str,
    expiry_seconds: Option<u64>,
) -> Result<(String, String), CryptoError>
```

**Algorithm**:
1. Decrypt owner's UCAN private key
2. Derive signing and verifying keys
3. Call ucan_utils::generate_flexible_resource_owner_ucan()
4. Return result

### Role-Based Delegation

**Status**: ✅ Implemented (Phase 2)

**File**: `crypto_utils/src/crypto_utils.rs`

#### issue_flexible_delegated_resource_ucan()
**Purpose**: Issue delegated UCAN with automatic template selection based on role

**Signature**:
```rust
pub async fn issue_flexible_delegated_resource_ucan<F, Fut>(
    &self,
    encrypted_delegator_private_key: &str,
    proof_ucan_string: &str,
    verifier_ucan_pub_b64: &str,
    resource_id: &str,
    recipient_ucan_pub_key: &str,
    recipient_role: &str,  // "owner", "node", or "viewer"
    proof_resolver: &F,
) -> Result<(String, String), CryptoError>
where
    F: Fn(String) -> Fut,
    Fut: Future<Output = Option<String>>,
```

**Returns**: `(delegated_ucan_token, encrypted_private_key)`

**Algorithm**:
1. Extract UcanFacts from proof UCAN (contains both templates)
2. Select template based on recipient_role:
   - "owner" or "node" → use owner_template
   - "viewer" → use viewer_template
3. Build capabilities from selected template
4. For each doc in template.capabilities:
   - Add capability: `{prefix}:resource:{resource_id}:{doc_name}` with ability
5. Add selected template to facts (for potential re-delegation)
6. Issue delegated UCAN with proof chain
7. Encrypt private key with recipient's public key
8. Return (token, encrypted_key)

**Role Extraction**: share_resource() extracts role from recipient's user token

**Template Inheritance**: Delegated UCAN includes the selected template in facts, enabling viewers to potentially re-delegate (if needed in future)

### Delegated UCAN Facts Propagation

**Status**: ✅ Fixed (2025-11-08)

**Critical Bug**: Delegated UCANs were missing the entire `fct` field

**Problem**:
- Owner UCANs correctly contained facts: owner_template, viewer_template, doc_types, docs, role
- Node's delegated UCANs had NO facts at all - completely empty `fct` field
- Root cause: `generate_delegated_ucan()` only copied capabilities and proof, not facts

**Solution** (crypto_utils/src/ucan_utils.rs:1026):

**Updated generate_delegated_ucan() signature**:
```rust
pub async fn generate_delegated_ucan(
    delegator_signing_key: &SigningKey,
    delegator_verifying_key: &VerifyingKey,
    recipient_ucan_pub_key: &str,
    permissions: Vec<(String, String)>,
    proof_ucan_string: &str,
    // NEW PARAMETERS:
    template_value: Option<serde_json::Value>,     // Template JSON from parent
    recipient_role: &str,                          // "owner", "node", or "viewer"
    doc_types_value: Option<serde_json::Value>,    // Asset vs CRDT classification
    docs_list: Option<Vec<String>>,                // List of document names
) -> Result<(String, String), UcanError>
```

**Facts Added to Delegated UCAN** (lines 1067-1088):
```rust
// 7. Add template to facts based on role
if let Some(template) = template_value {
    let template_key = match recipient_role {
        "owner" | "node" => "owner_template",
        "viewer" => "viewer_template",
        _ => "owner_template",
    };
    builder = builder.with_fact(template_key, template);
}

// 8. Add role to facts
builder = builder.with_fact("role", recipient_role.to_string());

// 9. Add doc_types to facts if provided
if let Some(doc_types) = doc_types_value {
    builder = builder.with_fact("doc_types", doc_types);
}

// 10. Add docs list to facts if provided
if let Some(docs) = docs_list {
    builder = builder.with_fact("docs", docs);
}
```

**Updated issue_flexible_delegated_resource_ucan()** (crypto_utils/src/crypto_utils.rs:571):

**Facts Extraction** (lines 625-650):
```rust
// 4. Extract facts from parent UCAN for delegation
let facts = ucan_to_prove.facts();

// Determine template key based on role
let template_key = match recipient_role {
    "owner" | "node" => "owner_template",
    "viewer" => "viewer_template",
    _ => "owner_template",
};

// Extract template value, doc_types, and docs from parent UCAN facts
let template_value = facts.as_ref()
    .and_then(|f| f.get(template_key))
    .cloned();

let doc_types_value = facts.as_ref()
    .and_then(|f| f.get("doc_types"))
    .cloned();

let docs_list = facts.as_ref()
    .and_then(|f| f.get("docs"))
    .and_then(|v| v.as_array())
    .map(|arr| {
        arr.iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect()
    });
```

**Delegation Call** (lines 656-666):
```rust
let (new_token, new_cid) = ucan_utils::generate_delegated_ucan(
    &delegator_signing_key,
    &delegator_verifying_key,
    recipient_ucan_pub_key,
    permissions_to_grant,
    proof_ucan_string,
    template_value,      // Pass extracted template
    recipient_role,      // Pass role
    doc_types_value,     // Pass doc_types
    docs_list,          // Pass docs
)
.await?;
```

**Result**:
- Delegated UCANs now contain complete `fct` structure matching owner UCANs
- Facts propagate through delegation chain
- Node can parse doc_types to classify assets vs CRDTs
- Node can parse templates for sync behavior

**Updated Legacy Methods**:
- `issue_delegated_folder_ucan()` - Passes `None` for template (folders don't use templates)
- `issue_delegated_resource_ucan()` - Passes `None` for template (uses explicit permissions)

**Use in Folder Sharing**:
- `share_folder()` in services/src/folder_service.rs:168
- Calls `issue_flexible_delegated_resource_ucan()` for each resource
- Facts automatically extracted and propagated to node's delegated UCAN
- Enables node to correctly parse CRDT vs asset docs during sync

---

## Resource Model

### Two-Struct Architecture

**Status**: ✅ Implemented

**File**: `core/src/models/resource.rs`

**Design Pattern**: Clear separation between storage and runtime representations

**EncryptedResource** (Database/Network):
- Stored in database with encrypted data
- Transmitted over network between peers
- Contains: id, folder_id, timestamps, encrypted_data, encrypted_key, ucan_token, metadata
- Metadata field is UNENCRYPTED (title, type, search config) for quick access without decryption

**Resource** (Runtime/In-Memory):
- Always contains decrypted data when loaded
- Service layer handles encryption boundary
- Contains: id, folder_id, ucan_token, metadata, docs HashMap
- LoroDoc instances loaded in memory for efficient operations
- No version vector caching - computed on-demand from LoroDoc

**Rationale**: Service layer decrypts EncryptedResource → creates Resource → business logic operates on decrypted Resource → service encrypts back to EncryptedResource for storage.

### Resource Type Handling

**Status**: ✅ Implemented

**Design Change**: ResourceType enum removed from structs, moved to metadata field

**Rationale**:
- UCAN tokens define doc structure dynamically
- No hardcoded doc names per resource type
- ResourceType becomes UI metadata, not backend logic
- Each resource can have any combination of docs based on UCAN capabilities

**Metadata Format**:
```
{
  "title": "Document Title",
  "type": "notes",  // UI hint only (notes, website, chat, etc.)
  "search": {
    "docs": ["main_doc", "comment_state"]  // Which docs to index for search
  }
}
```

### Key Methods

**Status**: ✅ Implemented

**File**: `core/src/models/resource.rs`

#### from_decrypted_data()
**Purpose**: Create Resource from decrypted JSON data

**Flow**:
1. Parse decrypted JSON (format: `{"doc_name": [bytes], ...}`)
2. For each doc, convert JSON array to Vec<u8>
3. Import snapshot using `document::import_snapshot()`
4. Store LoroDoc instance in docs HashMap
5. Return fully loaded Resource

**Usage**: Service layer calls this after decrypting encrypted_data from DB

#### filter_to_send()
**Purpose**: Filter docs and export snapshots for sending to a peer

**Two-UCAN Filtering**:
1. Parse our_ucan facts to find `dont_send_to_node` list
2. Parse peer_ucan capabilities to find which docs they can access
3. Intersection: only send docs that (we're allowed to send) AND (peer can receive)
4. Export each doc as SHALLOW snapshot using `document::export_shallow_snapshot()`
5. Return HashMap of doc_name → snapshot bytes

**Rationale**: Always send shallow snapshots (no history) to reduce data size. Peers don't need full oplog for collaboration.

#### get_state_vectors()
**Purpose**: Get version vectors for all documents

**Flow**:
1. For each LoroDoc in docs HashMap
2. Call `document::state_frontiers()` to get current state vector
3. Convert to JSON array format
4. Return JSON: `{"doc_name": {"state_vector": [bytes]}, ...}`

**Usage**: Peer sends these to request incremental updates

#### generate_updates()
**Purpose**: Generate updates for peer based on their state vectors

**Flow**:
1. Parse peer's state vectors from JSON
2. Check our_ucan facts for `dont_send_to_node` docs to exclude
3. Check peer_ucan capabilities to see which docs they can access
4. For each doc peer needs:
   - Extract their state vector
   - Call `document::export_updates(doc, &peer_state_vector)`
   - Get our current state vector with `document::state_frontiers()`
5. Return JSON with updates and current state vector per doc

**Incremental Sync**: Only sends operations peer doesn't have yet

#### apply_updates()
**Purpose**: Apply updates from peer with permission validation

**Flow**:
1. Parse peer_ucan to extract their doc capabilities
2. For each doc in updates:
   - Validate peer has capability for this doc
   - Reject if capability is `crud/readonly` (no write permission)
   - Accept if capability is `crud/merge` or `crud/appendonly`
3. Extract update bytes from JSON
4. Get or create LoroDoc in docs HashMap
5. Call `document::apply_updates(doc, &update_bytes)`

**Merge Behavior**: Currently uses regular CRDT merge for all capabilities (no append-only enforcement yet)

#### apply_updates_filtered()
**Purpose**: Apply updates with additional filtering for viewer's local-only docs

**Flow**:
1. Parse our_ucan facts to find `no_update_from_node` list
2. Filter out docs in that list (viewer keeps these local, doesn't accept node's updates)
3. Call `apply_updates()` with filtered updates

**Use Case**: Viewer has UI state, cart data that should never be overwritten by node

#### to_json()
**Purpose**: Export all docs as JSON for encryption and storage

**Flow**:
1. For each LoroDoc in docs HashMap
2. Call `document::export_shallow_snapshot()` to get snapshot bytes
3. Convert to JSON array format
4. Return JSON: `{"doc_name": [bytes], ...}`

**Usage**: Service layer calls this before encrypting Resource for DB storage

### UCAN Integration

**Status**: ✅ Complete (Phase 2)

**File**: `crypto_utils/src/ucan_utils.rs` (UCAN parsing) + `core/src/models/resource.rs` (usage)

**Capability Format**: `domain:resource:resource_id:doc_name`

**Facts Format**:
```json
{
  "owner_template": {
    "capabilities": {
      "doc_name": "crud/ability"
    }
  },
  "viewer_template": {
    "capabilities": {
      "doc_name": "crud/ability"
    },
    "no_update_from_node": ["uiState", "cartDoc"],
    "dont_send_to_node": ["uiState", "cartDoc"]
  }
}
```

**Implementation**: Full UCAN parsing in crypto_utils, used by Resource methods for filtering and permission validation

### Future Considerations

**Append-Only Enforcement**:
- Loro does NOT have native append-only mode
- Current implementation: regular CRDT merge for all capabilities
- Future: Could add application-layer validation by inspecting operations
- Alternative: Use signed operations where each addition includes author's signature
  - Enables verification without concurrent edit conflicts
  - Single-unit additions make signature verification feasible
  - To be implemented when needed

**Database Schema**:

**Status**: To be documented during Phase 5

New columns:
- `encrypted_data` - JSON with encrypted doc snapshots
- `encrypted_key` - AES key encrypted with recipient's pubkey
- `ucan_token` - UCAN token string
- `metadata` - Unencrypted JSON (title, type, search config)

Keep:
- `id`, `folder_id`, `created_at`, `updated_at`

Remove:
- Old Yrs-related fields
- `resource_vectors` table (no longer needed)
- `resource_keys` table (no longer needed)

---

## Encryption & Key Management

### Encryption Flow

**Status**: ✅ Implemented (Phase 2)

**File**: `services/src/resource_service.rs`

**Owner Encryption** (in create_resource):
```
1. Frontend sends Loro snapshots as JSON
2. Generate random AES-256 key
3. Encrypt JSON with AES-GCM
4. Encrypt AES key with owner's UCAN public key
5. Store encrypted_data and encrypted_key in DB
```

**Owner Decryption** (in decrypt_resources):
```
1. Fetch encrypted_data and encrypted_key from DB
2. Decrypt AES key using owner's UCAN private key
3. Decrypt JSON data with AES key
4. Parse JSON and load Loro documents
5. Return Resource with loaded docs
```

**Viewer Re-encryption** (in share_resource):
```
1. Load and decrypt resource with owner's key
2. Filter docs based on viewer's UCAN template
3. Export filtered docs as snapshots
4. Generate NEW random AES key
5. Encrypt filtered data with new key
6. Encrypt new key with viewer's UCAN public key
7. Store in share records table
```

**Update Flow** (in update_resource):
```
1. Frontend sends complete new Loro snapshots
2. Generate NEW random AES key (key rotation for forward secrecy)
3. Encrypt new snapshots with new key
4. Encrypt new key with owner's public key
5. Update BOTH encrypted_data AND encrypted_key in DB
6. Update updated_at timestamp
```

**Key Rotation Benefit**:
- Forward secrecy: Old encrypted data can't be decrypted if old key is compromised
- Each update creates a fresh encryption envelope
- Simple for single-user (Phase 2.5): Only owner's key in database
- Phase 3 consideration: Multi-user sharing will need P2P key distribution

### Key Lifecycle

**Status**: ✅ Implemented (Phase 2)

**Key Generation**:
- Each resource has unique AES-256 key generated at creation
- Keys stored encrypted with owner's UCAN public key
- Viewer shares get NEW AES keys (never reuse owner's key)

**Key Storage**:
- encrypted_key column in resources table (owner's copy)
- encrypted_key column in share_records table (viewer's copy)
- Each share has independent key for security isolation

**Key Rotation**: ✅ Implemented (Phase 2.5)
- **Status**: Enabled on every resource update
- **Implementation**: `services/src/resource_service.rs:313`
- **Mechanism**: Calls `encrypt_data_for_user()` which generates NEW random AES key
- **Database**: Updates both `encrypted_data` and `encrypted_key` fields
- **Single-user**: Simple implementation (only owner's key in database)
- **Multi-user consideration** (Phase 3):
  - Owner updates resource → new key generated
  - New key needs distribution to shared users via P2P
  - Each user maintains independent local encrypted copy
  - No need to re-encrypt in database (users store locally)

**Key Revocation**: Partial implementation
- Deleting share record removes viewer's access
- Viewer can no longer decrypt without encrypted_key
- No active key invalidation (relies on DB removal)

---

## Folder Sharing

### Architecture Overview

**Status**: ✅ Implemented (Phase 2.5 - 2025-11-08)

**Design Principle**: Folder sharing is a "thin wrapper" around resource sharing
- Creates folder-level ACL (folder_share_records)
- Creates resource-level ACLs for each resource (share_records)
- UCANs and encrypted data generated **on-demand during sync**, not pre-created
- Share records only contain UCAN tokens (access control)

### Two-Table ACL System

**folder_share_records**:
- Grants namespace access to folder
- Contains: folder_id, recipient_user_id, ucan_token (folder UCAN), ucan_cid
- Folder UCAN grants: `crud/read` + `share_folder` + `add_resources` capabilities
- Role: "node" (for sovereign nodes)

**folders table** (contains UCAN):
- Added `ucan` field (TEXT NOT NULL) to store owner's folder UCAN
- Migration: `persistance/migrations/2024-10-18-052005_create_initial_schema/up.sql:23`
- Model: `core/src/models/folder.rs` - Folder struct includes `pub ucan: String`
- Created with folder: When folder is created, owner's folder UCAN generated and stored
- Used during sync: Owner's folder UCAN proves `add_resources` permission when sending resources
- Updated before sending: When sending folder to node, `folder.ucan` replaced with recipient's token

**share_records**:
- Grants data access to individual resources
- Contains: resource_id, recipient_user_id, ucan_token (resource UCAN), ucan_cid
- Resource UCAN uses owner_template for nodes
- **NO encrypted_data or encrypted_key** pre-created
- Node requests encrypted data during sync when needed

### Folder Sharing Flow

**Service**: `share_folder()` (services/src/folder_service.rs:76)

**Steps**:
1. **Validation** (lines 86-110):
   - Validate folder exists
   - Validate recipient user exists
   - Check if already shared using efficient single query:
     ```rust
     folder_share_repo.find_by_folder_and_user(folder_id, recipient_user_id)
     ```

2. **Generate Folder UCAN** (lines 113-138):
   - Decrypt owner's UCAN signing keys
   - Call `generate_flexible_folder_token()`:
     - Capabilities: `crud/read` + `share_folder` + `add_resources`
     - Recipient role: "node"
     - 30-year lifetime
   - Generate CID from folder UCAN token

3. **Create Folder Share Record** (lines 141-153):
   - Store folder_id, recipient_user_id, ucan_token, ucan_cid
   - Permission level: Admin
   - Save to folder_share_records table

4. **Share All Resources** (lines 156-198):
   - Get all resources in folder
   - For each resource:
     - Read unencrypted `ucan_token` from resources table (owner's UCAN)
     - Call `issue_flexible_delegated_resource_ucan()`:
       - Validates parent UCAN permissions
       - Extracts owner_template from parent UCAN facts
       - Extracts doc_types, docs from parent UCAN facts
       - Builds permissions from template capabilities
       - Generates delegated UCAN with **facts propagation**
       - Returns (resource_ucan_token, resource_ucan_cid)
     - Create share_record with UCAN only:
       - **NO encrypted_data** (not pre-created)
       - **NO encrypted_key** (not pre-created)
       - Only ucan_token and ucan_cid
     - Save to share_records table

### On-Demand Encryption

**Design**: Share records only contain UCANs, not encrypted data

**Rationale**:
- Reduces storage overhead (no duplicate encrypted data)
- Enables dynamic filtering based on sync state
- Encrypted data created during sync when node requests it

**Sync Flow** (Phase 3):
```
1. Node requests resource with state vector
2. Owner/peer loads resource
3. Owner/peer filters docs based on node's UCAN
4. Owner/peer exports filtered snapshots
5. Owner/peer encrypts with node's public key (on-the-fly)
6. Owner/peer sends encrypted data + UCAN
7. Node stores encrypted data locally (not in database)
```

### Efficient Share Checking

**Repository Method**: `find_by_folder_and_user()` (persistance/src/repositories/folder_share_repository.rs:165)

**Before** (anti-pattern):
```rust
// Fetch all share records, filter in memory
let all_shares = repo.get_records_by_folder_id(folder_id).await?;
let existing = all_shares.iter().find(|s| s.recipient_user_id == user_id);
```

**After** (efficient):
```rust
// Single SQL query with WHERE clause
folder_share_records::table
    .filter(folder_share_records::folder_id.eq(folder_id))
    .filter(folder_share_records::recipient_user_id.eq(user_id))
    .filter(folder_share_records::operation_type.eq(ShareOperation::Share))
    .first::<FolderShareRecordModel>(&mut *conn)
    .optional()
```

**Benefit**: O(1) database query instead of O(n) memory filtering

### UCAN Facts Propagation

**Critical for Folder Sharing**: Delegated resource UCANs must contain facts for sync

**Facts Required**:
- `owner_template` or `viewer_template` - Sync capabilities
- `doc_types` - Asset vs CRDT classification
- `docs` - List of document names for parsing
- `role` - Recipient role for connection type

**Implementation**: See [Delegated UCAN Facts Propagation](#delegated-ucan-facts-propagation)

**Usage in Folder Sharing**:
- `share_folder()` calls `issue_flexible_delegated_resource_ucan()` (line 168)
- Facts automatically extracted from owner's UCAN
- Facts propagated to delegated UCAN for node
- Node can parse doc_types during sync to handle assets vs CRDTs differently

### Folder UCAN Capabilities

**Status**: ✅ Implemented (2025-11-09) - Template-based architecture

#### Token Architecture

**Three Types of Tokens**:

1. **Connection Tokens** - Establish peer-to-peer relationship
   - Owner's token for Node (issued by owner)
   - Node's token for Owner (issued by node)

2. **Folder UCANs** - Folder-level permissions with templates
   - Owner's folder UCAN (contains owner_template and node_template)
   - Node's folder UCAN (delegated from owner using template)

3. **Resource UCANs** - Resource-level permissions with templates
   - Owner's resource UCAN (contains owner_template and viewer_template)
   - Delegated resource UCANs (for node/viewer using templates)

#### Connection Tokens (Peer Authentication)

**Owner's Token for Node** (issued by owner to authenticate node):
```json
{
  "aud": "did:key:z6Mk...node_pub_key",
  "cap": {
    "sthalam:add_folder": {"use": [{}]},
    "sthalam:user-connect:owner_user_id": {"use": [{}]},
    "sthalam:user-share:owner_user_id": {"use": [{}]}
  },
  "fct": {
    "role": "node"
  },
  "iss": "did:key:z6Mk...owner_pub_key"
}
```

**Node's Token for Owner** (issued by node to authenticate owner):
```json
{
  "aud": "did:key:z6Mk...owner_pub_key",
  "cap": {
    "sthalam:user-connect:node_user_id": {"use": [{}]},
    "sthalam:user-share:node_user_id": {"use": [{}]}
  },
  "fct": {
    "role": "owner"
  },
  "iss": "did:key:z6Mk...node_pub_key"
}
```

#### Folder UCAN Token Structure (Template-Based)

**Owner's Folder UCAN** (contains templates in facts):
```json
{
  "aud": "did:key:z6Mk...owner_pub_key",
  "cap": {
    "sthalam:folder:2aed1e00-fe48-45a3-81cb-3fc27d323218": {
      "own": [{}],
      "get_share_link": [{}],
      "add_resources": [{}],
      "crud/read": [{}],
      "crud/update": [{}],
      "crud/delete": [{}],
      "share_folder": [{}]
    }
  },
  "fct": {
    "role": "owner",
    "owner_template": {
      "capabilities": {
        "own": "own",
        "get_share_link": "get_share_link",
        "add_resources": "add_resources",
        "crud/read": "crud/read",
        "crud/update": "crud/update",
        "crud/delete": "crud/delete",
        "share_folder": "share_folder"
      }
    },
    "node_template": {
      "capabilities": {
        "get_share_link": "get_share_link",
        "add_resources": "add_resources",
        "crud/read": "crud/read",
        "share_folder": "share_folder"
      }
    }
  },
  "iss": "did:key:z6Mk...owner_pub_key"
}
```

**Key Points**:
- Templates defined in frontend: `sthalam/frontend/desktop/src/config/permissions.ts`
- `owner_template`: Full permissions (owner and node roles)
- `node_template`: Subset of permissions (for sovereign node)
- Backend extracts appropriate template based on `recipient_role` parameter

#### The add_resources Capability

**Purpose**: Enables nodes to share resources from a folder with viewers/other nodes

**Why It's Critical**:

In the Osvauld architecture, the sovereign node acts as an intermediary between the owner and viewers:
```
Owner → Node → Viewers
```

When a viewer requests access to a resource:
1. Viewer connects to node (not owner directly)
2. Node needs to delegate resource access to viewer
3. Node must prove it has authority to add/share resources from that folder
4. The `add_resources` capability on the node's folder UCAN provides this proof

**Without add_resources**:
- Node could receive resources from owner
- Node could NOT delegate/share those resources to viewers
- Each viewer would need direct delegation from owner (defeats purpose of node)

**With add_resources**:
- Owner delegates folder to node with `add_resources` capability
- Node can create delegated resource UCANs for viewers
- Node proves authority by presenting folder UCAN during resource delegation
- Viewers receive valid resource UCANs chained to owner's root authority

#### Template-Based Delegation (Current Implementation)

**Status**: ✅ Implemented (2025-11-09)

**Architecture**: Frontend-driven template pattern with role-based delegation

**Files Modified**:
- Frontend config: `sthalam/frontend/desktop/src/config/permissions.ts`
- Frontend state: `sthalam/frontend/desktop/src/state/data.svelte.ts`
- Frontend UI: `sthalam/frontend/desktop/src/components/PublishWebsiteModal.svelte`
- UCAN generation: `crypto_utils/src/ucan_utils.rs`
- UCAN wrapper: `crypto_utils/src/crypto_utils.rs`
- Service layer: `services/src/folder_service.rs`
- Handler types: `tauri_handlers/src/types/common.rs`
- Handler functions: `tauri_handlers/src/handlers/folder.rs`
- Errors: `services/src/errors.rs`

**Node's Folder UCAN Token** (Received from Owner):
```json
{
  "aud": "S94M6UPEdc8z7BuY3eefidBW2iLo+nXNMH4Dr7KU6ps=",
  "cap": {
    "sthalam:folder:f80eb3c2-a2a4-4a2e-836f-22fbc75ffd24": {
      "add_resources": [{}],
      "crud/read": [{}],
      "get_share_link": [{}],
      "share_folder": [{}]
    },
    "sthalam:resource:f80eb3c2-a2a4-4a2e-836f-22fbc75ffd24/*": {
      "add_resources": [{}],
      "crud/read": [{}],
      "get_share_link": [{}],
      "share_folder": [{}]
    }
  },
  "fct": {
    "role": "node"
  },
  "iss": "did:key:z6Mk...owner_pub_key"
}
```

**Key Points**:
- Capabilities extracted from `node_template` in owner's folder UCAN
- Includes both folder-level and wildcard resource-level capabilities
- `role: "node"` indicates this is for a sovereign node
- No templates in delegated token (templates only in root owner UCAN)

**Node's Resource UCAN Token** (Delegated from Owner):
```json
{
  "aud": "did:key:z6Mk...node_pub_key",
  "cap": {
    "sthalam:resource:0831ecf7-5eb3-4e3c-854c-542a2bab7dff:collaborative_doc": {"crud/merge": [{}]},
    "sthalam:resource:0831ecf7-5eb3-4e3c-854c-542a2bab7dff:content_doc": {"crud/merge": [{}]},
    "sthalam:resource:0831ecf7-5eb3-4e3c-854c-542a2bab7dff:static_assets": {"crud/merge": [{}]},
    "sthalam:resource:0831ecf7-5eb3-4e3c-854c-542a2bab7dff:submissions_doc": {"crud/merge": [{}]},
    "sthalam:resource:0831ecf7-5eb3-4e3c-854c-542a2bab7dff:template_doc": {"crud/merge": [{}]},
    "sthalam:resource:0831ecf7-5eb3-4e3c-854c-542a2bab7dff:user_content_doc": {"crud/merge": [{}]}
  },
  "fct": {
    "role": "node",
    "docs": ["collaborative_doc", "content_doc", "static_assets", "submissions_doc", "template_doc", "user_content_doc"],
    "doc_types": {
      "collaborative_doc": "crdt",
      "content_doc": "crdt",
      "static_assets": "asset",
      "submissions_doc": "crdt",
      "template_doc": "crdt",
      "user_content_doc": "crdt"
    },
    "owner_template": {
      "capabilities": {
        "collaborative_doc": "crud/merge",
        "content_doc": "crud/merge",
        "static_assets": "crud/merge",
        "submissions_doc": "crud/merge",
        "template_doc": "crud/merge",
        "user_content_doc": "crud/merge"
      },
      "doc_types": {
        "collaborative_doc": "crdt",
        "content_doc": "crdt",
        "static_assets": "asset",
        "submissions_doc": "crdt",
        "template_doc": "crdt",
        "user_content_doc": "crdt"
      }
    }
  },
  "prf": ["bafkr4ifu7ooufdhyqsprzbjaix4hlxzgy4c6nr5roy2hbfq4u5lacgy62q"],
  "iss": "did:key:z6Mk...owner_pub_key"
}
```

**Key Points**:
- Delegated from owner using `owner_template` (node gets full permissions for resources)
- Contains `owner_template` in facts for future viewer delegation
- `prf` (proof) field chains to owner's root UCAN
- Node can further delegate to viewers using `viewer_template`

**Implementation Flow**:

1. **Folder Creation** (`services/src/folder_service.rs:create_folder`):
   ```rust
   pub async fn create_folder(
       name: String,
       description: Option<String>,
       folder_template_json: String,  // From frontend
       // ... other params
   ) -> ServiceResult<Folder>
   ```
   - Frontend passes `FOLDER_TEMPLATE` from `permissions.ts`
   - Backend calls `generate_folder_ucan_with_template()`
   - Templates embedded in owner's folder UCAN facts

2. **Folder Sharing** (`services/src/folder_service.rs:share_folder`):
   ```rust
   pub async fn share_folder(
       folder_id: &str,
       recipient_user_id: &str,
       recipient_role: &str,  // "owner" or "node"
       // ... other params
   ) -> ServiceResult<()>
   ```
   - Frontend passes `recipientRole: "node"`
   - Backend extracts template from owner's folder UCAN based on role
   - Delegates with capabilities from extracted template
   - Also shares all resources in folder with same role

3. **Template Extraction** (from folder UCAN):
   ```rust
   let template_key = match recipient_role {
       "owner" => "owner_template",
       "node" => "node_template",
       _ => return Err(InvalidRole),
   };

   let folder_ucan_parsed = validate_structure(&folder.ucan).await?;
   let facts = folder_ucan_parsed.facts().ok_or(...)?.clone();
   let template = facts.get(template_key).ok_or(...)?;
   let capabilities_map = template.get("capabilities")
       .and_then(|c| c.as_object())
       .ok_or(...)?;
   ```

**Resource Sync Validation** (Future):

When sending resources to node, owner includes their folder UCAN:
```rust
// In ResourceDataSync message
pub struct ResourceDataSync {
    pub resource: EncryptedResource,
    pub share_record: ShareRecord,
    pub owner_folder_ucan: String,  // Proves owner has add_resources
}
```

Node validates:
1. Parse `owner_folder_ucan`
2. Verify it contains `add_resources` capability for this folder
3. Accept resource only if capability is present
4. This proves sender has authority to add resources to this folder

**Architecture Benefits**:
- Clear separation: handlers define business logic, services execute
- Flexible: different contexts can grant different capabilities
- Secure: UCAN chain validates authority at each delegation step
- Scalable: nodes can serve many viewers without owner involvement

### Frontend Integration

**Modal**: `PublishWebsiteModal.svelte`

**Trigger** (line 132):
```typescript
sendMessage("shareFolder", {
    folderId,
    userId,
    permissions
})
```

**Handler**: `handle_share_folder()` (tauri_handlers/src/handlers/folder.rs:85)

**Response**: `BaseCryptoResponse::Success`

---

## Sync Protocol & Algorithms

### Simple Folder Sync Protocol

**Status**: ✅ Implemented (2025-11-08)
**Implementation**: Phase 2.5 - Owner → Node folder/resource push

#### Overview

The Simple Folder Sync Protocol is a unidirectional push-based sync from owner to node that occurs immediately after folder sharing. This is a "fire-and-forget" pattern where the owner sends the complete folder and all resources to the node without waiting for acknowledgment or implementing conflict resolution.

**Design Philosophy**:
- **Simplicity First**: No state vectors, no bidirectional sync, no conflict resolution
- **Immediate Delivery**: Sync triggered automatically after ACL creation
- **Partial Success Model**: Errors don't stop sync of other resources
- **Future-Proof**: Foundation for bidirectional sync in Phase 3

**Key Files**:
- `network/src/p2p/sync_handler.rs` - Entry point and orchestration
- `network/src/p2p/folder_sync.rs` - Folder-level sync logic
- `network/src/p2p/resource_sync.rs` - Resource-level sync logic
- `services/src/resource_service.rs` - prepare_resource_for_peer()

#### Protocol Flow

**Trigger**: User shares folder via `handle_share_folder()` in Tauri handler

**1. Handler Initiates Sync** (`tauri_handlers/src/handlers/folder.rs:108`)
   - After `share_folder()` creates ACLs and share records
   - Calls `sync_handler::send_folder()` with folder_id, recipient_user_id
   - Returns success immediately (doesn't wait for sync)

**2. Sync Handler Orchestration** (`network/src/p2p/sync_handler.rs`)
   - **Fire-and-forget**: Spawns async task (line 29)
   - Get recipient's devices from database (line 57)
   - Get or establish P2P connection:
     - Try `get_connection_by_id(device.id)` first (line 70)
     - If no connection: call `connect_with_ticket(device.id)` (line 79)
     - Wait for handshake completion before proceeding
   - Delegate to `folder_sync::send_folder_with_resources()` (line 82)

**3. Folder-Level Sync** (`network/src/p2p/folder_sync.rs`)
   - Get owner's folder to extract UCAN (line 27):
     - Call `get_folder_by_id()` service
     - Extract `owner_folder_ucan` from `folder.ucan` field
     - This is the owner's folder UCAN containing `add_resources` capability
   - Send folder metadata and share record (line 58):
     - Get folder via `get_folder_by_id()` service
     - Get folder share record via `get_folder_share_record()` service
     - **Update folder.ucan** with recipient's token (line 90):
       - Replace `folder.ucan` with `folder_share_record.ucan_token`
       - Node receives folder with THEIR token, not owner's
     - Send `Message::FolderDataSync` with updated folder
   - Send all resources (line 45):
     - Delegate to `resource_sync::send_all_resources_for_folder()`
     - Pass `owner_folder_ucan` as proof of add_resources permission

**4. Resource-Level Sync** (`network/src/p2p/resource_sync.rs`)
   - **Bulk operation** optimized for multiple resources:
     - Get all resources for folder (single query, line 32)
     - Get all share records for folder (single query, line 52)
     - Get recipient user for public key (line 72)
   - **Loop through each resource** (line 85):
     - Match resource with share record
     - Call `prepare_resource_for_peer()` (line 102)
     - Send `Message::ResourceDataSync` (line 131)
     - **Partial success**: Errors logged, continues with other resources

#### Resource Preparation Algorithm

**Function**: `prepare_resource_for_peer()` (`services/src/resource_service.rs:647`)

**Purpose**: Decrypt resource with owner's key, filter documents based on UCAN permissions, re-encrypt for peer with peer's key

**Algorithm**:

1. **Fetch Encrypted Resource** (line 658)
   - Get `EncryptedResource` from database by resource_id
   - Contains: encrypted_data, encrypted_key, ucan_token, metadata

2. **Decrypt with Owner's Key** (line 668)
   - Call `crypto_utils.decrypt_resource(encrypted_data, encrypted_key)`
   - Uses owner's PGP private key to decrypt AES key
   - Uses AES key to decrypt data
   - Returns JSON string with Loro document snapshots

3. **Parse to Resource Object** (line 682)
   - Call `Resource::from_decrypted_data()`
   - Parses JSON to HashMap<String, Vec<Value>>
   - Loads 6 Loro documents: template_doc, content_doc, user_content_doc, collaborative_doc, submissions_doc, static_assets
   - Returns in-memory Resource with loaded docs

4. **Filter Documents by UCAN** (line 693)
   - **Critical step**: `resource.filter_to_send(owner_ucan, peer_ucan)`
   - Location: `core/src/models/resource.rs:169`
   - Compare capabilities in owner UCAN vs peer UCAN
   - **Algorithm**:
     - Parse both UCANs to extract capabilities
     - For each document in owner's resource:
       - Check if peer has capability for this doc
       - If yes: include doc in filtered result
       - If no: exclude doc (peer won't receive it)
   - Returns filtered HashMap with only permitted documents

5. **Serialize Filtered Data** (line 703)
   - Convert filtered HashMap to JSON string
   - Only includes documents peer has access to

6. **Re-encrypt for Peer** (line 710)
   - **Critical**: Use `recipient.public_key` (PGP), NOT `ucan_pub_key`!
   - Call `encrypt_data_for_user(filtered_json, peer_public_key)`
   - Generates NEW random AES-256 key (not shared with owner's)
   - Encrypts filtered data with AES-256-GCM
   - Encrypts AES key with peer's PGP public key
   - Returns: (new_encrypted_data, new_encrypted_key)

7. **Create Peer's EncryptedResource** (line 717)
   - Call `original_encrypted.re_encrypt_for_recipient()`
   - Returns new EncryptedResource with:
     - Same id, folder_id, metadata (unencrypted)
     - Peer's encrypted_data (filtered and re-encrypted)
     - Peer's encrypted_key (new AES key, encrypted for peer)
     - Peer's UCAN token (from share record)

**Key Insight**: Each peer gets their own encrypted copy with a unique AES key, containing only the documents they have permission to access.

#### Connection Management

**Pattern**: Connection indexed by `device.id` for consistency

**Get or Establish Connection**:
```
1. Try: get_connection_by_id(device.id)
   - Returns existing PeerConnection if active

2. If no connection:
   - Call: connect_with_ticket(device.id)
   - Derives NodeId from device.id (public key)
   - Establishes Iroh P2P connection
   - Performs handshake (validates UCANs, exchanges tokens)
   - Waits for handshake completion
   - Returns ready-to-use PeerConnection

3. Use connection for sync
```

**Important**: `connect_with_ticket()` is BLOCKING - waits for handshake to complete before returning. This ensures the connection is fully ready before any messages are sent.

#### Message Types

**FolderDataSync** (`Message::FolderDataSync`):
- **Purpose**: Send folder metadata and share record
- **Payload**:
  - `folder`: Folder object (id, name, description, timestamps)
  - `folder_share_record`: FolderShareRecord (UCAN token, permissions)
- **Handler**: `folder_sync::handle_folder_data_sync()` (node side)
- **Action**: Save folder and share record in single transaction

**ResourceDataSync** (`Message::ResourceDataSync`):
- **Purpose**: Send encrypted resource and share record
- **Payload**:
  - `resource`: EncryptedResource (re-encrypted for peer, filtered docs)
  - `share_record`: ShareRecord (peer's UCAN token, resource permissions)
- **Handler**: `resource_sync::handle_resource_data_sync()` (node side)
- **Action** (TODO): Validate UCAN, save resource and share record in transaction

#### Error Handling & Partial Success

**Fire-and-Forget Pattern**:
- Sync runs in spawned async task
- Errors logged, don't propagate to handler
- User gets immediate success response

**Partial Success Model**:
- Resource loop continues even if individual resources fail
- Scenarios:
  - Share record missing: Log error, skip resource, continue
  - Decryption fails: Log error, skip resource, continue
  - Encryption fails: Log error, skip resource, continue
  - Send fails: Log error, skip resource, continue
- **Result**: Some resources may sync successfully while others fail

**Philosophy**: Better to deliver partial folder than fail entire operation

#### Security & Encryption

**Two-Layer Encryption Model**:

1. **Owner's Encryption** (original storage):
   - Resource encrypted with random AES-256 key
   - AES key encrypted with owner's PGP public key
   - Only owner can decrypt

2. **Peer's Encryption** (re-encrypted for sync):
   - NEW random AES-256 key generated
   - Filtered documents encrypted with new AES key
   - AES key encrypted with peer's PGP public key
   - Only peer can decrypt their copy

**Key Separation**:
- `User.public_key`: PGP/GPG public key for encryption/decryption
- `User.ucan_pub_key`: EdDSA public key for UCAN signing/verification
- **Critical Bug**: Using wrong key causes "Failed to parse certificate: unexpected EOF"

**UCAN-Based Filtering**:
- Owner UCAN contains all document capabilities
- Peer UCAN contains subset based on role (owner_template vs viewer_template)
- `filter_to_send()` ensures peer only receives permitted documents
- Examples:
  - Owner might have all 6 docs
  - Node might have 5 docs (all except user_content)
  - Viewer might have 3 docs (template, content, static_assets)

#### Performance Optimizations

**Bulk Operations**:
- Single query for all resources in folder (not N queries)
- Single query for all share records (not N queries)
- Avoids N+1 problem

**Sequential Resource Send**:
- Resources sent one at a time (not all in parallel)
- Prevents memory explosion with large resources
- Allows partial success if some fail

**Async Background Processing**:
- Sync doesn't block handler response
- User can continue working immediately
- No UI freeze during large syncs

#### Current Limitations & TODOs

**Node Side Reception** (Partial Implementation):
- ✅ Folder reception works: `handle_folder_data_sync()` saves folder
- ✅ Resource reception works: `handle_resource_data_sync()` validates and saves resources
- ✅ UCAN validation implemented: `validate_peer_can_add_resources()` checks folder UCAN
- ✅ Transactional save: `save_resource_with_share_records()` in ResourceRepository

**Bidirectional Sync**:
- ✅ Owner → node push (Phase 2.5)
- ✅ Peer → peer CRDT merge sync (Phase 3)
- ⏳ Node → owner updates relay (Phase 3)
- ⏳ Viewer ↔ node sync (Phase 3)

**Conflict Resolution**:
- ✅ CRDT merge protocol with state vectors
- ✅ Incremental updates based on version vectors
- ✅ Bidirectional update application

**Incremental Updates**:
- ✅ State vector sync for incremental updates
- ✅ Loro state vector diffing
- ✅ Updates-based protocol

### Resource Sync Protocol (CRDT Merge)

**Status**: ✅ Implemented (2025-11-09)
**Implementation**: Phase 3 - Bidirectional CRDT sync between peers

#### Overview

The Resource Sync Protocol enables bidirectional CRDT merge synchronization between peers (owner ↔ node, owner ↔ viewer, node ↔ viewer). This protocol uses Loro state vectors for incremental updates and supports concurrent editing with automatic conflict resolution.

**Design Philosophy**:
- **State Vector Based**: Only send operations peer doesn't have
- **Bidirectional**: Both peers send and receive updates
- **UCAN Filtered**: Sync only documents peer has permission for
- **Incremental**: Minimal data transfer using Loro version vectors
- **Fire-and-forget**: Initiated via Tauri handler, runs asynchronously

**Key Files**:
- `network/src/p2p/resource_sync.rs` - All resource sync handlers
- `services/src/resource_service.rs` - UCAN-filtered sync business logic
- `tauri_handlers/src/handlers/resource.rs` - Tauri command handler
- `network/src/p2p/sync_handler.rs` - High-level sync orchestration

#### Protocol Flow

**Trigger**: User calls `syncResource` action from frontend

**1. Tauri Handler Initiates Sync** (`tauri_handlers/src/handlers/resource.rs:242`)
   - Frontend: `sendMessage("syncResource", { resourceId: "abc123" })`
   - Handler: `handle_sync_resource()` receives `SyncResourceInput`
   - Spawns async task (fire-and-forget pattern)
   - Calls `network::p2p::sync_handler::sync_resource()`

**2. Sync Handler Orchestration** (`network/src/p2p/sync_handler.rs:125`)
   - Get current user from P2PService
   - Get all share records for resource (find users with access)
   - Get UCANs from service layer:
     - Calls `services::get_resource_ucans_for_sync()`
     - Returns (resource_ucan, folder_ucan)
   - Create `ResourceSyncRequestMsg` with UCANs
   - For each user with access:
     - Skip self (don't sync with ourselves)
     - Get recipient's devices
     - Get or establish P2P connection
     - Send `Message::ResourceSyncRequest`

**3. Peer Receives Sync Request** (`network/src/p2p/resource_sync.rs:400`)
   - Handler: `handle_resource_sync_request()`
   - Validate UCANs (resource_ucan and folder_ucan)
   - Check if resource exists locally:
     - **Resource NOT found**: Send `ResourceNotFoundRequest` (full resource transfer flow)
     - **Resource found**: Proceed with CRDT merge flow

**4. CRDT Merge Flow - Get State Vectors** (resource_sync.rs:459)
   - Call `services::get_resource_state_vectors_by_ucan()`
     - Loads resource from database
     - Imports Loro documents from snapshots
     - Gets state vectors for each document
     - **Filters by UCAN**: Only include docs peer has permission for
   - Extract asset IDs (placeholder for now)
   - Create `ResourceUpdateMsg::StateVectorRequest`:
     - resource_id
     - state_vectors (JSON with filtered docs)
     - asset_ids
     - ucan_token (proves permissions)
   - Send `Message::MergeUpdate(StateVectorRequest)` back to initiator

**5. Initiator Receives State Vectors** (`network/src/p2p/resource_sync.rs:555`)
   - Handler: `handle_state_vector_request()`
   - Parse peer's state vectors
   - Call `services::generate_updates_for_peer()`:
     - Loads our resource
     - Compares our state vs peer's state vectors
     - Generates incremental Loro updates
     - **Filters by UCAN**: Only send updates for permitted docs
     - Returns updates + our current state vectors
   - Extract our state vectors from updates
   - Create `ResourceUpdateMsg::UpdatesResponse`:
     - resource_id
     - updates (incremental Loro operations)
     - state_vectors (our current state)
     - missing_asset_ids
     - ucan_token
   - Send `Message::MergeUpdate(UpdatesResponse)` back to peer

**6. Peer Applies Updates** (`network/src/p2p/resource_sync.rs:632`)
   - Handler: `handle_updates_response()`
   - Parse peer's updates and state vectors
   - Call `services::apply_peer_updates()`:
     - Loads our resource
     - **Validates UCAN permissions**: Only accept updates for permitted docs
     - Applies peer's updates using Loro's CRDT merge
     - Generates our updates back based on peer's current state
     - **Bidirectional sync**: Both sides converge to same state
     - Saves merged resource to database
   - Returns our updates (if any)
   - **Note**: Current implementation doesn't send our updates back (TODO)

#### Service Layer Functions

**get_resource_ucans_for_sync()** (`services/src/resource_service.rs:763`)
- **Purpose**: Get both resource and folder UCANs needed for sync
- **Algorithm**:
  1. Get resource to find folder_id
  2. Find share record with operation="share" for this user
  3. Find folder share record for user
  4. Return (resource_ucan, folder_ucan)
- **Returns**: `(String, String)` - Tuple of UCAN tokens

**get_resource_state_vectors_by_ucan()** (`services/src/resource_service.rs:463`)
- **Purpose**: Get state vectors filtered by UCAN permissions
- **Algorithm**:
  1. Load encrypted resource from database
  2. Decrypt using owner's UCAN private key
  3. Parse to Resource object (loads Loro documents)
  4. Extract doc capabilities from UCAN token
  5. Filter: only get state vectors for docs in UCAN
  6. For each permitted doc:
     - Call `document::state_frontiers()`
     - Convert to JSON array
  7. Return JSON: `{"doc_name": {"state_vector": [bytes]}, ...}`
- **UCAN Filtering**: Ensures we only expose state for permitted docs

**generate_updates_for_peer()** (`services/src/resource_service.rs:519`)
- **Purpose**: Generate incremental updates based on peer's state
- **Algorithm**:
  1. Load our resource
  2. Parse peer's state vectors from JSON
  3. Extract our doc capabilities from UCAN
  4. For each doc we have permission to send:
     - Skip if peer doesn't have capability
     - Parse peer's state vector
     - Call `document::export_updates(doc, &peer_state_vector)`
     - Get our current state vector
     - Add to updates JSON
  5. Return JSON with updates and current state per doc
- **Incremental**: Only sends operations peer doesn't have

**apply_peer_updates()** (`services/src/resource_service.rs:582`)
- **Purpose**: Apply peer's updates and generate our updates back
- **Algorithm**:
  1. Parse peer's UCAN to get their permissions
  2. Load our resource
  3. Parse peer's updates from JSON
  4. For each doc in peer's updates:
     - **Validate**: Check peer has capability for this doc
     - **Reject** if peer only has `crud/readonly`
     - Parse update bytes
     - Get or create LoroDoc
     - Call `document::apply_updates(doc, &update_bytes)`
  5. Parse peer's current state vectors
  6. Generate our updates back:
     - For each doc we can send:
       - Parse peer's state vector
       - Call `document::export_updates(doc, &peer_state_vector)`
       - Add to response
  7. Save merged resource to database
  8. Return our updates JSON
- **Bidirectional**: Merges peer's updates AND generates our updates
- **Permission Check**: Validates every document update against UCAN

**get_all_share_records_for_resource()** (`services/src/share_service.rs`)
- **Purpose**: Get all users who have access to a resource
- **Returns**: `Vec<ShareRecord>` - All share records for resource
- **Usage**: Sync handler uses this to find all peers to sync with

#### Message Types

**ResourceSyncRequest** (`Message::ResourceSyncRequest`):
```rust
pub struct ResourceSyncRequestMsg {
    pub resource_ucan: String,  // Proves access to resource
    pub folder_ucan: String,    // Proves folder membership
}
```
- **Purpose**: Initiate sync for a resource
- **Sent by**: Peer who wants to sync
- **Response**: StateVectorRequest OR ResourceNotFoundRequest

**MergeUpdate::StateVectorRequest**:
```rust
StateVectorRequest {
    resource_id: String,
    state_vectors: String,     // JSON with state vectors per doc
    asset_ids: Vec<String>,    // Asset identifiers
    ucan_token: String,        // Proves permissions
}
```
- **Purpose**: "Here are my state vectors, send me your updates"
- **Sent by**: Peer who has the resource
- **Response**: UpdatesResponse

**MergeUpdate::UpdatesResponse**:
```rust
UpdatesResponse {
    resource_id: String,
    updates: String,                    // JSON with Loro updates per doc
    state_vectors: String,              // JSON with our current state
    missing_asset_ids: Vec<String>,     // Assets we need
    ucan_token: String,                 // Proves permissions
}
```
- **Purpose**: "Here are updates you're missing + my current state"
- **Sent by**: Peer responding to StateVectorRequest
- **Response**: None (sync complete) or UpdatesResponse (if bidirectional)

#### State Vector Format

**JSON Structure**:
```json
{
  "template_doc": {
    "state_vector": [1, 229, 249, 220, 160, ...]
  },
  "content_doc": {
    "state_vector": [1, 152, 156, 175, 191, ...]
  },
  "collaborative_doc": {
    "state_vector": [0]
  }
}
```

**Interpretation**:
- Empty `[0]`: No operations yet (new document)
- Non-empty: Loro's compact version vector encoding
- **Filtered by UCAN**: Only includes docs peer has permission for

#### Updates Format

**JSON Structure**:
```json
{
  "content_doc": {
    "state_vector": [1, 152, 156, 175, 191, ...],
    "updates": [108, 111, 114, 111, 0, 0, ...]
  },
  "template_doc": {
    "state_vector": [1, 229, 249, 220, 160, ...],
    "updates": [108, 111, 114, 111, 0, 0, ...]
  }
}
```

**Fields**:
- `state_vector`: Sender's current state after applying these updates
- `updates`: Loro binary updates (operations from recipient's state to sender's state)

#### UCAN Permission Validation

**During State Vector Exchange**:
- Sender filters state vectors to only include permitted docs
- Recipient can only see state for docs they have access to

**During Update Application**:
- `apply_peer_updates()` validates EVERY document:
  - Parse peer's UCAN to get capabilities
  - For each doc in updates:
    - Check if peer has capability for this doc
    - Check ability is not `crud/readonly`
    - Reject update if validation fails
- **Security**: Prevents malicious peers from updating restricted docs

**Capability Checks**:
```rust
// Extract peer's capabilities
let peer_capabilities = crypto_utils::ucan_utils::extract_doc_capabilities(&peer_ucan)?;

// For each doc in updates
for (doc_name, _) in &updates_map {
    // Check peer has capability
    let capability = peer_capabilities.get(doc_name)
        .ok_or("No capability for this doc")?;

    // Reject readonly
    if capability == "crud/readonly" {
        return Err("Cannot update readonly doc");
    }

    // Accept crud/merge or crud/appendonly
}
```

#### Connection Management

**Pattern**: Same as folder sync - get or establish connection

**Get or Establish Connection**:
1. Try: `get_connection_by_id(device.id)`
2. If no connection: `connect_with_ticket(device.id)`
3. Wait for handshake completion
4. Use connection for sync

#### Error Handling

**Fire-and-Forget Pattern**:
- Sync runs in spawned task
- Errors logged, don't propagate to handler
- User gets immediate success response

**Partial Success Model**:
- If one peer sync fails, continue with other peers
- Each peer sync is independent
- Philosophy: Better to sync with some peers than fail entirely

**Permission Errors**:
- UCAN validation failures logged and rejected
- Peer continues with next document
- Malformed updates logged and skipped

#### Security Guarantees

**UCAN-Based Access Control**:
- Only sync docs present in peer's UCAN capabilities
- Validate permissions on every update application
- Reject readonly document updates

**Encryption**:
- Resources remain encrypted in database
- Decrypted only in memory during sync
- Updates applied to decrypted Loro documents
- Re-encrypted before saving

**State Vector Safety**:
- State vectors don't leak document content
- Only reveal operation count and version
- Safe to share with peers

#### Performance Optimizations

**Incremental Updates**:
- State vectors enable minimal data transfer
- Only send operations peer doesn't have
- Loro's efficient binary encoding

**Batch Processing**:
- Single query to get all share records
- Single resource load for all peers
- Amortizes database access

**Async Background**:
- Doesn't block UI
- User can continue working
- Fire-and-forget pattern

#### Current Limitations

**Bidirectional Completion**:
- ✅ Peer applies our updates
- ⏳ Peer's response updates not sent back (TODO)
- Workaround: Each peer initiates sync separately

**Asset Sync**:
- ⏳ Asset transfer not implemented
- `missing_asset_ids` placeholder
- Future: Separate asset transfer protocol

**Conflict Visualization**:
- ⏳ No UI notification of merge
- ⏳ No conflict visualization
- Loro handles conflicts automatically (CRDT)

### Message Types

**Status**: ✅ Phase 3 Complete - CRDT merge messages implemented (see above)

**File**: `core/src/models/p2p.rs`

**Enum**: `Message`

**Sync Messages**:
- `ResourceSyncRequest` - Initiate resource sync with UCANs
- `MergeUpdate` - Envelope for CRDT merge protocol

**Enum**: `ResourceUpdateMsg` (inside MergeUpdate)

**Variants**:
- `StateVectorRequest` - Send state vectors, request updates
- `UpdatesResponse` - Send incremental updates + current state
- `ResourceNotFoundRequest` - Request full resource (not implemented)
- `ResourceTransfer` - Send complete resource (not implemented)
- `ResourceTransferAck` - Acknowledge receipt (not implemented)

### Sync Behavior Logic

**Status**: TODO - To be documented during Phase 3

**readonly**:
- Algorithm for rejecting viewer writes: TBD

**merge**:
- Algorithm for bidirectional sync: TBD

**appendonly**:
- Algorithm for append-only operations: TBD

---

## Search Indexing

### Configuration Format

**Status**: TODO - To be documented during Phase 4

**In Resource.metadata**:
```json
{
  "title": "Unencrypted Title",
  "search": {
    "docs": ["content", "comments"]
  }
}
```

### Text Extraction Algorithm

**Status**: TODO - To be documented during Phase 4

**File**: `search_indexer/src/extractor.rs`

**Function**: `extract_content()`

**Algorithm**:
1. Parse metadata.search.docs to know which docs to index
2. Extract title from metadata field (unencrypted)
3. For each doc in search config, get LoroDoc from resource
4. Traverse Loro structure based on doc type (LoroText/LoroMap/LoroList)
5. Extract all text content
6. Concatenate and return title + content

**Details**: TBD during implementation

### Loro Text Traversal

**Status**: TODO - To be documented during Phase 4

How to extract text from:
- LoroText: TBD
- LoroMap: TBD
- LoroList: TBD

---

## Code Patterns

### UCAN Service Architecture

**Status**: ✅ Implemented (Phase 3 - November 2025)
**Last Updated**: 2025-11-10

**Purpose**: Centralized UCAN token validation, issuance, and key management to eliminate code duplication across services.

**File**: `services/src/ucan_service.rs`

#### Architecture Layers

```
┌─────────────────────────────────────────┐
│   Domain Services                        │
│   (auth, folder, resource, user)         │
│   - Domain-specific business logic       │
│   - Call ucan_service for common ops    │
└─────────────────┬───────────────────────┘
                  │
┌─────────────────▼───────────────────────┐
│   ucan_service.rs (NEW)                  │
│   ├─ Validation: Business rules         │
│   ├─ Issuance: Token generation         │
│   └─ Helpers: Key management            │
└─────────────────┬───────────────────────┘
                  │
┌─────────────────▼───────────────────────┐
│   crypto_utils (ucan_utils.rs)           │
│   - Cryptographic primitives             │
│   - Low-level UCAN operations            │
└──────────────────────────────────────────┘
```

#### Public Functions

**Validation Functions** (Business-level validation):
- `validate_peer_can_add_folder(token, domain)` - Validates peer has add_folder capability
- `validate_peer_can_add_resources(folder_ucan, folder_id, domain)` - Validates folder access with capability check
- `validate_folder_access_for_resource(folder_ucan, resource_folder_id, domain)` - Validates folder ownership for resource requests

**Issuance Functions** (Token generation with boilerplate elimination):
- `issue_one_time_connection_token(capability_str, role, crypto_utils, repo_ctx)` - For QR codes/connection strings
- `issue_peer_connection_token(domain, peer_pub_key, role, crypto_utils, repo_ctx)` - For P2P connections with role-based capabilities
- `issue_folder_owner_token(folder_id, domain, template_json, crypto_utils, repo_ctx)` - For folder creation
- `issue_delegated_folder_token(folder_id, domain, capabilities, recipient_pub_key, role, crypto_utils, repo_ctx)` - For folder sharing
- `issue_resource_owner_token(resource_id, domain, template_json, crypto_utils, repo_ctx)` - For resource creation
- `get_ucan_public_key(crypto_utils, repo_ctx)` - Get UCAN public key

**Helper Functions** (Internal):
- `get_decrypted_ucan_keys(repo_ctx, crypto_utils)` - Private helper that eliminates repeated "get key → decrypt" pattern

#### Migration Impact

**Before**: 8 instances of duplicated boilerplate across services:
```rust
// Repeated pattern in auth_service, user_service, folder_service, resource_service
let encrypted_key = repo_ctx.store_repo.get_ucan_key().await?;
let crypto = crypto_utils.read().await;
crypto.some_ucan_method(&encrypted_key, ...).await?;
```

**After**: Single centralized call:
```rust
// All services now use
crate::ucan_service::issue_*_token(...).await?;
```

**Code Reduction**: ~50+ lines of duplicated code eliminated

#### Usage Examples

**Before (auth_service.rs)**:
```rust
let encrypted_ucan_key = repo_ctx.store_repo.get_ucan_key().await?;
let crypto = crypto_utils.read().await;
crypto.generate_one_time_user_connect_token(&encrypted_ucan_key, capability_str, role).await?;
```

**After**:
```rust
crate::ucan_service::issue_one_time_connection_token(capability_str, role, crypto_utils, &repo_ctx).await?;
```

**Before (folder_service.rs)**:
```rust
let encrypted_key = repo_ctx.store_repo.get_ucan_key().await?;
let (signing_key, verifying_key) = {
    let crypto = crypto_utils.read().await;
    crypto.decrypt_ucan_key(&encrypted_key)?
};
crypto_utils::ucan_utils::generate_flexible_folder_token(&signing_key, &verifying_key, ...).await?;
```

**After**:
```rust
crate::ucan_service::issue_delegated_folder_token(folder_id, domain, capabilities, recipient_pub_key, role, crypto_utils, &repo_ctx).await?;
```

#### Services Refactored

1. **auth_service.rs**: `generate_one_time_ucan_token()` now uses `ucan_service`
2. **user_service.rs**: `issue_connect_ucan_token()` and `get_ucan_pub_key()` now use `ucan_service`
3. **folder_service.rs**: Folder creation and sharing now use `ucan_service`
4. **resource_service.rs**: Resource creation now uses `ucan_service`

#### Design Decisions

**Why Not Include Extraction Functions?**
- Extraction functions work with parsed `Ucan` objects from the `ucan` crate
- These are domain-specific and tightly coupled to crypto_utils
- Services call `crypto_utils::ucan_utils::extract_*()` directly when needed

**Why Keep Domain-Specific Logic in Services?**
- Folder template extraction based on recipient role (folder_service)
- Resource sync permission filtering (resource_service)
- Role-based capability mapping (user_service)
- These require domain knowledge that shouldn't be in ucan_service

**Future Extensions**:
- Add more validation patterns as needed (e.g., `validate_can_generate_shareable_link`)
- Add token generation for new features (e.g., public folder view tokens)
- Keep the service focused on common patterns, not domain-specific logic

---

### Service Layer Pattern

**Status**: ✅ Implemented (Phase 2)

**Files**:
- `services/src/resource_service.rs` - Encryption/decryption coordination (724 lines)
- `core/src/models/resource.rs` - Business logic methods (456 lines)

**Principle**: Service layer handles encryption boundary, Resource struct has business logic

**Flow**:
1. Service decrypts DB row, returns Resource + decrypted JSON
2. Resource parses JSON and loads Loro docs
3. Business logic works with Resource methods
4. Service encrypts modified Resource back to DB

**Separation**: Service = coordination & encryption, Resource = domain logic

### 14 Service Methods Implemented

**Helper Functions**:
1. **decrypt_resources()** - Unified batch decryption
   - Takes Vec<ResourceWithKey>
   - Decrypts AES keys with UCAN private key
   - Decrypts data with AES keys
   - Calls Resource::from_decrypted_data()
   - Returns Vec<Resource>

**CRUD Operations**:
2. **create_resource()** - Create with flexible UCAN
   - Accepts resource_payload (Loro snapshots JSON)
   - Accepts ucan_template_json (frontend-defined templates)
   - Accepts metadata_json (title, type, search config)
   - Generates flexible owner UCAN
   - Encrypts and stores in DB

3. **get_resource_by_id_direct()** - Fetch single resource
   - Loads from DB
   - Decrypts using decrypt_resources()
   - Returns single Resource

4. **update_resource()** - Update Loro snapshots
   - Accepts new resource_payload (complete new snapshots)
   - Reuses existing AES key
   - Re-encrypts with same key
   - Updates DB

5. **delete_resource()** - Soft delete
   - Marks resource as deleted
   - Preserves data for recovery

6. **get_resources_for_folder()** - List folder resources
   - Fetches all resources in folder
   - Batch decrypts with decrypt_resources()
   - Returns Vec<Resource>

7. **get_all_resources()** - List all user resources
   - Fetches all user's resources
   - Batch decrypts
   - Returns Vec<Resource>

**Sharing**:
8. **share_resource()** - Role-based sharing
   - Extracts recipient role from their user token
   - Decrypts resource with owner's key
   - Calls Resource::filter_to_send() with role-based UCAN
   - Generates delegated UCAN with template selection
   - Re-encrypts with NEW AES key for viewer
   - Stores share record in DB

9. **get_share_records_for_resource()** - List shares
   - Returns all share records for resource
   - Used for UI display of who has access

**UCAN Management**:
10. **get_resource_ucan_key()** - Get UCAN keypair
    - Retrieves and decrypts UCAN private key
    - Returns keypair for signing operations

11. **validate_authority_for_update()** - Permission validation
    - Checks if user can update resource
    - Validates UCAN chain

12. **resolve_proof()** - UCAN proof resolution
    - Resolves proof chain for delegation
    - Used by UCAN validation

**Internal Helpers** (13-14):
- Encryption/decryption utilities
- Key generation helpers

### Key Design Patterns

**Unified Decryption**:
- Single decrypt_resources() helper handles all decryption
- Takes Vec for batch processing
- Single resource case wraps in Vec, unwraps result
- Consistent error handling

**Frontend Control**:
- Frontend sends complete Loro snapshots (no merging in backend)
- Frontend defines UCAN templates (no hardcoded doc structure)
- Backend just encrypts/decrypts and enforces permissions

**Role-Based Template Selection**:
- share_resource() extracts role from recipient token
- Automatically selects owner_template or viewer_template
- No manual template selection needed

**Security Isolation**:
- Each share gets NEW AES key (never reuse owner's key)
- Viewer can't decrypt owner's original data
- Compromised viewer key doesn't expose owner's data

### Error Handling

**Status**: TODO - To be documented as patterns emerge

### State Management

**Status**: TODO - To be documented as patterns emerge

---

## Implementation Notes

### Phase 1: Core Protocol

**Status**: ✅ Complete (100%)

#### document.rs Loro Wrappers

**Status**: ✅ Complete

**File**: `core/src/models/document.rs`

**Implementation Details**:

**Functions Implemented**:
1. `create_doc()` - Creates new LoroDoc
2. `import_snapshot(bytes)` - Creates doc from snapshot (full or shallow)
3. `import_into(doc, bytes)` - Imports into existing doc
4. `export_snapshot(doc)` - Exports with full history (owner/node)
5. `export_shallow_snapshot(doc)` - Exports without history (viewers)
6. `export_updates(doc, from_version)` - Incremental updates for sync
7. `apply_updates(doc, updates)` - Apply incoming updates
8. `oplog_vv(doc)` - Version vector with full history (owner/node)
9. `state_frontiers(doc)` - Current state frontiers (viewers)

**Key Learnings**:

**Shallow Snapshots for Viewers**:
- Loro supports `ExportMode::shallow_snapshot(&frontiers)` like Git shallow clone
- Removes old history, keeps only current state
- Viewers don't need full operation history
- Limitation: Can only sync updates from after the shallow snapshot point

**Version Tracking**:
- `oplog_vv()` - For owner/node, tracks all recorded history
- `state_frontiers()` - For viewers, tracks current applied state
- Both encode to bytes for transmission

**Export Modes**:
- `ExportMode::Snapshot` - Full state + all history
- `ExportMode::shallow_snapshot(&frontiers)` - State without history
- `ExportMode::updates(&version_vector)` - Incremental from version

**Error Handling**: Uses `LoroError` directly from Loro crate

**Compilation**: ✅ Compiles successfully

#### Resource Model

**Status**: ✅ Complete

**File**: `core/src/models/resource.rs`

**Implementation Details**:

**Two-Struct Architecture**:
- EncryptedResource: Database/network representation with encrypted_data, encrypted_key
- Resource: Runtime representation with HashMap<String, LoroDoc>
- Clean separation between storage and business logic

**Key Learnings**:

**No Cached State Vectors**:
- Initially planned to cache state vectors in struct
- Decision: Compute on-demand from LoroDoc
- Rationale: LoroDoc.state_frontiers() is fast, caching adds complexity
- Benefit: Always accurate, no sync issues between cache and doc state

**ResourceType Moved to Metadata**:
- Originally had ResourceType enum in struct
- Moved to metadata field as unencrypted JSON
- Rationale: UCAN tokens define doc structure, not backend enums
- Benefit: Frontend controls resource types, backend is generic

**Two-UCAN Filtering**:
- filter_to_send() takes both our_ucan and peer_ucan
- Checks our dont_send_to_node rules
- Checks peer's capabilities
- Only sends intersection (what we can send AND what they can receive)

**Always Shallow Snapshots**:
- filter_to_send() uses export_shallow_snapshot()
- Peers don't need full operation history
- Reduces data transfer size
- Sufficient for collaboration

**Gotchas**:

**Viewer Filtering Direction**:
- apply_updates_filtered() filters incoming updates (what viewer accepts)
- filter_to_send() filters outgoing docs (what node sends)
- Two different filtering directions for same viewer restrictions

**Permission Validation**:
- apply_updates() rejects crud/readonly docs
- Must check ability string matches exactly "crud/readonly"
- Other abilities (crud/merge, crud/appendonly) currently treated same

**Metadata in Updates** (Bug Fixed 2025-11-07):
- `from_decrypted_data()` expects: `HashMap<String, Vec<Value>>` - every field must be an array
- Frontend must NOT include metadata fields (client_id, last_modified, title) in update payload
- Only send Loro document snapshots (template_doc, content_doc, etc.) as arrays
- Metadata fields would cause parser error: "expected a sequence, got string"
- Add flow correctly separates resourcePayload (documents) from metadataJson (metadata)
- Update flow must follow same pattern

#### UCAN Utils

**Status**: ✅ Complete

**File**: `crypto_utils/src/ucan_utils.rs` + `crypto_utils/src/crypto_utils.rs`

**Implementation Details**:

**UcanFacts Structure**:
- Two templates in one struct: owner_template + viewer_template
- Each template has capabilities + sync rules
- Enables single owner UCAN to define all access patterns

**Template Extraction**:
- extract_owner_template() and extract_viewer_template() are separate functions
- Both use shared helper: extract_template_from_facts()
- Optional fields default to empty arrays (no_update_from_node, dont_send_to_node)

**Flexible UCAN Generation**:
- Frontend sends complete UcanFacts as JSON
- Backend just serializes and adds to facts section
- No hardcoded doc names or capabilities in backend
- Enables arbitrary doc structures per resource

**Role-Based Delegation**:
- issue_flexible_delegated_resource_ucan() extracts UcanFacts from proof
- Selects template based on recipient_role string
- "owner" or "node" → owner_template
- "viewer" → viewer_template
- Automatic, no manual intervention

**Edge Cases**:

**Missing Templates**:
- If owner_template or viewer_template missing from facts, extraction fails
- Returns UcanError with descriptive message
- share_resource() propagates error to caller

**Invalid Role**:
- If recipient_role not "owner", "node", or "viewer", defaults to viewer_template
- Safe fallback: least privilege by default

**Empty Capabilities**:
- Templates can have empty capabilities HashMap (valid but useless)
- No validation at UCAN generation time
- Validation happens at permission check time (no capabilities = no access)

---

### Phase 2: Service Layer

**Status**: ✅ Complete (100%)

#### Resource Service Rewrite

**Status**: ✅ Complete

**File**: `services/src/resource_service.rs`

**Implementation Details**:

**Code Reduction**:
- Before: 1200+ lines, 34 methods
- After: 724 lines, 14 methods
- Reduction: 40% code reduction, 58% method reduction
- Removed: All sync methods (9), vector clocks (3), resource keys (2), UI helpers (6)

**Unified Decryption Pattern**:
- Single decrypt_resources() helper handles all decryption
- Takes Vec<ResourceWithKey> for batch processing
- Consistent error handling across all decrypt paths
- Reduces code duplication

**Frontend-Driven Updates**:
- update_resource() accepts complete new Loro snapshots
- No merging or diff computation in backend
- Frontend handles all CRDT operations
- Backend just encrypts and stores

**Role-Based Sharing Flow**:
```
1. Extract recipient role from their user token (via extract_facts)
2. Load and decrypt resource
3. Call issue_flexible_delegated_resource_ucan() with role
4. Automatic template selection (owner/node vs viewer)
5. Filter docs with Resource::filter_to_send()
6. Generate new AES key
7. Encrypt filtered data
8. Store share record
```

**Key Learnings**:

**Batch Decryption Optimization**:
- decrypt_resources() processes all resources in one batch
- Amortizes crypto_utils lock acquisition
- Better than per-resource decryption in loop

**AES Key Reuse**:
- Owner's resources: Same AES key across updates
- Viewer shares: Always new AES key (never reuse owner's)
- Rationale: Owner key compromise doesn't expose shares

**Template Selection Logic**:
- Based on recipient's role field in their user token
- Not based on share type or manual selection
- Automatic and deterministic

**Gotchas**:

**Share Record Schema**:
- Stores resource_id (shared resource)
- Stores shared_with_user_id (recipient)
- Stores encrypted_data (filtered + re-encrypted)
- Stores encrypted_key (recipient's encrypted AES key)
- Stores ucan_token (delegated UCAN)
- Same schema as resources table but different table

**Update Without UCAN**:
- update_resource() doesn't regenerate UCAN
- Reuses existing ucan_token and encrypted_ucan_key
- Only updates encrypted_data
- Assumption: UCAN templates don't change on update

**No Sync Methods**:
- Deliberately removed all network sync methods
- Deferred to Phase 3 (network protocol redesign)
- Phase 2 only handles local CRUD and sharing

#### Encryption Implementation

**Status**: ✅ Complete

**Implementation Details**:

**AES-256-GCM**:
- Used for data encryption
- Random IV per encryption operation
- Authenticated encryption (prevents tampering)

**Key Encryption**:
- AES keys encrypted with Ed25519 public keys (via X25519 conversion)
- UCAN public keys used for encryption
- UCAN private keys used for decryption

**Encryption Path**:
```
Resource.to_json() → JSON string → AES-256-GCM → Base64 → encrypted_data
Random AES key → Encrypt with UCAN pubkey → Base64 → encrypted_key
```

**Decryption Path**:
```
encrypted_key → Base64 decode → Decrypt with UCAN privkey → AES key
encrypted_data → Base64 decode → AES-256-GCM decrypt → JSON string → Resource::from_decrypted_data()
```

#### Key Generation

**Status**: ✅ Complete

**Implementation Details**:

**Resource Creation**:
- Generate random 32-byte AES key
- Encrypt with owner's UCAN public key
- Store encrypted_key in resources table

**Resource Sharing**:
- Generate NEW random 32-byte AES key (don't reuse owner's)
- Encrypt filtered data with new key
- Encrypt new key with viewer's UCAN public key
- Store encrypted_key in share_records table

**Key Isolation**:
- Owner and viewer have independent AES keys
- Viewer key compromise doesn't affect owner's data
- Each share has unique key

**Key Retrieval**:
- get_resource_ucan_key() decrypts UCAN private key
- Returns Ed25519 keypair for signing
- Used for UCAN delegation and signing operations

---

### Phase 3: Network Protocol

**Status**: Not Started

#### Message Handlers

**Implementation Details**: TBD

#### Sync Logic

**Implementation Details**: TBD

---

### Phase 4: Search Indexing

**Status**: Not Started

#### Text Extraction

**Implementation Details**: TBD

---

### Phase 5: Database Migration

**Status**: Not Started

#### Migration Script

**Implementation Details**: TBD

---

## Technical Decisions Log

### Decision: Service Layer = Encryption Boundary
**Date**: 2025-11-06
**Rationale**: Clean separation between encrypted storage and business logic
**Impact**: Resource struct always has decrypted data, service coordinates encryption

### Decision: UCAN Contains Sync Behavior
**Date**: 2025-11-06
**Rationale**: Self-describing tokens, no server-side config needed
**Impact**: Capabilities directly map to sync behavior (readonly/merge/appendonly)

### Decision: Metadata Field Unencrypted
**Date**: 2025-11-06
**Rationale**: Quick access to title and search config without decryption
**Impact**: Title, search config stored in plain JSON

### Decision: Two-Template Architecture
**Date**: 2025-11-07
**Rationale**: Single owner UCAN defines both owner and viewer permissions, enables automatic role-based delegation
**Impact**: Owner UCAN has owner_template and viewer_template in facts, delegation automatically selects correct template

### Decision: 40% Code Reduction in resource_service.rs
**Date**: 2025-11-07
**Rationale**: Remove all sync methods (defer to Phase 3), remove vector clocks, remove UI helpers
**Impact**: Cleaner service layer focused on CRUD and sharing only, 1200+ lines → 724 lines

### Decision: Frontend Controls UCAN Structure
**Date**: 2025-11-07
**Rationale**: Backend shouldn't hardcode doc names or capabilities, makes protocol generic
**Impact**: create_resource() accepts ucan_template_json parameter, frontend defines all doc structure

### Decision: Role-Based Delegation
**Date**: 2025-11-07
**Rationale**: Automatic template selection based on recipient role, no manual intervention needed
**Impact**: share_resource() extracts role from recipient token, automatically uses owner_template or viewer_template

### Decision: Independent AES Keys for Shares
**Date**: 2025-11-07
**Rationale**: Security isolation - viewer key compromise shouldn't expose owner's data
**Impact**: Each share gets NEW random AES key, never reuse owner's key

### Decision: Frontend-Driven Updates
**Date**: 2025-11-07
**Rationale**: Backend shouldn't merge CRDTs, frontend has better context
**Impact**: update_resource() accepts complete new Loro snapshots, no merging in backend

### Decision: Folder Sharing as Thin Wrapper
**Date**: 2025-11-08
**Rationale**: Folder sharing is just ACL creation, not data duplication. UCANs and encrypted data generated on-demand during sync
**Impact**: share_folder() creates folder_share_records and share_records with UCANs only, no pre-created encrypted_data

### Decision: On-Demand Encryption for Shared Resources
**Date**: 2025-11-08
**Rationale**: Pre-creating encrypted data for all shares is wasteful, sync protocol can generate on-the-fly
**Impact**: Share records only contain UCAN tokens, encrypted data created during sync when node requests it

### Decision: Efficient Share Checking with find_by_folder_and_user()
**Date**: 2025-11-08
**Rationale**: Avoid fetching all share records and filtering in memory, use single SQL query
**Impact**: Added find_by_folder_and_user() repository method with WHERE clause, O(1) instead of O(n)

### Decision: UCAN Facts Must Propagate Through Delegation
**Date**: 2025-11-08
**Rationale**: Delegated UCANs need facts (templates, doc_types, docs) for node to correctly parse and sync resources
**Impact**: Updated generate_delegated_ucan() signature to accept and propagate facts, updated issue_flexible_delegated_resource_ucan() to extract and pass facts

### Decision: Add add_resources Capability to Folder UCANs
**Date**: 2025-11-09
**Rationale**: Nodes need authority to delegate resource access to viewers. Without add_resources capability, nodes can receive resources but cannot share them with viewers, breaking the Owner → Node → Viewer architecture.
**Impact**:
- Handler layer determines capabilities: `crud/read` + `share_folder` + `add_resources`
- Service layer uses provided capabilities when generating folder UCAN
- ResourceDataSync includes owner_folder_ucan field to prove sender has add_resources permission
- Node can validate authority before accepting resources

### Decision: Store Folder UCAN in folders Table
**Date**: 2025-11-09
**Rationale**: Need easy access to owner's folder UCAN when sending resources to prove add_resources permission. Querying folder_share_records would require finding the owner's self-share record (inefficient).
**Impact**:
- Added `ucan` field to folders table (up.sql migration)
- Folder created with owner's UCAN token stored in folder.ucan
- During resource sync, read folder.ucan to get owner's UCAN with add_resources
- Before sending folder to node, replace folder.ucan with recipient's token from folder_share_record

### Decision: Handler Determines Capabilities, Service Executes
**Date**: 2025-11-09
**Rationale**: Separation of concerns - business logic (what capabilities) belongs in handler layer, execution (how to generate UCAN) belongs in service layer. Enables different contexts to grant different capabilities without modifying service code.
**Impact**:
- Moved capability determination from folder_service.rs to tauri_handlers/src/handlers/folder.rs
- Service accepts `folder_permissions` parameter and extracts abilities
- Handler constructs capability list with domain-specific knowledge
- Service is now reusable across different contexts (Tauri, CLI, API)

### Decision: Role-Based Capabilities in Connection Tokens
**Date**: 2025-11-09
**Rationale**: Connection tokens need different capabilities based on recipient role. Nodes need `add_folder` to enable folder sharing validation. Viewers don't need this capability. Service layer should determine capabilities based on role for flexibility and reusability.
**Impact**:
- Added `additional_capabilities` parameter to `generate_delegation_and_connection_token()` (ucan_utils.rs)
- Service layer (`issue_connect_ucan_token`) determines capabilities based on issued role:
  - `role="node"` → includes `{domain}:add_folder` capability
  - `role="viewer"` → no additional capabilities
- Generic crypto function accepts capabilities as parameter (follows folder sharing pattern)
- Enables future extension: new roles can easily get different capability sets
- Node can validate owner has `add_folder` when receiving folder share requests

### Decision: Add add_folder Capability to Connection Tokens
**Date**: 2025-11-09
**Rationale**: When owner shares folder with node, node must validate that owner has permission to add folders. Owner's connection token (issued by node to owner) needs to include `add_folder` capability for this validation.
**Impact**:
- Connection tokens with `role="node"` now include `{domain}:add_folder` capability
- Both owner and node receive tokens with `add_folder` (reciprocal node roles)
- Enables folder sharing validation: node checks owner's token for `add_folder` capability
- Future folder sync can validate sender authority using this capability

---

## Future Considerations

### Performance Optimizations
- State vector caching strategy: TBD
- In-memory resource caching: TBD
- Lazy doc loading: TBD

### Security Enhancements
- Key rotation mechanism: TBD
- Rate limiting: TBD
- UCAN expiration handling: TBD

### Features to Consider
- Asset streaming for large files: TBD
- Partial doc sync: TBD
- Compression: TBD

---

## Glossary

- **CRDT**: Conflict-free Replicated Data Type
- **Loro**: CRDT library (replacing Yrs)
- **UCAN**: User Controlled Authorization Networks
- **State Vector**: Compact CRDT operation history
- **Version Vector**: Loro's state vector equivalent
- **Snapshot**: Complete serialized Loro document
- **Update**: Incremental CRDT operations

---

## TODO: Sections to Add as We Implement

- [ ] Detailed encryption algorithms
- [ ] State vector comparison logic
- [ ] Loro snapshot format
- [ ] Update encoding format
- [ ] Asset sync protocol details
- [ ] Error codes and handling
- [ ] Performance benchmarks
- [ ] API reference

---

**This document will be continuously updated during implementation.**
