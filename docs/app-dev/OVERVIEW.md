# App Development Overview

Building Lua apps for osvauld's P2P platform.

## What is an Osvauld App

An osvauld app is a Lua program that runs inside a P2P-synced environment. Apps use:
- **Lua** for business logic and data handling
- **Slint** or **Raylib** for rendering
- **Loro CRDTs** for automatic multi-peer data sync
- **UCAN permits** for capability-based authorization

## Two Approaches

### Traditional: manifest.json + app files

Each role gets its own directory with manifest, UI, and logic:

```
my-app/
├── shared/                  # Shared library (validation, derivation)
│   ├── manifest.json
│   ├── validation.lua       # Business rules (runs on node)
│   └── init.lua             # Derivation registration (runs on node)
├── shop-owner/              # Owner role app
│   ├── manifest.json
│   ├── app.slint
│   └── app.lua
├── shop-customer/           # Customer role app
│   ├── manifest.json
│   ├── app.slint
│   └── app.lua
└── permit_template.json     # Page-level permits
```

### page.lua DSL

A single `page.lua` defines roles, layers, and apps declaratively:

```
my-app/
├── page.lua                 # THE source of truth
├── owner-app/
│   ├── app.slint
│   └── app.lua
└── viewer-app/
    ├── app.slint
    └── app.lua
```

## page.lua Reference

### `page(name, version)`

Defines page metadata. Must be called first.

```lua
page("My Shop", "1.0.0")
```

### `role(name, options)`

Defines a role with capabilities.

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `parent` | string | nil | Parent role for inheritance |
| `can_share` | bool | false | Can share page with others |
| `can_delegate` | bool | false | Can delegate permits |
| `can_relay` | bool | false | Can relay data (nodes only) |

```lua
role("owner", { can_share = true, can_delegate = true })
role("customer", { parent = "owner" })
role("node", { parent = "owner", can_relay = true })
```

### `layer(pattern, type, options)`

Defines a data layer with access control.

- `pattern`: Layer name or pattern (e.g., `"products"`, `"orders/{aud}"`)
- `type`: `"list"` (LoroList) or `"map"` (LoroMap)
- `options`: Table with role permissions

Permissions: `read`, `write`, `sync`, `create`

```lua
-- Simple static layer
layer("products", "list", {
    owner = {"read", "write", "sync"},
    customer = {"read", "sync"}
})

-- Dynamic per-user layer
layer("orders/{aud}", "list", {
    owner = {"read", "sync"},
    ["{aud}"] = {"read", "write", "sync"}
})

-- Local-only layer (no sync)
layer("drafts", "map", {
    owner = {"read", "write"},
    customer = {"read", "write", "create"}
})

-- Layer with inline validation
layer("orders/{aud}", "list", {
    owner = {"read", "sync"},
    ["{aud}"] = {"read", "write", "sync"},
    validate = function(ops, ctx)
        for _, op in ipairs(ops) do
            if op.op == "insert" and op.value.status ~= "pending" then
                return false, "Orders must start as pending"
            end
        end
        return true, nil
    end
})

-- Derived layer
layer("derived/summary", "map", {
    owner = {"read", "sync"},
    node = {"write"},
    derive_from = "orders/*",
    transform = function(source, item)
        return { id = item.id, total = item.total }
    end
})
```

### `app(name, options)`

Defines an application for a role.

| Option | Type | Description |
|--------|------|-------------|
| `client` | table | Client (UI) definition |
| `node` | table | Node (headless) definition |
| `for_role` | string/array | Role(s) that can use this app |

**Client options**: `ui` (path to .slint), `logic` (path to .lua), `tick` (bool), `models` (array)

**Node options**: `logic` (path to .lua), `tick` (bool)

```lua
-- Client-only app
app("Customer View", {
    client = { ui = "customer/app.slint", logic = "customer/app.lua" },
    for_role = "customer"
})

-- App with node logic (for derivation)
app("Shop Owner", {
    client = { ui = "owner/app.slint", logic = "owner/app.lua" },
    node = { logic = "owner/node.lua", tick = true },
    for_role = "owner"
})

-- App for multiple roles
app("Shared Dashboard", {
    client = { ui = "dashboard/app.slint", logic = "dashboard/app.lua" },
    for_role = {"owner", "admin"}
})
```

