# Derivation

Computed views from source layers, running on the node.

## Dynamic Layers (Background)

Before understanding derivation, you need to understand **dynamic layers** -- the problem derivation solves.

Osvauld has two kinds of layers:

### Static Layers

Fixed layers that exist for every page. Defined with concrete names in the permit template:

```json
"layers": {
    "{page_id}/products": { "type": "list", "sync": true, "write": true }
}
```

After `{page_id}` expansion, the full layer name is known at startup (e.g., `shop123/products`). The scribe pre-creates these layers automatically.

### Dynamic Layers

Per-user layers created on demand at runtime. Defined with **patterns** using `{aud}` and `*` wildcards:

```json
"layer_patterns": {
    "{page_id}/orders/{aud}": { "sync": true, "write": true }
}
```

`{aud}` expands to the **audience DID** -- the user receiving the permit. So customer Alice gets write access to `shop123/orders/did:key:alice`, and customer Bob gets `shop123/orders/did:key:bob`. Each customer can only write to their own layer.

**How dynamic layers get created:**

1. **From Lua**: When a customer calls `loro:get_or_create_layer(page_id .. "/orders/" .. my_did, "list")`, the scribe creates the layer on demand after checking the permit allows it.

2. **From peer sync**: When the node receives a sync update for a layer that doesn't exist yet (e.g., a new customer's orders), it creates the layer if the permit's `layer_patterns` match and `create: true` is set.

**Permit setup for dynamic layers:**

```json
// Node role: can create and sync any orders layer
"layer_patterns": {
    "{page_id}/orders/*": { "sync": true, "create": true }
}

// Customer role: can only write to their own
"layer_patterns": {
    "{page_id}/orders/{aud}": { "sync": true, "write": true }
}
```

The `*` wildcard matches any single path segment, so `{page_id}/orders/*` matches all customer order layers. The `create: true` permission lets the node create new dynamic layers when a new customer connects.

### The Problem Dynamic Layers Create

Dynamic layers give each user private data partitions -- but this creates a visibility challenge:

- The **owner** needs to see all orders across all customers
- The owner's permit has `{page_id}/orders/*` with `sync: true` -- they CAN see the raw data
- But with 100 customers, that's 100 separate layers to iterate and monitor

**Derivation solves this** by aggregating dynamic layers into a single summary layer.

## What is Derivation

Derivation creates **computed views** from dynamic per-user layers into shared summary layers. It runs on the node, which has access to all layers.

**Why it exists**: Rather than forcing the owner to iterate over every `orders/{did}` layer, the node aggregates them into a single `derived/orders_summary` map that the owner app can bind to directly.

**Use cases**:
- **Aggregation**: Combine all customer orders into an orders summary for the owner
- **Privacy**: Strip private details (customer name, phone) from a booking calendar so other customers only see "booked" slots
- **Computed state**: Calculate totals, counts, or status summaries across all dynamic layers

## Registration Pattern (init.lua)

Derivation rules are registered in `shared/init.lua`, which runs on the node during page initialization:

```lua
local page_id = permit:page_id()

derivation:register({
    -- Source pattern: all per-customer order layers
    source = page_id .. "/orders/*",

    -- Target: aggregated summary layer
    target = page_id .. "/derived/orders_summary",

    -- Key function: extract unique key for the derived map
    key_fn = function(order)
        return order.id
    end,

    -- Transform: create summary entry from full order
    transform = function(source_layer_name, order)
        return {
            id = order.id,
            customer = extract_did(source_layer_name),
            status = order.status,
            total = order.total,
            created_at = order.created_at,
        }
    end,

    -- Filter: only include submitted orders (not drafts)
    filter = function(order)
        return order.status ~= "draft"
    end,
})
```

### `derivation:register()` Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `source` | string | Yes | Source layer pattern (uses `*` wildcard) |
| `target` | string | Yes | Target derived layer name |
| `key_fn` | function | Yes | Extracts unique key from each entry |
| `transform` | function | Yes | `(source_layer_name, entry)` → transformed entry |
| `filter` | function | No | Return true to include, false to skip |

### Other Derivation APIs

```lua
-- Force rebuild of a specific derived layer
derivation:rebuild(page_id .. "/derived/orders_summary")

-- Rebuild all derived layers
derivation:rebuild_all()

-- Handle source changes (called from node.lua)
derivation:on_source_change(layer_name)
```

## Example: E-commerce Orders Summary

