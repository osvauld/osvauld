-- My-Booking Validation Logic
-- Validates incoming updates on the node before syncing
--
-- This file is loaded by the node's HeadlessRuntime to enforce
-- business rules on booking state transitions and prevent double-bookings.

-- Booking State Machine
-- Defines valid transitions and writable fields for each state
local BOOKING_STATES = {
    draft = {
        writable = {'date', 'start_time', 'end_time', 'service', 'notes', 'customer_name', 'customer_phone'},
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
        owner_transitions = {'completed', 'cancelled'}
    },
    completed = {
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
-- Returns: "schedule", "blocked", "bookings", "calendar", "app", or "unknown"
local function get_layer_type(layer_name, page_id)
    if layer_name == page_id .. "/schedule" then
        return "schedule"
    end

    if layer_name == page_id .. "/blocked" then
        return "blocked"
    end

    if layer_name == page_id .. "/derived/calendar" then
        return "calendar"
    end

    -- Pattern: {page_id}/bookings/{user_did}
    local bookings_prefix = page_id .. "/bookings/"
    if layer_name:sub(1, #bookings_prefix) == bookings_prefix then
        return "bookings"
    end

    -- App layers
    if layer_name:sub(1, 4) == "app:" then
        return "app"
    end

    return "unknown"
end

-- Helper: Extract user_did from bookings layer name
local function extract_user_did(layer_name, page_id)
    local bookings_prefix = page_id .. "/bookings/"
    if layer_name:sub(1, #bookings_prefix) == bookings_prefix then
        return layer_name:sub(#bookings_prefix + 1)
    end
    return nil
end

-- Validate schedule layer operations
-- Only owner can write to schedule
local function validate_schedule_ops(ops, from_did, role)
    if role ~= "owner" then
        return false, "Only owner can modify schedule layer"
    end
    return true, nil
end

-- Validate blocked layer operations
-- Only owner can write to blocked times
local function validate_blocked_ops(ops, from_did, role)
    if role ~= "owner" then
        return false, "Only owner can modify blocked times"
    end
    return true, nil
end

-- Validate derived calendar layer operations
-- Only node can write to derived layer (via derivation)
local function validate_calendar_ops(ops, from_did, role)
    if role ~= "node" then
        return false, "Only node can modify derived calendar layer"
    end
    return true, nil
end

-- Helper: Check if op is an array-level insert (new item) vs field-level insert (new field in existing item)
-- Array-level: path ends with "[N]" (e.g., "[0]", "[1]")
-- Field-level: path contains "[N].field" (e.g., "[0].status", "[0].confirmed_at")
local function is_array_level_op(op)
    local path = op.path or ""
    -- Array-level if path matches "[N]" pattern (no dot after the bracket)
    -- Pattern: starts with optional prefix, then [number], then end of string
    return path:match("%[%d+%]$") ~= nil
end

-- Validate bookings layer operations
-- Customers can only modify their own bookings layer
-- Booking state transitions must follow state machine
local function validate_bookings_ops(ops, from_did, role, layer_owner_did, page_id)
    -- Check layer ownership
    if role == "customer" or role == "viewer" then
        -- Customers can only write to their own bookings layer
        if from_did ~= layer_owner_did then
            return false, "Customers can only modify their own bookings layer"
        end
    end

    -- Validate each operation
    for _, op in ipairs(ops) do
        local op_type = op.op or "unknown"

        -- For insert operations, distinguish between:
        -- 1. Array-level insert (new booking) - validate the full booking
        -- 2. Field-level insert (new field in existing booking) - skip validation (handled by update)
        if op_type == "insert" and op.value then
            -- Only validate if this is an array-level insert (new booking)
            if is_array_level_op(op) then
                local booking = op.value
                local status = booking.status or "pending"

                -- Synced bookings must start in pending state (drafts are local-only)
                if status ~= "pending" then
                    return false, "Synced bookings must start in pending state"
                end

                -- Validate required fields for new booking
                if not booking.date or booking.date == "" then
                    return false, "Booking must have a date"
                end
                if not booking.start_time or booking.start_time == "" then
                    return false, "Booking must have a start time"
                end
                if not booking.end_time or booking.end_time == "" then
                    return false, "Booking must have an end time"
                end
            end
            -- Field-level inserts (e.g., adding confirmed_at timestamp) are allowed
        end

        -- For update operations, validate state transitions and field writability
        if op_type == "update" and op.value then
            local new_booking = op.value
            local old_booking = op.old_value or {}
            local current_status = old_booking.status or "draft"
            local new_status = new_booking.status or current_status

            local state_rules = BOOKING_STATES[current_status]
            if not state_rules then
                return false, "Invalid booking status: " .. current_status
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
                        "Role '%s' cannot transition booking from '%s' to '%s'",
                        role, current_status, new_status
                    )
                end
            end

            -- Check field writability (only for non-owners)
            if role ~= "owner" then
                for field, new_value in pairs(new_booking) do
                    local old_value = old_booking[field]
                    -- Skip if field hasn't changed
                    if new_value ~= old_value then
                        -- Status changes are handled above
                        if field ~= "status" and field ~= "id" and field ~= "created_at" then
                            -- Allow timestamp updates
                            if field == "submitted_at" or field == "cancelled_at" or
                               field == "confirmed_at" or field == "completed_at" or field == "updated_at" then
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

    -- Schedule layer validation
    if layer_type == "schedule" then
        return validate_schedule_ops(ops, from_did, role)
    end

    -- Blocked layer validation
    if layer_type == "blocked" then
        return validate_blocked_ops(ops, from_did, role)
    end

    -- Derived calendar layer validation
    if layer_type == "calendar" then
        return validate_calendar_ops(ops, from_did, role)
    end

    -- Bookings layer validation
    if layer_type == "bookings" then
        local layer_owner_did = extract_user_did(layer_name, page_id)
        return validate_bookings_ops(ops, from_did, role, layer_owner_did, page_id)
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
    BOOKING_STATES = BOOKING_STATES
}
