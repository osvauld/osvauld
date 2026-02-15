-- Osvauld Demos - Page Definition
-- Collection of demo apps showcasing different features
page("Demos", "1.0.0")

-- =============================================================================
-- Layers
-- =============================================================================

-- Snake Game layers
layer("navigation", "map", {
    sync = true, write = true
})

layer("scores", "list", {
    sync = true, write = true
})

layer("game_state", "map", {
    sync = false, write = true
})

-- Tank Game layers
layer("obstacles", "map", {
    sync = true, write = true
})

-- Math Simulation layers
layer("sim_config", "map", {
    sync = true, write = true
})

layer("sim_state", "map", {
    sync = false, write = true
})

-- Group Chat layers

-- Dynamic per-channel messages — LoroMap keyed by message ID
layer("channels/{channel_id}/messages", "map", {
    sync = true, write = true, create = true,

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
    sync = true, write = true, create = true
})


-- =============================================================================
-- Apps
-- =============================================================================

-- Group Chat - Real-time messaging
app("Group Chat", {
    client = {
        ui = "group-chat/app.slint",
        logic = "group-chat/app.lua"
    }
})

-- Snake Game - Classic snake with tick loop
app("Snake Game", {
    client = {
        ui = "snake-game/app.slint",
        logic = "snake-game/app.lua",
        tick = true
    }
})

-- Math Simulation - Particle physics demo
app("Math Simulation", {
    client = {
        ui = "math-sim/app.slint",
        logic = "math-sim/app.lua",
        tick = true
    }
})

-- Sthalam Guide - Tab navigation demo
app("Sthalam Guide", {
    client = {
        ui = "guide/app.slint",
        logic = "guide/app.lua"
    }
})

-- Tank Game - Multiplayer tank battle
app("Tank Game", {
    client = {
        ui = "tank-game/app.slint",
        logic = "tank-game/app.lua",
        tick = true
    }
})

-- Sthalam Landing Page
app("Sthalam", {
    client = {
        ui = "sthalam-landing/app.slint",
        logic = "sthalam-landing/app.lua"
    }
})

-- Protocol Documentation
app("Protocol Docs", {
    client = {
        ui = "protocol-docs/app.slint",
        logic = "protocol-docs/app.lua"
    }
})
