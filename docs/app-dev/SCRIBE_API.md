# Scribe API Reference

The `scribe:` module provides unified access to identity, layers, bindings, and communication in osvauld apps.

## Quick Reference

| Method | Purpose |
|--------|---------|
| `scribe:page_id()` | Get current page ID |
| `scribe:my_did()` | Get user's DID |
| `scribe:my_name()` | Get user's display name |
| `scribe:list(name)` | Get or create a list layer |
| `scribe:map(name)` | Get or create a map layer |
| `scribe:list_layers(pattern)` | List layers matching pattern |
| `scribe:bind(ui_prop, layer, opts)` | Declarative layer → UI sync |
| `scribe:rebind(ui_prop, layer)` | Switch binding to different layer |
| `scribe:send(func, args)` | Send ephemeral message |

---

## Identity

```lua
local page_id = scribe:page_id()   -- Current page ID
local my_did = scribe:my_did()     -- User's DID (did:key:...)
local my_name = scribe:my_name()   -- User's display name (or nil)
```

---

## Layers

### Getting Layers

```lua
-- Get or create a list layer (LoroList)
local products = scribe:list(page_id .. "/products")

-- Get or create a map layer (LoroMap)
local settings = scribe:map(page_id .. "/settings")

-- List layers matching a pattern
local order_layers = scribe:list_layers(page_id .. "/orders/*")
-- Returns: {"page123/orders/did:key:alice", "page123/orders/did:key:bob", ...}
```

### List Layer Operations

```lua
local items = scribe:list(page_id .. "/items")

items:push(value)          -- Append to end
items:get(index)           -- Get by index (0-BASED)
items:set(index, value)    -- Update at index
items:delete(index)        -- Remove at index
items:length()             -- Count of items
```

### Map Layer Operations

```lua
local settings = scribe:map(page_id .. "/settings")

settings:set("key", value)   -- Set value (table, string, number, bool)
settings:get("key")          -- Get value (nil if not found)
settings:delete("key")       -- Remove key
settings:keys()              -- Get all keys as table
settings:length()            -- Count of keys
```

### Layer Naming Conventions

```
{page_id}/products           -- shared data (all users see same data)
{page_id}/orders/{user_did}  -- per-user partitioned
{page_id}/derived/*          -- computed by node (map)
{page_id}/drafts             -- local-only (sync: false)
{page_id}/assets             -- file metadata
app:App Name                 -- app state layer
```

---

## Declarative Bindings

The `scribe:bind()` method creates automatic layer → UI synchronization, eliminating manual `on_loro_change` handlers and `refresh_*_ui` functions.

### Basic Usage

```lua
function on_init()
    page_id = scribe:page_id()

    -- Simple binding: layer data syncs directly to UI property
    scribe:bind("products", "products")

    -- With transform: shape data before sending to UI
    scribe:bind("products", "products", {
        transform = function(item)
            return {
                id = item.id,
                name = item.name,
                price = "$" .. item.price
            }
        end
    })
end
```

### Options

| Option | Type | Description |
|--------|------|-------------|
| `transform` | function(item) | Transform each item before UI sync |
| `key` | string | Field to use as stable identity for surgical updates (e.g., "id", "did") |
| `max_items` | number | Cap UI model size (keeps latest N items, drops oldest) |

**Note**: Sorting is NOT done in bindings — it's a Slint view concern. This enables surgical updates with stable indices. Sort your data in Slint using expressions or SortModel.

### Surgical Updates with `key`

The `key` option enables fine-grained UI updates. Instead of replacing the entire model on every change, only affected items are updated via Set/Insert/Remove ops:

**For Map layers** (e.g., `derived/orders_summary`): The `key` option is **required** for reactive UI updates. Map deltas translate directly to surgical VecModel ops using an internal key→index cache — no full Replace ever needed.

**For List layers** (e.g., `messages`): List deltas (Retain/Insert/Delete) are converted to surgical ops automatically without needing a `key` option.

