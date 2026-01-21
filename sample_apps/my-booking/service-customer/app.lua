-- Service Customer App Logic
-- Book appointments and manage own bookings
-- Uses Event Bus pattern for unified human/AI interaction

-- Booking State Machine
local BOOKING_STATES = {
    draft = {
        writable = {'date', 'start_time', 'end_time', 'service', 'notes', 'customer_name', 'customer_phone'},
        customer_transitions = {'pending'}
    },
    pending = {
        writable = {'notes'},
        customer_transitions = {'cancelled'}
    },
    confirmed = {
        writable = {},
        customer_transitions = {}
    },
    completed = {
        writable = {},
        customer_transitions = {}
    },
    cancelled = {
        writable = {},
        customer_transitions = {}
    }
}

-- Local state
local page_id = nil
local my_did = nil
local schedule_layer = nil       -- Provider's weekly schedule (read-only)
local blocked_layer = nil        -- Provider's blocked times (read-only)
local calendar_layer = nil       -- Derived calendar showing booked slots (read-only)
local bookings_layer = nil       -- My bookings (read/write)
local drafts_layer = nil         -- Local drafts (local-only)

-- Form state
local form_state = {
    booking_date = "",
    booking_time = "",
    booking_service = "",
    booking_notes = "",
    customer_name = "",
    customer_phone = ""
}

-- Currently selected booking for operations
local selected_booking_id = nil

-- Day names (used for schedule lookups)
local DAYS = {"sunday", "monday", "tuesday", "wednesday", "thursday", "friday", "saturday"}

-- Current date for day view navigation
local current_date = nil  -- datetime object

-- Selected slot for booking
local selected_slot_id = nil

-- Grid start hour (from schedule)
local grid_start_hour = 9

-- Initialize the app
function on_init()
    page_id = permit:page_id()
    my_did = permit:my_did()

    -- Schedule layer (read-only)
    schedule_layer = loro:get_layer(page_id .. "/schedule", "map")

    -- Blocked times (read-only)
    blocked_layer = loro:get_layer(page_id .. "/blocked", "list")

    -- Derived calendar showing booked slots (read-only, privacy-protected)
    calendar_layer = loro:get_layer(page_id .. "/derived/calendar", "map")

    -- My bookings layer
    local bookings_layer_name = permit:my_layer("bookings")
    bookings_layer = loro:get_or_create_layer(bookings_layer_name, "list")

    -- Local drafts (never syncs)
    drafts_layer = loro:get_or_create_layer(page_id .. "/drafts", "map")

    -- Initialize current date to today
    current_date = datetime()

    refresh_day_view()
    refresh_bookings_ui()
end

-- Called when any Loro layer changes
function on_loro_change(layer_name, change_type)
    if layer_name == page_id .. "/schedule" then
        refresh_day_view()
    elseif layer_name == page_id .. "/blocked" then
        refresh_day_view()
    elseif layer_name == page_id .. "/derived/calendar" then
        refresh_day_view()
    elseif layer_name == permit:my_layer("bookings") then
        refresh_bookings_ui()
    end
end

-- Called when a new layer is discovered
function on_layer_discovered(layer_name)
    if layer_name == page_id .. "/schedule" then
        schedule_layer = loro:get_layer(layer_name, "map")
        refresh_day_view()
    elseif layer_name == page_id .. "/blocked" then
        blocked_layer = loro:get_layer(layer_name, "list")
        refresh_day_view()
    elseif layer_name == page_id .. "/derived/calendar" then
        calendar_layer = loro:get_layer(layer_name, "map")
        refresh_day_view()
    end
end

-- ============================================================================
-- EVENT BUS HANDLERS
-- ============================================================================

function on_field_changed(field_name, value)
    form_state[field_name] = value
end

function on_modal_action(modal_name, action)
    if modal_name == "booking" then
        if action == "open" then
            ui:set("booking_modal_visible", true)
        elseif action == "submit" then
            -- Submit directly to synced bookings layer (not drafts)
            do_submit_booking_direct()
            clear_booking_form()
        elseif action == "cancel" or action == "close" then
            clear_booking_form()
        end
    end
