# Unified Handshake Implementation - Three-Way Handshake Pattern

**Date**: 2025-11-14
**Status**: ✅ COMPLETE - Three-Way Handshake Implemented and Tested
**Context**: Mutual token exchange for first-time connections with proper token directionality

---

## Executive Summary

Implementing a **three-way handshake protocol** for first-time peer connections to ensure both parties exchange and save persistent connection tokens. The key insight: the `first_connection` flag in the UCAN token determines whether to perform token exchange (3-way) or simple verification (2-way).

### Core Design Principles

1. **Token-Driven**: UCAN token determines handshake flow (first_connection flag)
2. **Mutual Trust**: Both parties issue tokens to each other on first connection
3. **Type-Safe Messages**: Dedicated message types for first connection vs reconnection
4. **Immediate Token Storage**: Issued tokens saved before storing peer in database
5. **No One-Time Tokens in DB**: Replace with persistent tokens immediately

---

## Handshake Message Types

### Three Message Types for Clarity

```rust
pub enum HandshakeMessage {
    // Step 1: Initiator → Responder (all connections)
    HandshakeRequest(HandshakeRequest),

    // Step 2 & 3: First connection (3-way handshake)
    FirstConnectionResponse(FirstConnectionResponse),
    FirstConnectionComplete(FirstConnectionComplete),

    // Step 2: Reconnection (2-way handshake)
    ReconnectionResponse(ReconnectionResponse),
}
```

### Message Structures

#### HandshakeRequest (Common)
```rust
pub struct HandshakeRequest {
    pub ucan_token: String,         // One-time OR persistent token
    pub peer_user: User,
    pub peer_device: Device,
    pub signed_ucan_pub: String,    // PGP-signed UCAN public key
}
```

#### FirstConnectionResponse
```rust
pub struct FirstConnectionResponse {
    pub issued_ucan: String,        // NEW persistent token for initiator
    pub peer_user: User,
    pub peer_device: Device,
    pub devices: Vec<Device>,
    pub signed_ucan_pub: String,
}
```

#### FirstConnectionComplete
```rust
pub struct FirstConnectionComplete {
    pub issued_ucan: String,        // NEW persistent token for responder
}
```

#### ReconnectionResponse
```rust
pub struct ReconnectionResponse {
    pub peer_user: User,
    pub peer_device: Device,
    pub devices: Vec<Device>,
    pub signed_ucan_pub: String,
}
```

---

## Three-Way Handshake Flow (First Connection)

### Phase 1: Token Generation (kunki)

**Command**: `./kunki token`

**What Happens**:
1. Node loads certificate with passphrase
2. Gets node's UCAN Ed25519 private key
3. Generates **one-time connection token**:
   - **Audience**: `"*"` (wildcard, single-use)
   - **Capabilities**: `"sthalam:user-connect:{node_ucan_pub_key}"` with `"use"` permission
   - **Facts**:
     ```json
     {
       "token_type": "one_time_connection",
       "role": "owner",
       "first_connection": true
     }
     ```
   - **Expiry**: 30 days
   - **Issuer**: `did:key:{node_ucan_pub_key}`

4. Creates connection string JSON:
   ```json
   {
     "user_public_key": "PGP certificate",
     "device_public_key": "base64 device key",
     "username": "node",
     "ucan_token": "eyJ...",
     "ucan_pub_key": "base64 UCAN pub key"
   }
   ```

5. Base64 encodes entire JSON → connection string

**Token Meaning**: "I (node) grant 'owner' role connection permission to whoever presents this token (one time only)"

### Phase 2: Owner Saves Node Info

**What Happens in Owner's App**:
1. User pastes connection string
2. App decodes and extracts fields
3. Creates User record for node:
   ```
   id: (from PGP fingerprint)
   username: "node"
   public_key: (PGP certificate)
   ucan_token: "eyJ..." (one-time token)  ← Temporary
   ucan_pub_key: (base64)
   owner: false
   ```

4. Creates Device record for node
5. **Saves both to database**

**State**: Owner has node's one-time token stored temporarily

