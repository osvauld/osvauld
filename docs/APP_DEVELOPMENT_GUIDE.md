# App Development Guide

Guide for building multi-user apps with sync, derivation, and validation.

## Quick Start

### 1. Setup Test Databases

```bash
# Create setup binary (see integration_tests/src/bin/setup_test_dbs.rs for template)
cargo build -p integration_tests --bin setup_booking_db

# Run setup - creates owner, customer, and node databases
./target/debug/setup_booking_db --db-dir /tmp/sthalam/myapp
```

### 2. Start AI Interface

```bash
# Headless mode (for automated testing)
./target/debug/ai_interface --name myapp --instances owner,customer --node

# With UI (for manual testing)
./target/debug/ai_interface --name myapp --instances owner,customer --node --show-ui
```

### 3. Interact via Debug Client

```bash
# Login
./scripts/dc /tmp/sthalam/myapp/owner.sock login test123

# List spaces/pages
./scripts/dc /tmp/sthalam/myapp/owner.sock spaces
./scripts/dc /tmp/sthalam/myapp/owner.sock pages "<space_id>"

# Open app
./scripts/dc /tmp/sthalam/myapp/owner.sock open_app "<page_id>" "My App"

# Evaluate Lua
./scripts/dc /tmp/sthalam/myapp/owner.sock eval 'return get_items()'
```

---

## App Structure

```
sample_apps/my-app/
├── space_permit_template.json    # Space-level permissions
├── permit_template.json          # Page-level layer permissions
├── shared/
│   ├── manifest.json             # Required: {"name": "shared", ...}
│   ├── init.lua                  # Derivation rules (runs on node)
│   └── validation.lua            # Validation logic (runs on node)
├── owner-app/
│   ├── manifest.json             # {"name": "Owner App", "entry": "app.lua"}
│   ├── app.lua                   # Owner-side logic
│   └── app.slint                 # Owner UI
└── customer-app/
    ├── manifest.json
    ├── app.lua                   # Customer-side logic
    └── app.slint
```

---

## Layer Naming Convention

Layers follow the pattern: `{page_id}/{layer_path}`

| Layer Type | Pattern | Example |
|------------|---------|---------|
| Shared data | `{page_id}/items` | `abc123/products` |
| Per-user data | `{page_id}/items/{user_did}` | `abc123/orders/did:key:xyz` |
| Derived data | `{page_id}/derived/{name}` | `abc123/derived/calendar` |
| App code | `app:{app_name}` | `app:shared`, `app:Owner App` |

---

## Permit Templates

### space_permit_template.json

```json
{
  "relay": true,
  "patterns": [
    {
      "pattern": "{page_id}/**",
      "sync": true
    }
  ]
}
```

- `relay: true` - Enables node mode (derivation runs on node)

### permit_template.json

```json
{
  "owner": {
    "patterns": [
      {"pattern": "{page_id}/products", "sync": true, "write": true},
      {"pattern": "{page_id}/orders/*", "sync": true, "write": true}
    ]
  },
  "viewer": {
    "patterns": [
      {"pattern": "{page_id}/products", "sync": true},
      {"pattern": "{page_id}/orders/{aud}", "sync": true, "write": true, "create": true},
      {"pattern": "{page_id}/derived/*", "sync": true}
    ]
  },
  "node": {
    "patterns": [
      {"pattern": "{page_id}/**", "sync": true, "create": true}
    ]
  }
}
```

- `{aud}` - Replaced with viewer's DID (restricts to own data)
- `write` - Can modify existing entries
- `create` - Can create new layers matching pattern
- `sync` - Layer syncs to this role

---

## Derivation (shared/init.lua)

Derivation runs **only on the node** (`is_node=true`). It transforms source layers into derived layers.

```lua
local page_id = permit:page_id()

derivation:register({
    -- Source: pattern with * for per-user layers
    source = page_id .. "/orders/*",

    -- Target: derived layer (node writes, others read)
    target = page_id .. "/derived/summary",

    -- Key function: unique key for each derived entry
    key_fn = function(order)
        return order.date .. "_" .. order.id
    end,

    -- Transform: strip sensitive data for privacy
    transform = function(source_layer, order)
        return {
            date = order.date,
            total = order.total,
            status = order.status
            -- customer details stripped for privacy
        }
    end,

    -- Filter: only include matching entries
    filter = function(order)
        return order.status == "confirmed"
    end
})
```

### Derivation Quirks

