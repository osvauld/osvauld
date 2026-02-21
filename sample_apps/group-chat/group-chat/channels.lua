-- Channels — switch, create, discover via protocol
-- Default channels (general/random/dev) are static layers in the permit template.
-- Custom user-created channels use create_layer() for DID-namespaced dynamic layers.
-- Discovery: viewers learn about custom channels via on_layer_discovered (protocol path).

local M = {}

-- Module state (set by init)
local page_id = nil
local my_did = nil
local active_channel = "general"
local active_period = nil

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

local function current_period()
    if clock and clock.day then
        return clock:day()
    end
    return os.date("!%Y-%m-%d")
end

local function channel_shard_layer_path(channel_id, period)
    return "channels/" .. channel_id .. "/messages/" .. period
end

local function channel_wildcard_layer_path(channel_id)
    return "channels/" .. channel_id .. "/messages/*"
end

local function ensure_channel_shard(channel_id, period)
    local expected_path = channel_shard_layer_path(channel_id, period)
    local existing = scribe:list_layers(expected_path)
    if existing and #existing > 0 then
        return existing[1]
    end

    local ok, result = pcall(function()
        return scribe:create_layer("channels/{channel}/messages/{period}", {
            channel = channel_id,
            period = period,
        })
    end)

    if ok and result and result.layer_name then
        local prefix = page_id .. "/"
        if result.layer_name:sub(1, #prefix) == prefix then
            return result.layer_name:sub(#prefix + 1)
        end
        return result.layer_name
    end

    return expected_path
end

-- Get the binding layer pattern for a channel.
local function get_channel_layer_path(channel_id)
    return channel_wildcard_layer_path(channel_id)
end

-- Extract channel id from a dynamic layer path.
-- Pattern: "channels/{channel_id}/messages/{period}" → channel_id
-- Returns channel_id or nil if not a channel layer
local function parse_channel_layer_path(layer_path)
    local channel_id, period = layer_path:match("^channels/([^/]+)/messages/([^/]+)$")
    if channel_id and period then
        return channel_id, layer_path
    end

    -- Backward compatibility: channels/<did>/<id>/messages
    local did_compat, channel_id_compat = layer_path:match("^channels/([^/]+)/([^/]+)/messages$")
    if did_compat and channel_id_compat then
        return channel_id_compat, layer_path
    end

    -- Backward compatibility: channels/<id>/messages
    local legacy_id = layer_path:match("^channels/([^/]+)/messages$")
    if legacy_id then
        return legacy_id, layer_path
    end

    return nil, nil
end

local function current_channel_layer(channel_id)
    local period = current_period()
    local layer_path = ensure_channel_shard(channel_id, period)
    return layer_path, period
end

local function refresh_active_layer()
    local layer_path, period = current_channel_layer(active_channel)
    active_period = period
    messages_layer = scribe:map(page_id .. "/" .. layer_path)
    return layer_path
end

local function ensure_known_channel(channel_id)
    if known_channels[channel_id] then
        return
    end

    known_channels[channel_id] = {
        id = channel_id,
        name = channel_id,
        topic = "",
        layer_path = channel_wildcard_layer_path(channel_id),
    }
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
            layer_path = channel_wildcard_layer_path(ch.id),
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
            local period = current_period()
            local ch_layer_path = channel_shard_layer_path(ch_id, period)
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

    ensure_known_channel(channel_id)
    refresh_active_layer()
    active_layer_path = channel_wildcard_layer_path(channel_id)

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

    -- Notify parent with wildcard pattern (for binding/rebinding)
    if on_channel_switch then
        on_channel_switch(channel_id, active_layer_path)
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

    local period = current_period()
    local ok, result = pcall(function()
        return scribe:create_layer("channels/{channel}/messages/{period}", {
            channel = id,
            period = period,
        })
    end)
    if not ok then
        print("[channels] create_layer FAILED: " .. tostring(result))
    end

    local layer_path = channel_shard_layer_path(id, period)
    if ok and result and result.layer_name then
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
        layer_path = channel_wildcard_layer_path(id),
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

    local channel_id, _layer_path = parse_channel_layer_path(bare_path)
    if not channel_id then return false end

    -- Skip if already known (default channels or own creation)
    if known_channels[channel_id] then return false end

    known_channels[channel_id] = {
        id = channel_id,
        name = channel_id,
        topic = "",
        layer_path = channel_wildcard_layer_path(channel_id),
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
    local period = current_period()
    if not messages_layer or period ~= active_period then
        refresh_active_layer()
    end
    return messages_layer
end

function M.get_total_message_count(channel_id)
    local target = channel_id or active_channel
    local pattern = channel_wildcard_layer_path(target)
    local layers = scribe:list_layers(pattern)
    if not layers then
        return 0
    end

    local total = 0
    for _, layer_path in ipairs(layers) do
        local layer = scribe:map(page_id .. "/" .. layer_path)
        total = total + (layer:length() or 0)
    end
    return total
end

return M