end

function on_click(target)
    local action, id = target:match("^([^:]+):?(.*)$")

    -- Navigation actions
    if action == "go_prev_day" then
        go_prev_day()
        return
    elseif action == "go_next_day" then
        go_next_day()
        return
    elseif action == "go_today" then
        go_today()
        return
    elseif action == "select_slot" then
        -- id is the slot_id from day_slots, format: "date_time" e.g., "2026-01-19_09:00"
        selected_slot_id = id
        ui:set("selected_slot_id", id)

        -- Parse slot_id to get date and time (works for both available and booked slots)
        local date_part, time_part = id:match("^(.+)_(%d+:%d+)$")
        if date_part and time_part then
            form_state.booking_date = date_part
            form_state.booking_time = time_part
            ui:set("selected_date", date_part)

            -- Calculate end time using schedule slot duration
            local slot_duration = 60
            if schedule_layer then
                local day_of_week = datetime(date_part):getweekday()
                local day_name = DAYS[day_of_week]
                local schedule = schedule_layer:get(day_name)
                if schedule then
                    slot_duration = schedule.slot_duration or 60
                end
            end
            local end_time = calculate_end_time(time_part, slot_duration)
            ui:set("selected_time", time_part .. " - " .. end_time)
        end
    elseif action == "book_slot" then
        on_modal_action("booking", "open")
    elseif action == "cancel_booking" then
        do_cancel_booking(id)
    elseif action == "submit_booking" then
        do_submit_booking(id)
    end
end

-- Day navigation callbacks
function go_prev_day()
    if current_date then
        current_date = current_date:copy():adddays(-1)
        refresh_day_view()
    end
end

function go_next_day()
    if current_date then
        current_date = current_date:copy():adddays(1)
        refresh_day_view()
    end
end

function go_today()
    current_date = datetime()
    refresh_day_view()
end

-- ============================================================================
-- CORE LOGIC
-- ============================================================================

-- Submit booking directly to synced bookings layer (skips drafts)
-- This goes to {page_id}/bookings/{my_did} - accessible only by booker and owner
function do_submit_booking_direct()
    if permit:role() ~= "customer" then
        log_warn("Only customers can create bookings")
        return
    end

    if not bookings_layer then
        log_warn("Bookings layer not available")
        return
    end

    local date = form_state.booking_date or ""
    local time = form_state.booking_time or ""
    if date == "" or time == "" then
        log_warn("Date and time required")
        return
    end

    -- Get duration from UI (15, 30, 45, or 60 minutes)
    local duration = ui:get("slot_duration") or 60

    -- Calculate end time based on selected duration
    local end_time = calculate_end_time(time, duration)

    local booking = {
        id = generate_id(),
        date = date,
        start_time = time,
        end_time = end_time,
        duration = duration,
        service = form_state.booking_service or "Consultation",
        notes = form_state.booking_notes or "",
        customer_name = form_state.customer_name or my_did,
        customer_phone = form_state.customer_phone or "",
        status = "pending",  -- Directly pending (not draft)
        created_at = os.date("%Y-%m-%d %H:%M:%S")
    }

    -- Push directly to synced bookings layer
    -- This is {page_id}/bookings/{my_did} - syncs to kunki and owner
    bookings_layer:push(booking)
    selected_booking_id = booking.id

    log_info("Booking submitted to " .. permit:my_layer("bookings") .. ": " .. booking.id)
    refresh_bookings_ui()
    refresh_day_view()
end

