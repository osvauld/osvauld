-- My-Shop Initialization
-- Registers derivation rules for aggregated views
--
-- This file is loaded by the node's LuaRuntime during app startup.
-- Derivation rules only execute on the node (no-op on owner/customer).

-- Helper: Extract user_did from layer name
-- Pattern: {page_id}/orders/{user_did}
local function extract_did(layer_name)
    -- Find last "/" and extract what follows
    local last_slash = layer_name:match(".*/()")
    if last_slash then
        return layer_name:sub(last_slash)
    end
    return layer_name
end

-- Get page_id for proper layer naming
local page_id = permit:page_id()

-- Register orders_summary derivation
-- Aggregates all orders from orders/* into a single summary layer
derivation:register({
    -- Source pattern: all per-customer order layers
    source = page_id .. "/orders/*",

    -- Target: aggregated summary layer for owner (matches {page_id}/*/* pattern)
    target = page_id .. "/derived/orders_summary",

    -- Key function: extract order ID for derived map
    key_fn = function(order)
        return order.id
    end,

    -- Transform function: create summary entry from full order
    -- source_layer: e.g., "shop123/orders/did:key:customer_a"
    -- order: full order data
    transform = function(source_layer, order)
        return {
            id = order.id,
            customer = extract_did(source_layer),
            status = order.status,
            total = order.total,
            created_at = order.created_at,
            submitted_at = order.submitted_at,
            updated_at = order.updated_at,
            -- Use quantity from order (items is the product name string)
            item_count = order.quantity or 1,
        }
    end,

    -- Optional filter: only include submitted orders (not drafts)
    filter = function(order)
        return order.status ~= "draft"
    end,
})

-- Log initialization
print("[my-shop] Derivation rules registered")