### Phase 3: Connection Establishment

Owner calls `sync_handler::send_folder()` or connects manually → triggers `connect_with_ticket(device_id)`

### Phase 4: Initiate Handshake (Owner → Node)

**File**: `network/src/p2p/p2p_service.rs` → `perform_handshake_and_create_peer()`

```rust
// When is_initiator = true:
1. Look up device by device_id
2. Get user_id from device
3. Get user record → extract user.ucan_token (one-time token from Phase 2)
4. Call auth::initiate_handshake(conn, ucan_token, owner_user, owner_device)
```

**File**: `network/src/p2p/auth.rs` → `initiate_handshake()`

```rust
1. Sign owner's ucan_pub_key with PGP
2. Send HandshakeRequest {
     ucan_token: "eyJ..." (node's one-time token),
     peer_user: owner's User,
     peer_device: owner's Device,
     signed_ucan_pub: "..."
   }
```

### Phase 5: Process Request (Node → Owner)

**File**: `network/src/p2p/auth.rs` → `process_handshake_request()`

```rust
1. Parse and validate token:
   - Verify PGP signature on signed_ucan_pub
   - Decode UCAN → sees first_connection: true, role: "owner"
   - Returns HandshakeType::PeerFirstConnection

2. Route to handle_peer_first_connection()
```

**File**: `network/src/p2p/auth.rs` → `handle_peer_first_connection()`

```rust
1. Issue NEW persistent token for owner:
   - Audience: owner's ucan_pub_key (NOT wildcard!)
   - Capabilities:
     * "sthalam:user:*" → "connect", "share"
     * "sthalam:folder:*" → "add_folder"
   - Facts:
     {
       "token_type": "owner_connection",
       "role": "owner",
       "first_connection": false
     }
   - Expiry: Far future (2099)

2. Save owner's User record with one-time token (TEMPORARY):
   User {
     id: owner_id,
     username: "test",
     ucan_token: "one_time_token_from_request",  ← TEMPORARY! Will be updated in Phase 7
     ucan_pub_key: owner_ucan_pub,
     owner: true,
     ...
   }

3. Save owner User + Device to database

4. Send FirstConnectionResponse {
     issued_ucan: "token_node_issued_for_owner",  ← Token FOR owner to use when connecting TO node
     peer_user: node's User,
     peer_device: node's Device,
     devices: [node's Device],
     signed_ucan_pub: "..."
   }
```

**Key**: Node saves owner with the one-time token temporarily. This will be replaced in Phase 7 with the persistent token owner issues FOR node.

### Phase 6: Process Response (Owner → Node)

**File**: `network/src/p2p/auth.rs` → `process_first_connection_response()`

```rust
1. Receive FirstConnectionResponse with issued_ucan (token node issued FOR owner)

2. Save node's User record with token NODE issued FOR owner:
   User {
     id: node_id,
     username: "node",
     ucan_token: "token_node_issued_for_owner",  ← Token FOR owner to use when connecting TO node
     ucan_pub_key: node_ucan_pub,
     owner: false,
     ...
   }

3. Issue NEW persistent token FOR node:
   - Audience: node's ucan_pub_key
   - Capabilities: Same as above
   - Facts:
     {
       "token_type": "node_connection",
       "role": "node",           ← Node's role!
       "first_connection": false
     }

4. Send FirstConnectionComplete {
     issued_ucan: "token_owner_issued_for_node",  ← Token FOR node to use when connecting TO owner
     peer_user_id: owner_id
   }
```

**Key**: Owner saves node user with the token node issued FOR owner. This is what owner will present when connecting TO node. Owner then issues a token FOR node, which node will present when connecting TO owner.

### Phase 7: Process Complete (Node)

**File**: `network/src/p2p/auth.rs` → `process_first_connection_complete()`

```rust
1. Receive FirstConnectionComplete with issued_ucan and peer_user_id

2. Update owner's user.ucan_token in database (replace one-time token from Phase 5):
   UPDATE users
   SET ucan_token = 'token_owner_issued_for_node'  ← Token FOR node to use when connecting TO owner
   WHERE id = owner_id

3. Mark handshake complete
```

