-- Text Editor — block-based collaborative editor backed by a LoroTree.
page("Text Editor", "0.1.0")

layer("doc", "tree", {
    sync = true, write = true, create = true,
})

app("Text Editor", {
    client = {
        ui = "text-editor/app.slint",
        logic = "text-editor/app.lua",
    }
})
