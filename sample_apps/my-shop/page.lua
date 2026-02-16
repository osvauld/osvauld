-- My Shop - Page Definition
-- Single source of truth for roles, layers, and apps
page("My Shop", "1.0.0")

-- =============================================================================
-- Roles
-- =============================================================================

-- Owner: Full control over the shop
role("owner", {
    can_share = true,
    can_delegate = true
})

-- Customer: Can browse and place orders
role("customer", {
    parent = "owner"
})

-- Node: Relay and derivation execution
role("node", {
    parent = "owner",
    can_relay = true
})

-- Admin: Shop management without ownership
role("admin", {
    parent = "owner"
})

-- =============================================================================
-- Layers
-- =============================================================================

-- Products catalog (owned by shop owner)
layer("products", "list", {
    owner = {"read", "write", "sync"},
    customer = {"read", "sync"},
    node = {"read", "sync"},
    admin = {"read", "write", "sync"}
})

-- Per-customer orders (dynamic layer)
layer("orders/{aud}", "list", {
    owner = {"read", "sync"},
    ["{aud}"] = {"read", "write", "sync"},
    node = {"read", "sync", "create"},

    -- Validation: Order state machine
    validate = function(ops, ctx)
        -- Orders must start in pending state
        for _, op in ipairs(ops) do
            if op.op == "insert" and op.value then
                local status = op.value.status or "pending"
                if status ~= "pending" then
                    return false, "New orders must start in pending state"
                end
            end
        end
        return true, nil
    end
})

-- Derived orders summary (written by node)
layer("derived/orders_summary", "map", {
    owner = {"read", "sync"},
    node = {"write"},

    derive_from = "orders/*",
    transform = function(source, item)
        return {
            id = item.id,
            status = item.status,
            total = item.total,
            customer_did = source
        }
    end
})

-- Local drafts (not synced)
layer("drafts", "map", {
    owner = {"read", "write"},
    customer = {"read", "write", "create"}
})

-- =============================================================================
-- Apps
-- =============================================================================

-- Shop Owner app
app("Shop Owner", {
    client = {
        ui = "shop-owner/app.slint",
        logic = "shop-owner/app.lua"
    },
    node = {
        logic = "shop-owner/node.lua",
        tick = true
    },
    for_role = "owner"
})

-- Shop Customer app
app("Shop Customer", {
    client = {
        ui = "shop-customer/app.slint",
        logic = "shop-customer/app.lua"
    },
    for_role = "customer"
})
