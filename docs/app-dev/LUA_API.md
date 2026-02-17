# Lua API Reference

Complete reference for all Lua APIs available in osvauld apps.

## Core Modules

| Module | Purpose | Context |
|--------|---------|---------|
| `scribe:` | Unified layer/identity/binding API | All |
| `permit:` | Role and capability checks | All |
| `ui:` | UI binding | App runtime |
| `api:` | Function export for AI/tests | App runtime |
| `derivation:` | Node-only transforms | Node only |
| `page:` | In-page navigation + change subscriptions | App runtime |
| `timer:` | Timers and intervals | All |
| `binding:` | Reactive binding state for surgical updates | App runtime |
| `emoji:` | Emoji lookup by shortcode | App runtime |

---

## `scribe:` Unified API

The `scribe:` module provides identity, layer access, declarative bindings, and ephemeral messaging.

**See [SCRIBE_API.md](SCRIBE_API.md) for complete documentation.**

### Quick Reference

```lua
-- Identity
local page_id = scribe:page_id()
local my_did = scribe:my_did()
local my_name = scribe:my_name()

-- Layers
local products = scribe:list(page_id .. "/products")
local settings = scribe:map(page_id .. "/settings")
local order_layers = scribe:list_layers(page_id .. "/orders/*")

-- Declarative bindings (auto-sync layer → UI)
-- NOTE: Sorting is done in Slint UI, not here (enables surgical updates)
scribe:bind("products", "products", {
    key = "id",  -- Stable identity for surgical updates
    transform = function(p) return { id = p.id, name = p.name } end
})

-- Per-user layer with {me} placeholder
scribe:bind("my_orders", "orders/{me}")

-- Wildcard aggregation
scribe:bind("all_orders", "orders/*")

-- Ephemeral messages
scribe:send("cursor", { x = 100, y = 200 })
```

### Layer Operations

```lua
-- List layer (0-based indices)
local list = scribe:list(page_id .. "/items")
list:push(value)           -- Append
list:get(index)            -- Get by index
list:set(index, value)     -- Update
list:delete(index)         -- Remove
list:length()              -- Count

-- Map layer
local map = scribe:map(page_id .. "/settings")
map:set("key", value)      -- Set
map:get("key")             -- Get
map:delete("key")          -- Remove
map:keys()                 -- All keys
map:length()               -- Count
```

---

## `permit:` Role and Capabilities

```lua
local role = permit:role()             -- App-defined role (owner/customer/etc)

-- Check write capability
if permit:can_write("layer_name") then
    -- allowed to modify this layer
end
```

---

## `ui:` UI Binding

```lua
-- Set property (syncs to Slint AppAPI)
ui:set("property_name", value)
-- Setting an array creates a VecModel automatically (replaces all items)
ui:set("items", items_array)

-- Get property
local value = ui:get("property_name")

-- Surgical array operations (efficient, preserves scroll/selection)
ui:push("array_name", item)             -- Append to end
ui:update("array_name", index, item)    -- Update at index (0-based)
ui:insert("array_name", index, item)    -- Insert at index
ui:remove("array_name", index)          -- Remove at index
ui:clear("array_name")                  -- Remove all items
```

**Note:** Use `ui:push/update/remove` for surgical updates that preserve UI state (scroll position, animations). Use `ui:set` with an array for full replacement.

---

## `api:` Function Export

Export functions for AI agents and integration tests to call.

```lua
-- Export a function
api.export("add_product", function(name, price)
    -- Use same code path as human UI interaction
    on_field_changed("product_name", name)
    on_field_changed("product_price", tostring(price))
    on_modal_action("add_product", "submit")
end)

-- Add description metadata (optional, for AI discovery)
api.describe("add_product", {
    description = "Add product via UI flow",
    params = {
        { name = "name", type = "string" },
        { name = "price", type = "number" },
    },
    effects = { "products layer updated" },
    syncs = { "products syncs to peers" }
})

-- Call an exported function
api.call("add_product", "Widget", 99)

-- List all exported functions
local functions = api.list()
```

---

## `butler:` System Functions

```lua
-- Send ephemeral message (fire-and-forget, not persisted, QUIC datagram)
butler:send_ephemeral('{"type":"cursor","x":100,"y":200}')

-- Get asset URL
local url = butler:asset_url(asset_id)
```

---

## `derivation:` Node-Only Transforms

See [DERIVATION.md](DERIVATION.md) for full details.

```lua
-- Register a derivation rule (in init.lua, runs on node)
derivation:register({
    source = page_id .. "/orders/*",
    target = page_id .. "/derived/orders_summary",
    key_fn = function(order) return order.id end,
    transform = function(source_layer_name, order)
        return { id = order.id, status = order.status, total = order.total }
    end,
    filter = function(order)
        return order.status ~= "draft"
    end
})

-- Force rebuild of a derived layer
derivation:rebuild(page_id .. "/derived/orders_summary")

-- Rebuild all derived layers
derivation:rebuild_all()

-- Handle source changes (in node.lua)
derivation:on_source_change(layer_name)
```

---

## `page:` Navigation

### In-Page Navigation

Navigate to a sibling app within the same page. Apps can only link to other apps in the same page — the page is the boundary.

```lua
-- Navigate to another app by manifest name
page:open_app("Protocol Docs")

-- Example: CTA button navigates to docs app
function on_click(target)
    if target == "cta:Read the Docs" then
        page:open_app("Protocol Docs")
    end
end
```