1. **Filter returns false** - Entry is excluded from derived layer (not an error)
2. **Multiple sources to same target** - All matching sources merge into target
3. **Derivation timing** - Runs when source layer changes are applied on node

---

## Validation (shared/validation.lua)

Validation runs **on the node** before applying incoming updates.

```lua
function validate_ops(layer_name, ops, from_did, role, page_id)
    -- Return: valid (boolean), error_message (string or nil)

    for _, op in ipairs(ops) do
        local op_type = op.op      -- "insert", "update", "delete", "set"
        local path = op.path       -- "[0]", "[0].status", "key", etc.
        local value = op.value     -- New value
        local old_value = op.old_value  -- Previous value (for updates)
        local index = op.index     -- Array index (if applicable)
        local key = op.key         -- Map key (if applicable)
    end

    return true, nil
end
```

### Critical Quirk: Op Path Structure

The diff system generates **field-level ops**, not object-level ops!

When updating an object (e.g., changing status from "pending" to "confirmed"):

```lua
-- WRONG assumption: op.value is the full object
if op.op == "insert" then
    local item = op.value
    if not item.date then  -- FAILS! op.value might be "confirmed" (just the field)
        return false, "Must have date"
    end
end

-- CORRECT: Check if it's an array-level insert vs field-level insert
local function is_array_level_op(op)
    local path = op.path or ""
    -- Array-level: path ends with "[N]" (e.g., "[0]", "[1]")
    -- Field-level: path has more after bracket (e.g., "[0].status")
    return path:match("%[%d+%]$") ~= nil
end

if op.op == "insert" and is_array_level_op(op) then
    -- This is a new array item - op.value is the full object
    local item = op.value
    if not item.date then
        return false, "Must have date"
    end
end
-- Field-level inserts (adding new fields to existing items) are allowed
```

### Op Types

| op.op | op.path | op.value | When |
|-------|---------|----------|------|
| `insert` | `[0]` | Full object | New array item |
| `insert` | `[0].newField` | Field value | New field added to existing item |
| `update` | `[0]` | Full object | Array item replaced |
| `update` | `key` | New value | Map value changed |
| `delete` | `[0]` | nil | Array item removed |
| `set` | `path` | New value | Value replaced |

---

## App Lua API

### Core Objects

```lua
-- Permit info
permit:page_id()        -- Current page ID
permit:role()           -- "owner", "customer", "viewer", "node"
permit:my_layer(name)   -- "{page_id}/{name}/{my_did}" for per-user layers

-- Layer operations (Loro CRDT)
local layer = loro:get_or_create_layer(name, type)  -- "map" or "list"
local layer = loro:get_layer(name, type)            -- nil if doesn't exist
local layers = loro:list_layers(pattern)            -- Pattern matching

-- Map operations
layer:get(key)
layer:set(key, value)
layer:delete(key)
layer:keys()

-- List operations
layer:get(index)        -- 0-indexed
layer:set(index, value)
layer:push(value)
layer:delete(index)
layer:length()

-- UI binding
ui:set(property, value)
ui:get(property)
```

### Lifecycle Callbacks

```lua
function on_init()
    -- Called when app loads, initialize layers and UI
end

function on_loro_change(layer_name, change_type)
    -- Called when any layer changes (local or synced)
end

function on_layer_discovered(layer_name)
    -- Called when a new layer matching subscribed pattern appears
end

function on_field_changed(field_name, value)
    -- Called when UI input changes (Event Bus pattern)
end

function on_click(target)
    -- Called on button clicks (Event Bus pattern)
end

function on_modal_action(modal_name, action)
    -- Called for modal open/close/submit
end
```

### Test Helper Pattern

Always expose test functions for automation:

```lua
-- For automated testing via dc eval
function get_items()
    local result = {}
    -- ... collect data
    return result
end

function create_item_via_ui(name, price)
    on_field_changed("item_name", name)
    on_field_changed("item_price", price)
    on_modal_action("create", "submit")
    return true
end
```

---

## Testing Flow

### Automated Test Script Pattern

