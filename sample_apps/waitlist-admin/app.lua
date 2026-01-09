-- Waitlist Admin App Logic
-- Shared data layer: "entries" (LoroList of signups)
-- Displays all entries in a table with remove functionality

-- ============================================================================
-- LORO EVENT HANDLERS
-- ============================================================================

-- Called when Loro changes (from local writes OR peer sync)
function on_loro_change(layer_name, delta, full_data)
    print("========================================")
    print("LORO CHANGE: " .. layer_name)

    if layer_name == "entries" then
        return handle_entries_change()
    end

    return { properties = {}, models = {} }
end

-- Handle changes to entries layer - rebuild the entries model
function handle_entries_change()
    local entries_list = loro:get_list("entries")
    local count = entries_list:len()

    print("Entries count: " .. count)

    -- Build array of entry objects for the UI model
    local ops = {}

    -- Clear and rebuild model
    table.insert(ops, { op = "clear", model = "entries" })

    for i = 0, count - 1 do
        local entry = entries_list:get(i)
        if entry and not entry.deleted then
            table.insert(ops, {
                op = "push",
                model = "entries",
                item = {
                    id = entry.id or ("entry_" .. i),
                    name = entry.name or "Unknown",
                    email = entry.email or "unknown@email.com",
                    timestamp = format_timestamp(entry.timestamp),
                }
            })
        end
    end

    return {
        properties = {
            { key = "entry_count", value = count },
        },
        models = ops
    }
end

-- ============================================================================
-- UI CALLBACKS
-- ============================================================================

-- Called when admin clicks "Remove" button
function on_remove_entry(entry_id)
    print("========================================")
    print("REMOVE ENTRY: " .. entry_id)

    -- Find and mark entry as deleted (soft delete)
    local entries = loro:get_list("entries")
    for i = 0, entries:len() - 1 do
        local entry = entries:get(i)
        if entry and entry.id == entry_id then
            -- For Loro, we can't delete items from list, so mark as deleted
            entry.deleted = true
            entries:set(i, entry)
            print("Marked entry as deleted: " .. entry_id)
            break
        end
    end

    return { properties = {}, models = {} }
end

-- Called when admin clicks "Refresh" button
function on_refresh_entries()
    print("========================================")
    print("REFRESH ENTRIES")

    -- Trigger a re-read of the entries
    return handle_entries_change()
end

-- ============================================================================
-- HELPERS
-- ============================================================================

-- Format Unix timestamp to readable date
function format_timestamp(timestamp)
    if not timestamp then
        return "-"
    end

    -- Simple date format: YYYY-MM-DD HH:MM
    local date = os.date("*t", timestamp)
    return string.format("%04d-%02d-%02d %02d:%02d",
        date.year, date.month, date.day, date.hour, date.min)
end
