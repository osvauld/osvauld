-- Group Chat App Logic
-- Real-time messaging with emoji and typing indicators

-- Emoji mapping (text to emoji)
local EMOJI_MAP = {
    Smile = ":)",
    Laugh = "XD",
    Heart = "<3",
    Thumbs = "+1",
    Fire = "**",
    Party = "!!",
    Rocket = ">>",
    Star = "*",
    Check = "OK",
    Wave = "Hi",
    Eyes = "o_o",
    Think = "...",
    Clap = "^^",
    ["100"] = "100",
    Sad = ":(",
    Angry = ">:(",
    Cool = "B)",
    Wink = ";)",
    LOL = "lol",
    Love = "<3<3",
}

-- State
local page_id = nil
local messages_layer = nil
local my_did = nil
local my_short_did = nil

-- Typing indicator state
local typing_users = {}  -- did -> timestamp
local last_typing_sent = 0

-- Initialize the app
function on_init()
    page_id = permit:page_id()
    my_did = permit:my_did()
    my_short_did = my_did:sub(-8)

    -- Messages layer (synced)
    messages_layer = loro:get_or_create_layer(page_id .. "/messages", "list")

    -- Load existing messages
    refresh_messages_ui()
end

-- Refresh messages from Loro
function refresh_messages_ui()
    local all_messages = {}
    local len = messages_layer:length()

    for i = 0, len - 1 do
        local msg = messages_layer:get(i)
        if msg then
            table.insert(all_messages, {
                id = msg.id or "",
                sender = msg.sender_did or "",
                sender_short = msg.sender_name or "???",
                text = msg.text or "",
                timestamp = format_time(msg.timestamp),
                is_mine = msg.sender_did == my_did,
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

-- Send a message
function send_message(text)
    if not text or text == "" then return end

    local msg = {
        id = generate_id(),
        sender_did = my_did,
        sender_name = my_short_did,
        text = text,
        timestamp = os.time(),
    }

    messages_layer:push(msg)
    refresh_messages_ui()

    -- Clear draft
    ui:set("draft_text", "")
    ui:set("show_emoji_picker", false)
end

-- Insert emoji into draft
function insert_emoji(emoji_name)
    local emoji = EMOJI_MAP[emoji_name] or emoji_name
    local current = ui:get("draft_text") or ""
    ui:set("draft_text", current .. emoji)
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
        insert_emoji(emoji_name)
        ui:set("show_emoji_picker", false)
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
        typing_users[user_did] = os.time()
        refresh_typing_indicator()
    end
end

-- Refresh typing indicator text
function refresh_typing_indicator()
    local now = os.time()
    local active_typers = {}

    for did, ts in pairs(typing_users) do
        if now - ts < 3 then
            local short = did:sub(-8)
            table.insert(active_typers, short)
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

-- Handle Loro changes
function on_loro_change(layer_name, change_type)
    if layer_name:match("/messages$") then
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