### In-Page Navigation

Apps within a page can navigate to sibling apps using `page:open_app()`:

```lua
function on_click(target)
    if target == "cta:Read the Docs" then
        page:open_app("Protocol Docs")  -- switches to the "Protocol Docs" app tab
    end
end
```

The name must match the target app's `"name"` in its `manifest.json` (or the name passed to `app()` in page.lua). Navigation only works between apps in the same page. See [LUA_API.md](LUA_API.md#page-navigation) for details.

## App Lifecycle

1. **Load**: manifest.json parsed, .slint compiled, .lua executed
2. **`on_init()`**: App startup -- set up `scribe:bind()` declarations, initialize state
3. **`scribe:bind()` auto-sync**: Layer changes automatically sync to UI properties
4. **`tick()`**: Called ~60fps if `tick_enabled: true` in manifest (Slint) or always (Raylib)
5. **`on_ephemeral(user_did, func, args)`**: Real-time structured messages from peers
6. **`on_key_pressed(key)`**: Keyboard input (requires FocusScope in Slint)

### Optional Callbacks (for advanced scenarios)

- **`on_layer_discovered(layer_name)`**: New layer found (wildcards auto-include)

## Event Handlers Reference

| Handler | When Called | Parameters |
|---------|------------|------------|
| `on_init()` | App startup | None |
| `on_click(target)` | Button clicked | target_id |
| `on_field_changed(field, val)` | Form input changed | field_name, value |
| `on_modal_action(modal, action)` | Modal event | modal_name, action |
| `on_ephemeral(did, func, args)` | Real-time event | user_did, func_name, args_table |
| `on_key_pressed(key)` | Keyboard input | key_string |
| `on_peer_joined(did)` | Peer connected | user_did |
| `on_peer_left(did)` | Peer disconnected | user_did |
| `tick()` | Game loop (~60fps) | None |
| `on_layer_discovered(name)` | New layer found (optional) | layer_name |

## Sample Apps

| App | Directory | Key Patterns |
|-----|-----------|-------------|
| **Snake Game** | `sample_apps/osvauld-demos/snake-game/` | `tick_enabled`, keyboard input, game loop, scores list |
| **Math Simulation** | `sample_apps/osvauld-demos/math-sim/` | `tick_enabled`, particle arrays, lazy VecModel creation |
| **Group Chat** | `sample_apps/osvauld-demos/group-chat/` | Real-time sync, ephemeral events, text input |
| **Guide** | `sample_apps/osvauld-demos/guide/` | Tab navigation, static content |
| **Tank Game** | `sample_apps/osvauld-demos/tank-game/` | Multiplayer, datagrams, node game loop, obstacles |
| **Snake Raylib** | `sample_apps/osvauld-demos/snake-raylib/` | Raylib renderer, game loop |
| **Tank Raylib** | `sample_apps/osvauld-demos/tank-raylib/` | Raylib renderer, multiplayer |
| **My Shop** | `sample_apps/my-shop/` | E-commerce, multi-role, derivation, validation |
| **My Booking** | `sample_apps/my-booking/` | Time slots, scheduling, privacy-preserving derivation |
| **Canvas App** | `sample_apps/canvas-app/` | Collaboration, shapes, remote cursors |
| **Photo Gallery** | `sample_apps/photo-gallery/` | Asset handling, file uploads |

### Running Demos

```bash
python scripts/run_demo.py snake
python scripts/run_demo.py math
python scripts/run_demo.py chat
python scripts/run_demo.py tank
python scripts/run_demo.py guide
```

## Related Docs

- [SCRIBE_API.md](SCRIBE_API.md) -- Unified scribe: module (layers, bindings, ephemerals)
- [MANIFEST.md](MANIFEST.md) -- manifest.json field reference
- [LUA_API.md](LUA_API.md) -- Complete Lua API reference
- [RENDERERS.md](RENDERERS.md) -- Slint and Raylib renderer details
- [PERMITS.md](PERMITS.md) -- permit_template.json and role hierarchy
- [VALIDATION.md](VALIDATION.md) -- validation.lua patterns
- [DERIVATION.md](DERIVATION.md) -- Derived layers and node logic
- [TESTING.md](TESTING.md) -- Control server API and test scripts
