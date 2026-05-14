---@diagnostic disable: undefined-global, lowercase-global
-- text-editor-egui — Notion-style block editor on egui + LoroTree.
--
-- Data model (same as sample_apps/text-editor — the renderer is what differs):
--   `doc` is a LoroTree. Each node is a block with meta props
--   { kind = "p" | "h1" | "h2" | "list" | "code", text = "..." }. Per-block
--   text uses a nested LoroText container at meta["text"] so two peers can
--   edit the same block concurrently and Loro converges character-by-character.
--
-- Render model (egui-native):
--   Each frame we walk the tree, render each block as a row, and let egui's
--   built-in `dnd_drag_source` / `dnd_drop_zone` handle reorder. No
--   `drag_engine`, no `virtual-pointer`, no Slint VecModel — egui is
--   immediate-mode, so the source of truth is scribe and we re-read every
--   repaint. Wake-ups from peers fire `request_repaint` (see renderer_egui
--   spawn_app G4), so remote edits show up without polling.

local DOC = "doc"

-- ── Design tokens (from design_handoff_block_editor) ───────────────────
-- Dark surfaces from sthalam-theme.css; foreground + hover tints adapted
-- from block-editor.jsx's `ED` token table for a dark backing.
local THEME = {
    page_bg     = "#0A0B10",  -- bg-page
    panel_bg    = "#0D0E13",  -- bg-surface
    raised_bg   = "#14151C",  -- bg-raised
    fg          = "#F5F5F7",
    fg_secondary = "#B6B7C3",
    fg_muted    = "#7F8192",
    fg_faint    = "#4D4E5C",
    hairline    = "#1A1B23",  -- ~ rgba(255,255,255,0.06) flattened
    hover_bg    = "#16171E",  -- ~ rgba(255,255,255,0.035) on page bg
    accent      = "#8A86E5",
    -- Global zoom multiplier applied to every widget. egui's default text
    -- scale is desktop-tool dense (Body=13pt); 1.15 lifts prose into the
    -- ~16px reading range without per-call size overrides leaking into
    -- buttons/toolbar.
    zoom        = 1.15,
}

-- Document geometry from the spec's GEO table.
local GEO = {
    doc_left_pad = 56.0,
    doc_right_pad = 56.0,
    doc_top_pad = 32.0,
    column_max_width = 720.0,  -- inside the side pads
    row_pad_y = 4.0,
}

-- ── Layout helpers (composed from primitives) ──────────────────────────
-- The runtime exposes only thin egui pass-throughs (add_space, set_max_width,
-- horizontal, vertical, available_width). Anything app-specific — including
-- "Notion-style centered reading column" — composes here in Lua.
local function centered_column(width, body)
    local avail = ui:available_width()
    local pad = math.max((avail - width) * 0.5, 0)
    ui:horizontal(function()
        ui:add_space(pad)
        ui:vertical(function()
            ui:set_max_width(width)
            body()
        end)
    end)
end

-- ── kinds + styling ────────────────────────────────────────────────────
-- Notion-style block kinds. Tab through them with the per-block kind button,
-- or (Phase 2) via the slash menu. Each kind picks a different widget mix:
--   p/list/nlist/quote     → plain text_edit, varying prefixes
--   h1/h2/h3               → text_edit_sized at heading point sizes
--   todo                   → checkbox + text_edit, "done" prop carries state
--   code                   → multiline monospace text_edit
--   divider                → ui:separator(), no text edit
local KINDS = { "p", "h1", "h2", "h3", "list", "nlist", "todo", "quote", "code", "divider" }


-- Index of this block among its same-kind, same-parent older siblings + self.
-- Used to render numbered-list prefixes (1., 2., 3., …). Resets across other
-- kinds so a paragraph between two nlist runs starts the second run at 1.
local function nlist_index(nodes, here)
    local count = 0
    for _, n in ipairs(nodes) do
        if n.parent == here.parent then
            local k = (type(n.props) == "table" and n.props.kind) or "p"
            if k == "nlist" then
                count = count + 1
                if n.id == here.id then return count end
            else
                count = 0
            end
        end
    end
    return count
end