```python
#!/usr/bin/env python3
import socket, json, time

AI_SOCKET = "/tmp/sthalam/myapp/ai.sock"

def call(target, action, params=None):
    """Call AI interface."""
    s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    s.connect(AI_SOCKET)
    cmd = {"target": target, "action": action, "id": 1}
    if params:
        cmd["params"] = params
    s.send((json.dumps(cmd) + '\n').encode())
    return json.loads(s.recv(65536).decode())

def lua_eval(target, code):
    """Evaluate Lua in target instance."""
    result = call(target, "eval", {"code": code})
    return result.get("result", {}).get("result")

# Example flow
call("owner", "login", {"passphrase": "test123"})
call("owner", "open_app", {"page_id": "...", "app_name": "Owner App"})
time.sleep(2)  # Wait for app to load

# Create item
lua_eval("owner", 'return create_item_via_ui("Test", 100)')
time.sleep(3)  # Wait for sync

# Verify on customer
call("customer", "login", {"passphrase": "test123"})
items = lua_eval("customer", 'return get_items()')
assert len(items) > 0
```

---

## Common Issues

### 1. "Booking must have a date" on status updates

**Cause**: Validation treating field-level inserts as new items.
**Fix**: Use `is_array_level_op()` check (see Validation Quirks above).

### 2. Derived layer empty after changes

**Cause**: Filter function returning false, or derivation not triggered.
**Debug**: Check kunki logs for `Running ops-based derivation` and `Entry filtered out`.

### 3. Changes not syncing

**Cause**: Permit patterns don't include the layer, or missing `sync: true`.
**Debug**: Check permit_template.json patterns match layer names.

### 4. Validation rejected - wrong role

**Cause**: Owner trying to write to customer-only layer or vice versa.
**Debug**: Check `role` parameter in `validate_ops` and permit patterns.

### 5. App functions not available in eval

**Cause**: App not fully loaded yet.
**Fix**: Add `time.sleep(2)` after `open_app` before calling eval.

---

## UI Form Development

### Form State Pattern

When building UI forms that can be controlled both by UI interactions and API functions, use a form state pattern:

```lua
local form_state = {
    -- Schedule editing
    edit_day = "",
    edit_enabled = true,
    edit_start = "09:00",
    edit_end = "17:00",
    edit_slot_duration = 60,
    -- Block time
    block_date = "",
    block_all_day = true,
    block_start = "",
    block_end = "",
    block_reason = "",
}
```

### Type Conversions

UI fields come as strings. Handle type conversions in `on_field_changed`:

```lua
function on_field_changed(field_name, value)
    if field_name == "edit_enabled" or field_name == "block_all_day" then
        -- Convert to boolean
        if type(value) == "string" then
            form_state[field_name] = (value == "true" or value == "1")
        else
            form_state[field_name] = value
        end
    elseif field_name == "edit_slot_duration" then
        -- Convert to number
        form_state[field_name] = tonumber(value) or 60
    else
        form_state[field_name] = value
    end
end
```

### Legacy Field Mapping

When adding new form fields, map legacy names for backwards compatibility:

```lua
elseif field_name == "schedule_start" then
    -- Legacy: also set new field name
    form_state.schedule_start = value
    form_state.edit_start = value
```

### Conditional Form Logic

For fields like "all day" that hide/show other fields:

```lua
-- In block_time_via_ui
local is_all_day = (start_time == nil or start_time == "") and (end_time == nil or end_time == "")
on_field_changed("block_all_day", is_all_day and "true" or "false")
```

### App Refresh vs Reopen

**Important**: `refresh_app` updates the Lua code but may not reload it immediately. For reliable code updates during development:

```python
# Refresh app (updates files, triggers reload)
call("provider", "refresh_app", {
    "app_name": "Service Provider",
    "app_dir": "/path/to/app"
})

# For guaranteed fresh code, reopen the app
call("provider", "open_app", {
    "page_id": page_id,
    "app_name": "Service Provider"
})
time.sleep(3)  # Wait for app to load
```

---

## Debug Commands

```bash
# Check tmux session
tmux attach -t myapp

# View specific pane logs
tmux capture-pane -t myapp:kunki -p -S -100 | tail -50

# Check for validation errors
tmux capture-pane -t myapp:kunki -p | grep -i "validation"

# Check for derivation
tmux capture-pane -t myapp:kunki -p | grep -i "derivation"

# List all sockets
ls -la /tmp/sthalam/myapp/*.sock

# Direct ping
./scripts/dc /tmp/sthalam/myapp/owner.sock ping
```

---

## Setup Script Template

See `integration_tests/src/bin/setup_test_dbs.rs` for a complete example. Key steps:

1. Create owner database with identity
2. Create space from template directory
3. Import page with apps
4. Create node database
5. Connect owner to node, publish space
6. Create customer database
7. Connect customer via viewer link

The setup script pre-configures all sync relationships so the app is ready to test immediately.
