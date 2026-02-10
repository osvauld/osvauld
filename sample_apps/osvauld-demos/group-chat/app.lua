-- Group Chat App — Slack-like with channels, threads, reactions
-- Entry point: on_init, on_click routing, presence, typing

local channels = require("channels")
local messages = require("messages")
local threads = require("threads")
local helpers = require("ui_helpers")
local read_tracker = require("read_tracker")

local presence = presence_lib

-- State
local page_id = nil
local my_did = nil
local my_name = nil

-- Typing indicator state
local typing_users = {}
local last_typing_sent = 0

-- Emoji shortcodes (for reaction via context menu)
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

function on_init()
    page_id = scribe:page_id()
    my_did = scribe:my_did()
    my_name = USERNAME or scribe:my_name() or my_did:sub(-8)

    -- Init modules
    messages.init(my_did, my_name)
    threads.init(my_did, my_name)
    read_tracker.init(page_id, my_did, my_name)

    -- Init presence
    presence.init(page_id, my_did, my_name)

    -- Set profile footer state
    ui:set("my_display_name", my_name)
    ui:set("my_online", true)

    -- Bind presence to online users sidebar
    scribe:bind("online_users", "presence", {
        key = "did",
        transform = function(entry)
            if not entry or not entry.did then return nil end
            return {
                did = entry.did:sub(-8),
                name = entry.name or entry.did:sub(-8),
                is_online = entry.status == "online",
            }
        end
    })

    -- Bind messages for default channel BEFORE channels.init(),
    -- because init() calls switch_channel("general") which fires the callback.
    scribe:bind("messages", "channels/general/messages", {
        key = "id",
        transform = function(msg)
            if msg.thread_parent_id and msg.thread_parent_id ~= "" then
                return nil  -- filter thread replies from main view
            end
            return messages.to_ui_message(msg)
        end
    })

    -- Init channels — on switch, rebind messages to new channel layer
    channels.init(page_id, my_did, my_name, function(channel_id, messages_layer)
        scribe:rebind("messages", "channels/" .. channel_id .. "/messages")

        -- Also refresh thread if open (may be in same channel)
        if threads.is_open() then
            threads.refresh_replies(messages_layer)
        end

        -- Mark as read and update sidebar badges
        read_tracker.mark_read(channel_id, messages_layer)
        channels.refresh_channel_list()
    end, read_tracker)

    -- Init UI state
    ui:set("auto_scroll", true)
    ui:set("show_emoji_picker", false)
    ui:set("replying_to_id", "")
    ui:set("replying_to_text", "")
    ui:set("selected_message_id", "")
    ui:set("thread_open", false)
    ui:set("thread_draft", "")

    -- Initial read tracking (binding handles UI sync)
    refresh_messages()
end

-- Refresh messages side effects (read tracking + badges)
-- The binding system handles UI sync; this only does read-tracking
function refresh_messages()
    local layer = channels.get_messages_layer()
    read_tracker.mark_read(channels.get_active_channel(), layer)
    channels.refresh_channel_list()
end