From `sample_apps/my-shop/shared/init.lua`:

```lua
local page_id = permit:page_id()

-- Helper: extract DID from layer name
local function extract_did(layer_name)
    return layer_name:match("/orders/(.+)$")
end

derivation:register({
    source = page_id .. "/orders/*",
    target = page_id .. "/derived/orders_summary",
    key_fn = function(order)
        return order.id
    end,
    transform = function(source_layer, order)
        return {
            id = order.id,
            customer = extract_did(source_layer),
            status = order.status,
            total = order.total,
            created_at = order.created_at,
        }
    end,
    filter = function(order)
        return order.status ~= "draft"
    end,
})
```

**What this does**: For every customer order layer matching `{page_id}/orders/*`, extract non-draft orders and write summaries (with customer DID, status, total) to the `derived/orders_summary` map layer. The owner can read this summary without seeing raw customer data.

## Example: Privacy-Preserving Calendar

From `sample_apps/my-booking/shared/init.lua`:

```lua
derivation:register({
    source = page_id .. "/bookings/*",
    target = page_id .. "/derived/calendar",
    key_fn = function(booking)
        return booking.date .. "_" .. booking.start_time
    end,
    transform = function(source_layer, booking)
        return {
            date = booking.date,
            start_time = booking.start_time,
            end_time = booking.end_time,
            booked = true
            -- PRIVACY: customer_name, phone, notes are STRIPPED
        }
    end,
    filter = function(booking)
        return booking.status == "confirmed"
    end,
})
```

**What this does**: Creates a public calendar showing time slots as "booked" without revealing who booked them. Customer names, phone numbers, and notes are stripped in the transform.

## Node Game Loop (node.lua)

For real-time multiplayer games, `node.lua` runs active logic instead of just derivation. The tank game uses `timer.setInterval` for a server-side game loop:

```lua
-- Game state
local game = { enemies = {}, stage = 1, frame_count = 0 }
local players = {}

-- Game step at ~30fps
timer.setInterval(function()
    game.frame_count = game.frame_count + 1

    -- Move enemies
    move_enemies()

    -- Check stage completion
    check_stage_complete()

    -- Broadcast enemy positions via ephemeral
    if game.frame_count % 3 == 0 then
        broadcast_enemies()
    end
end, 33)  -- ~30fps

-- Handle player inputs via ephemeral
function on_ephemeral(from_did, func, args)
    if func == "player" then
        players[from_did] = { x = args.x, y = args.y, direction = args.dir }
    elseif func == "bullet" then
        check_bullet_hit(args.x, args.y, args.owner)
    end
end

-- Broadcast to all connected peers
function broadcast_enemies()
    local data = {}
    for id, enemy in pairs(game.enemies) do
        table.insert(data, { id = id, x = enemy.x, y = enemy.y })
    end
    scribe:send("enemies", { data = data })
end
```

## init.lua vs node.lua

| File | Purpose | When it Runs |
|------|---------|-------------|
| `init.lua` | Register derivation rules | Once, at page initialization |
| `node.lua` | Active node logic (game loop, event processing) | Continuously on the node |

Both are specified in the shared manifest:
```json
{
    "name": "shared",
    "description": "Shared logic",
    "entry_node": "node.lua"
}
```

If you only need derivation, `init.lua` is sufficient. If you need active logic (game loop, real-time event processing), use `node.lua`.

## Policy Requirements

Derived layers must be configured in `app.osv` layer access rules:

- **Node role**: `write: true` (node writes the derived data)
- **Owner role**: `write: false` (owner can read but not write)
- **Customer role**: typically no access to derived layers

```osv
layer orders_summary as map {
  path "derived/orders_summary";
  namespace shared;
  allow node to read, write, sync;
  allow owner to read, sync;
}
```

## Gotchas

- **Source pattern uses `*` wildcard**: `page_id .. "/orders/*"` matches all order layers
- **Transform receives `(source_layer_name, entry)`** -- extract the DID from the layer name if needed (e.g., `layer_name:match("/orders/(.+)$")`)
- **Derived layers should grant write only to the node role** in `app.osv`
- **`filter` is optional** -- omit it to include all entries
- **`key_fn` must return a unique string** -- this becomes the key in the derived map layer
- **Derivation runs on the node only** -- clients never execute derivation logic
- **Force rebuild with `derivation:rebuild()`** if the derived layer gets out of sync
