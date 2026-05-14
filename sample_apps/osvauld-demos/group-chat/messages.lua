-- Messages — send, edit, delete, reactions (all using LoroMap key access)

local helpers = require("ui_helpers")

local M = {}

-- Module state
local my_did = nil
local my_name = nil

-- Emoji shortcode mapping
local EMOJI_SHORTCODES = {
    Smile = "grinning_face", Laugh = "joy", Heart = "heart", Thumbs = "thumbsup",
    Fire = "fire", Party = "tada", Rocket = "rocket", Star = "star",
    Check = "white_check_mark", Wave = "wave", Eyes = "eyes", Think = "thinking",
    Clap = "clap", ["100"] = "100", Sad = "cry", Angry = "angry",
    Cool = "sunglasses", Wink = "wink", LOL = "rofl", Love = "heart_eyes",
    Sparkles = "sparkles", Pray = "pray", Muscle = "muscle", Tada = "confetti_ball",
    Crown = "crown", Gem = "gem_stone", Lightning = "zap", Trophy = "trophy",
    Brain = "brain", Ghost = "ghost",
}

function M.init(did, name)
    my_did = did
    my_name = name
end

local function now_unix()
    if clock and clock.count then
        return clock:count()
    end
    return os.time()
end

-- Transform raw LoroMap message data to UI format
-- Filters to top-level only, sorts by timestamp
function M.transform_messages(messages_layer)
    if not messages_layer then return {} end

    local raw_msgs = {}
    local keys = messages_layer:keys()
    if not keys then return {} end

    for _, key in ipairs(keys) do
        local msg = messages_layer:get(key)
        if msg and (not msg.thread_parent_id or msg.thread_parent_id == "") then
            table.insert(raw_msgs, msg)
        end
    end

    -- Sort by timestamp
    table.sort(raw_msgs, function(a, b)
        return (a.timestamp or 0) < (b.timestamp or 0)
    end)

    -- Transform to UI format
    local ui_msgs = {}
    for _, msg in ipairs(raw_msgs) do
        table.insert(ui_msgs, M.to_ui_message(msg))
    end

    return ui_msgs
end

-- Convert raw message to UI Message struct
function M.to_ui_message(msg)
    if not msg then return nil end
    local is_mine = msg.sender_did == my_did
    local reactions = M.transform_reactions(msg.reactions)

    return {
        id = msg.id or "",
        sender = is_mine and "You" or (msg.sender_name or "???"),
        sender_short = helpers.sender_initials(is_mine and "You" or (msg.sender_name or "?")),
        text = msg.text or "",
        timestamp = helpers.format_time(msg.timestamp),
        is_mine = is_mine,
        deleted = msg.deleted or false,
        edited = msg.edited or false,
        reply_to = msg.reply_to or "",
        reply_preview = msg.reply_preview or "",
        thread_parent_id = msg.thread_parent_id or "",
        thread_reply_count = msg.thread_reply_count or 0,
        reactions = reactions,
        has_attachment = (msg.attachment_hash and msg.attachment_hash ~= "") and true or false,
        attachment_name = msg.attachment_name or "",
        attachment_hash = msg.attachment_hash or "",
    }
end

-- Transform reactions array: { emoji, users[] } -> { emoji, count, has_mine }
function M.transform_reactions(reactions)
    if not reactions or type(reactions) ~= "table" then return {} end

    local result = {}
    for _, r in ipairs(reactions) do
        if r.emoji then
            table.insert(result, {
                emoji = r.emoji,
                count = r.users and #r.users or 0,
                has_mine = helpers.contains(r.users, my_did),
            })
        end
    end
    return result
end

-- Send a new message to the given messages layer
function M.send(messages_layer, text, reply_to_id, reply_preview)
    if not text or text == "" or not messages_layer then return end

    local msg_id = helpers.generate_id()
    local msg = {
        id = msg_id,
        sender_did = my_did,
        sender_name = my_name,
        text = text,
        timestamp = now_unix(),
        deleted = false,
        edited = false,
        thread_parent_id = "",
        thread_reply_count = 0,
        reply_to = reply_to_id or "",
        reply_preview = reply_preview or "",
        reactions = {},
        attachment_hash = "",
        attachment_name = "",
    }

    messages_layer:set(msg_id, msg)
    return msg_id
end

-- Send a message with an attachment
function M.send_with_attachment(messages_layer, text, hash, filename)
    if not messages_layer then return end

    local msg_id = helpers.generate_id()
    local msg = {
        id = msg_id,
        sender_did = my_did,
        sender_name = my_name,
        text = text or "",
        timestamp = now_unix(),
        deleted = false,
        edited = false,
        thread_parent_id = "",
        thread_reply_count = 0,
        reply_to = "",
        reply_preview = "",
        reactions = {},
        attachment_hash = hash or "",
        attachment_name = filename or "",
    }

    messages_layer:set(msg_id, msg)
    return msg_id
end

-- Edit message text (only if sender is me)
function M.edit(messages_layer, msg_id, new_text)
    if not messages_layer or not msg_id then return false end

    local msg = messages_layer:get(msg_id)
    if not msg then return false end
    if msg.sender_did ~= my_did then return false end

    msg.text = new_text
    msg.edited = true
    messages_layer:set(msg_id, msg)
    return true
end

-- Soft-delete message (only if sender is me)
function M.delete(messages_layer, msg_id)
    if not messages_layer or not msg_id then return false end

    local msg = messages_layer:get(msg_id)
    if not msg then return false end
    if msg.sender_did ~= my_did then return false end

    msg.deleted = true
    messages_layer:set(msg_id, msg)
    return true
end

-- Toggle reaction on a message
function M.toggle_reaction(messages_layer, msg_id, emoji_char)
    if not messages_layer or not msg_id then return end

    local msg = messages_layer:get(msg_id)
    if not msg then return end

    local reactions = msg.reactions or {}
    local found = false

    for i, reaction in ipairs(reactions) do
        if reaction.emoji == emoji_char then
            found = true
            local user_idx = helpers.find_index(reaction.users, my_did)
            if user_idx then
                table.remove(reaction.users, user_idx)
                if #reaction.users == 0 then
                    table.remove(reactions, i)
                end
            else
                table.insert(reaction.users, my_did)
            end
            break
        end
    end

    if not found then
        table.insert(reactions, { emoji = emoji_char, users = { my_did } })
    end

    msg.reactions = reactions
    messages_layer:set(msg_id, msg)
end

-- Insert emoji character into draft text
function M.insert_emoji_to_draft(emoji_name)
    local shortcode = EMOJI_SHORTCODES[emoji_name]
    local emoji_char = shortcode and emoji:get(shortcode)
    local to_insert = emoji_char or emoji_name
    local current = ui:get("draft_text") or ""
    ui:set("draft_text", current .. to_insert)
end

-- Resolve emoji name to character
function M.resolve_emoji(emoji_name)
    local shortcode = EMOJI_SHORTCODES[emoji_name]
    return shortcode and emoji:get(shortcode) or emoji_name
end

-- Get raw message by ID from layer
function M.get_raw(messages_layer, msg_id)
    if not messages_layer or not msg_id then return nil end
    return messages_layer:get(msg_id)
end

return M