-- Create a new booking draft (local only, for two-step flow)
function do_create_booking()
    if permit:role() ~= "customer" then
        log_warn("Only customers can create bookings")
        return
    end

    if not drafts_layer then
        log_warn("Drafts layer not available")
        return
    end

    local date = form_state.booking_date or ""
    local time = form_state.booking_time or ""
    if date == "" or time == "" then
        log_warn("Date and time required")
        return
    end

    -- Get duration from UI
    local duration = ui:get("slot_duration") or 60
    local end_time = calculate_end_time(time, duration)

    local booking = {
        id = generate_id(),
        date = date,
        start_time = time,
        end_time = end_time,
        duration = duration,
        service = form_state.booking_service or "Consultation",
        notes = form_state.booking_notes or "",
        customer_name = form_state.customer_name or "",
        customer_phone = form_state.customer_phone or "",
        status = "draft",
        created_at = os.date("%Y-%m-%d %H:%M:%S")
    }

    -- Store as draft (local only)
    drafts_layer:set(booking.id, booking)
    selected_booking_id = booking.id

    log_info("Booking draft created: " .. booking.id)
    refresh_bookings_ui()
end

-- Submit booking (move from drafts to synced bookings)
function do_submit_booking(booking_id)
    if permit:role() ~= "customer" then
        log_warn("Only customers can submit bookings")
        return
    end

    if not bookings_layer or not drafts_layer then return end

    local target_id = booking_id
    if not target_id or target_id == "" then
        target_id = selected_booking_id
    end
    if not target_id then
        log_warn("No booking selected to submit")
        return
    end

    -- Get draft
    local booking = drafts_layer:get(target_id)
    if not booking then
        log_warn("Draft not found: " .. target_id)
        return
    end

    -- Validate required fields
    if not booking.date or booking.date == "" then
        log_warn("Booking must have a date")
        return
    end
    if not booking.customer_name or booking.customer_name == "" then
        log_warn("Booking must have customer name")
        return
    end

    -- Update status and submit
    booking.status = "pending"
    booking.submitted_at = os.date("%Y-%m-%d %H:%M:%S")

    -- Push to synced bookings
    bookings_layer:push(booking)

    -- Remove from drafts
    drafts_layer:delete(target_id)

    log_info("Booking submitted: " .. target_id)
    refresh_bookings_ui()
end

-- Cancel own booking
function do_cancel_booking(booking_id)
    if permit:role() ~= "customer" then
        log_warn("Only customers can cancel their bookings")
        return
    end

    local target_id = booking_id
    if not target_id or target_id == "" then
        target_id = selected_booking_id
    end
    if not target_id then
        log_warn("No booking to cancel")
        return
    end

    -- First check drafts
    if drafts_layer then
        local draft = drafts_layer:get(target_id)
        if draft then
            drafts_layer:delete(target_id)
            log_info("Draft deleted: " .. target_id)
            refresh_bookings_ui()
            return
        end
    end

    -- Check synced bookings
    if not bookings_layer then return end

    local len = bookings_layer:length()
    for i = 0, len - 1 do
        local booking = bookings_layer:get(i)
        if booking and booking.id == target_id then
            local status = booking.status
            -- Can only cancel pending bookings
            if status == "pending" then
                booking.status = "cancelled"
                booking.cancelled_at = os.date("%Y-%m-%d %H:%M:%S")
                bookings_layer:set(i, booking)
                log_info("Booking cancelled: " .. target_id)
            else
                log_warn("Cannot cancel booking in " .. status .. " state")
            end
            refresh_bookings_ui()
            return
        end
    end

    log_warn("Booking not found: " .. target_id)
end

-- Clear booking form
function clear_booking_form()
    form_state.booking_date = ""
    form_state.booking_time = ""
    form_state.booking_service = ""
    form_state.booking_notes = ""

    selected_slot_id = nil
    ui:set("selected_slot_id", "")
    ui:set("selected_date", "")
    ui:set("selected_time", "")
    ui:set("booking_modal_visible", false)
end

-- ============================================================================
-- AI AUTOMATION API
-- ============================================================================

-- Book a slot via UI simulation
function book_slot_via_ui(date, time, service, notes, name, phone)
    -- Select the slot
    on_click("select_slot:" .. date .. "_" .. time)

    -- Fill form
    on_field_changed("booking_service", service or "Consultation")
    on_field_changed("booking_notes", notes or "")
    on_field_changed("customer_name", name or "")
    on_field_changed("customer_phone", phone or "")

    -- Open modal and submit
    on_modal_action("booking", "open")
    on_modal_action("booking", "submit")

    -- Submit the draft
    if selected_booking_id then
        do_submit_booking(selected_booking_id)
    end

    return true
