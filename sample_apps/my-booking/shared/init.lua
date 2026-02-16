-- My-Booking Initialization
-- Registers derivation rules for calendar view
--
-- This file is loaded by the node's LuaRuntime during app startup.
-- Derivation rules only execute on the node (no-op on owner/customer).
--
-- Privacy: The derived calendar only shows that a slot is booked,
-- without revealing customer details to other customers.

-- Get page_id for proper layer naming
local page_id = permit:page_id()

-- Register calendar derivation
-- Aggregates all bookings from bookings/* into a public calendar view
-- PRIVACY: Strips all customer details, only shows booked slots
derivation:register({
    -- Source pattern: all per-customer booking layers
    source = page_id .. "/bookings/*",

    -- Target: public calendar layer showing booked slots only
    target = page_id .. "/derived/calendar",

    -- Key function: create unique key from date and time
    -- Format: "2026-01-25_10:00" for slot at 10:00 on Jan 25, 2026
    key_fn = function(booking)
        return booking.date .. "_" .. booking.start_time
    end,

    -- Transform function: strip all customer details for privacy
    -- source_layer: e.g., "page123/bookings/did:key:customer_a"
    -- booking: full booking data with customer info
    transform = function(source_layer, booking)
        -- Return ONLY slot info - no customer details!
        return {
            date = booking.date,
            start_time = booking.start_time,
            end_time = booking.end_time,
            booked = true
            -- STRIPPED: customer_name, customer_phone, notes, service, customer DID
        }
    end,

    -- Filter: only show confirmed bookings in calendar
    -- Pending bookings are not visible to other customers yet
    filter = function(booking)
        return booking.status == "confirmed"
    end,
})

-- Register blocked times derivation
-- Provider's blocked times (lunch, vacation) also show as unavailable
derivation:register({
    source = page_id .. "/blocked",
    target = page_id .. "/derived/calendar",

    key_fn = function(blocked)
        return blocked.date .. "_" .. blocked.start_time
    end,

    transform = function(source_layer, blocked)
        return {
            date = blocked.date,
            start_time = blocked.start_time,
            end_time = blocked.end_time,
            booked = true  -- Shows as unavailable, same as customer bookings
        }
    end,

    filter = function(blocked)
        -- Only include entries with both date and start_time
        return blocked.date ~= nil and blocked.start_time ~= nil
    end,
})

-- Note: Derivation runs automatically via on_source_ops() in Scribe when layers change.
-- No need for on_loro_change here - the Scribe's Lua runtime handles this internally.

-- Log initialization
print("[my-booking] Calendar derivation registered - privacy mode enabled")
print("[my-booking] Blocked times derivation registered")
print("[my-booking] Auto-rebuild on bookings/blocked changes enabled")
