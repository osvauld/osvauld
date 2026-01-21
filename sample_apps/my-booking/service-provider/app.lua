-- Service Provider App Logic
-- Manages schedule and processes customer bookings
-- Uses Event Bus pattern for unified human/AI interaction

-- Local state
local page_id = nil
local schedule_layer = nil       -- Weekly schedule (map)
local blocked_layer = nil        -- Blocked times (list)
local calendar_layer = nil       -- Derived calendar (booked slots, provider writes)
local booking_layers = {}        -- Map of user_did -> bookings_layer (for status updates)

-- Form state tracking (synced with UI via Event Bus)
local form_state = {
    -- Schedule editing
    edit_day = "",           -- "monday", "tuesday", etc.
    edit_enabled = true,
    edit_start = "09:00",
    edit_end = "17:00",
    edit_slot_duration = 60,
    -- Block time
    block_date = "",
    block_all_day = false,
    block_start = "",
    block_end = "",
    block_reason = "",
    -- Legacy fields for compatibility
    schedule_day = "",
    schedule_start = "",
    schedule_end = "",
    schedule_duration = ""
}

-- Day names for schedule
local DAYS = {"monday", "tuesday", "wednesday", "thursday", "friday", "saturday", "sunday"}

-- Initialize the app
function on_init()
    page_id = permit:page_id()

    -- Schedule layer (weekly availability)
    schedule_layer = loro:get_or_create_layer(page_id .. "/schedule", "map")

    -- Blocked times layer
    blocked_layer = loro:get_or_create_layer(page_id .. "/blocked", "list")

    -- Note: derived/calendar is managed by kunki node, not provider
    -- calendar_layer = loro:get_layer(page_id .. "/derived/calendar", "map")

    -- Track booking layers for status updates
    local pattern = page_id .. "/bookings/*"
    local layers = loro:list_layers(pattern)
    for _, layer_name in ipairs(layers) do
        local bookings_layer = loro:get_layer(layer_name, "list")
        if bookings_layer then
            local user_did = layer_name:match("/bookings/(.+)$")
            if user_did then
                booking_layers[user_did] = bookings_layer
            end
        end
    end

    refresh_schedule_ui()
    refresh_blocked_ui()
    refresh_bookings_ui()
end

-- Called when any Loro layer changes
function on_loro_change(layer_name, change_type)
    if layer_name == page_id .. "/schedule" then
        refresh_schedule_ui()
    elseif layer_name == page_id .. "/blocked" then
        refresh_blocked_ui()
    elseif layer_name:match("/bookings/") then
        refresh_bookings_ui()
    end
end