end

-- Cancel a booking
function cancel_booking_via_ui(booking_id)
    on_click("cancel_booking:" .. booking_id)
    return true
end

-- ============================================================================
-- UI REFRESH
-- ============================================================================

-- Refresh the single day view
function refresh_day_view()
    if not current_date then
        current_date = datetime()
    end

    local date_str = current_date:fmt("%Y-%m-%d")
    local day_label = current_date:fmt("%A, %B %d, %Y")

    ui:set("current_date", date_str)
    ui:set("current_day_label", day_label)

    -- Get slots for this day
    local day_slots = get_slots_for_date(date_str)
    ui:set("day_slots", day_slots)

    -- Generate time slots from schedule
    local time_slots, start_hour = generate_time_slots()
    ui:set("time_slots", time_slots)
    ui:set("grid_start_hour", start_hour)
    grid_start_hour = start_hour

    -- Update current time for the red indicator line
    local now = datetime()
    ui:set("current_hour", now:gethours())
    ui:set("current_minute", now:getminutes())
end

-- Get slots for a specific date - generates from schedule and marks booked ones
function get_slots_for_date(date_str)
    local slots = {}

    -- Build lookup of booked slots from calendar_layer
    local booked_lookup = {}
    if calendar_layer then
        local keys = calendar_layer:keys()
        for _, key in ipairs(keys or {}) do
            local slot = calendar_layer:get(key)
            if slot and slot.date == date_str then
                local lookup_key = slot.start_time
                booked_lookup[lookup_key] = true
            end
        end
    end

    -- Get day of week for the date
    local dt = datetime(date_str)
    local day_of_week = dt:getweekday()  -- 1=Sunday, 2=Monday, etc.
    local day_name = DAYS[day_of_week]

    -- Get schedule for this day
    if not schedule_layer then
        return slots
    end

    local schedule = schedule_layer:get(day_name)
    if not schedule or not schedule.enabled then
        return slots  -- No schedule for this day
    end

    -- Parse schedule times
    local start_h, start_m = (schedule.start or "09:00"):match("(%d+):(%d+)")
    local end_h, end_m = (schedule["end"] or "17:00"):match("(%d+):(%d+)")
    local slot_duration = schedule.slot_duration or 60

    local schedule_start_minutes = tonumber(start_h) * 60 + tonumber(start_m)
    local schedule_end_minutes = tonumber(end_h) * 60 + tonumber(end_m)
    local grid_start_minutes = grid_start_hour * 60

    -- Generate slots based on schedule
    local current_minutes = schedule_start_minutes
    while current_minutes + slot_duration <= schedule_end_minutes do
        local slot_start_h = math.floor(current_minutes / 60)
        local slot_start_m = current_minutes % 60
        local slot_end_minutes = current_minutes + slot_duration
        local slot_end_h = math.floor(slot_end_minutes / 60)
        local slot_end_m = slot_end_minutes % 60

        local start_time = string.format("%02d:%02d", slot_start_h, slot_start_m)
        local end_time = string.format("%02d:%02d", slot_end_h, slot_end_m)

        -- Check if this slot is booked
        local status = "available"
        if booked_lookup[start_time] then
            status = "booked"
        end

        table.insert(slots, {
            slot_id = date_str .. "_" .. start_time,
            minutes_from_grid_start = current_minutes - grid_start_minutes,
            duration_minutes = slot_duration,
            status = status,
            label = start_time .. " - " .. end_time,
        })

        current_minutes = current_minutes + slot_duration
    end

    return slots
end

