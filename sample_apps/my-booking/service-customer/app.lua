-- Service Customer App Logic
-- Book appointments and manage own bookings
-- Uses scribe:bind() for declarative layer-UI sync where applicable

local BOOKING_STATES = {
    draft = { writable = {'date', 'start_time', 'end_time', 'service', 'notes', 'customer_name', 'customer_phone'}, customer_transitions = {'pending'} },
    pending = { writable = {'notes'}, customer_transitions = {'cancelled'} },
    confirmed = { writable = {}, customer_transitions = {} },
    completed = { writable = {}, customer_transitions = {} },
    cancelled = { writable = {}, customer_transitions = {} }
}

local page_id = nil
local my_did = nil
local schedule_layer = nil
local blocked_layer = nil
local calendar_layer = nil
local bookings_layer = nil
local drafts_layer = nil

local form_state = {
    booking_date = "", booking_time = "", booking_service = "", booking_notes = "",
    customer_name = "", customer_phone = ""
}

local selected_booking_id = nil
local DAYS = {"sunday", "monday", "tuesday", "wednesday", "thursday", "friday", "saturday"}
local current_date = nil
local selected_slot_id = nil
local grid_start_hour = 9

function on_init()
    page_id = scribe:page_id()
    my_did = scribe:my_did()

    schedule_layer = scribe:map(page_id .. "/schedule")
    blocked_layer = scribe:list(page_id .. "/blocked")
    calendar_layer = scribe:map(page_id .. "/derived/calendar")

    local bookings_layer_name = page_id .. "/bookings/" .. my_did
    bookings_layer = scribe:list(bookings_layer_name)
    drafts_layer = scribe:map(page_id .. "/drafts")

    current_date = datetime()
    refresh_day_view()
    refresh_bookings_ui()
end

function on_field_changed(field_name, value) form_state[field_name] = value end

function on_modal_action(modal_name, action)
    if modal_name == "booking" then
        if action == "open" then ui:set("booking_modal_visible", true)
        elseif action == "submit" then do_submit_booking_direct(); clear_booking_form()
        elseif action == "cancel" or action == "close" then clear_booking_form() end
    end
end

function on_click(target)
    local action, id = target:match("^([^:]+):?(.*)$")
    if action == "go_prev_day" then go_prev_day()
    elseif action == "go_next_day" then go_next_day()
    elseif action == "go_today" then go_today()
    elseif action == "select_slot" then
        selected_slot_id = id
        ui:set("selected_slot_id", id)
        local date_part, time_part = id:match("^(.+)_(%d+:%d+)$")
        if date_part and time_part then
            form_state.booking_date = date_part
            form_state.booking_time = time_part
            ui:set("selected_date", date_part)
            local slot_duration = 60
            if schedule_layer then
                local day_of_week = datetime(date_part):getweekday()
                local day_name = DAYS[day_of_week]
                local schedule = schedule_layer:get(day_name)
                if schedule then slot_duration = schedule.slot_duration or 60 end
            end
            local end_time = calculate_end_time(time_part, slot_duration)
            ui:set("selected_time", time_part .. " - " .. end_time)
        end
    elseif action == "book_slot" then on_modal_action("booking", "open")
    elseif action == "cancel_booking" then do_cancel_booking(id)
    elseif action == "submit_booking" then do_submit_booking(id) end
end

function go_prev_day() if current_date then current_date = current_date:copy():adddays(-1); refresh_day_view() end end
function go_next_day() if current_date then current_date = current_date:copy():adddays(1); refresh_day_view() end end
function go_today() current_date = datetime(); refresh_day_view() end

function do_submit_booking_direct()
    if permit:role() ~= "customer" then log_warn("Only customers can create bookings"); return end
    if not bookings_layer then log_warn("Bookings layer not available"); return end

    local date = form_state.booking_date or ""
    local time = form_state.booking_time or ""
    if date == "" or time == "" then log_warn("Date and time required"); return end

    local duration = ui:get("slot_duration") or 60
    local end_time = calculate_end_time(time, duration)

    local booking = {
        id = generate_id(), date = date, start_time = time, end_time = end_time, duration = duration,
        service = form_state.booking_service or "Consultation", notes = form_state.booking_notes or "",
        customer_name = form_state.customer_name or my_did, customer_phone = form_state.customer_phone or "",
        status = "pending", created_at = os.date("%Y-%m-%d %H:%M:%S")
    }
    bookings_layer:push(booking)
    selected_booking_id = booking.id
    log_info("Booking submitted: " .. booking.id)
    refresh_bookings_ui()
    refresh_day_view()