```lua
-- Map layer: key is REQUIRED for reactive updates
scribe:bind("orders", "derived/orders_summary", {
    key = "id",  -- Map key → array index tracking
    transform = function(item)
        return {
            id = item.id,
            customer = item.customer_name,
            total = string.format("$%.2f", item.total),
            status = item.status
        }
    end
})

-- List layer: surgical updates work without key
scribe:bind("messages", "messages", {
    transform = function(msg)
        return { id = msg.id, text = msg.text, sender = msg.sender_name }
    end
})

-- Map layer with key: presence tracking
scribe:bind("online_users", "presence", {
    key = "did",  -- Use "did" field as stable identity
    transform = function(entry)
        return { name = entry.name, status = entry.status }
    end
})

-- Leaderboard: data stays unsorted, Slint sorts for display
scribe:bind("leaderboard", "scores", {
    key = "player",
    transform = function(entry)
        return { player = entry.player, score = entry.score }
    end
})
-- In Slint: sort the leaderboard model by score for display
```

**Benefits:**
- Only changed rows update (no full list re-render)
- Scroll position preserved when items change
- Animations work smoothly (individual row changes)
- Better performance for large lists
- Indices remain stable (Slint handles sort as view concern)
- Map layers with `key` get Set/Insert/Remove ops directly from Map deltas

### Capping Model Size with `max_items`

For layers that grow unboundedly (e.g., chat messages), use `max_items` to cap the UI model size. The binding keeps the latest N items and drops the oldest:

```lua
-- Keep last 200 messages in the UI model
-- The full history remains in the Loro document
scribe:bind("messages", "messages", {
    max_items = 200,
    transform = function(msg)
        return { id = msg.id, text = msg.text, sender = msg.sender_name }
    end
})
```

This works with both the delta path (generates Remove ops for overflow) and the Replace fallback (trims the array before sending to UI).

### Dynamic Rebinding with `rebind()`

Use `scribe:rebind()` to switch which layer backs an existing binding at runtime. The transform, key, and max_items options are preserved — only the backing layer changes. Caches are cleared and a full initial sync is performed from the new layer.

```lua
-- Initial bind to "general" channel messages
scribe:bind("messages", "channels/general/messages", {
    key = "id",
    transform = function(msg)
        if msg.thread_parent_id and msg.thread_parent_id ~= "" then
            return nil  -- filter thread replies
        end
        return format_message(msg)
    end
})

-- Later, when user switches channels:
function on_channel_switch(channel_id)
    scribe:rebind("messages", "channels/" .. channel_id .. "/messages")
    -- UI gets a Replace with new channel's data
    -- Future deltas from the new channel are surgical (Insert/Set/Remove)
end
```

**Behavior:**
- Transform/key/max_items from the original `bind()` are preserved
- Internal key→index cache is cleared (new layer has different keys)
- A full Replace is sent to UI with the new layer's current data
- Subsequent changes to the new layer produce surgical delta updates
- Errors if `ui_property` was never bound (call `bind()` first)
- No-op in headless mode (`ui_enabled: false`)

### {me} Placeholder

The `{me}` placeholder expands to the current user's DID, useful for per-user layers:

```lua
-- Expands to: {page_id}/orders/{my_did}
scribe:bind("my_orders", "orders/{me}")
```

### Wildcard Patterns

Wildcards aggregate multiple layers into a single UI property:

```lua
-- Collects all customer order layers into one array
scribe:bind("all_orders", "orders/*", {
    transform = function(layer_name, item)
        local customer_did = layer_name:match("orders/(.+)$")
        return {
            customer = customer_did,
            order_id = item.id,
            total = item.total
        }
    end
})
```

### Full Example

```lua
function on_init()
    page_id = scribe:page_id()
    my_did = scribe:my_did()

    -- Products list with formatting
    scribe:bind("products", "products", {
        transform = function(p)
            return {
                id = p.id,
                name = p.name,
                price = string.format("$%.2f", p.price),
                in_stock = p.stock > 0
            }
        end
    })

    -- Leaderboard (sort in Slint, not here — enables surgical updates)
    scribe:bind("leaderboard", "scores", {
        transform = function(entry)
            return {
                player = entry.player or "???",
                score = entry.score or 0,
                date = entry.date or ""
            }
        end
    })

    -- My orders only
    scribe:bind("my_orders", "orders/{me}")
end
```

---

## Ephemeral Messages

Send real-time messages that are not persisted (cursors, typing indicators, game state).

### Structured Ephemerals

