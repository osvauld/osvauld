-- RichText Hello — minimal app validating @osvauld/widgets RichTextEdit
--
-- doc layer is a LoroMap; the "body" key holds the editor's text. The host
-- declarative `scribe:bind_text` binding pushes layer changes to the
-- `doc_text` Slint property automatically (no on_init load, no manual refresh).
-- The app only writes back through `on_field_changed`.

local DOC_LAYER = "doc"
local BODY_KEY = "body"

function on_init()
    scribe:bind_text("doc_text", DOC_LAYER, BODY_KEY)
end

-- Generic Event Bus callback. AppAPI.on_field_changed(field, value).
function on_field_changed(field, value)
    if field == "body" then
        scribe:map(DOC_LAYER):set(BODY_KEY, value)
    end
end

-- Test helpers (called from e2e_tests via control-socket eval).
function set_body(text)
    scribe:map(DOC_LAYER):set(BODY_KEY, text)
end

function get_body()
    local v = scribe:map(DOC_LAYER):get(BODY_KEY)
    if type(v) == "string" then return v end
    return ""
end

-- Read the Slint property the bind_text writes to. Used by e2e tests
-- to confirm the reactive sync path (layer change → UI property update).
function get_doc_text_property()
    local v = ui:get("doc_text")
    if type(v) == "string" then return v end
    return ""
end
