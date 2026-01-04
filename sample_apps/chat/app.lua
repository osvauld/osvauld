-- Chat App Logic (Stateful + Stateless Hybrid)
-- This Lua script runs in a sandboxed OS thread worker
--
-- **Architecture**:
-- - Loro CRDT: Single source of truth for data (messages, ui_state)
-- - Lua: Business logic, transformations, configuration
-- - Stateful: Configuration, caches, derived state
-- - Stateless: No duplication of Loro data

-- ============================================================================
-- CONFIGURATION STATE (Stateful - doesn't duplicate Loro data)
-- ============================================================================
local config = {
    -- App settings (not in Loro, just configuration)
    max_visible_messages = 100,
    enable_spam_filter = true,
    author_name = "You",

    -- Feature flags
    enable_timestamps = true,
    enable_read_receipts = false,
}

-- Cache for computed values (optional optimization)
local cache = {
    last_message_count = 0,
    filtered_count = 0,
}

-- ============================================================================
-- LORO EVENT HANDLERS (Stateless - pure functions)
-- ============================================================================

-- Called when Loro changes (from local writes OR peer sync)
-- **Pattern**: Query Loro → Compute → Return operations
--
-- Parameters:
--   layer_name: string - name of the changed layer
--   delta: table|nil - incremental delta operations (not implemented yet)
--   full_data: table - complete layer state (fallback)
--
-- Returns: Lua table with operations
-- {
--   properties = { { key = "draft", value = "" }, ... },
--   models = { { op = "push", model = "messages", item = {...} }, ... }
-- }
function on_loro_change(layer_name, delta, full_data)
    print("========================================")
    print("LORO CHANGE EVENT: " .. layer_name)

    if layer_name == "messages" then
        return handle_messages_change()
    elseif layer_name == "ui_state" then
        return handle_ui_state_change()
    end

    print("Unknown layer: " .. layer_name)
    print("========================================")
    return { properties = {}, models = {} }
end

-- Handle changes to messages layer
-- **Stateless**: Queries Loro, applies business logic, returns VecModel ops
function handle_messages_change()
    local handler_start = os.clock() * 1000

    -- 1. Query Loro (temporary, local variable)
    local messages_list = loro:get_list("messages")
    local total_count = messages_list:len()

    print("Messages layer changed at " .. string.format("%.3f", handler_start) .. "ms, total count: " .. total_count)

    -- 2. Apply business logic (filtering, transformations)
    local visible = {}
    local spam_count = 0

    for i = 0, total_count - 1 do
        local msg = messages_list:get(i)

        if msg and not msg.deleted then
            -- Apply spam filter (business logic in Lua)
            if config.enable_spam_filter and is_spam(msg) then
                print("Filtered spam message: " .. (msg.id or "unknown"))
                spam_count = spam_count + 1
            else
                -- Add timestamps if enabled
                if config.enable_timestamps and msg.timestamp then
                    msg.formatted_time = format_timestamp(msg.timestamp)
                end

                table.insert(visible, msg)

                -- Only show max_visible_messages
                if #visible >= config.max_visible_messages then
                    print("Reached max visible messages limit")
                    break
                end
            end
        end
    end

    -- Update cache
    cache.last_message_count = total_count
    cache.filtered_count = spam_count

    print("Visible messages: " .. #visible .. " (filtered " .. spam_count .. " spam)")

    -- 3. Return operations to update VecModel
    local ops = {
        properties = {
            -- Note: UI uses messages.length directly, no separate counter needed
            { key = "username", value = config.author_name },
        },
        models = {}
    }

    -- Clear and rebuild VecModel
    -- **Note**: Delta optimization can come later for Insert/Delete ops
    table.insert(ops.models, { op = "clear", model = "messages" })

    for _, msg in ipairs(visible) do
        table.insert(ops.models, {
            op = "push",
            model = "messages",
            item = {
                id = msg.id,
                author = msg.author,
                content = msg.content,
                -- Include formatted timestamp if available
                timestamp = msg.formatted_time,
            }
        })
    end

    local handler_end = os.clock() * 1000
    print("Returning " .. #visible .. " visible messages")
    print("Handler processing took: " .. string.format("%.3f", handler_end - handler_start) .. "ms")
    print("========================================")

    -- messages_list garbage collected here (no permanent storage!)
    return ops
end

-- Handle changes to ui_state layer (scroll position, draft, etc.)
-- **Stateless**: Queries Loro, returns property updates
function handle_ui_state_change()
    -- Try to get UI state layer (may not exist on first run)
    local success, ui_state = pcall(function()
        return loro:get_map("ui_state")
    end)

    if not success then
        print("ui_state layer doesn't exist yet, skipping")
        print("========================================")
        return { properties = {}, models = {} }
    end

    local draft = ui_state:get("draft") or ""

    print("UI state changed - draft length: " .. #draft)

    local ops = {
        properties = {
            { key = "draft", value = draft },
        },
        models = {}
    }

    print("UI state updated")
    print("========================================")

    return ops
end

-- ============================================================================
-- UI CALLBACKS (Can be stateful for local logic)
-- ============================================================================

-- Called when send button is clicked or Enter is pressed
-- **Pattern**: Receives draft from Slint UI, writes to Loro, returns UI update
-- **Arguments**: draft (string) - current draft text from Slint UI property
function on_send(draft)
    local start_time = os.clock() * 1000  -- Convert to milliseconds
    print("========================================")
    print("SEND BUTTON CLICKED at " .. string.format("%.3f", start_time) .. "ms")

    -- 1. Validate draft (passed as argument from Slint)
    draft = draft or ""

    if draft == "" or #draft == 0 then
        print("Draft is empty, ignoring")
        print("========================================")
        return { properties = {}, models = {} }
    end

    print("Sending message: " .. draft)

    -- 2. Use stateful config for author name
    local author = config.author_name

    -- 3. Write to Loro (this will trigger on_loro_change asynchronously)
    local messages = loro:get_list("messages")
    local message_id = "msg_" .. (messages:len() + 1)

    messages:push({
        id = message_id,
        author = author,
        content = draft,
        timestamp = os.time(),
        deleted = false,
    })

    local push_time = os.clock() * 1000
    print("Message written to Loro with ID: " .. message_id .. " at " .. string.format("%.3f", push_time) .. "ms")
    print("Loro push took: " .. string.format("%.3f", push_time - start_time) .. "ms")
    print("Draft cleared")
    print("========================================")

    -- 4. Return immediate UI update (clear draft)
    -- The Loro observer will fire and rebuild the message list
    return {
        properties = {
            { key = "draft", value = "" },
        },
        models = {}
    }

    -- Note: messages is garbage collected here!
end

-- ============================================================================
-- BUSINESS LOGIC HELPERS (Stateful - use config)
-- ============================================================================

-- Spam detection (uses config.enable_spam_filter)
function is_spam(message)
    if not message or not message.content then
        return false
    end

    local content_lower = message.content:lower()

    -- Simple spam rules
    local spam_keywords = { "spam", "viagra", "casino", "lottery", "prize" }

    for _, keyword in ipairs(spam_keywords) do
        if content_lower:find(keyword) then
            return true
        end
    end

    return false
end

-- Format Unix timestamp to readable time
function format_timestamp(timestamp)
    if not timestamp then
        return ""
    end

    -- Format as HH:MM (simple format)
    local time = os.date("*t", timestamp)
    return string.format("%02d:%02d", time.hour, time.min)
end

-- ============================================================================
-- CONFIGURATION HELPERS (Stateful API)
-- ============================================================================

-- Change configuration (these could be exposed to UI)
function set_author_name(name)
    config.author_name = name
    print("Author name changed to: " .. name)
end

function toggle_spam_filter()
    config.enable_spam_filter = not config.enable_spam_filter
    print("Spam filter: " .. (config.enable_spam_filter and "enabled" or "disabled"))

    -- Trigger re-render
    return handle_messages_change()
end

-- ============================================================================
-- SUMMARY OF STATEFULNESS
-- ============================================================================
--
-- ✅ STATEFUL (Good):
-- - config table: App settings, feature flags
-- - cache table: Computed values for optimization
-- - Local variables in functions
--
-- ❌ NOT DUPLICATING LORO DATA:
-- - NO: state.messages = {} (data lives in Loro)
-- - NO: state.draft = "" (lives in Loro's ui_state layer)
-- - Messages queried temporarily, then garbage collected
--
-- 🔄 PATTERN:
-- - Query Loro → Apply business logic → Return operations
-- - Loro is single source of truth for data
-- - Lua has business logic + configuration
-- ============================================================================

print("Chat app loaded with config:")
print("  - Author: " .. config.author_name)
print("  - Spam filter: " .. (config.enable_spam_filter and "enabled" or "disabled"))
print("  - Max messages: " .. config.max_visible_messages)