-- Called when a new layer is discovered
function on_layer_discovered(layer_name)
    local expected_prefix = page_id .. "/bookings/"
    if layer_name:sub(1, #expected_prefix) == expected_prefix then
        local bookings_layer = loro:get_layer(layer_name, "list")
        if bookings_layer then
            local user_did = layer_name:match("/bookings/(.+)$")
            if user_did then
                booking_layers[user_did] = bookings_layer
                refresh_bookings_ui()
            end
        end
    end
end

-- ============================================================================
-- EVENT BUS HANDLERS
-- ============================================================================

function on_field_changed(field_name, value)
    -- Handle type conversions and legacy field mappings
    if field_name == "edit_enabled" or field_name == "block_all_day" then
        -- Convert to boolean
        if type(value) == "string" then
            form_state[field_name] = (value == "true" or value == "1")
        else
            form_state[field_name] = value
        end
    elseif field_name == "edit_slot_duration" or field_name == "schedule_duration" then
        -- Convert to number, map legacy field
        local num_val = tonumber(value) or 60
        form_state.edit_slot_duration = num_val
        form_state.schedule_duration = tostring(num_val)
    elseif field_name == "schedule_day" then
        -- Legacy: also set edit_day
        form_state.schedule_day = value
        form_state.edit_day = value
    elseif field_name == "schedule_start" then
        -- Legacy: also set edit_start
        form_state.schedule_start = value
        form_state.edit_start = value
    elseif field_name == "schedule_end" then
        -- Legacy: also set edit_end
        form_state.schedule_end = value
        form_state.edit_end = value
    else
        form_state[field_name] = value
    end
end

function on_modal_action(modal_name, action)
    if modal_name == "schedule" then
        if action == "open" then
            -- Populate form with current day's schedule
            local day = form_state.edit_day
            if day ~= "" and schedule_layer then
                local entry = schedule_layer:get(day)
                if entry then
                    form_state.edit_enabled = entry.enabled or false
                    form_state.edit_start = entry.start or "09:00"
                    form_state.edit_end = entry["end"] or "17:00"
                    form_state.edit_slot_duration = entry.slot_duration or 60
                else
                    form_state.edit_enabled = false
                    form_state.edit_start = "09:00"
                    form_state.edit_end = "17:00"
                    form_state.edit_slot_duration = 60
                end
                -- Sync to UI
                ui:set("edit_day", form_state.edit_day)
                ui:set("edit_enabled", form_state.edit_enabled)
                ui:set("edit_start", form_state.edit_start)
                ui:set("edit_end", form_state.edit_end)
                ui:set("edit_slot_duration", form_state.edit_slot_duration)
            end
            ui:set("schedule_modal_visible", true)
        elseif action == "submit" then
            do_update_schedule()
            clear_schedule_form()
        elseif action == "cancel" or action == "close" then
            clear_schedule_form()
        end
    elseif modal_name == "block" then
        if action == "open" then
            -- Reset block form to defaults
            form_state.block_date = ""
            form_state.block_all_day = true
            form_state.block_start = ""
            form_state.block_end = ""
            form_state.block_reason = ""
            ui:set("block_date", "")
            ui:set("block_all_day", true)
            ui:set("block_start", "")
            ui:set("block_end", "")
            ui:set("block_reason", "")
            ui:set("block_modal_visible", true)
        elseif action == "submit" then
            do_add_blocked()
            clear_block_form()
        elseif action == "cancel" or action == "close" then
            clear_block_form()
        end
    end
end

function on_click(target)
    local action, id = target:match("^([^:]+):?(.*)$")

    if action == "confirm_booking" then
        local customer_did, booking_id = id:match("^([^|]+)|(.+)$")
        if customer_did and booking_id then
            do_update_booking_status(customer_did, booking_id, "confirmed")
        end
    elseif action == "cancel_booking" then
        local customer_did, booking_id = id:match("^([^|]+)|(.+)$")
        if customer_did and booking_id then
            do_update_booking_status(customer_did, booking_id, "cancelled")
        end
    elseif action == "complete_booking" then
        local customer_did, booking_id = id:match("^([^|]+)|(.+)$")
        if customer_did and booking_id then
            do_update_booking_status(customer_did, booking_id, "completed")
        end
    elseif action == "remove_blocked" then
        do_remove_blocked(id)
    end
end

-- ============================================================================
-- CORE LOGIC
-- ============================================================================

-- Update schedule for a day
function do_update_schedule()
    if permit:role() ~= "owner" then
        log_warn("Only owner can modify schedule")
        return
    end

    if not schedule_layer then return end

    -- Use new form fields, fall back to legacy
    local day = form_state.edit_day or form_state.schedule_day or ""
    if day == "" then return end

    local schedule_entry = {
        enabled = form_state.edit_enabled,
        start = form_state.edit_start or form_state.schedule_start or "09:00",
        ["end"] = form_state.edit_end or form_state.schedule_end or "17:00",
        slot_duration = form_state.edit_slot_duration or tonumber(form_state.schedule_duration) or 60
    }

    schedule_layer:set(day, schedule_entry)
    log_info("Schedule updated for " .. day .. " (enabled: " .. tostring(schedule_entry.enabled) .. ")")
    refresh_schedule_ui()
end

-- Add blocked time
function do_add_blocked()
    if permit:role() ~= "owner" then
        log_warn("Only owner can block times")
        return
    end

    if not blocked_layer then return end

    local date = form_state.block_date or ""
    if date == "" then return end

    local blocked_entry = {
        id = generate_id(),
        date = date,
        -- If all_day is checked, don't include start/end times
        start_time = form_state.block_all_day and "" or (form_state.block_start or ""),
        end_time = form_state.block_all_day and "" or (form_state.block_end or ""),
        reason = form_state.block_reason or ""
    }

    blocked_layer:push(blocked_entry)
    local time_desc = form_state.block_all_day and "all day" or (blocked_entry.start_time .. " - " .. blocked_entry.end_time)
    log_info("Blocked time added for " .. date .. " (" .. time_desc .. ")")
    refresh_blocked_ui()
end

-- Remove blocked time
function do_remove_blocked(blocked_id)
    if permit:role() ~= "owner" then
        log_warn("Only owner can remove blocked times")
        return
    end

    if not blocked_layer then return end

    local len = blocked_layer:length()
    for i = 0, len - 1 do
        local entry = blocked_layer:get(i)
        if entry and entry.id == blocked_id then
            blocked_layer:delete(i)
            log_info("Removed blocked time: " .. blocked_id)
            refresh_blocked_ui()
            break
        end
    end
end

-- Update booking status
function do_update_booking_status(customer_did, booking_id, new_status)
    if permit:role() ~= "owner" then
        log_warn("Only owner can update booking status")
        return
    end

    local bookings_layer = booking_layers[customer_did]
    if not bookings_layer then
        log_warn("Booking layer not found for customer: " .. customer_did)
        return
    end

    local len = bookings_layer:length()
    for i = 0, len - 1 do
        local booking = bookings_layer:get(i)
        if booking and booking.id == booking_id then
            local current = booking.status or "draft"
            local valid = {
                pending = { confirmed = true, cancelled = true },
                confirmed = { completed = true, cancelled = true }
            }
            if valid[current] and valid[current][new_status] then
                booking.status = new_status
                booking.updated_at = os.date("%Y-%m-%d %H:%M:%S")
                if new_status == "confirmed" then
                    booking.confirmed_at = os.date("%Y-%m-%d %H:%M:%S")
                elseif new_status == "completed" then
                    booking.completed_at = os.date("%Y-%m-%d %H:%M:%S")
                elseif new_status == "cancelled" then
                    booking.cancelled_at = os.date("%Y-%m-%d %H:%M:%S")
                end
                bookings_layer:set(i, booking)
                log_info("Booking " .. booking_id .. " -> " .. new_status)
                refresh_bookings_ui()
            else
                log_warn("Invalid transition: " .. current .. " -> " .. new_status)
            end
            break
        end
    end
end

-- Clear forms
function clear_schedule_form()
    -- New fields
    form_state.edit_day = ""
    form_state.edit_enabled = true
    form_state.edit_start = "09:00"
    form_state.edit_end = "17:00"
    form_state.edit_slot_duration = 60
    -- Legacy fields
    form_state.schedule_day = ""
    form_state.schedule_start = ""
    form_state.schedule_end = ""
    form_state.schedule_duration = ""
    -- Sync to UI
    ui:set("edit_day", "")
    ui:set("edit_enabled", true)
    ui:set("edit_start", "09:00")
    ui:set("edit_end", "17:00")
    ui:set("edit_slot_duration", 60)
    ui:set("schedule_modal_visible", false)
end

function clear_block_form()
    form_state.block_date = ""
    form_state.block_all_day = true
    form_state.block_start = ""
    form_state.block_end = ""
    form_state.block_reason = ""
    -- Sync to UI
    ui:set("block_date", "")
    ui:set("block_all_day", true)
    ui:set("block_start", "")
    ui:set("block_end", "")
    ui:set("block_reason", "")
    ui:set("block_modal_visible", false)
end

-- ============================================================================
-- AI AUTOMATION API
-- ============================================================================

-- Set schedule for a day
function set_schedule_via_ui(day, start_time, end_time, slot_duration)
    on_modal_action("schedule", "open")
    on_field_changed("schedule_day", day or "monday")
    on_field_changed("schedule_start", start_time or "09:00")
    on_field_changed("schedule_end", end_time or "17:00")
    on_field_changed("schedule_duration", tostring(slot_duration or 60))
    on_modal_action("schedule", "submit")
    return true
end

-- Block a time
function block_time_via_ui(date, start_time, end_time, reason)
    on_modal_action("block", "open")
    on_field_changed("block_date", date or "")
    -- If times are provided, set all_day to false
    local is_all_day = (start_time == nil or start_time == "") and (end_time == nil or end_time == "")
    on_field_changed("block_all_day", is_all_day and "true" or "false")
    on_field_changed("block_start", start_time or "")
    on_field_changed("block_end", end_time or "")
    on_field_changed("block_reason", reason or "")
    on_modal_action("block", "submit")
    return true
end

-- Confirm a booking
function confirm_booking_via_ui(customer_did, booking_id)
    on_click("confirm_booking:" .. customer_did .. "|" .. booking_id)
    return true
end

-- Cancel a booking
function cancel_booking_via_ui(customer_did, booking_id)
    on_click("cancel_booking:" .. customer_did .. "|" .. booking_id)
    return true
end

-- Complete a booking
function complete_booking_via_ui(customer_did, booking_id)
    on_click("complete_booking:" .. customer_did .. "|" .. booking_id)
    return true
end

-- ============================================================================
-- UI REFRESH
-- ============================================================================

function refresh_schedule_ui()
    local schedule = {}
    if schedule_layer then
        for _, day in ipairs(DAYS) do
            local entry = schedule_layer:get(day)
            if entry then
                table.insert(schedule, {
                    day = day,
                    enabled = entry.enabled or false,
                    start_time = entry.start or "09:00",
                    end_time = entry["end"] or "17:00",
                    slot_duration = entry.slot_duration or 60
                })
            else
                table.insert(schedule, {
                    day = day,
                    enabled = false,
                    start_time = "09:00",
                    end_time = "17:00",
                    slot_duration = 60
                })
            end
        end
    end
    ui:set("schedule", schedule)
end

function refresh_blocked_ui()
    local blocked = {}
    if blocked_layer then
        local len = blocked_layer:length()
        for i = 0, len - 1 do
            local entry = blocked_layer:get(i)
            if entry then
                table.insert(blocked, {
                    id = entry.id or "",
                    date = entry.date or "",
                    start_time = entry.start_time or "",
                    end_time = entry.end_time or "",
                    reason = entry.reason or ""
                })
            end
        end
    end
    table.sort(blocked, function(a, b)
        return a.date < b.date
    end)
    ui:set("blocked", blocked)
end

function refresh_bookings_ui()
    local all_bookings = {}

    for customer_did, bookings_layer in pairs(booking_layers) do
        local len = bookings_layer:length()
        for i = 0, len - 1 do
            local booking = bookings_layer:get(i)
            if booking then
                table.insert(all_bookings, {
                    id = booking.id or "",
                    customer_did = customer_did,
                    date = booking.date or "",
                    start_time = booking.start_time or "",
                    end_time = booking.end_time or "",
                    service = booking.service or "",
                    customer_name = booking.customer_name or "",
                    customer_phone = booking.customer_phone or "",
                    notes = booking.notes or "",
                    status = booking.status or "pending",
                    created_at = booking.created_at or ""
                })
            end
        end
    end

    table.sort(all_bookings, function(a, b)
        if a.date == b.date then
            return a.start_time < b.start_time
        end
        return a.date < b.date
    end)

    ui:set("bookings", all_bookings)
end

-- ============================================================================
-- TEST HELPERS
-- ============================================================================

function get_schedule()
    local schedule = {}
    if schedule_layer then
        for _, day in ipairs(DAYS) do
            local entry = schedule_layer:get(day)
            if entry then
                schedule[day] = entry
            end
        end
    end
    return schedule
end

function get_blocked()
    local blocked = {}
    if blocked_layer then
        local len = blocked_layer:length()
        for i = 0, len - 1 do
            local entry = blocked_layer:get(i)
            if entry then
                table.insert(blocked, entry)
            end
        end
    end
    return blocked
end

function get_all_bookings()
    local all_bookings = {}
    for customer_did, bookings_layer in pairs(booking_layers) do
        local len = bookings_layer:length()
        for i = 0, len - 1 do
            local booking = bookings_layer:get(i)
            if booking then
                booking.customer_did = customer_did
                table.insert(all_bookings, booking)
            end
        end
    end
    return all_bookings
end

function get_bookings_count()
    local count = 0
    for _, bookings_layer in pairs(booking_layers) do
        count = count + bookings_layer:length()
    end
    return count
end

-- ============================================================================
-- UTILITIES
-- ============================================================================

function generate_id()
    return string.format("%x", os.time()) .. "-" .. string.format("%04x", math.random(0, 65535))
end

function log_info(msg)
    print("[INFO] " .. msg)
end

function log_warn(msg)
    print("[WARN] " .. msg)
end
