-- My Booking - Page Definition
-- Service booking app with provider/customer roles
page("Booking", "1.0.0")

-- =============================================================================
-- Roles
-- =============================================================================

-- Owner: Service provider with full control
role("owner", {
    can_share = true,
    can_delegate = true
})

-- Customer: Can browse and book services
role("customer", {
    parent = "owner"
})

-- Node: Relay and derivation execution
role("node", {
    parent = "owner",
    can_relay = true
})

-- =============================================================================
-- Layers
-- =============================================================================

-- Service schedule (time slots, availability)
layer("schedule", "map", {
    owner = {"read", "write", "sync"},
    customer = {"read", "sync"},
    node = {"read", "sync"}
})

-- Blocked time slots
layer("blocked", "list", {
    owner = {"read", "write", "sync"},
    customer = {"read", "sync"},
    node = {"read", "sync"}
})

-- Derived calendar view (written by node)
layer("derived/calendar", "map", {
    owner = {"read", "sync"},
    customer = {"read", "sync"},
    node = {"read", "write", "sync"},

    derive_from = "schedule",
    transform = function(source, item)
        return {
            id = item.id,
            date = item.date,
            available = item.available
        }
    end
})

-- Local drafts (not synced)
layer("drafts", "map", {
    owner = {"read", "write"},
    customer = {"read", "write", "create"}
})

-- Per-customer bookings (dynamic layer)
layer("bookings/{aud}", "list", {
    owner = {"read", "sync"},
    ["{aud}"] = {"read", "write", "sync"},
    node = {"read", "sync", "create"}
})

-- =============================================================================
-- Apps
-- =============================================================================

-- Service Provider app
app("Service Provider", {
    client = {
        ui = "service-provider/app.slint",
        logic = "service-provider/app.lua"
    },
    for_role = "owner"
})

-- Service Customer app
app("Service Customer", {
    client = {
        ui = "service-customer/app.slint",
        logic = "service-customer/app.lua"
    },
    for_role = "customer"
})
