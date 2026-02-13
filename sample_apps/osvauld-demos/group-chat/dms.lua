-- DMs & Private Channels — explicit dynamic layers with named collaborators
-- 1:1 DMs: sorted-DID stable id, 2 participants
-- Private channels: named, N participants, same explicit grant mechanism

local M = {}

-- Module state (set by init)
local page_id = nil
local my_did = nil
local my_name = nil
local active_dm = nil
local dm_entries = {}

-- Current DM's messages layer (LoroMap)
local messages_layer = nil

-- Current DM's bare layer path (for binding/rebinding)
local active_layer_path = nil

-- Callback to refresh messages when DM changes
local on_dm_switch = nil

-- Generate a stable DM id from two DIDs (sorted, last 8 chars each)
local function make_dm_id(did_a, did_b)
    local a, b = did_a, did_b
    if a > b then a, b = b, a end
    return "dm-" .. a:sub(-8) .. "-" .. b:sub(-8)
end

local function dm_owner_did(did_a, did_b)
    if did_a < did_b then
        return did_a
    end
    return did_b
end

-- Sanitize a name for use as layer id
local function sanitize_id(name)
    return name:lower():gsub("%s+", "-"):gsub("[^%w%-]", "")
end

function M.init(pid, did, name, callback)
    page_id = pid
    my_did = did
    my_name = name
    on_dm_switch = callback

    dm_entries = {}
    M.refresh_dm_list()
end

