-- Osvauld Demos - Page Definition
-- Collection of demo apps showcasing different features
page("Demos", "1.0.0")

-- =============================================================================
-- Roles
-- =============================================================================

-- Owner: Full control over all demos
role("owner", {
    can_share = true,
    can_delegate = true
})

-- Collaborator: Can participate in demos
role("collaborator", {
    parent = "owner"
})

-- Node: Relay for multiplayer demos
role("node", {
    parent = "owner",
    can_relay = true
})

-- =============================================================================
-- Layers
-- =============================================================================

-- Snake Game layers
layer("navigation", "map", {
    owner = {"read", "write"},
    collaborator = {"read", "write"},
    node = {"read", "write"}
})

layer("scores", "list", {
    owner = {"read", "write", "sync"},
    collaborator = {"read", "write", "sync"},
    node = {"read", "write", "sync"}
})

layer("game_state", "map", {
    owner = {"read", "write"},
    collaborator = {"read", "write"},
    node = {"read", "write"}
})

-- Tank Game layers
layer("obstacles", "map", {
    owner = {"read", "write", "sync"},
    collaborator = {"read", "write", "sync"},
    node = {"read", "write", "sync"}
})

-- Math Simulation layers
layer("sim_config", "map", {
    owner = {"read", "write", "sync"},
    collaborator = {"read", "write", "sync"},
    node = {"read", "write", "sync"}
})

layer("sim_state", "map", {
    owner = {"read", "write"},
    collaborator = {"read", "write"},
    node = {"read", "write"}
})

-- Group Chat layers

-- Channel metadata (name, topic, created_at)
layer("channels_meta", "map", {
    owner = {"read", "write", "sync"},
    collaborator = {"read", "write", "sync"},
    node = {"read", "write", "sync"}
})

-- Dynamic per-channel messages — LoroMap keyed by message ID
-- Everyone can read/write/sync
layer("channels/{channel_id}/messages", "map", {
    owner = {"read", "write", "sync"},
    collaborator = {"read", "write", "sync"},
    node = {"read", "write", "sync", "create"},

    validate = function(ops, ctx)
        for _, op in ipairs(ops) do
            if op.op == "update" and op.old_value then
                local old = op.old_value
                local new = op.value
                -- Text/deleted/edited: only sender can change
                if (new.text ~= old.text or new.deleted ~= old.deleted or new.edited ~= old.edited)
                   and ctx.from_did ~= old.sender_did then
                    return false, "Only sender can edit their own message"
                end
            end
        end
        return true, nil
    end
})

-- Assets for file sharing
layer("assets", "map", {
    owner = {"read", "write", "sync", "create"},
    collaborator = {"read", "write", "sync"},
    node = {"read", "write", "sync", "create"}
})

-- Read positions — LOCAL ONLY, never synced to peers
-- Each user tracks their own last-read timestamp per channel
-- Keyed by DID, value = { channels = { [channel_id] = last_read_timestamp } }
layer("read_positions", "map", {
    owner = {"read", "write"},
    collaborator = {"read", "write"},
    node = {"read", "write"}
})

-- =============================================================================
-- Apps
-- =============================================================================

-- Group Chat - Real-time messaging
app("Group Chat", {
    client = {
        ui = "group-chat/app.slint",
        logic = "group-chat/app.lua"
    },
    for_role = {"owner", "collaborator"}
})

-- Snake Game - Classic snake with tick loop
app("Snake Game", {
    client = {
        ui = "snake-game/app.slint",
        logic = "snake-game/app.lua",
        tick = true
    },
    for_role = {"owner", "collaborator"}
})

-- Math Simulation - Particle physics demo
app("Math Simulation", {
    client = {
        ui = "math-sim/app.slint",
        logic = "math-sim/app.lua",
        tick = true
    },
    for_role = {"owner", "collaborator"}
})

-- Sthalam Guide - Tab navigation demo
app("Sthalam Guide", {
    client = {
        ui = "guide/app.slint",
        logic = "guide/app.lua"
    },
    for_role = {"owner", "collaborator"}
})

-- Tank Game - Multiplayer tank battle
app("Tank Game", {
    client = {
        ui = "tank-game/app.slint",
        logic = "tank-game/app.lua",
        tick = true
    },
    for_role = {"owner", "collaborator"}
})

-- Sthalam Landing Page
app("Sthalam", {
    client = {
        ui = "sthalam-landing/app.slint",
        logic = "sthalam-landing/app.lua"
    },
    for_role = {"owner", "collaborator"}
})

-- Protocol Documentation
app("Protocol Docs", {
    client = {
        ui = "protocol-docs/app.slint",
        logic = "protocol-docs/app.lua"
    },
    for_role = {"owner", "collaborator"}
})