The app name must match the `"name"` field in the target app's `manifest.json`. Navigation triggers the same flow as clicking a tab in the shell — the current app shuts down and the target app launches in the same window.

**Note:** `page:open_app()` is a no-op in headless/node mode (no UI to navigate).

## `binding:` Reactive Binding State

Advanced module for manual surgical UI updates. Use this when you need fine-grained control over how ops are processed.

```lua
local BindingState = binding.BindingState

-- Create binding state for a UI model
local state = BindingState.new("online_users", "did")

-- Pair with on_layer_discovered() or a timer-based refresh for manual flows
timer.setInterval(1000, function()
    local items = scribe:list("presence")
    state:init_from_data(items, function(entry)
        return { name = entry.name, status = "online" }
    end)
end)
```

### BindingState Methods

| Method | Description |
|--------|-------------|
| `BindingState.new(model_name, key_field)` | Create new state (key_field default: "id") |
| `state:process_ops(ops, transform)` | Apply ops surgically to UI |
| `state:init_from_data(full_data, transform)` | Initialize from full data (replace all) |
| `state:count()` | Get current item count |
| `state:has(key)` | Check if key exists |
| `state:get(key)` | Get item by key |
| `state:values()` | Get all items as array |
| `state:remove_by_key(key)` | Remove item by key |

**Note:** Most apps should use `scribe:bind()` with the `key` option instead of manual BindingState management. This module is for advanced scenarios.

---

## `timer:` Timers and Intervals

Timers work in both client (app) and node contexts.

```lua
-- Recurring timer (game loops, heartbeats)
local interval_id = timer.setInterval(function()
    -- runs every 50ms
end, 50)

-- One-shot timer
local timeout_id = timer.setTimeout(function()
    -- runs once after 1000ms
end, 1000)

-- Cancel timers
timer.clearInterval(interval_id)
timer.clearTimeout(timeout_id)
```

**Common use cases:**
- Presence heartbeat: `timer.setInterval(send_heartbeat, 15000)`
- Delayed actions: `timer.setTimeout(action, 1000)`
- Periodic UI updates: `timer.setInterval(refresh_status, 1000)`

**Note:** For game loops (~60fps), use the raylib renderer with `update(dt)` + `draw()` callbacks. See [RENDERERS.md](RENDERERS.md).

---

## `emoji:` Emoji Lookup

```lua
local smile = emoji:get("smile")       -- Returns "😄" or nil
local name = emoji:name("😄")          -- Returns "grinning face with smiling eyes"
local results = emoji:search("heart")  -- Returns up to 10 matches
-- Each result: { emoji = "❤️", name = "red heart", shortcode = "heart" }
```

---

## Runtime Helpers

### `require_role(role)` / `has_role(role)`

Permission guards.

```lua
function on_delete_product()
    if permit:role() ~= "owner" then
        log_warn("Only owners can delete products")
        return
    end
    -- ... delete logic
end
```

---

## Event Handlers

Full list of callbacks the runtime invokes:

```lua
function on_init()
    -- App startup: initialize layers, set up scribe:bind() declarations
    page_id = scribe:page_id()
    scribe:bind("products", "products")  -- Declarative sync
end

function on_layer_discovered(layer_name)
    -- New layer found (e.g., customer's order layer appeared)
    -- NOTE: Wildcard bindings (orders/*) auto-include new layers
    --
    -- IMPORTANT: This is called in two situations:
    --   1. Live: a new layer arrives via CRDT sync from a peer
    --   2. Startup replay: after on_init, the runtime replays ALL existing
    --      non-protocol local layers through this callback automatically
    --
    -- This means apps do NOT need to manually query/rebuild dynamic layer
    -- indexes (e.g., DM lists, custom channels) on startup. Just implement
    -- on_layer_discovered and it handles both new and pre-existing layers.
    --
    -- Apps should be idempotent — guard against processing the same layer twice:
    --   if known_layers[layer_name] then return end
end

function on_click(target)
    -- UI button/toucharea clicked
end

function on_field_changed(field_name, value)
    -- Form input changed
end

function on_modal_action(modal_name, action)
    -- Modal open/close/submit
end

function on_ephemeral(user_did, payload)
    -- Real-time event (cursor, typing indicator)
end

function on_key_pressed(key)
    -- Keyboard input (requires FocusScope in Slint)
end

function on_peer_joined(user_did)
    -- Peer connected
end

function on_peer_left(user_did)
    -- Peer disconnected
end

function update(dt)
    -- Raylib game update loop (~target_fps)
end

function draw()
    -- Raylib draw callback
end

function on_datagram(data)
    -- Datagram received (fire-and-forget multiplayer data)
end

-- Node-only:
function validate_ops(layer_name, ops, from_did, role, page_id)
    -- Validation before sync (runs on node)
    return true, nil  -- or false, "error message"
end
```

---

## Gotchas

- **Layer indices are 0-based** -- `list:get(0)` is the first item, despite Lua convention of 1-based indexing
- **Layer names must be prefixed with page_id**: `page_id .. "/items"`, not just `"items"`
- **`ui:set` with an array creates a VecModel automatically** -- no need to pre-declare in manifest `models`
- **Use `scribe:bind()` instead of manual refresh** -- eliminates most `refresh_*_ui` boilerplate
- **`{me}` placeholder in bind patterns** -- expands to user's DID at bind time
- **`on_datagram`** receives a Lua table (already decoded), not a raw string
