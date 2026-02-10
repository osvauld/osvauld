-- Read Tracker — local-only read positions and unread/mention detection

local M = {}

local page_id, my_did, my_name = nil, nil, nil
local read_positions_layer = nil

function M.init(pid, did, name)
    page_id = pid
    my_did = did
    my_name = name
    read_positions_layer = scribe:map(page_id .. "/read_positions")
end

-- Mark channel as read (set last_read to latest msg timestamp)
function M.mark_read(channel_id, messages_layer)
    if not read_positions_layer or not messages_layer then return end
    local latest_ts = 0
    local keys = messages_layer:keys()
    if keys then
        for _, key in ipairs(keys) do
            local msg = messages_layer:get(key)
            if msg and (msg.timestamp or 0) > latest_ts then
                latest_ts = msg.timestamp
            end
        end
    end
    if latest_ts > 0 then
        local my_positions = read_positions_layer:get(my_did) or { channels = {} }
        my_positions.channels[channel_id] = latest_ts
        read_positions_layer:set(my_did, my_positions)
    end
end

-- Get my last read timestamp for a channel
function M.get_last_read(channel_id)
    if not read_positions_layer then return 0 end
    local my_positions = read_positions_layer:get(my_did)
    if not my_positions or not my_positions.channels then return 0 end
    return my_positions.channels[channel_id] or 0
end

-- Count unread messages and detect @mentions in a channel
function M.count_unread(channel_id, messages_layer)
    if not messages_layer then return 0, false end
    local last_read = M.get_last_read(channel_id)
    local count = 0
    local has_mention = false
    local keys = messages_layer:keys()
    if keys then
        for _, key in ipairs(keys) do
            local msg = messages_layer:get(key)
            if msg and (msg.timestamp or 0) > last_read
               and (not msg.thread_parent_id or msg.thread_parent_id == "")
               and not msg.deleted then
                count = count + 1
                if msg.text and my_name and msg.text:find("@" .. my_name) then
                    has_mention = true
                end
            end
        end
    end
    return count, has_mention
end

return M