function on_click(target)
    -- Sidebar collapse toggles
    if target == "toggle_channels" then
        ui:set("channels_collapsed", not ui:get("channels_collapsed"))
        return
    end
    if target == "toggle_online" then
        ui:set("online_collapsed", not ui:get("online_collapsed"))
        return
    end

    -- Channel switching
    if target:match("^switch_channel:") then
        local id = target:sub(16)
        channels.switch_channel(id)
        -- Close thread when switching channels
        threads.close()
        ui:set("selected_message_id", "")
        ui:set("show_emoji_picker", false)
        return
    end

    -- Thread operations
    if target:match("^open_thread:") then
        local msg_id = target:sub(13)
        threads.open(channels.get_messages_layer(), msg_id)
        ui:set("selected_message_id", "")
        return
    end

    if target == "close_thread" then
        threads.close()
        return
    end

    if target == "thread_submit" then
        threads.send_reply(channels.get_messages_layer())
        refresh_messages()
        return
    end

    -- Reactions
    if target:match("^toggle_reaction:") then
        local parts = {}
        for part in target:gmatch("[^:]+") do table.insert(parts, part) end
        if #parts >= 3 then
            messages.toggle_reaction(channels.get_messages_layer(), parts[2], parts[3])
            refresh_messages()
            if threads.is_open() then
                threads.refresh_replies(channels.get_messages_layer())
            end
        end
        return
    end

    if target:match("^add_reaction:") then
        local msg_id = target:sub(14)
        ui:set("selected_message_id", msg_id)
        ui:set("show_emoji_picker", true)
        return
    end

    -- Context menu
    if target:match("^toggle_menu:") then
        local msg_id = target:sub(13)
        local current = ui:get("selected_message_id") or ""
        if current == msg_id then
            ui:set("selected_message_id", "")
        else
            ui:set("selected_message_id", msg_id)
        end
        return
    end

    if target == "context_menu:reply" then
        local msg_id = ui:get("selected_message_id") or ""
        local raw = messages.get_raw(channels.get_messages_layer(), msg_id)
        if raw then
            ui:set("replying_to_id", msg_id)
            ui:set("replying_to_text", helpers.truncate_text(raw.text, 40))
        end
        ui:set("selected_message_id", "")
        return
    end

    if target == "context_menu:thread" then
        local msg_id = ui:get("selected_message_id") or ""
        threads.open(channels.get_messages_layer(), msg_id)
        ui:set("selected_message_id", "")
        return
    end

    -- Edit message
    if target:match("^edit_message:") then
        local msg_id = target:sub(14)
        local raw = messages.get_raw(channels.get_messages_layer(), msg_id)
        if raw and raw.sender_did == my_did then
            -- Put text in draft for editing (simple approach)
            ui:set("draft_text", raw.text)
            ui:set("replying_to_id", "edit:" .. msg_id)
            ui:set("replying_to_text", "Editing message...")
        end
        ui:set("selected_message_id", "")
        return
    end

    -- Delete message
    if target:match("^delete_message:") then
        local msg_id = target:sub(16)
        messages.delete(channels.get_messages_layer(), msg_id)
        ui:set("selected_message_id", "")
        refresh_messages()
        if threads.is_open() then
            threads.refresh_replies(channels.get_messages_layer())
        end
        return
    end

    -- Emoji picker
    if target == "emoji_picker:toggle" then
        ui:set("show_emoji_picker", not ui:get("show_emoji_picker"))
        return
    end

    if target == "emoji_picker:close" then
        ui:set("show_emoji_picker", false)
        return
    end

    if target:match("^emoji:") then
        local emoji_name = target:sub(7)
        local selected = ui:get("selected_message_id") or ""
        if selected ~= "" then
            -- Add reaction to selected message
            local emoji_char = messages.resolve_emoji(emoji_name)
            messages.toggle_reaction(channels.get_messages_layer(), selected, emoji_char)
            ui:set("show_emoji_picker", false)
            ui:set("selected_message_id", "")
            refresh_messages()
            if threads.is_open() then
                threads.refresh_replies(channels.get_messages_layer())
            end
        else
            -- Insert into draft text
            messages.insert_emoji_to_draft(emoji_name)
            ui:set("show_emoji_picker", false)
        end
        return
    end

    -- Reply cancel
    if target == "cancel_reply" then
        ui:set("replying_to_id", "")
        ui:set("replying_to_text", "")
        return
    end

    -- Autoscroll
    if target == "toggle_autoscroll" then
        ui:set("auto_scroll", not ui:get("auto_scroll"))
        return
    end

    -- Attach file
    if target == "attach_file" then
        pick_asset_file("images")
        return
    end

    -- Create channel modal
    if target == "create_channel" then
        ui:set("new_channel_name", "")
        ui:set("show_create_channel", true)
        return
    end

    if target == "cancel_create_channel" then
        ui:set("show_create_channel", false)
        ui:set("new_channel_name", "")
        return
    end

    if target == "confirm_create_channel" then
        local name = ui:get("new_channel_name") or ""
        if name ~= "" then
            channels.create_channel(name)
        end
        ui:set("show_create_channel", false)
        ui:set("new_channel_name", "")
        return
    end
