# Permits

UCAN-based authorization system in osvauld.

## Overview

Osvauld uses [UCAN](https://ucan.xyz/) (User Controlled Authorization Networks) for capability-based authorization. Permits are signed tokens that grant specific capabilities.

```
┌─────────────────────────────────────────────────────────────┐
│  Permit (UCAN Token)                                         │
│  ┌─────────────────────────────────────────────────────────┐│
│  │ Header: { alg: "EdDSA", typ: "JWT" }                    ││
│  ├─────────────────────────────────────────────────────────┤│
│  │ Payload:                                                 ││
│  │   iss: did:key:issuer...    (who issued this)           ││
│  │   aud: did:key:audience...  (who can use this)          ││
│  │   exp: 1735689600           (expiration)                ││
│  │   fct: { capabilities... }  (what it allows)            ││
│  │   prf: [parent_permit...]   (delegation chain)          ││
│  ├─────────────────────────────────────────────────────────┤│
│  │ Signature: Ed25519 signature by issuer                   ││
│  └─────────────────────────────────────────────────────────┘│
└─────────────────────────────────────────────────────────────┘
```

## Permit Types

### Connection Permits

| Type | Purpose | Issued By |
|------|---------|-----------|
| `one_time_connection` | First connection bootstrap | Self (owner) |
| `owner_connection` | Reconnection to node | Node |
| `node_connection` | Node joins network | Owner |
| `viewer_auth` | Shareable link (aud: `*`) | Node |

### Space Permits

| Type | Purpose | Issued By |
|------|---------|-----------|
| `space_owner` | Full space control | Self |
| `space_share` | Delegate space to node | Owner |
| `space_viewer` | Read-only space access | Node |

### Page Permits

| Type | Purpose | Issued By |
|------|---------|-----------|
| `page_owner` | Full page control | Space owner |
| `page_share` | Delegate page to node | Owner |
| `page_viewer` | Read/write page layers | Node |

### Sync Permits

| Type | Purpose | Issued By |
|------|---------|-----------|
| `sync_space_consent` | Consent to sync space | Viewer |
| `sync_page_consent` | Consent to sync page | Viewer |

## Capability Extraction

Gurkha extracts capabilities from permit facts:

```rust
pub struct Capabilities {
    // Operations
    pub can_read: bool,
    pub can_write: bool,
    pub can_share: bool,
    pub can_own: bool,

    // Layer access
    pub layers: HashMap<String, LayerCapability>,

    // Peer capabilities
    pub can_relay: bool,
    pub can_accept_publish: bool,

    // Auth capabilities
    pub can_connect: bool,
    pub can_delegate: bool,
    pub sync_enabled: bool,
}

pub struct LayerCapability {
    pub layer_type: String,  // "map", "list", "text"
    pub sync: bool,
    pub write: bool,
}
```

## Permit Templates

Apps define permit templates that specify what capabilities to grant:

```json
{
  "owner_template": {
    "operations": {
      "own": "allow",
      "read": "allow",
      "write": "allow",
      "share": "allow"
    },
    "peer_capabilities": {
      "relay": false,
      "share": true,
      "accept_publish": true
    },
    "layers": {
      "{page_id}/shapes": { "type": "map", "sync": true, "write": true },
      "{page_id}/messages": { "type": "list", "sync": true, "write": true }
    },
    "issue_on": {
      "node": { /* node permit template */ },
      "collaborator": { /* collaborator permit template */ }
    }
  }
}
```

### Template Variables

| Variable | Replaced With |
|----------|---------------|
| `{page_id}` | Actual page ID |
| `{user_did}` | User's DID |
| `*` | Wildcard (aud: `*` for shareable links) |

## Delegation Chain

Permits form a delegation chain:

```
Owner Permit
    │
    │ issue_on: node
    ▼
Node Permit
    │
    │ issue_on: viewer
    ▼
Viewer Permit
```

Each permit can only delegate capabilities it has:

```
Owner: read + write + share → Node: read + write + share
Node: read + write + share → Viewer: read + write (no share)
```

## Validation

Gurkha validates permits by checking:

1. **Signature**: Ed25519 signature valid
2. **Expiration**: Not expired
3. **Audience**: `aud` matches recipient or is `*`
4. **Chain**: Parent permit is valid and authorized delegation
5. **Capabilities**: Requested capabilities are subset of granted

```rust
fn validate_permit(permit: &Permit, expected_aud: &str) -> Result<()> {
    // 1. Verify signature
    verify_signature(&permit)?;

    // 2. Check expiration
    if permit.exp < current_time() {
        return Err(PermitExpired);
    }

    // 3. Check audience
    if permit.aud != expected_aud && permit.aud != "*" {
        return Err(AudienceMismatch);
    }

    // 4. Validate proof chain
    for parent in &permit.prf {
        validate_permit(parent, &permit.iss)?;
    }

    Ok(())
}
```

## Shareable Links

Shareable links use wildcard audience (`aud: *`):

```
1. Owner requests shareable link from node
2. Node issues permit with aud: *
3. Owner shares link containing permit
4. Viewer presents permit to node
5. Node validates and issues real permit (aud: viewer_did)
```

```json
{
  "iss": "did:key:node...",
  "aud": "*",
  "fct": {
    "type": "viewer_auth",
    "space_id": "space_123"
  }
}
```

## Consent Permits

Viewers issue consent permits to authorize sync:

```
1. Node sends space/page data to viewer
2. Viewer wants to enable real-time sync
3. Viewer issues consent permit back to node
4. Node can now send sync updates to viewer
```

```json
{
  "iss": "did:key:viewer...",
  "aud": "did:key:node...",
  "fct": {
    "type": "sync_page_consent",
    "page_id": "page_123",
    "layers": {
      "page_123/shapes": { "sync": true, "write": true }
    }
  }
}
```

## Lua API

```lua
-- Get current permit info
local page_id = permit:page_id()
local role = permit:role()  -- "owner", "collaborator", "viewer"
local my_did = permit:my_did()

-- Check capabilities
if permit:can_write("shapes") then
    shapes_layer:set(id, data)
end

-- Role-based logic
if permit:role() == "owner" then
    -- Owner-only actions
end
```

## Permit Flow Examples

### Owner → Node Publishing

```
Owner                                   Node
  │                                       │
  │  Hello (one_time_connection permit)   │
  │──────────────────────────────────────>│
  │                                       │
  │  Welcome (owner_connection permit)    │
  │<──────────────────────────────────────│
  │                                       │
  │  PermitGrant (node permit)            │
  │──────────────────────────────────────>│
  │                                       │
  │  PublishSpace (space_owner permit)    │
  │──────────────────────────────────────>│
  │                                       │
  │  PublishPage (page_owner permit)      │
  │──────────────────────────────────────>│
```

### Viewer Accessing Shared Link

```
Viewer                                  Node
  │                                       │
  │  SpaceRequest (viewer_auth, aud: *)   │
  │──────────────────────────────────────>│
  │                                       │
  │  SpaceData (space_viewer, aud: did)   │
  │<──────────────────────────────────────│
  │                                       │
  │  SyncConsentGrant (consent permits)   │
  │──────────────────────────────────────>│
  │                                       │
  │  SyncOffer (page data)                │
  │<──────────────────────────────────────│
```

## Security Considerations

### Permit Storage

- Permits are stored encrypted at rest
- Short expiration times recommended
- Revocation not built-in (use expiration)

### Capability Scope

- Grant minimum necessary capabilities
- Use layer-specific permissions
- Separate read and write where possible

### Delegation Limits

- Limit delegation depth in templates
- Consider not allowing viewer re-sharing
- Monitor delegation chains for abuse
