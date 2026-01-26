-- Group Chat App Logic
-- Real-time messaging with emoji and typing indicators
-- Emoji shortcode mapping (UI name -> emoji shortcode)
local EMOJI_SHORTCODES = {
    Smile = "grinning_face",
    Laugh = "joy",
    Heart = "heart",
    Thumbs = "thumbsup",
    Fire = "fire",
    Party = "tada",
    Rocket = "rocket",
    Star = "star",
    Check = "white_check_mark",
    Wave = "wave",
    Eyes = "eyes",
    Think = "thinking",
    Clap = "clap",
    ["100"] = "100",
    Sad = "cry",
    Angry = "angry",
    Cool = "sunglasses",
    Wink = "wink",
    LOL = "rofl",
    Love = "heart_eyes",
}
-- State
local page_id = nil
local messages_layer = nil
local reactions_layer = nil
local my_did = nil
local my_short_did = nil
-- Typing indicator state
local typing_users = {}  -- did -> timestamp
local last_typing_sent = 0
-- Initialize the app
function on_init()
    page_id = permit:page_id()
    my_did = permit:my_did()
    -- Use USERNAME global if set, otherwise short DID
    my_short_did = USERNAME or my_did:sub(-8)
    -- Messages layer (synced)
    messages_layer = loro:get_or_create_layer(page_id .. "/messages", "list")
    -- Reactions layer (synced)
    reactions_layer = loro:get_or_create_layer(page_id .. "/reactions", "map")
    -- Load existing messages
    refresh_messages_ui()
    -- Initialize online count to 1 (yourself)
    ui:set("online_count", 1)
end
-- Refresh messages from Loro
function refresh_messages_ui()
    local all_messages = {}
    local len = messages_layer:length()
    for i = 0, len - 1 do
        local msg = messages_layer:get(i)
        if msg then
            local is_mine = msg.sender_did == my_did
            local reactions = get_reactions_for_message(msg.id)
            table.insert(all_messages, {
                id = msg.id or "",
                sender = msg.sender_did or "",
                -- Show "You" for own messages, actual name for others
                sender_short = is_mine and "You" or (msg.sender_name or "???"),
                text = msg.text or "",
                timestamp = format_time(msg.timestamp),
                is_mine = is_mine,
                deleted = msg.deleted or false,
                reply_to = msg.reply_to or "",
                reply_preview = msg.reply_preview or "",
                reactions = reactions,
            })
        end
    end
    ui:set("messages", all_messages)
end
-- Format timestamp
function format_time(ts)
    if not ts then return "" end
    return os.date("%H:%M", ts)
end
-- Generate unique ID
function generate_id()
    return string.format("%x-%04x", os.time(), math.random(0, 65535))
end
-- Truncate text for previews
function truncate_text(text, max_len)
    if #text > max_len then
        return text:sub(1, max_len) .. "..."
    end
    return text
end

-- Send a message
function send_message(text)
    if not text or text == "" then return end

    local replying_to = ui:get("replying_to_id") or ""
    local reply_preview = ui:get("replying_to_text") or ""

    local msg = {
        id = generate_id(),
        sender_did = my_did,
        sender_name = my_short_did,
        text = text,
        timestamp = os.time(),
        deleted = false,
    }

    -- Add reply fields if replying
    if replying_to ~= "" then
        msg.reply_to = replying_to
        msg.reply_preview = truncate_text(reply_preview, 30)
    end

    messages_layer:push(msg)
    refresh_messages_ui()
    -- Clear draft and reply state
    ui:set("draft_text", "")
    ui:set("show_emoji_picker", false)
    ui:set("replying_to_id", "")
    ui:set("replying_to_text", "")
end
-- Insert emoji into draft (uses emoji binding for Unicode lookup)
function insert_emoji(emoji_name)
    local shortcode = EMOJI_SHORTCODES[emoji_name]
    local emoji_char = shortcode and emoji:get(shortcode)
    -- Fallback to name if emoji not found
    local to_insert = emoji_char or emoji_name
    local current = ui:get("draft_text") or ""
    ui:set("draft_text", current .. to_insert)