**Key**: Node replaces the temporary one-time token (saved in Phase 5) with the persistent token owner issued FOR node. This is what node will present when connecting TO owner.

### Result

**Database State After Three-Way Handshake**:

**Node's Database**:
```
User: owner
  ucan_token: "token_owner_issued_for_node"  ← Token owner issued FOR node
  role: owner
```
**What this means**: When node wants to connect TO owner, node presents this token to prove authorization.

**Owner's Database**:
```
User: node
  ucan_token: "token_node_issued_for_owner"  ← Token node issued FOR owner
  role: node
```
**What this means**: When owner wants to connect TO node, owner presents this token to prove authorization.

**Critical Understanding**:
- Each party saves the token the OTHER party issued FOR them
- Node needs "token_owner_issued_for_node" to connect to owner (validates against owner's UCAN key)
- Owner needs "token_node_issued_for_owner" to connect to node (validates against node's UCAN key)
- The tokens are DIFFERENT and have DIFFERENT roles embedded in them
- Each token is stored in the database of the party who will USE it (not who issued it)

---

## Two-Way Handshake Flow (Reconnection)

### Phase 1: Initiator sends HandshakeRequest

```rust
// Owner initiating reconnection
1. Look up node's user → extract user.ucan_token (persistent token from first connection)
2. Send HandshakeRequest {
     ucan_token: "persistent_node_connection_token",
     peer_user: owner's User,
     peer_device: owner's Device,
     signed_ucan_pub: "..."
   }
```

### Phase 2: Responder validates and responds

**File**: `network/src/p2p/auth.rs` → `process_handshake_request()`

```rust
1. Parse token → sees first_connection: false
2. Routes to handle_reconnection()
```

**File**: `network/src/p2p/auth.rs` → `handle_reconnection()`

```rust
1. Verify owner exists in database:
   - Query: SELECT * FROM users WHERE id = owner_id
   - If not found: ERROR

2. Send ReconnectionResponse {
     peer_user: node's User,
     peer_device: node's Device,
     devices: [node's Device],
     signed_ucan_pub: "..."
   }
```

### Phase 3: Initiator processes response

**File**: `network/src/p2p/auth.rs` → `process_reconnection_response()`

```rust
1. Update peer user/device info in connection state
2. Mark handshake complete
```

**No token exchange needed!** Both sides already have persistent tokens.

---

## Token Roles and Capabilities

### Owner Connection Token (issued by node FOR owner)

**Issued by**: Node
**Issued for**: Owner
**Who presents it**: Owner (when connecting to node)
**Where stored**: Owner's database (in node's user record)
**Purpose**: Allows owner to authenticate when connecting to node

```json
{
  "aud": "owner_ucan_pub_key",  ← Token can only be used by owner
  "cap": {
    "sthalam:user:*": {"connect": [{}], "share": [{}]},
    "sthalam:folder:*": {"add_folder": [{}]}
  },
  "fct": {
    "token_type": "owner_connection",
    "role": "owner",  ← Describes the HOLDER'S role (owner)
    "first_connection": false
  }
}
```

### Node Connection Token (issued by owner FOR node)

**Issued by**: Owner
**Issued for**: Node
**Who presents it**: Node (when connecting to owner)
**Where stored**: Node's database (in owner's user record)
**Purpose**: Allows node to authenticate when connecting to owner

```json
{
  "aud": "node_ucan_pub_key",  ← Token can only be used by node
  "cap": {
    "sthalam:user:*": {"connect": [{}], "share": [{}]},
    "sthalam:folder:*": {"add_folder": [{}]}
  },
  "fct": {
    "token_type": "node_connection",
    "role": "node",  ← Describes the HOLDER'S role (node)
    "first_connection": false
  }
}
```

**Key Points**:
- Token describes the **holder's role**, not the issuer's role
- Each party stores the token in the OTHER party's user record
- The stored token is what will be PRESENTED when connecting, not what will be VALIDATED

