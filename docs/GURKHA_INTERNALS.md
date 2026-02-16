# Gurkha Internals

Deep dive into the gurkha authorization engine: permit parsing, decision logic, and UCAN token management.

## Overview

Gurkha is osvauld's pure authorization domain crate. It handles permit parsing, validation, and authorization decisions with **zero infrastructure dependencies** - no database, no network, no global state. All functions are stateless and receive keys as parameters.

**Design Philosophy:**
- **Pure domain logic** - Standalone, replaceable
- **Facts-driven** - All authorization in UCAN `facts` field
- **Stateless** - Functions take signing keys as `&[u8; 32]`
- **Self-describing permits** - Permits carry their own delegation templates

---

## Architecture

### Three-Layer Design

```
┌─────────────────────────────────────────────────┐
│  service.rs - External API                      │
│  Stateless functions: issue_*, delegate_*       │
└──────────────┬──────────────────────────────────┘
               │
┌──────────────▼──────────────────────────────────┐
│  decision/ - Pure Logic (NO crypto)             │
│  Sub-modules: connection, resource, delegation, │
│               sync, layer, consent               │
└──────────────┬──────────────────────────────────┘
               │
┌──────────────▼──────────────────────────────────┐
│  builder.rs / crypto.rs - UCAN JWT Signing      │
│  Signs TokenDecision → UCAN token               │
└─────────────────────────────────────────────────┘
```

**Core data structure:** `parser::Permit` - All permit parsing flows through this type.

---

## Key Types

### Permit (parser/mod.rs)

The central data structure wrapping UCAN JWTs. Parsed fields include:

- `facts` - Authorization facts (primary source of truth)
- `peer_capabilities` - What peer can do (relay, share, etc.)
- `layers` - Static layer access configuration
- `layer_patterns` - Pattern-based layer access (wildcards)
- `issue_on` - Recursive delegation templates
- `sync_facts` - Sync-specific metadata
- `presence` - Presence configuration
- `ephemeral_funcs` - Allowed ephemeral function names
- `dynamic_layer_schemas` - Runtime layer creation rules

**Key methods:**
- `can_write_layer(layer_name)` - Check write access
- `can_read_layer(layer_name)` - Check read access
- `should_sync_layer(layer_name)` - Check sync permission
- `static_layers()` - Get all static layer names
- `get_issue_template(action)` - Extract delegation template
- `is_owner()` - Check owner role

### TokenDecision

What to put in a new token. Pure data structure used by decision functions:

```rust
pub struct TokenDecision {
    pub audience: String,          // Recipient DID
    pub capabilities: Vec<String>, // What they can do
    pub facts: serde_json::Value,  // Authorization facts
    pub expiry: Option<u64>,       // Expiration timestamp
    pub proofs: Vec<String>,       // Parent token CIDs
    pub proof_tokens: Vec<String>, // Full parent tokens
}
```

Decision functions produce `TokenDecision`, which is then signed by `builder.rs`.

### DelegationDecision

What to give a delegatee. Similar to `TokenDecision` but includes delegation template:

```rust
pub struct DelegationDecision {
    pub audience: String,
    pub capabilities: Vec<String>,
    pub facts: serde_json::Value,
    pub template: DelegationTemplate, // For further delegation
    pub proofs: Vec<String>,
}
```

### SyncContext

Dual-permit context for sync decisions:

```rust
pub struct SyncContext {
    pub our_permit: Permit,
    pub peer_permit: Permit,
}
```

Methods:
- `should_send_updates(layer_name)` → `SyncDecision`
- `can_receive_updates(layer_name)` → `bool`

---

## Decision Functions (Pure Logic)

All decision functions are pure - no side effects, deterministic output.

### Handshake Decisions

| Function | Purpose | Returns |
|----------|---------|---------|
| `decide_hello_response` | Handle incoming Hello | AcceptFirstConnection / AcceptReconnection / AcceptPeer / Reject |
| `decide_welcome_response` | Handle incoming Welcome | Accept / RejectNodeMismatch / RejectAudienceMismatch |
| `decide_permit_grant_response` | Handle PermitGrant | Accept / Reject |

### Sync Decisions

| Function | Purpose | Returns |
|----------|---------|---------|
| `should_send_updates` | Determine sync behavior | SyncDecision (Incremental / FullSnapshot / DontSend) |
| `can_receive_updates` | Can we accept updates? | bool |

### Authorization Checks

| Function | Purpose | Returns |
|----------|---------|---------|
| `can_access_layer` | Layer access check | bool (3-way lookup: direct, page_id prefix, strip prefix) |
| `can_access_with_layer_permits` | Multi-permit check | bool (page permit + layer permits + schema fallback) |
| `matches_dynamic_schema` | Node-side schema match | bool (checks if layer matches dynamic schema pattern) |

---

## Service Functions (Stateless API)