end

function on_submit()
    local text = ui:get("draft_text")
    if not text or text == "" then return end

    local replying_to = ui:get("replying_to_id") or ""

    -- Check if editing
    if replying_to:match("^edit:") then
        local msg_id = replying_to:sub(6)
        messages.edit(channels.get_messages_layer(), msg_id, text)
        ui:set("draft_text", "")
        ui:set("replying_to_id", "")
        ui:set("replying_to_text", "")
        refresh_messages()
        return
    end

    -- Normal send
    local reply_preview = ui:get("replying_to_text") or ""
    messages.send(channels.get_messages_layer(), text, replying_to, reply_preview)

    ui:set("draft_text", "")
    ui:set("show_emoji_picker", false)
    ui:set("replying_to_id", "")
    ui:set("replying_to_text", "")
    ui:set("selected_message_id", "")

    refresh_messages()
end

function on_text_input(text)
    local now = os.time()
    if now - last_typing_sent >= 2 then
        scribe:send("typing", { user = my_name, channel = channels.get_active_channel() })
        last_typing_sent = now
    end
end

function on_ephemeral(user_did, func, args)
    if func == "typing" and user_did ~= my_did then
        -- Only show typing for current channel
        if args and args.channel == channels.get_active_channel() then
            local user = args.user or user_did:sub(-8)
            typing_users[user_did] = { timestamp = os.time(), name = user }
            refresh_typing_indicator()
        end
    end
end

function refresh_typing_indicator()
    local now = os.time()
    local active = {}
    local stale = {}

    for did, info in pairs(typing_users) do
        if now - info.timestamp < 3 then
            table.insert(active, info.name)
        else
            table.insert(stale, did)
        end
    end

    for _, did in ipairs(stale) do
        typing_users[did] = nil
    end

    if #active == 0 then
        ui:set("typing_text", "")
    elseif #active == 1 then
        ui:set("typing_text", active[1] .. " is typing...")
    else
        ui:set("typing_text", #active .. " people typing...")
    end
end

-- Asset upload callback
function on_asset_uploaded(asset)
    if asset and asset.hash then
        messages.send_with_attachment(
            channels.get_messages_layer(),
            "[File: " .. (asset.filename or "file") .. "]",
            asset.hash,
            asset.filename or "file"
        )
        refresh_messages()
    end
end

-- Layer change callback for non-bound layers (channels_meta, read_positions, etc.)
-- Bound layers (messages, presence) are handled by the binding system automatically.
function on_loro_change(layer_name)
    if layer_name:match("channels_meta") then
        channels.refresh_channel_list()
        return
    end

    if layer_name:match("read_positions") then
        channels.refresh_channel_list()
        return
    end

    -- Active channel messages changed (handles read tracking + thread refresh)
    local active = channels.get_active_channel()
    if active and layer_name:match("channels/" .. active .. "/messages") then
        refresh_messages()
        if threads.is_open() then
            threads.refresh_replies(channels.get_messages_layer())
        end
    end
end

-- Periodic typing cleanup
timer.setInterval(1000, function()
    refresh_typing_indicator()
end)

-- API exports
api.export("send_message", function(text)
    messages.send(channels.get_messages_layer(), text)
    refresh_messages()
end)
api.export("switch_channel", function(id) channels.switch_channel(id) end)
api.export("get_active_channel", function() return channels.get_active_channel() end)
api.export("get_message_count", function()
    local layer = channels.get_messages_layer()
    if not layer then return 0 end
    return layer:length()
end)
