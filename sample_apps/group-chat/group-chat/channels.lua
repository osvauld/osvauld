-- Channels — switch, create, discover via protocol
-- Default channels (general/random/dev) are static layers in the permit template.
-- Custom user-created channels use create_layer() for DID-namespaced dynamic layers.
-- Discovery: viewers learn about custom channels via on_layer_discovered (protocol path).

local M = {}

-- Module state (set by init)
local page_id = nil
local my_did = nil
local active_channel = "general"

-- Current channel's messages layer (LoroMap)
local messages_layer = nil

-- Current channel's bare layer path (for binding/rebinding)
local active_layer_path = nil

-- Callback to refresh messages when channel changes
local on_channel_switch = nil

-- Read tracker module reference (set by init)
local read_tracker = nil

-- Default channels — these are static layers in the permit template
local DEFAULT_CHANNELS = {
    { id = "general", name = "general", topic = "General discussion" },
    { id = "random", name = "random", topic = "Random stuff" },
    { id = "dev", name = "dev", topic = "Development talk" },
}

-- Known channels: { [channel_id] = { id, name, topic, layer_path } }
-- Populated from defaults + discovered dynamic layers + own creations
local known_channels = {}

-- Check if a channel ID is a default (static) channel
local function is_default_channel(channel_id)
    for _, ch in ipairs(DEFAULT_CHANNELS) do
        if ch.id == channel_id then return true end
    end
    return false
end

-- Get the layer path for a channel.
-- Default channels: simple static path (e.g., "channels/general/messages")
-- Custom channels: DID-namespaced path from known_channels
local function get_channel_layer_path(channel_id)
    if is_default_channel(channel_id) then
        return "channels/" .. channel_id .. "/messages"
    end
    local ch = known_channels[channel_id]
    if ch and ch.layer_path then
        return ch.layer_path
    end
    -- Fallback for unknown channels
    return "channels/" .. channel_id .. "/messages"
end

-- Extract channel id from a dynamic layer path.
-- Pattern: "channels/{creator_did}/{channel_id}/messages" → channel_id
-- Returns channel_id or nil if not a channel layer
local function parse_channel_layer_path(layer_path)
    -- Match: channels/<did>/<id>/messages
    local did, channel_id = layer_path:match("^channels/([^/]+)/([^/]+)/messages$")
    if did and channel_id then
        return channel_id, layer_path
    end
    return nil, nil
end

function M.init(pid, did, name, callback, tracker)
    page_id = pid
    my_did = did
    on_channel_switch = callback
    read_tracker = tracker

    -- Seed known_channels with defaults
    for _, ch in ipairs(DEFAULT_CHANNELS) do
        known_channels[ch.id] = {
            id = ch.id,
            name = ch.name,
            topic = ch.topic,
            layer_path = "channels/" .. ch.id .. "/messages",
        }
    end

    M.refresh_channel_list()
    M.switch_channel("general")
end

function M.refresh_channel_list()
    local channel_list = {}
    for key, ch in pairs(known_channels) do
        local ch_id = ch.id or key
        local unread = 0
        local has_mention = false
        if read_tracker then
            local ch_layer_path = get_channel_layer_path(ch_id)
            local ch_msgs_layer = scribe:map(page_id .. "/" .. ch_layer_path)
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

    local layer_path = get_channel_layer_path(channel_id)
    active_layer_path = layer_path

    -- Get messages layer using the full path
    messages_layer = scribe:map(page_id .. "/" .. layer_path)

    -- Mark as read immediately on switch
    if read_tracker then
        read_tracker.mark_read(channel_id, messages_layer)
    end

    -- Update UI
    local ch = known_channels[channel_id]
    ui:set("active_channel", channel_id)
    ui:set("active_channel_topic", ch and ch.topic or "")

    -- Refresh channel list (to update active state)
    M.refresh_channel_list()

    -- Notify parent with layer_path (for binding/rebinding)
    if on_channel_switch then
        on_channel_switch(channel_id, layer_path)
    end
end

function M.create_channel(name)
    if not name or name == "" then return end

    -- Sanitize: lowercase, replace spaces with hyphens
    local id = name:lower():gsub("%s+", "-"):gsub("[^%w%-]", "")
    if id == "" then return end

    -- Check if already exists
    if known_channels[id] then
        M.switch_channel(id)
        return
    end

    -- Custom channels use create_layer() for DID-namespaced dynamic path
    print("[channels] create_channel: calling scribe:create_layer for id=" .. id)
    local ok, result = pcall(function()
        return scribe:create_layer("channels/{id}/messages", id)
    end)
    if not ok then
        print("[channels] create_layer FAILED: " .. tostring(result))
        return
    end
    print("[channels] create_layer result: " .. tostring(result and result.layer_name))
    local layer_path = nil
    if result and result.layer_name then
        -- Strip page_id/ prefix to get bare path
        local prefix = page_id .. "/"
        if result.layer_name:sub(1, #prefix) == prefix then
            layer_path = result.layer_name:sub(#prefix + 1)
        else
            layer_path = result.layer_name
        end
    end

    -- Register in known_channels (creator knows the path directly)
    known_channels[id] = {
        id = id,
        name = id,
        topic = "",
        layer_path = layer_path,
    }

    M.refresh_channel_list()
    M.switch_channel(id)
end

--- Called when a new layer is discovered via protocol sync.
--- Parses channel layer paths and registers them as known channels.
function M.on_layer_discovered(layer_name)
    -- Strip page_id/ prefix if present
    local bare_path = layer_name
    local prefix = page_id .. "/"
    if layer_name:sub(1, #prefix) == prefix then
        bare_path = layer_name:sub(#prefix + 1)
    end

    local channel_id, layer_path = parse_channel_layer_path(bare_path)
    if not channel_id then return false end

    -- Skip if already known (default channels or own creation)
    if known_channels[channel_id] then return false end

    known_channels[channel_id] = {
        id = channel_id,
        name = channel_id,
        topic = "",
        layer_path = layer_path,
    }

    M.refresh_channel_list()
    return true
end

function M.get_active_channel()
    return active_channel
end

function M.get_active_layer_path()
    return active_layer_path
end

function M.get_messages_layer()
    return messages_layer
end

return M