All service functions are stateless and take signing keys as parameters:

### Connection Tokens

- `issue_one_time(signing_key, issuer_did, audience_did)` - One-time connection token
- `issue_peer_connection(...)` - Peer-to-peer connection token
- `issue_page_viewer_auth(...)` - Page viewer authentication
- `issue_space_viewer_auth(...)` - Space viewer authentication

### Resource Tokens

- `issue_page_owner_token(...)` - Page owner permit
- `issue_space_owner_token(...)` - Space owner permit
- `delegate_page(...)` - Delegate page access to another DID
- `delegate_space(...)` - Delegate space access to another DID

### Consent Tokens

- `issue_sync_space_consent(...)` - Space sync consent
- `issue_sync_page_consent(...)` - Page sync consent
- `issue_sync_layer_consent(...)` - Layer-specific sync consent

### Dynamic Layer Permits

- `issue_layer_permit(...)` - Issue permit for a specific layer
- `issue_layer_authority_permit(...)` - Issue authority permit (creator)
- `reissue_permit_with_layers(...)` - Add layers to existing permit

---

## Pattern Matching

Gurkha supports flexible pattern matching for layer authorization:

### Pattern Functions

- `expand_pattern(pattern, page_id, did)` - Replace `{page_id}` and `{aud}` variables
- `matches_schema_pattern(layer_path, schema_pattern)` - Match `{id}` wildcards
- `matches_wildcard(layer_name, pattern)` - Match `*` wildcards (single segment)
- `matches_dynamic_path_any_did(path, schema)` - Node-side auth (DID in position 1)

### Examples

```rust
// Pattern expansion
expand_pattern("orders/{aud}", "page123", "did:key:alice")
// → "orders/did:key:alice"

// Schema matching
matches_schema_pattern("channels/general/messages", "channels/{id}/messages")
// → true

// Wildcard matching
matches_wildcard("orders/did:key:alice", "orders/*")
// → true
```

---

## Self-Describing Permits

Permits carry `issue_on` templates defining what they can delegate:

```json
{
  "facts": {
    "issue_on": {
      "node": {
        "token_type": "page_share",
        "peer_capabilities": { "relay": true },
        "layers": { ... }
      },
      "viewer": {
        "token_type": "page_viewer",
        "layer_patterns": { ... }
      }
    }
  }
}
```

**Delegation flow:**
1. `delegate_page(key, delegator_token, "node", audience_did)`
2. Extracts `issue_on.node` template from delegator token
3. Creates new token for `audience_did` with template's configuration
4. Signs and returns new permit

**No role registry needed** - The permit IS the authority.

---

## Key Files

| File | Lines | Purpose |
|------|-------|---------|
| `parser/mod.rs` | ~1257 | Permit struct, DelegationTemplate, parsing logic |
| `decision/sync.rs` | - | SyncContext, sync decision logic |
| `decision/layer.rs` | - | Layer access checks, pattern matching |
| `decision/delegation.rs` | - | Delegation decisions, template extraction |
| `decision/connection.rs` | - | Connection token decisions |
| `decision/resource.rs` | - | Space/page owner token decisions |
| `decision/consent.rs` | - | Sync consent decisions |
| `service.rs` | ~756 | All stateless permit functions |
| `builder.rs` | - | GurkhaPermitBuilder (signs decisions) |
| `crypto.rs` | - | UCAN signing, Ed25519, CID computation |
| `types.rs` | - | Capability, DocType, SyncDecision, SyncFacts |

---

## Test Support

Feature `test-support` exposes:
- `test_fixtures` - Pre-built permit chains for common scenarios
- `test_strategies` - Property-based testing strategies

Used by integration tests to construct realistic permit scenarios without boilerplate.

---

## Authorization Flow Example

```rust
// 1. Parse permit from token string
let permit = Permit::parse(token_string)?;

// 2. Check layer access
if permit.can_write_layer("page123/orders/did:key:alice") {
    // Allowed
}

// 3. Extract delegation template
let template = permit.get_issue_template("viewer")?;

// 4. Create delegation decision
let decision = decide_delegation(
    &permit,
    "viewer",
    "did:key:bob",
    expiry
)?;

// 5. Sign decision into new permit
let new_token = service::delegate_page(
    signing_key,
    token_string,
    "viewer",
    "did:key:bob"
)?;
```

---

## Cross-References

- [PERMITS.md](app-dev/PERMITS.md) - Permit template authoring guide
- [VALIDATION.md](app-dev/VALIDATION.md) - Validation patterns using permits
- [ARCHITECTURE.md](ARCHITECTURE.md) - How gurkha fits in the system
- [PROTOCOL.md](PROTOCOL.md) - Wire protocol using gurkha permits

---

## Related Agent

See `.opencode/agents/gurkha.md` for the gurkha specialist agent.
