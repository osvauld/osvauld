-- Canvas App - Page Definition
-- Multiplayer canvas with real-time collaboration
page("Canvas", "1.0.0")

-- =============================================================================
-- Roles
-- =============================================================================

-- Owner: Full control over the canvas
role("owner", {
    can_share = true,
    can_delegate = true
})

-- Collaborator: Can draw and sync
role("collaborator", {
    parent = "owner"
})

-- Node: Relay for real-time sync
role("node", {
    parent = "owner",
    can_relay = true
})

-- =============================================================================
-- Layers
-- =============================================================================

-- Canvas shapes (rectangles, circles, etc.)
layer("shapes", "map", {
    owner = {"read", "write", "sync"},
    collaborator = {"read", "write", "sync"},
    node = {"read", "sync"}
})

-- Connectors between shapes
layer("connectors", "map", {
    owner = {"read", "write", "sync"},
    collaborator = {"read", "write", "sync"},
    node = {"read", "sync"}
})

-- =============================================================================
-- Apps
-- =============================================================================

-- Canvas User app (shared by owner and collaborators)
app("Canvas", {
    client = {
        ui = "canvas-user/app.slint",
        logic = "canvas-user/app.lua",
        tick = true
    },
    for_role = {"owner", "collaborator"}
})
