-- Presence Plugin
-- CRDT-based presence for multiplayer apps
--
-- Usage:
--   local presence = require("presence")
--   presence.init(page_id, my_did, my_name)
--   -- No need to call update() in tick anymore!
--   local users = presence.get_online_users()
--
-- The plugin uses timer.setInterval internally for heartbeats.

local M = {}

-- Configuration
local STALE_SECS = 60           -- Consider user offline after 60s without heartbeat
local HEARTBEAT_MS = 15000      -- Send heartbeat every 15 seconds

-- State
local page_id = nil
local my_did = nil
local my_name = nil
local presence_layer = nil
local heartbeat_timer_id = nil

-- =============================================================================
-- Core API
-- =============================================================================

--- Initialize presence for this page
---
--- **Context**: Call once when app loads
--- **We do**: Set up presence layer and start heartbeat timer
---
--- @param pid string Page ID
--- @param did string Our DID
--- @param name string? Our display name (defaults to short DID)
function M.init(pid, did, name)
    page_id = pid
    my_did = did
    my_name = name or did:sub(-8)
    presence_layer = scribe:map(page_id .. "/presence")

    -- Write our initial presence entry
    M.write_presence("online")

    -- Start heartbeat timer (replaces tick-based update)
    heartbeat_timer_id = timer.setInterval(HEARTBEAT_MS, function()
        M.write_presence("online")
    end)

    print(string.format("[presence] Initialized for %s (heartbeat every %ds)",
        my_did:sub(-8), HEARTBEAT_MS / 1000))
end

--- Write our own presence entry to CRDT
---
--- @param status string "online" or "offline"
function M.write_presence(status)
    if not presence_layer then return end

    presence_layer:set(my_did, {
        did = my_did,  -- Include DID so bindings can filter by it
        status = status,
        name = my_name,
        last_seen = os.time(),
    })
end

--- Get list of online users (excluding self)
---
--- @return table[] List of {did, name, last_seen}
function M.get_online_users()
    if not presence_layer then return {} end

    local now = os.time()
    local online = {}

    local keys = presence_layer:keys()
    for _, did in ipairs(keys) do
        if did ~= my_did then
            local entry = presence_layer:get(did)
            if entry then
                local age = now - (entry.last_seen or 0)
                local is_online = (entry.status == "online") and (age < STALE_SECS)

                if is_online then
                    table.insert(online, {
                        did = did,
                        name = entry.name or did:sub(-8),
                        last_seen = entry.last_seen,
                    })
                end
            end
        end
    end

    return online
end

--- Get online count (including self)
---
--- @return number
function M.get_online_count()
    return #M.get_online_users() + 1
end

--- Mark ourselves offline and stop heartbeat
---
--- **Context**: Call on app close/shutdown
function M.shutdown()
    -- Stop heartbeat timer
    if heartbeat_timer_id then
        timer.clear(heartbeat_timer_id)
        heartbeat_timer_id = nil
    end

    -- Mark as offline
    M.write_presence("offline")

    print(string.format("[presence] Shutdown for %s", my_did and my_did:sub(-8) or "unknown"))
end

-- Legacy alias for backward compatibility
M.set_offline = M.shutdown

--- Legacy update function (no longer needed)
---
--- **Deprecated**: Heartbeats are now timer-based. This function does nothing.
function M.update()
    -- No-op: heartbeats are handled by timer.setInterval
end

-- =============================================================================
-- Server-side Validation (for node)
-- =============================================================================

--- Validate presence write operation
---
--- **Context**: Called by node's validate_ops() to enforce presence rules
--- **Rule**: Users can only write to their own DID key in the presence map
---
--- @param op table Operation: {op, path, key, value, old_value}
--- @param from_did string DID of the writer
--- @return boolean, string? valid, error_message
function M.validate_write(op, from_did)
    local op_type = op.op or "unknown"
    local key = op.key

    -- For map operations (set/update/delete), check that key == from_did
    if op_type == "set" or op_type == "update" or op_type == "insert" then
        if key ~= from_did then
            return false, string.format(
                "User %s cannot write to presence key %s (can only write to own DID)",
                from_did:sub(-8), key or "nil"
            )
        end
    end

    if op_type == "delete" then
        if key ~= from_did then
            return false, string.format(
                "User %s cannot delete presence key %s (can only delete own DID)",
                from_did:sub(-8), key or "nil"
            )
        end
    end

    return true, nil
end

return M
