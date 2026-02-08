-- Photo Gallery - Page Definition
-- Shared photo gallery with collaborative uploads
page("Gallery", "1.0.0")

-- =============================================================================
-- Roles
-- =============================================================================

-- Owner: Gallery owner with full control
role("owner", {
    can_share = true,
    can_delegate = true
})

-- Collaborator: Can view and upload photos
role("collaborator", {
    parent = "owner"
})

-- Node: Relay for photo sync
role("node", {
    parent = "owner",
    can_relay = true
})

-- =============================================================================
-- Layers
-- =============================================================================

-- Photo metadata list
layer("photos", "list", {
    owner = {"read", "write", "sync"},
    collaborator = {"read", "write", "sync"},
    node = {"read", "write", "sync"}
})

-- Binary assets (photo files)
layer("assets", "map", {
    owner = {"read", "write", "sync", "create"},
    collaborator = {"read", "write", "sync"},
    node = {"read", "write", "sync", "create"}
})

-- =============================================================================
-- Apps
-- =============================================================================

-- Gallery User app (shared by owner and collaborators)
app("Gallery", {
    client = {
        ui = "gallery-user/app.slint",
        logic = "gallery-user/app.lua"
    },
    for_role = {"owner", "collaborator"}
})
