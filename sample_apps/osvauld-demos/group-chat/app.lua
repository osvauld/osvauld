-- Group Chat App Logic
-- Real-time messaging with emoji, typing indicators, and @mentions
-- Uses scribe:bind() for declarative layer-UI sync

local presence = presence_lib

-- Emoji shortcode mapping
local EMOJI_SHORTCODES = {
    Smile = "grinning_face", Laugh = "joy", Heart = "heart", Thumbs = "thumbsup",
    Fire = "fire", Party = "tada", Rocket = "rocket", Star = "star",
    Check = "white_check_mark", Wave = "wave", Eyes = "eyes", Think = "thinking",
    Clap = "clap", ["100"] = "100", Sad = "cry", Angry = "angry",
    Cool = "sunglasses", Wink = "wink", LOL = "rofl", Love = "heart_eyes",
}

-- State
local page_id = nil
local messages_layer = nil
local reactions_layer = nil
local my_did = nil
local my_name = nil

-- Typing indicator state
local typing_users = {}
local last_typing_sent = 0

function on_init()
    page_id = scribe:page_id()
    my_did = scribe:my_did()
    my_name = USERNAME or scribe:my_name() or my_did:sub(-8)

    presence.init(page_id, my_did, my_name)

    -- Get layer references for writing
    messages_layer = scribe:list(page_id .. "/messages")
    reactions_layer = scribe:map(page_id .. "/reactions")

    -- Declarative binding: messages auto-sync to UI (keep last 200 in view)
    scribe:bind("messages", "messages", {
        max_items = 200,
        transform = function(msg)
            local is_mine = msg.sender_did == my_did
            local reactions = get_reactions_for_message(msg.id)
            return {
                id = msg.id or "",
                sender = msg.sender_did or "",
                sender_short = is_mine and "You" or (msg.sender_name or "???"),
                text = msg.text or "",
                timestamp = format_time(msg.timestamp),
                is_mine = is_mine,
                deleted = msg.deleted or false,
                reply_to = msg.reply_to or "",
                reply_preview = msg.reply_preview or "",
                reactions = reactions,
            }
        end
    })

    -- Reactions layer changes also need to refresh messages UI
    scribe:bind("reactions_data", "reactions")

    scribe:bind("online_users", "presence", {
        key = "did",
        transform = function(entry)
            if not entry or not entry.did then return nil end
            if entry.did == my_did then return nil end
            if entry.status ~= "online" then return nil end
            return { did = entry.did:sub(-8), name = entry.name or entry.did:sub(-8) }
        end
    })

    ui:set("show_mentions", false)
    ui:set("mention_suggestions", {})
    ui:set("show_online_panel", false)
    ui:set("auto_scroll", true)
    -- online_users is auto-synced via scribe:bind()
end

function get_reactions_for_message(msg_id)
    if not msg_id or not reactions_layer then return {} end
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

function refresh_online_users()
    local users = presence.get_online_users()
    local online = {}
    for _, u in ipairs(users) do
        if u.did ~= my_did then
            table.insert(online, { did = u.did:sub(-8), name = u.name or u.did:sub(-8) })
        end
    end
    ui:set("online_users", online)
end

function format_time(ts)
    if not ts then return "" end
    return os.date("%H:%M", ts)
end

function generate_id()
    return string.format("%x-%04x", os.time(), math.random(0, 65535))
end

function truncate_text(text, max_len)
    if #text > max_len then return text:sub(1, max_len) .. "..." end
    return text
end

function send_message(text)
    if not text or text == "" then return end

    local replying_to = ui:get("replying_to_id") or ""
    local reply_preview = ui:get("replying_to_text") or ""

    local msg = {
        id = generate_id(),
        sender_did = my_did,
        sender_name = my_name,
        text = text,
        timestamp = os.time(),
        deleted = false,
    }

    if replying_to ~= "" then
        msg.reply_to = replying_to
        msg.reply_preview = truncate_text(reply_preview, 30)
    end

    messages_layer:push(msg)
    ui:set("draft_text", "")
    ui:set("show_emoji_picker", false)
    ui:set("replying_to_id", "")
    ui:set("replying_to_text", "")
    ui:set("show_mentions", false)
end

function insert_emoji(emoji_name)
    local shortcode = EMOJI_SHORTCODES[emoji_name]
    local emoji_char = shortcode and emoji:get(shortcode)
    local to_insert = emoji_char or emoji_name
    local current = ui:get("draft_text") or ""
    ui:set("draft_text", current .. to_insert)
end