---

## Implementation Checklist

### Phase 1: Update Message Types ✅
- [x] Add `FirstConnectionResponse` struct to `core/src/models/p2p.rs`
- [x] Add `FirstConnectionComplete` struct
- [x] Add `ReconnectionResponse` struct
- [x] Update `HandshakeMessage` enum
- [x] Remove old `HandshakeResponse` struct

### Phase 2: Add Service Functions ✅
- [x] Add `update_user_token()` to `services/src/auth_service.rs`
- [x] Fix `save_first_connection_user()` to update token when user exists

### Phase 3: Update Auth Handlers ✅
- [x] Split `handle_peer_first_connection()` to issue token BEFORE saving
- [x] Add `process_first_connection_response()` handler
- [x] Add `process_first_connection_complete()` handler
- [x] Update `handle_reconnection()` to send `ReconnectionResponse`
- [x] Add `process_reconnection_response()` handler

### Phase 4: Update Message Router ✅
- [x] Update `peer_connection.rs` to handle new message types

### Phase 5: Testing ✅
- [x] Test first connection: verify both sides have persistent tokens
- [x] Test reconnection: verify no new tokens issued
- [x] Verify database state after handshake

---

## Files to Modify

1. **core/src/models/p2p.rs**
   - Add `FirstConnectionResponse`, `FirstConnectionComplete`, `ReconnectionResponse`
   - Update `HandshakeMessage` enum
   - Remove `HandshakeResponse`

2. **services/src/auth_service.rs**
   - Add `update_user_token(user_id: &str, token: &str, repo_ctx: &Arc<RepositoryContext>)`

3. **network/src/p2p/auth.rs**
   - Update `handle_peer_first_connection()` - issue token before save
   - Add `process_first_connection_response()`
   - Add `process_first_connection_complete()`
   - Update `handle_reconnection()` - send ReconnectionResponse
   - Add `process_reconnection_response()`
   - Remove `send_handshake_response()` helper

4. **network/src/p2p/peer_connection.rs**
   - Update message match to handle new types

---

## Key Differences from Previous Design

### Before (Two-Way with Optional Token)
```
HandshakeRequest → HandshakeResponse(issued_ucan: Option<String>)
```
- Single message type
- `issued_ucan` presence determined flow
- Confusing state management

### After (Three-Way with Dedicated Types)
```
First Connection:
  HandshakeRequest → FirstConnectionResponse → FirstConnectionComplete

Reconnection:
  HandshakeRequest → ReconnectionResponse
```
- Clear message types
- Type-safe handling
- Explicit flow control

---

## Security Considerations

1. **Token Validation**: PGP signature on `signed_ucan_pub` prevents MITM
2. **Audience Restriction**: Persistent tokens have specific audience (not wildcard)
3. **One-Time Use**: One-time tokens never stored in database
4. **Mutual Auth**: Both parties validate and issue tokens
5. **Reconnection Safety**: Verify user exists before accepting reconnection

---

## Implementation Summary

1. ✅ Documented three-way handshake design with concrete token flow
2. ✅ Implemented message type changes (FirstConnectionResponse, FirstConnectionComplete, ReconnectionResponse)
3. ✅ Implemented service layer functions (update_user_token, save_first_connection_user with token update)
4. ✅ Implemented auth.rs handlers (handle_peer_first_connection, process_first_connection_response, process_first_connection_complete)
5. ✅ Updated message router in peer_connection.rs
6. ✅ Tested first connection and reconnection flows - both sides have correct persistent tokens

## Key Bug Fixes

1. **Token Directionality**: Fixed confusion about which token goes where - each party now saves the token the OTHER party issued FOR them
2. **Token Update on Reconnection**: Fixed `save_first_connection_user()` to update token when user already exists instead of skipping
3. **Message Structure**: Added `peer_user_id` to `FirstConnectionComplete` so responder knows which user to update

## Next Steps

- Viewer handshake implementation (using same three-way pattern)
- Add comprehensive error handling and recovery
- Performance optimization for token validation
