-- Waitlist Signup App Logic
-- Shared data layer: "entries" (LoroList of signups)

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

-- Handle changes to entries layer
function handle_entries_change()
    local entries_list = loro:get_list("entries")
    local count = entries_list:len()

    print("Entries count: " .. count)

    -- Update the entry count in UI
    return {
        properties = {
            { key = "entry_count", value = count },
        },
        models = {}
    }
end

-- ============================================================================
-- UI CALLBACKS
-- ============================================================================

-- Called when user clicks "Join Waitlist" button
-- Arguments: name (string), email (string)
function on_submit_entry(name, email)
    print("========================================")
    print("SUBMIT ENTRY: " .. name .. " <" .. email .. ">")

    -- Validate inputs
    if not name or name == "" then
        return {
            properties = {
                { key = "status_message", value = "Please enter your name" },
            },
            models = {}
        }
    end

    if not email or email == "" or not email:find("@") then
        return {
            properties = {
                { key = "status_message", value = "Please enter a valid email" },
            },
            models = {}
        }
    end

    -- Check for duplicate email
    local entries = loro:get_list("entries")
    for i = 0, entries:len() - 1 do
        local entry = entries:get(i)
        if entry and entry.email == email then
            print("Duplicate email: " .. email)
            return {
                properties = {
                    { key = "status_message", value = "Email already registered" },
                },
                models = {}
            }
        end
    end

    -- Add new entry to Loro
    local entry_id = "entry_" .. (entries:len() + 1)
    entries:push({
        id = entry_id,
        name = name,
        email = email,
        timestamp = os.time(),
    })

    print("Entry added with ID: " .. entry_id)

    return {
        properties = {
            { key = "status_message", value = "Added to waitlist!" },
        },
        models = {}
    }
end
