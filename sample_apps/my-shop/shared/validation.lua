-- My-Shop Validation Logic
-- Validates incoming updates on the node before syncing
--
-- This file is loaded by the node's LuaRuntime to enforce
-- business rules on order state transitions and field modifications.

-- Order State Machine
-- Defines valid transitions and writable fields for each state
local ORDER_STATES = {
    draft = {
        writable = {'items', 'quantity', 'notes', 'shipping_address', 'total', 'product_id'},
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
    delivered = {
        writable = {},
        customer_transitions = {},
        owner_transitions = {}
    },
    cancelled = {
        writable = {},
        customer_transitions = {},
        owner_transitions = {}
    }
}

-- Helper: Check if value is in array
local function contains(arr, value)
    if not arr then return false end
    for _, v in ipairs(arr) do
        if v == value then return true end
    end
    return false
end

-- Helper: Extract layer type from layer name
-- Returns: "products", "orders", "app", or "unknown"
local function get_layer_type(layer_name, page_id)
    if layer_name == "products" then
        return "products"
    end

    -- Pattern: {page_id}/orders/{user_did}
    local orders_prefix = page_id .. "/orders/"
    if layer_name:sub(1, #orders_prefix) == orders_prefix then
        return "orders"
    end

    -- App layers
    if layer_name:sub(1, 4) == "app:" then
        return "app"
    end

    return "unknown"
end

-- Helper: Extract user_did from orders layer name
local function extract_user_did(layer_name, page_id)
    local orders_prefix = page_id .. "/orders/"
    if layer_name:sub(1, #orders_prefix) == orders_prefix then
        return layer_name:sub(#orders_prefix + 1)
    end
    return nil
end

-- Validate products layer operations
-- Only owner can write to products
local function validate_products_ops(ops, from_did, role)
    if role ~= "owner" then
        return false, "Only owner can modify products layer"
    end
    return true, nil
end

-- Validate orders layer operations
-- Customers can only modify their own orders layer
-- Order state transitions must follow state machine
local function validate_orders_ops(ops, from_did, role, layer_owner_did, page_id)
    -- Check layer ownership
    if role == "customer" or role == "viewer" then
        -- Customers can only write to their own orders layer
        if from_did ~= layer_owner_did then
            return false, "Customers can only modify their own orders layer"
        end
    end

    -- Validate each operation
    for _, op in ipairs(ops) do
        local op_type = op.op or "unknown"

        -- For insert operations (new orders), check initial state
        -- Note: draft orders are local-only (never sync), so synced orders start as pending
        if op_type == "insert" and op.value then
            local order = op.value
            local status = order.status or "pending"

            -- Synced orders must start in pending state (drafts are local-only)
            if status ~= "pending" then
                return false, "Synced orders must start in pending state"
            end
        end

        -- For update operations, validate state transitions and field writability
        if op_type == "update" and op.value then
            local new_order = op.value
            local old_order = op.old_value or {}
            local current_status = old_order.status or "draft"
            local new_status = new_order.status or current_status

            local state_rules = ORDER_STATES[current_status]
            if not state_rules then
                return false, "Invalid order status: " .. current_status
            end

            -- Check status transition
            if new_status ~= current_status then
                local allowed_transitions
                if role == "owner" then
                    allowed_transitions = state_rules.owner_transitions
                else
                    allowed_transitions = state_rules.customer_transitions
                end

                if not contains(allowed_transitions, new_status) then
                    return false, string.format(
                        "Role '%s' cannot transition order from '%s' to '%s'",
                        role, current_status, new_status
                    )
                end
            end

            -- Check field writability (only for non-owners)
            if role ~= "owner" then
                for field, new_value in pairs(new_order) do
                    local old_value = old_order[field]
                    -- Skip if field hasn't changed
                    if new_value ~= old_value then
                        -- Status changes are handled above
                        if field ~= "status" and field ~= "id" and field ~= "created_at" then
                            -- Check timestamp fields that are allowed
                            if field == "submitted_at" or field == "cancelled_at" or field == "updated_at" then
                                -- Allow timestamp updates
                            elseif not contains(state_rules.writable, field) then
                                return false, string.format(
                                    "Cannot modify field '%s' in '%s' state",
                                    field, current_status
                                )
                            end
                        end
                    end
                end
            end
        end
    end

    return true, nil
end

--- Main validation entry point
--- Called by Scribe before applying incoming updates
---
--- @param layer_name string Layer being modified
--- @param ops table Array of operations {op, path, key, index, value, old_value}
--- @param from_did string DID of the writer
--- @param role string Role: "owner", "customer", "viewer", "node", "admin"
--- @param page_id string The page ID
--- @return boolean, string? valid, error_message
function validate_ops(layer_name, ops, from_did, role, page_id)
    -- Determine layer type
    local layer_type = get_layer_type(layer_name, page_id)

    -- Products layer validation
    if layer_type == "products" then
        return validate_products_ops(ops, from_did, role)
    end

    -- Orders layer validation
    if layer_type == "orders" then
        local layer_owner_did = extract_user_did(layer_name, page_id)
        return validate_orders_ops(ops, from_did, role, layer_owner_did, page_id)
    end

    -- App layers - allow sync but no writes from non-owners
    if layer_type == "app" then
        if role ~= "owner" then
            return false, "Only owner can modify app layers"
        end
        return true, nil
    end

    -- Unknown layers - default allow (permit system handles authorization)
    return true, nil
end

-- Export for testing
return {
    validate_ops = validate_ops,
    ORDER_STATES = ORDER_STATES
}
