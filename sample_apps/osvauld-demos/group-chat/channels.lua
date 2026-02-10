-- Channels — switch, create, ensure defaults

local M = {}

-- Module state (set by init)
local page_id = nil
local my_did = nil
local my_name = nil
local channels_meta_layer = nil
local active_channel = "general"

-- Current channel's messages layer (LoroMap)
local messages_layer = nil

-- Callback to refresh messages when channel changes
local on_channel_switch = nil

-- Read tracker module reference (set by init)
local read_tracker = nil

function M.init(pid, did, name, callback, tracker)
    page_id = pid
    my_did = did
    my_name = name
    on_channel_switch = callback
    read_tracker = tracker

    channels_meta_layer = scribe:map(page_id .. "/channels_meta")

    M.ensure_default_channels()
    M.switch_channel("general")
end

function M.ensure_default_channels()
    if not channels_meta_layer then return end

    local defaults = {
        { id = "general", name = "general", topic = "General discussion" },
        { id = "random", name = "random", topic = "Random stuff" },
        { id = "dev", name = "dev", topic = "Development talk" },
    }

    for _, ch in ipairs(defaults) do
        local existing = channels_meta_layer:get(ch.id)
        if not existing then
            channels_meta_layer:set(ch.id, {
                id = ch.id,
                name = ch.name,
                topic = ch.topic,
                created_by = my_did,
                created_at = os.time(),
            })
        end
    end

    M.refresh_channel_list()
end

function M.refresh_channel_list()
    if not channels_meta_layer then return end

    local channel_list = {}
    local keys = channels_meta_layer:keys()
    if keys then
        for _, key in ipairs(keys) do
            local ch = channels_meta_layer:get(key)
            if ch then
                local ch_id = ch.id or key
                local unread = 0
                local has_mention = false
                if read_tracker then
                    local ch_msgs_layer = scribe:map(page_id .. "/channels/" .. ch_id .. "/messages")
                    unread, has_mention = read_tracker.count_unread(ch_id, ch_msgs_layer)
                end
                local is_active = ch_id == active_channel
                table.insert(channel_list, {
                    id = ch_id,
                    name = ch.name or key,
                    topic = ch.topic or "",
                    unread = is_active and 0 or unread,
                    is_active = is_active,
                    has_mention = is_active and false or has_mention,
                })
            end
        end
    end

    -- Sort alphabetically, but keep "general" first
    table.sort(channel_list, function(a, b)
        if a.id == "general" then return true end
        if b.id == "general" then return false end
        return a.name < b.name
    end)

    ui:set("channels", channel_list)
end

function M.switch_channel(channel_id)
    active_channel = channel_id

    -- Get messages layer for this channel (LoroMap)
    messages_layer = scribe:map(page_id .. "/channels/" .. channel_id .. "/messages")

    -- Mark as read immediately on switch
    if read_tracker then
        read_tracker.mark_read(channel_id, messages_layer)
    end

    -- Update UI
    local ch_meta = channels_meta_layer and channels_meta_layer:get(channel_id)
    ui:set("active_channel", channel_id)
    ui:set("active_channel_topic", ch_meta and ch_meta.topic or "")

    -- Refresh channel list (to update active state)
    M.refresh_channel_list()

    -- Notify parent to rebind messages
    if on_channel_switch then
        on_channel_switch(channel_id, messages_layer)
    end
end

function M.create_channel(name)
    if not name or name == "" then return end
    if not channels_meta_layer then return end

    -- Sanitize: lowercase, replace spaces with hyphens
    local id = name:lower():gsub("%s+", "-"):gsub("[^%w%-]", "")
    if id == "" then return end

    -- Check if already exists
    local existing = channels_meta_layer:get(id)
    if existing then
        M.switch_channel(id)
        return
    end

    channels_meta_layer:set(id, {
        id = id,
        name = id,
        topic = "",
        created_by = my_did,
        created_at = os.time(),
    })

    M.refresh_channel_list()
    M.switch_channel(id)
end

function M.get_active_channel()
    return active_channel
end

function M.get_messages_layer()
    return messages_layer
end

function M.get_channels_meta_layer()
    return channels_meta_layer
end

return M