end

function do_create_booking()
    if permit:role() ~= "customer" then log_warn("Only customers can create bookings"); return end
    if not drafts_layer then log_warn("Drafts layer not available"); return end

    local date = form_state.booking_date or ""
    local time = form_state.booking_time or ""
    if date == "" or time == "" then log_warn("Date and time required"); return end

    local duration = ui:get("slot_duration") or 60
    local end_time = calculate_end_time(time, duration)

    local booking = {
        id = generate_id(), date = date, start_time = time, end_time = end_time, duration = duration,
        service = form_state.booking_service or "Consultation", notes = form_state.booking_notes or "",
        customer_name = form_state.customer_name or "", customer_phone = form_state.customer_phone or "",
        status = "draft", created_at = os.date("%Y-%m-%d %H:%M:%S")
    }
    drafts_layer:set(booking.id, booking)
    selected_booking_id = booking.id
    log_info("Booking draft created: " .. booking.id)
    refresh_bookings_ui()
end

function do_submit_booking(booking_id)
    if permit:role() ~= "customer" then log_warn("Only customers can submit bookings"); return end
    if not bookings_layer or not drafts_layer then return end

    local target_id = booking_id
    if not target_id or target_id == "" then target_id = selected_booking_id end
    if not target_id then log_warn("No booking selected to submit"); return end

    local booking = drafts_layer:get(target_id)
    if not booking then log_warn("Draft not found: " .. target_id); return end

    if not booking.date or booking.date == "" then log_warn("Booking must have a date"); return end
    if not booking.customer_name or booking.customer_name == "" then log_warn("Booking must have customer name"); return end

    booking.status = "pending"
    booking.submitted_at = os.date("%Y-%m-%d %H:%M:%S")
    bookings_layer:push(booking)
    drafts_layer:delete(target_id)

    log_info("Booking submitted: " .. target_id)
    refresh_bookings_ui()
end

function do_cancel_booking(booking_id)
    if permit:role() ~= "customer" then log_warn("Only customers can cancel their bookings"); return end

    local target_id = booking_id
    if not target_id or target_id == "" then target_id = selected_booking_id end
    if not target_id then log_warn("No booking to cancel"); return end

    if drafts_layer then
        local draft = drafts_layer:get(target_id)
        if draft then
            drafts_layer:delete(target_id)
            log_info("Draft deleted: " .. target_id)
            refresh_bookings_ui()
            return
        end
    end

    if not bookings_layer then return end
    local len = bookings_layer:length()
    for i = 0, len - 1 do
        local booking = bookings_layer:get(i)
        if booking and booking.id == target_id then
            local status = booking.status
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

function book_slot_via_ui(date, time, service, notes, name, phone)
    on_click("select_slot:" .. date .. "_" .. time)
    on_field_changed("booking_service", service or "Consultation")
    on_field_changed("booking_notes", notes or "")
    on_field_changed("customer_name", name or "")
    on_field_changed("customer_phone", phone or "")
    on_modal_action("booking", "open")
    on_modal_action("booking", "submit")
    if selected_booking_id then do_submit_booking(selected_booking_id) end
    return true
end

function cancel_booking_via_ui(booking_id)
    on_click("cancel_booking:" .. booking_id)
    return true
end

function refresh_day_view()
    if not current_date then current_date = datetime() end
    local date_str = current_date:fmt("%Y-%m-%d")
    local day_label = current_date:fmt("%A, %B %d, %Y")
    ui:set("current_date", date_str)
    ui:set("current_day_label", day_label)

    local day_slots = get_slots_for_date(date_str)
    ui:set("day_slots", day_slots)

    local time_slots, start_hour = generate_time_slots()
    ui:set("time_slots", time_slots)
    ui:set("grid_start_hour", start_hour)
    grid_start_hour = start_hour

    local now = datetime()
    ui:set("current_hour", now:gethours())
    ui:set("current_minute", now:getminutes())
end

