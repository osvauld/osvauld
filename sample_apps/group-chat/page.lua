-- Group Chat — standalone page definition
page("Group Chat", "1.0.0")

-- =============================================================================
-- Layers
-- =============================================================================

-- Dynamic per-channel messages — LoroMap keyed by message ID
layer("channels/{channel_id}/messages", "map", {
    sync = true, write = true, create = true,

    validate = function(ops, ctx)
        for _, op in ipairs(ops) do
            if op.op == "update" and op.old_value then
                local old = op.old_value
                local new = op.value
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
-- App
-- =============================================================================

app("Group Chat", {
    client = {
        ui = "group-chat/app.slint",
        logic = "group-chat/app.lua"
    }
})
