-- Landing Page App Logic
-- Showcases osvauld platform with tabbed navigation

-- Local state
local page_id = nil
local nav_layer = nil

-- Features data
local features = {
    {
        icon = "Key",
        title = "Self-Sovereign Identity",
        description = "Ed25519 keypairs for signing, X25519 for encryption. Your identity lives on your devices."
    },
    {
        icon = "Shield",
        title = "UCAN-Based Permits",
        description = "Fine-grained, delegatable authorization. Share access without a central authority."
    },
    {
        icon = "Zap",
        title = "QUIC Transport",
        description = "Fast, encrypted connections via iroh. NAT traversal and relay built-in."
    },
    {
        icon = "Sync",
        title = "Loro CRDT Sync",
        description = "Conflict-free data sync. Works offline, merges automatically when peers reconnect."
    },
    {
        icon = "Code",
        title = "Dynamic Apps",
        description = "Hot-load Lua + Slint apps. No recompilation needed."
    },
    {
        icon = "Server",
        title = "Sovereign Nodes",
        description = "Optional always-on nodes for relay and offline sync."
    }
}

-- Crates data
local crates = {
    { name = "herald", purpose = "Identity, Ed25519/X25519 crypto, signing" },
    { name = "gurkha", purpose = "UCAN permit parsing and validation" },
    { name = "transport", purpose = "QUIC connections, streams, blobs" },
    { name = "courier", purpose = "P2P orchestration, sync protocol, actors" },
    { name = "butler", purpose = "Storage, services API, Loro CRDT scribes" },
    { name = "app_runtime", purpose = "Lua VM, Slint bindings, app lifecycle" }
}

-- Initialize the app
function on_init()
    page_id = permit:page_id()

    -- Navigation state (local only, not synced)
    nav_layer = loro:get_or_create_layer(page_id .. "/navigation", "map")

    -- Load saved tab or default to 0
    local saved_tab = nav_layer:get("current_tab")
    if saved_tab then
        ui:set("current_tab", saved_tab)
    else
        ui:set("current_tab", 0)
    end

    -- Set static data
    ui:set("features", features)
    ui:set("crates", crates)
end

-- Handle click events
function on_click(target)
    -- Tab navigation
    if target:match("^tab:") then
        local tab = tonumber(target:sub(5))
        if tab then
            ui:set("current_tab", tab)
            nav_layer:set("current_tab", tab)
        end
        return
    end

    -- Demo links (would navigate to other apps in real implementation)
    if target:match("^demo:") then
        local demo = target:sub(6)
        log_info("Demo requested: " .. demo)
        -- In a real implementation, this would open the demo app
        -- For now, just log the request
        return
    end
end

-- Loro change handler (not used much for this static app)
function on_loro_change(layer_name, change_type)
    -- Navigation is local-only, no sync needed
end

-- Utilities
function log_info(msg)
    print("[INFO] " .. msg)
end
