# Validation Patterns

How to implement `validation.lua` for business rule enforcement.

## How Validation Works

Validation runs on the **node** before syncing remote updates to other peers:

```
Remote update arrives
  → Scribe extracts ops (Layer::extract_ops)
  → Scribe sends ValidationRequest to kunki (via ValidationHandle)
  → kunki's LuaRuntime calls validate_ops() in validation.lua
  → validation.lua returns true/false
  → Scribe applies or rejects the update
```

If `validation.lua` doesn't exist or doesn't define `validate_ops`, all remote updates are allowed by default.

## Function Signature

```lua
function validate_ops(layer_name, ops, from_did, role, page_id)
    -- layer_name: string - Layer being updated (e.g., "my-page/orders/did:key:alice")
    -- ops: table - Array of operations
    -- from_did: string - DID of the remote peer
    -- role: string - Role of the peer ("owner", "customer", "node", etc.)
    -- page_id: string - Page ID for context
    --
    -- Returns: (boolean, string|nil) - (valid, error_message)
    return true, nil
end
```

## Operation Structure

Each operation in the `ops` array (`Parivarta` serialized to Lua table):

```lua
{
    op = "insert" | "update" | "delete" | "set",
    path = "orders[0].status",   -- Path in the CRDT
    key = "status",              -- Map key (for map ops)
    index = 0,                   -- List index (for list ops)
    value = { ... },             -- New value
    old_value = { ... },         -- Previous value (may be nil for inserts)
}
```

## Patterns

### Dispatch by Layer

Route validation to layer-specific handlers:

```lua
function validate_ops(layer_name, ops, from_did, role, page_id)
    if layer_name == "products" or layer_name == page_id .. "/products" then
        return validate_products(ops, from_did, role)
    end

    if layer_name:match("/orders/") then
        return validate_orders(ops, from_did, role, layer_name)
    end

    if layer_name:match("/presence$") then
        return validate_presence(ops, from_did)
    end

    return true, nil  -- Default allow
end
```

### Role-Based Write Protection

Only the owner can modify certain layers:

```lua
function validate_products(ops, from_did, role)
    if role ~= "owner" and role ~= "admin" then
        return false, "Only owner or admin can modify products"
    end
    return true, nil
end
```

### State Machine Validation

Enforce valid state transitions per role:

```lua
local ORDER_STATES = {
    draft = {
        writable = {'items', 'quantity', 'notes', 'total'},
        customer_transitions = {'pending'},
        owner_transitions = {}
    },
    pending = {
        writable = {'notes'},
        customer_transitions = {'cancelled'},
        owner_transitions = {'confirmed', 'cancelled'}
    },
    confirmed = {
        writable = {},
        customer_transitions = {},
        owner_transitions = {'shipped'}
    },
    shipped = {
        writable = {},
        customer_transitions = {},
        owner_transitions = {'delivered'}
    },
    delivered = { writable = {}, customer_transitions = {}, owner_transitions = {} },
    cancelled = { writable = {}, customer_transitions = {}, owner_transitions = {} }
}

function validate_order_status_transition(old_status, new_status, role)
    local state = ORDER_STATES[old_status]
    if not state then
        return false, "Unknown state: " .. tostring(old_status)
    end

    local allowed
    if role == "customer" then
        allowed = state.customer_transitions
    elseif role == "owner" then
        allowed = state.owner_transitions
    else
        return false, "Role " .. role .. " cannot transition orders"
    end

    for _, s in ipairs(allowed) do
        if s == new_status then return true, nil end
    end

    return false, string.format(
        "Role '%s' cannot transition from '%s' to '%s'",
        role, old_status, new_status
    )
end
```

### Per-User Layer Enforcement

Ensure users can only modify their own layers:

```lua
function validate_orders(ops, from_did, role, layer_name)
    -- Extract the DID from the layer name: {page_id}/orders/{did}
    local layer_did = layer_name:match("/orders/(.+)$")

    -- Only the layer owner or the shop owner can modify
    if role == "customer" and layer_did ~= from_did then
        return false, "Customers can only modify their own orders"
    end

    -- Validate each operation
    for _, op in ipairs(ops) do
        if op.value and op.value.status then
            local old_status = (op.old_value and op.old_value.status) or "draft"
            local ok, err = validate_order_status_transition(
                old_status, op.value.status, role
            )
            if not ok then return false, err end
        end
    end

    return true, nil
end
```

### Presence Enforcement

Users can only update their own entry in the presence map:

```lua
function validate_presence(ops, from_did)
    for _, op in ipairs(ops) do
        -- Use the built-in presence_lib if available
        if presence_lib then
            local valid, err = presence_lib.validate_write(op, from_did)
            if not valid then return false, err end
        else
            -- Manual check: key must be the sender's DID
            if op.key and op.key ~= from_did then
                return false, "Can only update own presence entry"
            end
        end
    end
    return true, nil
end
```

### Sender Validation (Chat Messages)

Ensure users can only send messages as themselves:

```lua
if layer_name:match("/messages$") then
    for _, op in ipairs(ops) do
        if op.op == "insert" and op.value then
            if op.value.sender_did ~= from_did then
                return false, "Cannot impersonate another user"
            end
        end
    end
    return true, nil
end
```

## Complete Example

From `sample_apps/my-shop/shared/validation.lua` (simplified):

```lua
local ORDER_STATES = {
    draft = { customer_transitions = {'pending'}, owner_transitions = {} },
    pending = { customer_transitions = {'cancelled'}, owner_transitions = {'confirmed', 'cancelled'} },
    confirmed = { customer_transitions = {}, owner_transitions = {'shipped'} },
    shipped = { customer_transitions = {}, owner_transitions = {'delivered'} },
    delivered = { customer_transitions = {}, owner_transitions = {} },
    cancelled = { customer_transitions = {}, owner_transitions = {} }
}

function validate_ops(layer_name, ops, from_did, role, page_id)
    -- Products: owner-only
    if layer_name:match("/products$") then
        if role ~= "owner" then
            return false, "Only owner can modify products"
        end
        return true, nil
    end

    -- Orders: per-user with state machine
    if layer_name:match("/orders/") then
        local layer_did = layer_name:match("/orders/(.+)$")
        if role == "customer" and layer_did ~= from_did then
            return false, "Can only modify own orders"
        end

        for _, op in ipairs(ops) do
            if op.value and op.value.status then
                local old_status = (op.old_value and op.old_value.status) or "draft"
                local state = ORDER_STATES[old_status]
                if not state then
                    return false, "Unknown state: " .. old_status
                end

                local allowed = role == "customer"
                    and state.customer_transitions
                    or state.owner_transitions

                local found = false
                for _, s in ipairs(allowed) do
                    if s == op.value.status then found = true; break end
                end
                if not found then
                    return false, role .. " cannot transition " .. old_status .. " → " .. op.value.status
                end
            end
        end
        return true, nil
    end

    return true, nil
end
```

## Gotchas

- Must return `(true, nil)` for valid or `(false, "error message")` for invalid
- `ops` is an array -- iterate all operations, not just the first
- `op.old_value` may be nil for inserts
- Validation runs **on node only**, not on the client
- If `validate_ops` is not defined, all updates are allowed
- The `layer_name` may or may not include the `page_id` prefix depending on context -- use pattern matching (`:match`) rather than exact comparison
