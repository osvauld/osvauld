# Permits

UCAN-based capability authorization for osvauld apps.

## Overview

Osvauld uses [UCAN](https://ucan.xyz/) tokens for capability-based authorization. A permit is a signed token granting specific capabilities -- what layers you can access, what operations you can perform, and what roles you can delegate.

```
Owner (operations.own: allow)
  └── issues → Node Permit (relay: true, can_delegate: true)
                  └── issues → Viewer Permit (layer_patterns: {page_id}/orders/{aud})
```

## permit_template.json Structure

Every app directory contains a `permit_template.json` that defines the capability hierarchy:

```json
{
    "owner_template": {
        "operations": { ... },
        "peer_capabilities": { ... },
        "layers": { ... },
        "layer_patterns": { ... },
        "dynamic_layer_schemas": { ... },
        "issue_on": {
            "node": { ... },
            "customer": { ... }
        }
    },
    "consent_template": { ... }
}
```

### Top-Level Sections

| Section | Purpose |
|---------|---------|
| `owner_template` | Capabilities the page owner gets |
| `consent_template` | Capabilities granted during sync consent (viewer → node) |

### owner_template Fields

| Field | Purpose | Example |
|-------|---------|---------|
| `operations` | What actions are allowed | `"own": "allow", "read": "allow"` |
| `peer_capabilities` | P2P capabilities | `"relay": false, "share": true` |
| `layers` | Fixed layer access with type/sync/write | See below |
| `layer_patterns` | Dynamic layer patterns with wildcards | See below |
| `dynamic_layer_schemas` | Schemas for runtime-created layers | See below |
| `issue_on` | Delegation templates for child roles | See below |

### Layer Definition

```json
"{page_id}/products": {
    "type": "list",
    "sync": true,
    "write": true
}
```

| Property | Type | Description |
|----------|------|-------------|
| `type` | string | `"list"`, `"map"`, or `"text"` |
| `sync` | bool | Whether this layer syncs with peers |
| `write` | bool | Whether this role can modify the layer |
| `create` | bool | Whether this role can create dynamic instances |

### Layer Patterns

Patterns use wildcards for dynamic, per-user layers:

```json
"layer_patterns": {
    "{page_id}/orders/*": {
        "sync": true,
        "write": true
    }
}
```

### Dynamic Layer Schemas

Dynamic layer schemas define layers created at runtime via `scribe:create_layer()`. Unlike fixed layers (declared in `layers`) or pattern-matched layers (declared in `layer_patterns`), dynamic layers are DID-namespaced and get their own layer permits issued by the node.

```json
"dynamic_layer_schemas": {
    "channels/{id}/messages": {
        "type": "map",
        "grant": "role",
        "role_permissions": {
            "owner": { "sync": true, "write": true },
            "collaborator": { "sync": true, "write": true },
            "node": { "sync": true, "write": true }
        }
    }
}
```

| Property | Type | Description |
|----------|------|-------------|
| `type` | string | `"list"` or `"map"` |
| `grant` | string | `"role"` (all members with matching roles) or `"explicit"` (creator + named participants) |
| `role_permissions` | object | Maps role names to `{ sync, write }` configs. Used when `grant: "role"`. |
| `permissions` | object | Default `{ sync, write }` for `grant: "explicit"` (creator gets this). |

**How it works:**

1. Lua app calls `scribe:create_layer("channels/{id}/messages", "general")`
2. Scribe generates a DID-namespaced path: `channels/{creator_did}/general/messages`
3. When the layer reaches the node, the node matches it against its `dynamic_layer_schemas`
4. Node issues **layer permits** to connected peers based on `grant` type:
   - `"role"`: All peers whose `relationship` matches a key in `role_permissions`
   - `"explicit"`: Only the creator (+ named participants in future)
5. Peers with a layer permit can receive and write to the dynamic layer

**Grant types:**

- **`role`** -- For shared channels. All members with a matching role get access automatically. Used for channels visible to all page members.
- **`explicit`** -- For private/DM channels. Only the creator gets access initially. Named participants will be added in a future phase.

**Important:** `role_permissions` keys must match the `relationship` field in peer permits. If a viewer's permit has `"relationship": "collaborator"`, there must be a `"collaborator"` entry in `role_permissions` for the viewer to receive dynamic layer data.

## Pattern Variables

| Variable | Expands To | Used In |
|----------|-----------|---------|
| `{page_id}` | Actual page ID | All layer names |
| `{aud}` | Token audience (recipient's DID) | Per-user layer patterns |
| `{iss}` | Token issuer's DID | Issuer-specific patterns |
| `*` | Any single segment | Wildcard matching |

**Key pattern**: `{page_id}/orders/{aud}` -- `{aud}` expands to the customer's DID, giving each customer their own private order layer.

## Role Hierarchy via `issue_on`

Roles are defined by nesting `issue_on` blocks. The delegation chain determines the role hierarchy:

```json
"issue_on": {
    "node": {
        "token_type": "page_share",
        "peer_capabilities": { "relay": true, "share": true },
        "layers": { ... },
        "relationship": "node",
        "issue_on": {
            "viewer": {
                "token_type": "page_viewer",
                "layer_patterns": {
                    "{page_id}/orders/{aud}": { "write": true, "sync": true }
                },
                "relationship": "customer"
            }
        }
    },
    "customer": {
        "token_type": "page_viewer",
        "relationship": "customer",
        ...
    }
}
```

- **`relationship`**: The role name the app sees via `permit:role()` (e.g., `"customer"`, `"node"`, `"admin"`)
- **`token_type`**: Internal permit classification (`page_share`, `page_viewer`, etc.)
- Roles at the same level under `issue_on` are parallel delegation paths
- Nested `issue_on` creates a chain: owner → node → viewer

## Adding New Roles

Add a new entry under `issue_on`:

```json
"issue_on": {
    "node": { ... },
    "customer": { ... },
    "supplier": {
        "token_type": "page_viewer",
        "peer_capabilities": {
            "relay": false,
            "share": false,
            "accept_publish": false
        },
        "operations": {
            "read": "allow",
            "write": "allow"
        },
        "layers": {
            "{page_id}/inventory": { "type": "list", "sync": true, "write": true },
            "app:Supplier View": { "type": "map", "sync": true, "write": false }
        },
        "auth_capabilities": {
            "can_connect": true,
            "sync_enabled": true
        },
        "relationship": "supplier"
    }
}
```

## consent_template

The consent template defines what capabilities a viewer grants back to the node for sync:

```json
"consent_template": {
    "token_type": "sync_page_consent",
    "operations": {
        "receive_layer_updates": "allow",
        "send_layer_updates": "allow"
    },
    "layers": {
        "{page_id}/products": { "sync": true, "write": false }
    },
    "relationship": "sync_consent"
}
```

Consent is sync-only -- it doesn't grant sharing or delegation capability.

## Complete Example: E-commerce App

From `sample_apps/my-shop/permit_template.json` (simplified):

```json
{
    "owner_template": {
        "operations": {
            "own": "allow", "read": "allow", "write": "allow",
            "share": "allow", "share_page": "allow"
        },
        "peer_capabilities": {
            "relay": false, "share": true, "accept_publish": true
        },
        "layers": {
            "{page_id}/products": { "type": "list", "sync": true, "write": true },
            "{page_id}/derived/orders_summary": { "type": "map", "sync": true, "write": false },
            "{page_id}/drafts": { "type": "map", "sync": false, "create": true },
            "app:Shop Owner": { "type": "map", "sync": true, "write": true },
            "app:Shop Customer": { "type": "map", "sync": true, "write": true }
        },
        "layer_patterns": {
            "{page_id}/orders/*": { "sync": true, "write": true }
        },
        "issue_on": {
            "node": {
                "token_type": "page_share",
                "peer_capabilities": { "relay": true, "share": true, "accept_publish": true },
                "layers": {
                    "{page_id}/products": { "type": "list", "sync": true, "write": false },
                    "{page_id}/derived/orders_summary": { "type": "map", "sync": true, "write": true }
                },
                "layer_patterns": {
                    "{page_id}/orders/*": { "sync": true, "create": true }
                },
                "relationship": "node",
                "issue_on": {
                    "viewer": {
                        "token_type": "page_viewer",
                        "layers": {
                            "{page_id}/products": { "type": "list", "sync": true, "write": false },
                            "app:Shop Customer": { "type": "map", "sync": true, "write": false }
                        },
                        "layer_patterns": {
                            "{page_id}/orders/{aud}": { "write": true, "sync": true }
                        },
                        "relationship": "customer"
                    }
                }
            }
        }
    },
    "consent_template": {
        "token_type": "sync_page_consent",
        "layers": {
            "{page_id}/products": { "sync": true, "write": false },
            "app:Shop Customer": { "sync": true, "write": false }
        },
        "layer_patterns": {
            "{page_id}/orders/*": { "sync": true }
        },
        "relationship": "sync_consent"
    }
}
```

## Lua Permit API

```lua
local page_id = permit:page_id()
local my_did = permit:my_did()
local role = permit:role()  -- "owner", "customer", "node", etc.

if permit:can_write("layer_name") then
    -- allowed
end

if role == "owner" then
    -- owner-only actions
end
```

## Gotchas

- **`{aud}` expands to the token audience** -- the DID of the user receiving the permit, giving each user their own layer
- **`layer_patterns` uses `*` wildcard** -- matches any single segment in the layer name
- **New apps need `"app:App Name"` layer** in ALL role templates (owner, node, viewer, consent)
- **`consent_template` is sync-only** -- it authorizes data flow, not sharing or delegation
- **Node needs `relay: true`** in `peer_capabilities` to relay data between peers
- **Derived layers**: `write: true` only for the `node` role; `write: false` for everyone else
- **`dynamic_layer_schemas` must be in ALL roles** -- owner, node, viewer, and consent templates all need the same schemas for dynamic layers to sync through the full chain
- **`role_permissions` keys must match `relationship` values** -- if viewer's permit has `"relationship": "collaborator"`, use `"collaborator"` (not `"viewer"`) in `role_permissions`
- **`ephemeral_funcs`**: List of allowed ephemeral function names for structured ephemeral messages
- **Clean the DB** (`rm -rf ~/.local/share/osvauld`) when changing permit templates -- old permits are cached