```lua
-- Send structured ephemeral
scribe:send("player_position", {
    x = 100,
    y = 200,
    direction = "up"
})

-- Receive in on_ephemeral callback
function on_ephemeral(from_did, func, args)
    if func == "player_position" then
        remote_players[from_did] = {
            x = args.x,
            y = args.y,
            direction = args.direction
        }
    end
end
```

---

## Migration from loro:/permit:

The `scribe:` module replaces the separate `loro:` and `permit:` modules:

| Old API | New API |
|---------|---------|
| `permit:page_id()` | `scribe:page_id()` |
| `permit:my_did()` | `scribe:my_did()` |
| `loro:get_or_create_layer(name, "list")` | `scribe:list(name)` |
| `loro:get_or_create_layer(name, "map")` | `scribe:map(name)` |
| `loro:list_layers(pattern)` | `scribe:list_layers(pattern)` |
| Manual `on_loro_change` + refresh | `scribe:bind()` |
| `butler:send_ephemeral(json)` | `scribe:send(func, args)` |

### Before (manual layer tracking)

```lua
local products_layer = nil
local order_layers = {}

function on_init()
    page_id = permit:page_id()
    my_did = permit:my_did()
    products_layer = loro:get_or_create_layer(page_id .. "/products", "list")
    refresh_products_ui()
end

function on_loro_change(layer_name, change_type)
    if layer_name:match("/products$") then
        refresh_products_ui()
    elseif layer_name:match("/orders/") then
        refresh_orders_ui()
    end
end

function on_layer_discovered(layer_name)
    if layer_name:match("/orders/") then
        local order_layer = loro:get_or_create_layer(layer_name, "list")
        order_layers[layer_name] = order_layer
        refresh_orders_ui()
    end
end

function refresh_products_ui()
    local items = {}
    local len = products_layer:length()
    for i = 0, len - 1 do
        local p = products_layer:get(i)
        table.insert(items, { id = p.id, name = p.name })
    end
    ui:set("products", items)
end
```

### After (declarative bindings)

```lua
function on_init()
    page_id = scribe:page_id()
    my_did = scribe:my_did()

    -- Declarative: products auto-sync to UI
    scribe:bind("products", "products", {
        transform = function(p)
            return { id = p.id, name = p.name }
        end
    })

    -- Wildcard: all order layers aggregated
    scribe:bind("all_orders", "orders/*")
end
-- No on_loro_change needed!
-- No on_layer_discovered needed!
-- No refresh_*_ui functions needed!
```

---

## Gotchas

- **Layer indices are 0-based**: `list:get(0)` is the first item
- **Layer names auto-prefix**: `scribe:bind("x", "products")` uses `{page_id}/products`
- **{me} expands at bind time**: Value is resolved when `bind()` is called
- **Wildcards match existing layers**: New layers are auto-included automatically
- **Transform receives full item**: Modify what's needed, return new table for UI
- **Data arrives unwrapped**: Layer content is extracted from Loro document structure (no need to dig into `{"root": [...]}`)
- **Wildcard transforms get layer_name**: `function(layer_name, item)` for wildcards vs `function(item)` for simple bindings

---

## When to Use Manual Handling

Most apps should use `scribe:bind()` exclusively. However, some advanced scenarios require manual handling:

| Scenario | Approach |
|----------|----------|
| Complex internal state (e.g., graph structure) | Use `on_layer_discovered` + periodic reload |
| Write access to dynamic per-user layers | Track layer references in a table |
| Custom aggregation beyond simple transforms | Combine layers programmatically |

For these cases, reload directly from layers instead of relying on change-op callbacks.

### Reactive Binding State

For advanced use cases, use the `binding` module for custom keyed state management:

```lua
local BindingState = binding.BindingState

-- Create binding state for a model
local presence_state = BindingState.new("online_users", "did")

timer.setInterval(1000, function()
    local snapshot = scribe:list("presence")
    presence_state:init_from_data(snapshot, function(entry)
        return { name = entry.name, status = "online" }
    end)
end)
```

**BindingState Methods:**
- `BindingState.new(model_name, key_field)` - Create new state
- `state:init_from_data(full_data, transform)` - Initialize/replace from full data
- `state:count()` - Get item count
- `state:has(key)` - Check if key exists
- `state:get(key)` - Get item by key