local function normalize_layer_path(layer_name)
    if not layer_name then return nil end

    local prefix = page_id .. "/"
    if layer_name:sub(1, #prefix) == prefix then
        return layer_name:sub(#prefix + 1)
    end

    return layer_name
end

local function split_first_two(path)
    local first, second = path:match("^([^/]+)/([^/]+)")
    return first, second
end

local function is_my_layer(layer_path)
    if not layer_path then return false end
    local _prefix, creator_did = split_first_two(layer_path)
    return creator_did == my_did
end

local function find_existing_dm_layer_path(dm_id)
    local suffix = "/" .. dm_id .. "/messages"
    local matches = scribe:list_layers("*" .. suffix)
    if not matches or #matches == 0 then
        return nil
    end

    table.sort(matches)
    return matches[1]
end

local function grant_participants(layer_path, participants)
    if not layer_path or not participants then return end

    local full_layer_name = page_id .. "/" .. layer_path
    for _, did in ipairs(participants) do
        if did ~= my_did then
            pcall(function()
                scribe:add_layer_access(full_layer_name, did)
            end)
        end
    end
end

function M.create_dm(other_did, other_name)
    if other_did == my_did then return end

    local dm_id = make_dm_id(my_did, other_did)

    -- Check if already exists
    local existing = dm_entries[dm_id]
    if existing then
        M.switch_dm(dm_id)
        return
    end

    -- Reuse an existing participant layer if already visible, otherwise create.
    local layer_path = find_existing_dm_layer_path(dm_id)
    if not layer_path then
        -- Canonical owner creates the DM layer; other participant waits to discover it.
        local owner_did = dm_owner_did(my_did, other_did)
        if my_did ~= owner_did then
            return false
        end

        local result = scribe:create_layer("dms/{id}/messages", dm_id)
        layer_path = normalize_layer_path(result and result.layer_name)
    end

    dm_entries[dm_id] = {
        id = dm_id,
        is_group = false,
        participants = { my_did, other_did },
        participant_names = { [my_did] = my_name, [other_did] = other_name },
        created_by = my_did,
        created_at = os.time(),
        layer_path = layer_path,
    }

    if is_my_layer(layer_path) then
        grant_participants(layer_path, { my_did, other_did })
    end

    M.refresh_dm_list()
    M.switch_dm(dm_id)
    return true
end

function M.open_dm(other_did, other_name)
    if other_did == my_did then return false end

    local dm_id = make_dm_id(my_did, other_did)
    local existing = dm_entries[dm_id]
    if existing then
        M.switch_dm(dm_id)
        return true
    end

    local layer_path = find_existing_dm_layer_path(dm_id)
    if not layer_path then
        return false
    end

    dm_entries[dm_id] = {
        id = dm_id,
        is_group = false,
        participants = { my_did, other_did },
        participant_names = { [my_did] = my_name, [other_did] = other_name },
        created_by = dm_owner_did(my_did, other_did),
        created_at = os.time(),
        layer_path = layer_path,
    }

    M.refresh_dm_list()
    M.switch_dm(dm_id)
    return true
end

function M.create_private_channel(name, members)
    if not name or name == "" then return end
    if not members or #members == 0 then return end

    local id = sanitize_id(name)
    if id == "" then return end

    -- Check if already exists
    local existing = dm_entries[id]
    if existing then
        M.switch_dm(id)
        return
    end

    -- Build participant lists (always include self)
    local participants = { my_did }
    local participant_names = { [my_did] = my_name }
    for _, member in ipairs(members) do
        if member.did ~= my_did then
            table.insert(participants, member.did)
            participant_names[member.did] = member.name
        end
    end

    -- Reuse an existing participant layer if already visible, otherwise create.
    local layer_path = find_existing_dm_layer_path(id)
    if not layer_path then
        local result = scribe:create_layer("dms/{id}/messages", id)
        layer_path = normalize_layer_path(result and result.layer_name)
    end

    dm_entries[id] = {
        id = id,
        name = name,
        is_group = true,
        participants = participants,
        participant_names = participant_names,
        created_by = my_did,
        created_at = os.time(),
        layer_path = layer_path,
    }

    if is_my_layer(layer_path) then
        grant_participants(layer_path, participants)
    end

    M.refresh_dm_list()
    M.switch_dm(id)
end

function M.switch_dm(dm_id)
    local meta = dm_entries[dm_id]
    if not meta then return end

    active_dm = dm_id

    local layer_path = meta.layer_path
    if not layer_path then return end

    active_layer_path = layer_path
    messages_layer = scribe:map(page_id .. "/" .. layer_path)

    -- Determine display name
    local display_name = meta.name
    if not display_name or display_name == "" then
        -- 1:1 DM: show other person's name
        if meta.participant_names then
            for did, name in pairs(meta.participant_names) do
                if did ~= my_did then
                    display_name = name
                    break
                end
            end
        end
        if not display_name then display_name = dm_id end
    end

    ui:set("active_dm", dm_id)
    ui:set("active_dm_name", display_name)

    M.refresh_dm_list()

    if on_dm_switch then
        on_dm_switch(dm_id, layer_path)
    end
end

function M.refresh_dm_list()
    local dm_list = {}
    for key, meta in pairs(dm_entries) do
        if meta and meta.participants then
            -- Only show DMs where I'm a participant
            local am_participant = false
            for _, did in ipairs(meta.participants) do
                if did == my_did then
                    am_participant = true
                    break
                end
            end

            if am_participant then
                -- Determine display name
                local display_name = meta.name
                if not display_name or display_name == "" then
                    -- 1:1 DM: show other person's name
                    if meta.participant_names then
                        for did, name in pairs(meta.participant_names) do
                            if did ~= my_did then
                                display_name = name
                                break
                            end
                        end
                    end
                    if not display_name then display_name = meta.id or key end
                end

                table.insert(dm_list, {
                    id = meta.id or key,
                    name = display_name,
                    is_active = (meta.id or key) == active_dm,
                    is_group = meta.is_group or false,
                })
            end
        end
    end

    -- Sort: most recent first
    table.sort(dm_list, function(a, b)
        return a.name < b.name
    end)

    ui:set("dms", dm_list)
end

--- Called when a new layer is discovered via protocol sync.
--- Parses DM layer paths and registers them.
function M.on_layer_discovered(layer_name)
    -- Strip page_id/ prefix if present
    local bare_path = layer_name
    local prefix = page_id .. "/"
    if layer_name:sub(1, #prefix) == prefix then
        bare_path = layer_name:sub(#prefix + 1)
    end

    -- Match: dms/<did>/<id>/messages
    local _did, dm_id = bare_path:match("^dms/([^/]+)/([^/]+)/messages$")
    if not dm_id then return false end

    -- If already known, just update the layer_path
    if dm_entries[dm_id] and dm_entries[dm_id].layer_path then
        return false
    end

    -- Register as discovered DM (we don't know participant names yet)
    if not dm_entries[dm_id] then
        dm_entries[dm_id] = {
            id = dm_id,
            is_group = false,
            participants = { my_did },
            participant_names = { [my_did] = my_name },
            created_at = os.time(),
            layer_path = bare_path,
        }
    else
        dm_entries[dm_id].layer_path = bare_path
    end

    M.refresh_dm_list()
    return true
end

function M.get_active_dm()
    return active_dm
end

function M.get_active_layer_path()
    return active_layer_path
end

function M.get_messages_layer()
    return messages_layer
end

return M