end
-- Handle click events
function on_click(target)
    if target == "emoji_picker:toggle" then
        local current = ui:get("show_emoji_picker")
        ui:set("show_emoji_picker", not current)
    elseif target == "emoji_picker:close" then
        ui:set("show_emoji_picker", false)
    elseif target:match("^emoji:") then
        local emoji_name = target:sub(7)
        -- Check if we're in reaction mode
        local selected_msg = ui:get("selected_message_id") or ""
        if selected_msg ~= "" then
            -- Get emoji character for reaction
            local shortcode = EMOJI_SHORTCODES[emoji_name]
            local emoji_char = shortcode and emoji:get(shortcode) or emoji_name
            toggle_reaction(selected_msg, emoji_char)
            ui:set("show_emoji_picker", false)
            ui:set("selected_message_id", "")
        else
            -- Insert emoji into text input
            insert_emoji(emoji_name)
            ui:set("show_emoji_picker", false)
        end
    -- Toggle context menu
    elseif target:match("^toggle_menu:") then
        local msg_id = target:sub(13)  -- Extract message ID after "toggle_menu:"
        local current_id = ui:get("selected_message_id") or ""
        if current_id == msg_id and ui:get("context_menu_visible") then
            -- Close menu if clicking same message
            ui:set("context_menu_visible", false)
            ui:set("selected_message_id", "")
        else
            -- Open menu for this message
            ui:set("selected_message_id", msg_id)
            ui:set("context_menu_visible", true)
        end
    -- Context menu handlers
    elseif target == "context_menu:close" then
        ui:set("context_menu_visible", false)
        ui:set("selected_message_id", "")
    elseif target == "context_menu:react" then
        ui:set("context_menu_visible", false)
        ui:set("show_emoji_picker", true)
        -- selected_message_id stays set for reaction mode
    elseif target == "context_menu:reply" then
        local msg_id = ui:get("selected_message_id") or ""
        local msg, _ = find_message_by_id(msg_id)
        if msg then
            ui:set("replying_to_id", msg_id)
            ui:set("replying_to_text", msg.text)
            ui:set("context_menu_visible", false)
            ui:set("selected_message_id", "")
        end
    elseif target == "context_menu:delete" then
        local msg_id = ui:get("selected_message_id") or ""
        delete_message(msg_id)
        ui:set("context_menu_visible", false)
        ui:set("selected_message_id", "")
    elseif target == "cancel_reply" then
        ui:set("replying_to_id", "")
        ui:set("replying_to_text", "")
    elseif target:match("^toggle_reaction:") then
        local parts = {}
        for part in target:gmatch("[^:]+") do
            table.insert(parts, part)
        end
        if #parts >= 3 then
            local msg_id = parts[2]
            local emoji = parts[3]
            toggle_reaction(msg_id, emoji)
        end
    end
end
-- Find message by ID
function find_message_by_id(msg_id)
    local len = messages_layer:length()
    for i = 0, len - 1 do
        local msg = messages_layer:get(i)
        if msg and msg.id == msg_id then
            return msg, i
        end
    end
    return nil, nil
end

-- Delete message (soft delete)
function delete_message(msg_id)
    local msg, index = find_message_by_id(msg_id)
    if msg and msg.sender_did == my_did then
        msg.deleted = true
        messages_layer:set(index, msg)
        refresh_messages_ui()
    end
end

-- Handle text input changes
function on_text_input(text)
    -- Send typing indicator (throttled)
    local now = os.time()
    if now - last_typing_sent >= 2 then
        send_typing_indicator()
        last_typing_sent = now
    end
end
-- Handle submit (send message)
function on_submit()
    local text = ui:get("draft_text")
    if text and text ~= "" then
        send_message(text)
    end
end
-- Send typing indicator via ephemeral
function send_typing_indicator()
    local payload = '{"type":"typing","user":"' .. my_short_did .. '"}'
    butler:send_ephemeral(payload)