-- Generate time slots based on schedule
-- Returns: slots array, start_hour
function generate_time_slots()
    local slots = {}
    local start_hour = 9
    local end_hour = 18  -- Default to 6pm

    -- Try to get actual schedule times
    if schedule_layer then
        local earliest_start = 24
        local latest_end = 0

        for _, day in ipairs({"monday", "tuesday", "wednesday", "thursday", "friday"}) do
            local entry = schedule_layer:get(day)
            if entry and entry.enabled then
                local sh = tonumber(entry.start:match("(%d+):")) or 9
                local eh = tonumber(entry["end"]:match("(%d+):")) or 18
                if sh < earliest_start then earliest_start = sh end
                if eh > latest_end then latest_end = eh end
            end
        end

        if earliest_start < 24 then start_hour = earliest_start end
        if latest_end > 0 then end_hour = latest_end end
    end

    -- Ensure we show at least 9am-6pm range
    if end_hour < 18 then end_hour = 18 end

    -- Generate hourly slots
    for hour = start_hour, end_hour - 1 do
        table.insert(slots, string.format("%02d:00", hour))
    end

    return slots, start_hour
end

-- Check if a specific slot is booked
function is_slot_booked(date, time)
    local key = date .. "_" .. time
    return booked_lookup[key] == true
end

function refresh_bookings_ui()
    local all_bookings = {}

    -- Add drafts
    if drafts_layer then
        local keys = drafts_layer:keys()
        if keys then
            for _, key in ipairs(keys) do
                local draft = drafts_layer:get(key)
                if draft then
                    table.insert(all_bookings, {
                        id = draft.id or "",
                        date = draft.date or "",
                        start_time = draft.start_time or "",
                        end_time = draft.end_time or "",
                        service = draft.service or "",
                        notes = draft.notes or "",
                        status = draft.status or "draft",
                        created_at = draft.created_at or ""
                    })
                end
            end
        end
    end

    -- Add synced bookings
    if bookings_layer then
        local len = bookings_layer:length()
        for i = 0, len - 1 do
            local booking = bookings_layer:get(i)
            if booking then
                table.insert(all_bookings, {
                    id = booking.id or "",
                    date = booking.date or "",
                    start_time = booking.start_time or "",
                    end_time = booking.end_time or "",
                    service = booking.service or "",
                    notes = booking.notes or "",
                    status = booking.status or "pending",
                    created_at = booking.created_at or ""
                })
            end
        end
    end

    -- Sort by date/time
    table.sort(all_bookings, function(a, b)
        if a.date == b.date then
            return a.start_time < b.start_time
        end
        return a.date < b.date
    end)

    ui:set("my_bookings", all_bookings)
end

-- ============================================================================
-- TEST HELPERS
-- ============================================================================

function get_my_bookings()
    local bookings = {}

    if drafts_layer then
        local keys = drafts_layer:keys()
        if keys then
            for _, key in ipairs(keys) do
                local draft = drafts_layer:get(key)
                if draft then
                    table.insert(bookings, draft)
                end
            end
        end
    end

    if bookings_layer then
        local len = bookings_layer:length()
        for i = 0, len - 1 do
            local booking = bookings_layer:get(i)
            if booking then
                table.insert(bookings, booking)
            end
        end
    end

    return bookings
end

function get_bookings_count()
    local count = 0
    if drafts_layer then
        local keys = drafts_layer:keys()
        if keys then count = count + #keys end
    end
    if bookings_layer then
        count = count + bookings_layer:length()
    end
    return count
end

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

function get_booked_slots()
    local booked = {}
    if calendar_layer then
        local keys = calendar_layer:keys()
        if keys then
            for _, key in ipairs(keys) do
                local slot = calendar_layer:get(key)
                if slot then
                    table.insert(booked, slot)
                end
            end
        end
    end
    return booked
end

-- ============================================================================
-- UTILITIES
-- ============================================================================

function calculate_end_time(start_time, duration_minutes)
    -- Use datetime library for time arithmetic
    local t = datetime(start_time)
    t:addminutes(duration_minutes)
    return t:fmt("%H:%M")
end

function generate_id()
    return string.format("%x", os.time()) .. "-" .. string.format("%04x", math.random(0, 65535))
end

function log_info(msg)
    print("[INFO] " .. msg)
end

function log_warn(msg)
    print("[WARN] " .. msg)
end
