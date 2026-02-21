# App Development Overview

Building Lua apps for osvauld's P2P platform.

## What is an Osvauld App

An osvauld app is a Lua program that runs inside a P2P-synced environment. Apps use:
- **Lua** for business logic and data handling
- **Slint** or **Raylib** for rendering
- **Loro CRDTs** for automatic multi-peer data sync
- **UCAN permits** for capability-based authorization

## App Package Structure (Current)

`app.osv` is now the declaration source of truth for app policy and role/layer definitions.

```
my-app/
├── app.osv                  # Roles, layers, grants, UI app declarations
├── shared/
│   ├── manifest.json
│   ├── validation.lua       # Business rules (runs on node)
│   └── init.lua             # Derivation registration (runs on node)
├── shop-owner/
│   ├── manifest.json
│   ├── app.slint
│   └── app.lua
└── shop-customer/
    ├── manifest.json
    ├── app.slint
    └── app.lua
```

See `docs/app-dev/APP_OSV_GRAMMAR.md` for the full grammar and semantic rules.

## Removed and Deprecated

- The old `page.lua` declaration DSL has been removed from the active app-authoring path.
- Policy artifact structs that mirrored template JSON have been removed from compiler output in favor of typed policy facts.
- `permit_template.json` and `space_permit_template.json` are legacy compatibility inputs in some runtime/import paths and are being phased out.

### In-Page Navigation

Apps within a page can navigate to sibling apps using `page:open_app()`:

```lua
function on_click(target)
    if target == "cta:Read the Docs" then
        page:open_app("Protocol Docs")  -- switches to the "Protocol Docs" app tab
    end
end
```

The name must match the target app's `"name"` in its `manifest.json` (or the `ui_app` declaration in `app.osv`). Navigation only works between apps in the same page. See [LUA_API.md](LUA_API.md#page-navigation) for details.

## App Lifecycle

1. **Load**: manifest.json parsed (as `domains::AppManifest`), routed to renderer by `renderer` field
2. **Renderer-specific preparation**:
   - **Slint**: Compile .slint UI file, set up VecModels
   - **Raylib**: Initialize window with `width`, `height`, `target_fps`
   - **Kunki (node runtime)**: Execute `entry_node` script (no UI)
3. **Execute app.lua** in Lua VM
4. **`on_init()`**: App startup -- set up `scribe:bind()` declarations, initialize state
5. **`scribe:bind()` auto-sync**: Layer changes automatically sync to UI properties
6. **Runtime loop**: Slint apps use timers for periodic work; Raylib apps use `update(dt)` + `draw()`
7. **`on_ephemeral(user_did, func, args)`**: Real-time structured messages from peers
8. **`on_key_pressed(key)`**: Keyboard input (requires FocusScope in Slint)

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
| `update(dt)` *(Raylib)* | Per-frame game update | delta_time_seconds |
| `draw()` *(Raylib)* | Per-frame render callback | None |
| `on_layer_discovered(name)` | New layer found (optional) | layer_name |

## Sample Apps

| App | Directory | Key Patterns |
|-----|-----------|-------------|
| **Snake Game** | `sample_apps/osvauld-demos/snake-game/` | keyboard input, game loop patterns, scores list |
| **Math Simulation** | `sample_apps/osvauld-demos/math-sim/` | particle arrays, lazy VecModel creation |
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
- [APP_OSV_GRAMMAR.md](APP_OSV_GRAMMAR.md) -- app.osv grammar and compiler output
- [PERMITS.md](PERMITS.md) -- UCAN permits and typed policy facts
- [VALIDATION.md](VALIDATION.md) -- validation.lua patterns
- [DERIVATION.md](DERIVATION.md) -- Derived layers and node logic
- [TESTING.md](TESTING.md) -- Control server API and test scripts
