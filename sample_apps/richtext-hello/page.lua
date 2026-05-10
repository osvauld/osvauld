-- RichText Hello — minimal smoke test for @osvauld/widgets RichTextEdit
page("RichText Hello", "0.1.0")

layer("doc", "map", {
    sync = true, write = true, create = true,
})

app("RichText Hello", {
    client = {
        ui = "richtext-hello/app.slint",
        logic = "richtext-hello/app.lua",
    }
})