end
-- Handle ephemeral messages
function on_ephemeral(user_did, payload)
    -- Parse JSON (simple)
    local msg_type = payload:match('"type":"([^"]+)"')
    local user = payload:match('"user":"([^"]+)"')

    if msg_type == "typing" and user_did ~= my_did then
        -- Store both timestamp and name
        typing_users[user_did] = {
            timestamp = os.time(),
            name = user or user_did:sub(-8)
        }
        refresh_typing_indicator()
    elseif msg_type == "peer_count" then
        -- Received current peer count on join (includes ourselves)
        local count = tonumber(payload:match('"count":(%d+)'))
        if count then
            ui:set("online_count", count)
        end
    elseif msg_type == "peer_joined" then
        -- Another peer joined - update online count
        local joined_did = payload:match('"user_did":"([^"]+)"')
        if joined_did and joined_did ~= my_did then
            local count = ui:get("online_count") or 1
            ui:set("online_count", count + 1)
        end
    elseif msg_type == "peer_left" then
        -- Another peer left - update online count
        local left_did = payload:match('"user_did":"([^"]+)"')
        if left_did and left_did ~= my_did then
            local count = ui:get("online_count") or 1
            ui:set("online_count", math.max(1, count - 1))
            -- Also clean up typing indicator
            typing_users[left_did] = nil
            refresh_typing_indicator()
        end
    end
end
-- Refresh typing indicator text
function refresh_typing_indicator()
    local now = os.time()
    local active_typers = {}
    for did, info in pairs(typing_users) do
        if now - info.timestamp < 3 then
            table.insert(active_typers, info.name)
        else
            typing_users[did] = nil
        end
    end
    if #active_typers == 0 then
        ui:set("typing_text", "")
    elseif #active_typers == 1 then
        ui:set("typing_text", active_typers[1] .. " is typing...")
    else
        ui:set("typing_text", #active_typers .. " people typing...")
    end
end
-- Periodic tick for cleanup
function tick()
    refresh_typing_indicator()
end
-- Get reactions for a message
function get_reactions_for_message(msg_id)
    local reactions_data = reactions_layer:get(msg_id)
    if not reactions_data then return {} end

    local reactions = {}
    for _, reaction in ipairs(reactions_data) do
        table.insert(reactions, {
            emoji = reaction.emoji,
            count = #reaction.users,
            has_my_reaction = contains(reaction.users, my_did)
        })
    end
    return reactions
end

-- Toggle a reaction on a message
function toggle_reaction(msg_id, emoji)
    local reactions_data = reactions_layer:get(msg_id) or {}

    -- Find existing reaction with this emoji
    local found = false
    for i, reaction in ipairs(reactions_data) do
        if reaction.emoji == emoji then
            found = true
            local user_index = find_index(reaction.users, my_did)
            if user_index then
                -- Remove my reaction
                table.remove(reaction.users, user_index)
                if #reaction.users == 0 then
                    -- Remove empty reaction
                    table.remove(reactions_data, i)
                end
            else
                -- Add my reaction
                table.insert(reaction.users, my_did)
            end
            break
        end
    end

    if not found then
        -- Create new reaction
        table.insert(reactions_data, {
            emoji = emoji,
            users = {my_did}
        })
    end

    reactions_layer:set(msg_id, reactions_data)
    refresh_messages_ui()
end

-- Helper: check if table contains value
function contains(tbl, value)
    for _, v in ipairs(tbl) do
        if v == value then return true end
    end
    return false
end

-- Helper: find index of value in table
function find_index(tbl, value)
    for i, v in ipairs(tbl) do
        if v == value then return i end
    end
    return nil
end

-- Handle Loro changes
function on_loro_change(layer_name, change_type)
    if layer_name:match("/messages$") or layer_name:match("/reactions$") then
        refresh_messages_ui()
    end
end
-- Handle peer presence
function on_peer_joined(user_did)
    local count = ui:get("online_count") or 1
    ui:set("online_count", count + 1)
end
function on_peer_left(user_did)
    local count = ui:get("online_count") or 1
    ui:set("online_count", math.max(1, count - 1))
    typing_users[user_did] = nil
    refresh_typing_indicator()
end
-- API exports for testing
api.export("send_message", send_message)
api.export("get_message_count", function() return messages_layer:length() end)