-- ── codepoint-safe diff ────────────────────────────────────────────────
-- Same approach as sample_apps/text-editor: longest common prefix + suffix
-- on the codepoint arrays, yielding (pos, del, ins) in *codepoints* which is
-- what LoroText:insert/delete consume. UTF-8 single-byte assumptions break
-- on real prose, so we never touch byte indices.
local function to_codepoints(s)
    local cps = {}
    for _, c in utf8.codes(s) do cps[#cps + 1] = c end
    return cps
end

local function diff_text(old, new)
    if old == new then return nil end
    local op, np = to_codepoints(old), to_codepoints(new)
    local pre = 0
    while pre < #op and pre < #np and op[pre + 1] == np[pre + 1] do
        pre = pre + 1
    end
    local suf = 0
    while suf < (#op - pre) and suf < (#np - pre)
        and op[#op - suf] == np[#np - suf] do
        suf = suf + 1
    end
    local del = #op - pre - suf
    local ins_pieces = {}
    for i = pre + 1, #np - suf do
        ins_pieces[#ins_pieces + 1] = utf8.char(np[i])
    end
    return pre, del, table.concat(ins_pieces)
end

-- Translate "egui handed me a new buffer for this block" into the smallest
-- LoroText insert/delete pair. Reads the scribe-side snapshot fresh each call
-- so concurrent remote ops don't make us compute the delta against a stale
-- view of the block.
local function apply_text_edit(node_id, new_text)
    local node_text = scribe:tree(DOC):text(node_id)
    local prev = node_text:to_string()
    local pos, del, ins = diff_text(prev, new_text)
    if pos == nil then return end
    if del > 0 then node_text:delete(pos, del) end
    if #ins > 0 then node_text:insert(pos, ins) end
end

-- Cross-frame focus tracking. egui hands us `has_focus` for the text edit each
-- frame; we record the last focused block id so the top toolbar can convert
-- whatever the user most recently edited, even if focus has since drifted to
-- a button (clicking a toolbar button drops focus from the editor).
local LAST_FOCUSED_ID = nil

-- Set on Shift+Enter: the id of the newly-created empty block we want to
-- focus on the NEXT frame (we can't focus on the same frame we create — the
-- new block isn't in the tree-walk yet). The render loop checks this before
-- each text edit and calls `ui:focus_next()` for the matching block.
local PENDING_FOCUS_ID = nil

-- Slash menu state. When the user types "/" as the very first character of a
-- block, we open a popup anchored below that block listing the kinds. As they
-- keep typing ("/he"), we filter. Selecting a kind converts the block and
-- clears the slash text. `target_id` is the node whose popup is open; we
-- only ever show one slash menu at a time.
local SLASH = { target_id = nil }
local SLASH_POPUP_ID = "slash_menu"

-- Helper: drive the slash menu off a freshly-rendered text_edit. Call right
-- after `ui:text_edit(...)`. Handles open/close, filtering, and selection.
local function drive_slash_menu(node, _kind, new_text, changed, has_focus)
    -- Open when "/" is the entire content (start-of-block slash trigger,
    -- matching Notion). Limiting to == "/" avoids triggering on existing
    -- code / paragraph content that happens to contain a slash.
    if has_focus and changed and new_text == "/" then
        SLASH.target_id = node.id
        ui:open_popup(SLASH_POPUP_ID)
    end

    -- Re-sync if egui closed the popup via click-outside / esc.
    if SLASH.target_id == node.id and not ui:is_popup_open(SLASH_POPUP_ID) then
        SLASH.target_id = nil
    end

    -- Close when the trigger text is no longer present (user backspaced
    -- past "/" or transformed it).
    if SLASH.target_id == node.id and new_text:sub(1, 1) ~= "/" then
        ui:close_popup(SLASH_POPUP_ID)
        SLASH.target_id = nil
    end

    if SLASH.target_id ~= node.id then return end

    -- Filter: text after the slash, lowercased, substring-match against kind.
    local query = new_text:sub(2):lower()
    ui:popup_below_last(SLASH_POPUP_ID, function()
        local matched = false
        for _, k in ipairs(KINDS) do
            if query == "" or k:find(query, 1, true) then
                matched = true
                if ui:button(k) then
                    -- Convert and clear the slash text. Use set_prop for
                    -- both fields so the tree update happens in one shot.
                    scribe:tree(DOC):set_prop(node.id, "kind", k)
                    apply_text_edit(node.id, "")
                    ui:close_popup(SLASH_POPUP_ID)
                    SLASH.target_id = nil
                    -- Keep focus on the converted block so the user can
                    -- start typing immediately.
                    PENDING_FOCUS_ID = node.id
                end
            end
        end
        if not matched then
            ui:label("no kinds match", { color = "#888899" })
        end
    end)
end

-- Newline-as-block-split. egui's multiline TextEdit treats Enter as a real
-- "\n" insert. We watch for any "\n" in the post-edit text and split there:
-- the part before stays on this block; the part after becomes a new block of
-- the SAME kind, inserted right below. Matches Notion's Enter behavior.
-- Block creation semantics:
--   Enter      → in-line "\n" (handled natively by egui's multiline TextEdit)
--   Shift+Enter → create a new EMPTY block below the current one
--
-- For Shift+Enter we strip the "\n" egui just inserted (so the current
-- block doesn't keep the line break) and `create()` a new empty same-kind
-- block. Text after the cursor stays in the current block — the new block
-- is intentionally empty so the user is dropped into a fresh slot.
local function shift_enter_pressed()
    return ui:key_pressed("enter") and ui:modifiers().shift
end

local function maybe_split_on_newline(node, kind, new_text)
    if not shift_enter_pressed() then return false end
    -- Strip the trailing "\n" egui inserted this frame (last one).
    local nl
    for i = #new_text, 1, -1 do
        if new_text:sub(i, i) == "\n" then nl = i; break end
    end
    local cleaned = nl
        and (new_text:sub(1, nl - 1) .. new_text:sub(nl + 1))
        or new_text
    apply_text_edit(node.id, cleaned)
    local new_id = scribe:tree(DOC):create(node.parent, node.index + 1, { kind = kind }, { "text" })
    -- Defer focus to the NEXT frame — the new block isn't in this frame's
    -- tree walk yet, so there's no text edit to focus right now.
    PENDING_FOCUS_ID = new_id
    return true
end

local function edit_and_split(node, kind, text, opts)
    -- Consume pending-focus (set by the prior frame's Shift+Enter). The
    -- one-shot flag is set BEFORE text_edit so the binding's "request focus
    -- on next text_edit" hook fires for this specific edit.
    if PENDING_FOCUS_ID == node.id then
        ui:focus_next()
        PENDING_FOCUS_ID = nil
    end
    local new_text, changed, has_focus = ui:text_edit(text, opts)
    if has_focus then LAST_FOCUSED_ID = node.id end
    if changed then
        if not maybe_split_on_newline(node, kind, new_text) then
            apply_text_edit(node.id, new_text)
        end
    end
    -- Slash menu sits adjacent to the text edit: open on "/" trigger,
    -- filter by trailing text, click to convert. Composed in Lua over
    -- the primitive `popup_below_last` binding.
    drive_slash_menu(node, kind, new_text, changed, has_focus)
end

-- Notion-style styling lives at the app level: borderless edits, sized
-- headings, quote in italic+muted, code in monospace multiline. The runtime
-- exposes one `text_edit(value, opts)` and one `label(text, opts)`; this
-- app maps its block kinds onto those.
-- Multiline = true everywhere so long sentences wrap. We treat a newline as
-- "split this block here": detect "\n" in the new text after each edit and
-- create a new block of the same kind for the post-newline content.
-- Sizes are PRE-ZOOM logical pts; multiplied by THEME.zoom (set via
-- ui:set_zoom) for final pixel size. Targets after zoom=1.15:
--   body 16px, h1 32px, h2 26px, h3 21px — close to Notion's web scale.
local TEXT_EDIT_OPTS = {
    h1    = { size = 28.0, frame = false, weight = "bold", multiline = true, rows = 1 },
    h2    = { size = 22.0, frame = false, weight = "bold", multiline = true, rows = 1 },
    h3    = { size = 18.0, frame = false, weight = "bold", multiline = true, rows = 1 },
    p     = { size = 14.0, frame = false, multiline = true, rows = 1 },
    list  = { size = 14.0, frame = false, multiline = true, rows = 1 },
    nlist = { size = 14.0, frame = false, multiline = true, rows = 1 },
    todo  = { size = 14.0, frame = false, multiline = true, rows = 1 },
    quote = { size = 14.0, frame = false, weight = "italic", multiline = true, rows = 1 },
}
local CODE_EDIT_OPTS = { size = 13.0, frame = true, multiline = true, monospace = true, code = true, rows = 2 }
local PREFIX_LABEL_OPTS = { color = THEME.fg_muted }

local function sized_edit(node, kind, text, opts)
    edit_and_split(node, kind, text, opts)
end

-- Edit the block text inline. Prefix is a static label shown to the left of
-- the editor (e.g. "• ", "1. "). Pass nil for no prefix.
local function plain_edit(node, text, kind, prefix)
    if prefix then ui:label(prefix, PREFIX_LABEL_OPTS) end
    edit_and_split(node, kind, text, TEXT_EDIT_OPTS[kind] or TEXT_EDIT_OPTS.p)
end

-- Render one block's content. Called by on_frame *inside* an outer horizontal
-- that already owns the drag grip — render_block contributes only the body
-- (prefix/editor + controls) for that row.
--
-- Divider and code branch off the inline horizontal layout because divider is
-- a single horizontal rule and code wants a multiline editor stacked below
-- its controls row.
function render_block(node, kind, text, props, nodes)
    if kind == "divider" then
        ui:separator()
        return
    end

    if kind == "code" then
        ui:vertical(function()
            ui:label("code", { color = THEME.fg_muted, monospace = true })
            local new_text, changed, has_focus = ui:text_edit(text, CODE_EDIT_OPTS)
            if has_focus then LAST_FOCUSED_ID = node.id end
            if changed then apply_text_edit(node.id, new_text) end
        end)
        return
    end

    if kind == "h1" or kind == "h2" or kind == "h3" then
        sized_edit(node, kind, text, TEXT_EDIT_OPTS[kind])
    elseif kind == "list" then
        plain_edit(node, text, kind, "• ")
    elseif kind == "nlist" then
        plain_edit(node, text, kind, tostring(nlist_index(nodes, node)) .. ". ")
    elseif kind == "quote" then
        plain_edit(node, text, kind, "❝ ")
    elseif kind == "todo" then
        local done = props.done == true
        local new_done = ui:checkbox(done)
        if new_done ~= done then
            scribe:tree(DOC):set_prop(node.id, "done", new_done)
        end
        plain_edit(node, text, kind, nil)
    else
        plain_edit(node, text, kind, nil)
    end
end

function on_init()
    -- Seed an empty paragraph so the editor isn't blank on first open. Both
    -- peers may seed concurrently; Loro merges the two creates as siblings,
    -- not duplicates of one node. Acceptable for the demo.
    local nodes = scribe:tree(DOC):walk()
    if #nodes == 0 then
        scribe:tree(DOC):create(nil, nil, { kind = "p" }, { "text" })
    end
end

function on_frame(ui)
    _G.ui = ui

    -- Theme — applied every frame so a hot reload of app.lua picks up
    -- changes without restarting the runtime. egui caches it; cheap.
    ui:set_visuals({
        panel_fill = THEME.page_bg,
        window_fill = THEME.panel_bg,
        fg = THEME.fg,
        extreme_bg = THEME.raised_bg,
        hyperlink = THEME.accent,
        -- Make rows blend with the page bg at rest. egui's dnd_drop_zone
        -- always paints `widgets.inactive.bg_fill` regardless of the frame
        -- you pass in, so we zero that here. The hover bg comes from our
        -- own hover_row Frame; drop-target tint stays via widgets.active.
        widget_inactive_fill = "#00000000",
        widget_inactive_stroke = "#00000000",
        widget_noninteractive_fill = "#00000000",
    })

    -- Doc-density zoom. set_zoom is idempotent and cheap; egui no-ops if
    -- the factor matches the current value.
    ui:set_zoom(THEME.zoom)

    -- Top toolbar — converts the last-focused block to the chosen kind. If
    -- no block has been focused yet (fresh session), appends a new empty
    -- block of that kind.
    ui:horizontal(function()
        ui:add_space(GEO.doc_left_pad)
        ui:label("text-editor", { size = 12.0, color = THEME.fg_muted, weight = "bold" })
        ui:label("  ", PREFIX_LABEL_OPTS)
        for _, kind in ipairs(KINDS) do
            if ui:button(kind) then
                if LAST_FOCUSED_ID then
                    scribe:tree(DOC):set_prop(LAST_FOCUSED_ID, "kind", kind)
                else
                    scribe:tree(DOC):create(nil, nil, { kind = kind }, { "text" })
                end
            end
        end
    end)
    ui:add_space(GEO.doc_top_pad)

    -- Walk the tree fresh every frame. Egui is immediate-mode: the cost of
    -- not caching is one in-process actor RPC per repaint, which is cheap.
    -- Caching here would mean writing invalidation, which is what bind_tree
    -- did for Slint and what we explicitly *don't* want on the egui path.
    local nodes = scribe:tree(DOC):walk()

    ui:scroll_area(function()
        centered_column(GEO.column_max_width, function()
            for _, node in ipairs(nodes) do
                local props = type(node.props) == "table" and node.props or {}
                local kind = props.kind or "p"
                local text = props.text or ""

                -- hover_row paints a soft bg only when this row was hovered
                -- last frame (one-frame lag, imperceptible). Drop zone is
                -- transparent at rest, so the only visible bg is the hover
                -- highlight here. The grip label only renders when hovered —
                -- Notion convention: clean line at rest, affordance on hover.
                local dropped = ui:drop_zone(function()
                    -- Grip lives INSIDE hover_row so pointer hover on the
                    -- gutter still counts as hovering the row — otherwise
                    -- the grip (transparent at rest) is unreachable. The
                    -- gutter is a fixed-width left pad, content starts at a
                    -- consistent column regardless of kind.
                    ui:hover_row(
                        { id = node.id, bg = THEME.hover_bg, rounding = 4,
                          padding = GEO.row_pad_y },
                        function(hovered)
                            ui:horizontal(function()
                                -- Use `::` for the grip glyph — egui's
                                -- default fonts (Ubuntu-Light, Hack) don't
                                -- include U+22EE (⋮), so it tofu's into a
                                -- square. Two stacked colons read as a
                                -- 2×2 dot pattern, which is close enough to
                                -- a grip and renders everywhere.
                                local grip_color = hovered and THEME.fg_faint or "#00000000"
                                ui:drag_source(
                                    node.id, node.id,
                                    function()
                                        ui:label("::", { color = grip_color, monospace = true })
                                    end,
                                    function()
                                        local preview = text ~= "" and text or "(empty)"
                                        if #preview > 40 then preview = preview:sub(1, 40) .. "…" end
                                        ui:label(":: " .. kind .. "  " .. preview,
                                            { color = THEME.fg_secondary })
                                    end
                                )
                                ui:add_space(8)
                                render_block(node, kind, text, props, nodes)
                            end)
                        end
                    )
                end)

                if dropped and dropped ~= node.id then
                    scribe:tree(DOC):move(dropped, node.parent, node.index)
                end
            end

            if ui:button("+ Add block") then
                scribe:tree(DOC):create(nil, nil, { kind = "p" }, { "text" })
            end
        end)
    end)
end

-- ── Test helpers (control-socket eval target) ─────────────────────────
-- Mirror sample_apps/text-editor's test surface so the e2e script can drive
-- the same operations regardless of renderer.

function get_block_count()
    return #scribe:tree(DOC):walk()
end

function get_block_text(index)
    local n = scribe:tree(DOC):walk()[index]
    if n and type(n.props) == "table" then return n.props.text end
    return nil
end

function get_block_id(index)
    local n = scribe:tree(DOC):walk()[index]
    return n and n.id or nil
end

function get_block_kind(index)
    local n = scribe:tree(DOC):walk()[index]
    if n and type(n.props) == "table" then return n.props.kind end
    return nil
end

function add_block(text)
    local id = scribe:tree(DOC):create(nil, nil, { kind = "p" }, { "text" })
    if text and text ~= "" then apply_text_edit(id, text) end
    return id
end

function set_block_text(index, text)
    local id = get_block_id(index)
    if id then apply_text_edit(id, text) end
end

function set_block_kind(index, kind)
    local id = get_block_id(index)
    if id then scribe:tree(DOC):set_prop(id, "kind", kind) end
end

function delete_block(index)
    local id = get_block_id(index)
    if id then scribe:tree(DOC):delete(id) end
end

-- Programmatic reorder for the e2e test (egui dnd can't be driven from outside).
-- intent: "above" | "below" — same semantics as the drop_zone path.
function move_block(src_index, target_index, intent)
    local nodes = scribe:tree(DOC):walk()
    local src, target = nodes[src_index], nodes[target_index]
    if not src or not target then return end
    if intent == "above" then
        scribe:tree(DOC):move(src.id, target.parent, target.index)
    elseif intent == "below" then
        scribe:tree(DOC):move(src.id, target.parent, target.index + 1)
    end
end

function insert_at_block(index, pos, content)
    local id = get_block_id(index)
    if id then scribe:tree(DOC):text(id):insert(pos, content) end
end

-- ── New-kind helpers (phase 1) ─────────────────────────────────────────

function get_block_done(index)
    local n = scribe:tree(DOC):walk()[index]
    if n and type(n.props) == "table" then return n.props.done == true end
    return nil
end

function set_block_done(index, done)
    local id = get_block_id(index)
    if id then scribe:tree(DOC):set_prop(id, "done", done == true) end
end

-- Add a block of a specific kind. text optional. Returns the new node id.
function add_block_kind(kind, text)
    local id = scribe:tree(DOC):create(nil, nil, { kind = kind }, { "text" })
    if text and text ~= "" then apply_text_edit(id, text) end
    return id
end