function on_click(target)
    if target == "emoji_picker:toggle" then
        ui:set("show_emoji_picker", not ui:get("show_emoji_picker"))
        ui:set("show_mentions", false)
    elseif target == "emoji_picker:close" then
        ui:set("show_emoji_picker", false)
    elseif target:match("^emoji:") then
        local emoji_name = target:sub(7)
        local selected_msg = ui:get("selected_message_id") or ""
        if selected_msg ~= "" then
            local shortcode = EMOJI_SHORTCODES[emoji_name]
            local emoji_char = shortcode and emoji:get(shortcode) or emoji_name
            toggle_reaction(selected_msg, emoji_char)
            ui:set("show_emoji_picker", false)
            ui:set("selected_message_id", "")
        else
            insert_emoji(emoji_name)
            ui:set("show_emoji_picker", false)
        end
    elseif target:match("^toggle_menu:") then
        local msg_id = target:sub(13)
        local current_id = ui:get("selected_message_id") or ""
        if current_id == msg_id and ui:get("context_menu_visible") then
            ui:set("context_menu_visible", false)
            ui:set("selected_message_id", "")
        else
            ui:set("selected_message_id", msg_id)
            ui:set("context_menu_visible", true)
        end
    elseif target == "context_menu:close" then
        ui:set("context_menu_visible", false)
        ui:set("selected_message_id", "")
    elseif target == "context_menu:react" then
        ui:set("context_menu_visible", false)
        ui:set("show_emoji_picker", true)
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
        for part in target:gmatch("[^:]+") do table.insert(parts, part) end
        if #parts >= 3 then
            toggle_reaction(parts[2], parts[3])
        end
    elseif target:match("^mention:") then
        insert_mention(target:sub(9))
    elseif target == "mentions:close" then
        ui:set("show_mentions", false)
    elseif target == "toggle_online_panel" then
        local current = ui:get("show_online_panel")
        ui:set("show_online_panel", not current)
        if not current then refresh_online_users() end
    elseif target == "toggle_autoscroll" then
        ui:set("auto_scroll", not ui:get("auto_scroll"))
    end
end

function find_message_by_id(msg_id)
    local len = messages_layer:length()
    for i = 0, len - 1 do
        local msg = messages_layer:get(i)
        if msg and msg.id == msg_id then return msg, i end
    end
    return nil, nil
end

function delete_message(msg_id)
    local msg, index = find_message_by_id(msg_id)
    if msg and msg.sender_did == my_did then
        msg.deleted = true
        messages_layer:set(index, msg)
    end
end

function on_text_input(text)
    local now = os.time()
    if now - last_typing_sent >= 2 then
        send_typing_indicator()
        last_typing_sent = now
    end
    check_for_mentions(text)
end

function check_for_mentions(text)
    if not text then
        ui:set("show_mentions", false)
        return
    end
    local at_pos = text:match(".*()@[%w]*$")
    if at_pos then
        local partial = text:match("@([%w]*)$") or ""
        local suggestions = get_mention_suggestions(partial)
        if #suggestions > 0 then
            ui:set("mention_suggestions", suggestions)
            ui:set("show_mentions", true)
        else
            ui:set("show_mentions", false)
        end
    else
        ui:set("show_mentions", false)
    end
end

function get_mention_suggestions(partial)
    local users = presence.get_online_users()
    local suggestions = {}
    for _, u in ipairs(users) do
        local name = u.name or u.did:sub(-8)
        if name:lower():find(partial:lower(), 1, true) then
            table.insert(suggestions, { did = u.did:sub(-8), name = name, online = true })
        end
    end
    return suggestions
end

function insert_mention(name)
    local text = ui:get("draft_text") or ""
    local new_text = text:gsub("@[%w]*$", "@" .. name .. " ")
    ui:set("draft_text", new_text)
    ui:set("show_mentions", false)
end

function on_submit()
    local text = ui:get("draft_text")
    if text and text ~= "" then send_message(text) end
end


function send_typing_indicator()
    scribe:send("typing", { user = my_name })
end

function on_ephemeral(user_did, func, args)
    if func == "typing" and user_did ~= my_did then
        local user = args and args.user or user_did:sub(-8)
        typing_users[user_did] = { timestamp = os.time(), name = user }
        refresh_typing_indicator()
    end
end

function refresh_typing_indicator()
    local now = os.time()
    local active_typers = {}
    local stale_dids = {}

    -- Collect active typers and stale DIDs
    for did, info in pairs(typing_users) do
        if now - info.timestamp < 3 then
            table.insert(active_typers, info.name)
        else
            table.insert(stale_dids, did)
        end
    end

    -- Remove stale entries after iteration
    for _, did in ipairs(stale_dids) do
        typing_users[did] = nil
    end

    -- Update UI
    if #active_typers == 0 then
        ui:set("typing_text", "")
    elseif #active_typers == 1 then
        ui:set("typing_text", active_typers[1] .. " is typing...")
    else
        ui:set("typing_text", #active_typers .. " people typing...")
    end
end

timer.setInterval(1000, function()
    refresh_typing_indicator()
end)

function toggle_reaction(msg_id, emoji_char)
    if not msg_id or not reactions_layer then return end
    local reactions_data = reactions_layer:get(msg_id) or {}
    local found = false
    for i, reaction in ipairs(reactions_data) do
        if reaction.emoji == emoji_char then
            found = true
            local user_index = find_index(reaction.users, my_did)
            if user_index then
                table.remove(reaction.users, user_index)
                if #reaction.users == 0 then table.remove(reactions_data, i) end
            else
                table.insert(reaction.users, my_did)
            end
            break
        end
    end
    if not found then
        table.insert(reactions_data, { emoji = emoji_char, users = {my_did} })
    end
    reactions_layer:set(msg_id, reactions_data)
end

function contains(tbl, value)
    for _, v in ipairs(tbl) do if v == value then return true end end
    return false
end

function find_index(tbl, value)
    for i, v in ipairs(tbl) do if v == value then return i end end
    return nil
end

-- API exports
api.export("send_message", send_message)
api.export("get_message_count", function() return messages_layer:length() end)
api.export("get_online_count", function() return presence.get_online_count() end)
api.export("get_online_users", function() return presence.get_online_users() end)