function get_slots_for_date(date_str)
    local slots = {}
    local booked_lookup = {}
    if calendar_layer then
        local keys = calendar_layer:keys()
        for _, key in ipairs(keys or {}) do
            local slot = calendar_layer:get(key)
            if slot and slot.date == date_str then booked_lookup[slot.start_time] = true end
        end
    end

    local dt = datetime(date_str)
    local day_of_week = dt:getweekday()
    local day_name = DAYS[day_of_week]

    if not schedule_layer then return slots end
    local schedule = schedule_layer:get(day_name)
    if not schedule or not schedule.enabled then return slots end

    local start_h, start_m = (schedule.start or "09:00"):match("(%d+):(%d+)")
    local end_h, end_m = (schedule["end"] or "17:00"):match("(%d+):(%d+)")
    local slot_duration = schedule.slot_duration or 60

    local schedule_start_minutes = tonumber(start_h) * 60 + tonumber(start_m)
    local schedule_end_minutes = tonumber(end_h) * 60 + tonumber(end_m)
    local grid_start_minutes = grid_start_hour * 60

    local current_minutes = schedule_start_minutes
    while current_minutes + slot_duration <= schedule_end_minutes do
        local slot_start_h = math.floor(current_minutes / 60)
        local slot_start_m = current_minutes % 60
        local slot_end_minutes = current_minutes + slot_duration
        local slot_end_h = math.floor(slot_end_minutes / 60)
        local slot_end_m = slot_end_minutes % 60

        local start_time = string.format("%02d:%02d", slot_start_h, slot_start_m)
        local end_time = string.format("%02d:%02d", slot_end_h, slot_end_m)

        local status = booked_lookup[start_time] and "booked" or "available"
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

function generate_time_slots()
    local slots = {}
    local start_hour = 9
    local end_hour = 18

    if schedule_layer then
        local earliest_start, latest_end = 24, 0
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

    if end_hour < 18 then end_hour = 18 end
    for hour = start_hour, end_hour - 1 do
        table.insert(slots, string.format("%02d:00", hour))
    end
    return slots, start_hour
end

function refresh_bookings_ui()
    local all_bookings = {}

    if drafts_layer then
        local keys = drafts_layer:keys()
        if keys then
            for _, key in ipairs(keys) do
                local draft = drafts_layer:get(key)
                if draft then
                    table.insert(all_bookings, {
                        id = draft.id or "", date = draft.date or "", start_time = draft.start_time or "",
                        end_time = draft.end_time or "", service = draft.service or "", notes = draft.notes or "",
                        status = draft.status or "draft", created_at = draft.created_at or ""
                    })
                end
            end
        end
    end

    if bookings_layer then
        local len = bookings_layer:length()
        for i = 0, len - 1 do
            local booking = bookings_layer:get(i)
            if booking then
                table.insert(all_bookings, {
                    id = booking.id or "", date = booking.date or "", start_time = booking.start_time or "",
                    end_time = booking.end_time or "", service = booking.service or "", notes = booking.notes or "",
                    status = booking.status or "pending", created_at = booking.created_at or ""
                })
            end
        end
    end

    table.sort(all_bookings, function(a, b)
        if a.date == b.date then return a.start_time < b.start_time end
        return a.date < b.date
    end)
    ui:set("my_bookings", all_bookings)
end

function get_my_bookings()
    local bookings = {}
    if drafts_layer then
        local keys = drafts_layer:keys()
        if keys then
            for _, key in ipairs(keys) do
                local draft = drafts_layer:get(key)
                if draft then table.insert(bookings, draft) end
            end
        end
    end
    if bookings_layer then
        local len = bookings_layer:length()
        for i = 0, len - 1 do
            local booking = bookings_layer:get(i)
            if booking then table.insert(bookings, booking) end
        end
    end
    return bookings
end

function get_bookings_count()
    local count = 0
    if drafts_layer then local keys = drafts_layer:keys(); if keys then count = count + #keys end end
    if bookings_layer then count = count + bookings_layer:length() end
    return count
end

function get_schedule()
    local schedule = {}
    if schedule_layer then
        for _, day in ipairs(DAYS) do
            local entry = schedule_layer:get(day)
            if entry then schedule[day] = entry end
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
                if slot then table.insert(booked, slot) end
            end
        end
    end
    return booked
end

function calculate_end_time(start_time, duration_minutes)
    local t = datetime(start_time)
    t:addminutes(duration_minutes)
    return t:fmt("%H:%M")
end

function generate_id()
    return string.format("%x", os.time()) .. "-" .. string.format("%04x", math.random(0, 65535))
end

function log_info(msg) print("[INFO] " .. msg) end
function log_warn(msg) print("[WARN] " .. msg) end
