-- Text Editor — block-based editor over a LoroTree.
--
-- Document model:
--   The `doc` layer is a `LoroTree`. Each tree node is a block carrying
--   { kind = "p" | "h1" | "h2" | "code" | "list_item", text = "..." } as its
--   meta props. Paragraph kinds always live at the root; `list_item` blocks
--   can nest via Tab / Shift-Tab, which call `tree:move(...)` to reparent.
--
-- Sync surface:
--   `scribe:bind_tree("blocks", "doc")` keeps the Slint VecModel in sync
--   with the tree on both local writes and remote applies. Each item is
--   { id, parent, depth, index, props = { kind, text } }.

local DOC = "doc"

-- Helper: locate a block in the current walk by id, returning its index field.
-- We re-walk on every call rather than caching — the tree is small in the
-- MVP and a stale cache caused more bugs than it saved.
local function find_block_index(node_id)
    local nodes = scribe:tree(DOC):walk()
    for _, n in ipairs(nodes) do
        if n.id == node_id then return n.index, n.parent end
    end
    return nil, nil
end

-- Find the node that comes just before `node_id` under the same parent.
-- Returns nil if there is no previous sibling (i.e. node is first child).
local function find_prev_sibling(node_id)
    local nodes = scribe:tree(DOC):walk()
    local target_parent, target_index
    for _, n in ipairs(nodes) do
        if n.id == node_id then
            target_parent = n.parent
            target_index = n.index
            break
        end
    end
    if target_index == nil then return nil end
    local best
    for _, n in ipairs(nodes) do
        if n.parent == target_parent and n.index < target_index then
            if not best or n.index > best.index then best = n end
        end
    end
    return best
end

-- Diff two strings into (pos, del_count, insert_string), all in *unicode
-- codepoints* — which is what Loro's LoroText:insert/delete expect. Returns
-- nil when the strings are identical.
--
-- Strategy: longest common prefix + longest common suffix, computed on
-- codepoint arrays so multi-byte UTF-8 sequences never split mid-codepoint.
-- The middle of `old` is what got deleted; the middle of `new` is what got
-- inserted. For typical single-keystroke edits this is one codepoint of
-- insert OR delete; for paste / undo it's a longer span. Selection-replace
-- collapses naturally to "delete some, insert some" at the same position.
local function to_codepoints(s)
    local cps = {}
    for _, c in utf8.codes(s) do
        cps[#cps + 1] = c
    end
    return cps
end

local function diff_text(old, new)
    if old == new then return nil end
    local op = to_codepoints(old)
    local np = to_codepoints(new)
    local pre = 0
    while pre < #op and pre < #np and op[pre + 1] == np[pre + 1] do
        pre = pre + 1
    end
    local suf = 0
    while suf < (#op - pre) and suf < (#np - pre)
        and op[#op - suf] == np[#np - suf] do
        suf = suf + 1
    end
    local del_count = #op - pre - suf
    local ins_pieces = {}
    for i = pre + 1, #np - suf do
        ins_pieces[#ins_pieces + 1] = utf8.char(np[i])
    end
    return pre, del_count, table.concat(ins_pieces)
end

-- Apply a Slint-side text edit by diffing against scribe's current snapshot
-- and emitting the smallest insert/delete pair on the per-block LoroText.
-- The snapshot read is an in-process actor RPC — fast — and gives us the
-- authoritative prev so concurrent remote ops don't make us miscompute the
-- delta against a stale view.
function on_block_text_edited(node_id, new_text)
    local node_text = scribe:tree(DOC):text(node_id)
    local prev = node_text:to_string()
    local pos, del, ins = diff_text(prev, new_text)
    if pos == nil then return end
    if del > 0 then node_text:delete(pos, del) end
    if #ins > 0 then node_text:insert(pos, ins) end
end

-- Find the node by id (full row).
local function find_node(node_id)
    for _, n in ipairs(scribe:tree(DOC):walk()) do
        if n.id == node_id then return n end
    end
    return nil
end

function on_init()
    scribe:bind_tree("blocks", DOC)

    -- Seed an empty paragraph so the editor isn't blank on first open.
    -- Both peers will end up with the same seed because creates are CRDT
    -- ops; the second peer's seed merges as a sibling, not a duplicate of
    -- the first. (Acceptable for MVP — real collab would gate via "first
    -- writer wins" or have alice seed and bob skip.)
    local nodes = scribe:tree(DOC):walk()
    if #nodes == 0 then
        -- Pre-materialise the per-block LoroText container at meta["text"]
        -- in the same commit as node creation. This makes the creator the
        -- sole owner of container instantiation; remote peers receive the
        -- wired-up container via sync, so first-keystroke writers never
        -- race on `meta.insert_container`. The walk's row JSON then carries
        -- a string at `props.text` from the moment Slint first sees the row.
        scribe:tree(DOC):create(nil, nil, { kind = "p" }, { "text" })
    end
end

-- Single Event Bus dispatcher. The renderer only auto-wires a small
-- handful of "generic" Slint callback names to Lua (see
-- renderer_slint::setup_global_callbacks); custom callbacks declared on
-- AppAPI fire in Slint but never reach Lua. So app.slint multiplexes all
-- editor events through on_field_changed with an action-prefixed `field`:
--   "text:<id>"      → block text edited; `value` is the new text
--   "enter:<id>"     → Enter key at end of block; create sibling
--   "backspace:<id>" → Backspace at offset 0; delete empty block
--   "kind:<id>"      → toolbar pressed; `value` is "p"|"h1"|"h2"|"code"|"list_item"
--   "indent:<id>"    → Tab pressed on a list_item; reparent under prev sibling
--   "outdent:<id>"   → Shift-Tab; reparent up to grandparent
-- We split here and dispatch.
function on_field_changed(field, value)
    local action, node_id = field:match("^([^:]+):(.+)$")
    if not action then return end

    if action == "text" then
        on_block_text_edited(node_id, value)
    elseif action == "enter" then
        on_block_enter(node_id)
    elseif action == "backspace" then
        on_block_backspace_at_start(node_id)
    elseif action == "kind" then
        scribe:tree(DOC):set_prop(node_id, "kind", value)
    elseif action == "indent" then
        on_block_indent(node_id)
    elseif action == "outdent" then
        on_block_outdent(node_id)
    end
end

-- Enter at end of a block → create a sibling below + focus it.
-- New block inherits `kind` from the current block when it's a list_item, so
-- pressing Enter inside a list continues the list. For other kinds we fall
-- back to "p" — Enter inside an h1 makes a paragraph, not another heading.
function on_block_enter(node_id)
    local cur = find_node(node_id)
    if not cur then return end
    local cur_kind = (type(cur.props) == "table" and cur.props.kind) or "p"
    local cur_text = (type(cur.props) == "table" and cur.props.text) or ""

    -- Empty list_item + Enter → outdent (or convert to "p" at depth 0). This
    -- is the standard "press Enter twice to escape the list" affordance.
    if cur_kind == "list_item" and cur_text == "" then
        if cur.depth and cur.depth > 0 then
            on_block_outdent(node_id)
        else
            scribe:tree(DOC):set_prop(node_id, "kind", "p")
        end
        return
    end

    local new_kind = cur_kind == "list_item" and "list_item" or "p"
    -- Same pre-materialisation pattern as on_init: create the LoroText
    -- container at meta["text"] in the same commit so concurrent first-edits
    -- on the new block can't race.
    local new_id = scribe:tree(DOC):create(cur.parent, cur.index + 1, { kind = new_kind }, { "text" })
    -- Slint side: a row with `block.id == AppAPI.focus_block_id` calls focus()
    -- on its RichTextEdit when this property changes.
    ui:set("focus_block_id", new_id)
end

-- Tab on a list_item → reparent under the previous sibling (indent one level).
-- No-op if there is no previous sibling (can't indent a first child) or if
-- this isn't a list_item (Tab on a paragraph is reserved for future use).
function on_block_indent(node_id)
    local cur = find_node(node_id)
    if not cur then return end
    local kind = (type(cur.props) == "table" and cur.props.kind) or "p"
    if kind ~= "list_item" then return end

    local prev = find_prev_sibling(node_id)
    if not prev then return end

    -- Reparent as the *last* child of prev. Passing nil index appends.
    scribe:tree(DOC):move(node_id, prev.id, nil)
    -- Keep focus on the moved node — its row id is stable, so diff_tree_rows
    -- emits Set and the editor child survives. We re-assert focus_block_id
    -- to nudge `wants-focus` in case it was already pointing here.
    ui:set("focus_block_id", node_id)
end

-- Shift-Tab on a list_item → reparent up to the grandparent, just after the
-- current parent's position. No-op at depth 0 (already at the root).
function on_block_outdent(node_id)
    local cur = find_node(node_id)
    if not cur then return end
    local kind = (type(cur.props) == "table" and cur.props.kind) or "p"
    if kind ~= "list_item" then return end
    -- walk() reports root nodes with parent == nil. No nil/empty parent →
    -- already at root, nothing to outdent.
    if not cur.parent then return end

    local parent_node = find_node(cur.parent)
    if not parent_node then return end

    -- Slot in just after the parent under the grandparent. parent_node.parent
    -- is nil for root nodes — passing nil to tree:move means "make it root".
    scribe:tree(DOC):move(node_id, parent_node.parent, parent_node.index + 1)
    ui:set("focus_block_id", node_id)
end

-- Backspace at start of an empty block → delete the block, focus previous.
-- For the MVP we only delete on truly empty blocks; merging text into the
-- previous block is Step B territory.
function on_block_backspace_at_start(node_id)
    local nodes = scribe:tree(DOC):walk()
    if #nodes <= 1 then return end  -- never delete the last surviving block

    local cur_idx
    for i, n in ipairs(nodes) do
        if n.id == node_id then cur_idx = i break end
    end
    if not cur_idx then return end

    local current = nodes[cur_idx]
    local text = (type(current.props) == "table" and current.props.text) or ""
    if text ~= "" then return end  -- non-empty: let TextInput delete one char

    scribe:tree(DOC):delete(node_id)
    local prev = nodes[cur_idx - 1]
    if prev then
        ui:set("focus_block_id", prev.id)
    end
end

-- Test helpers (called via control-socket eval).

function get_block_count()
    return #scribe:tree(DOC):walk()
end

function get_block_text(index)
    local nodes = scribe:tree(DOC):walk()
    local n = nodes[index]
    if n and type(n.props) == "table" then return n.props.text end
    return nil
end

function get_block_id(index)
    local nodes = scribe:tree(DOC):walk()
    local n = nodes[index]
    return n and n.id or nil
end

-- Programmatic helpers used by the e2e test.
function add_block(text)
    -- Create an empty paragraph then route the seed text through the same
    -- diff path the editor uses on `edited`. Keeps a single code path for
    -- "block text changed" — the LoroText container is lazily created and
    -- the test exercises the same write-side behaviour as a real keystroke.
    -- Pass text_keys={"text"} so the LoroText container is created up-front.
    -- The seed text (if any) flows through `on_block_text_edited` → diff →
    -- nested LoroText insert, exercising the same write path as a real
    -- keystroke. Tests therefore stress what we ship.
    local id = scribe:tree(DOC):create(nil, nil, { kind = "p" }, { "text" })
    if text and text ~= "" then on_block_text_edited(id, text) end
    return id
end

function set_block_text(index, text)
    local id = get_block_id(index)
    if id then on_block_text_edited(id, text) end
end

-- Direct positional insert into a block's nested LoroText. Used by the
-- concurrent-write test to drive two peers into char-level CRDT merge
-- territory without going through the diff path (which would compute the
-- diff against each peer's local snapshot and produce equivalent ops, but
-- is harder to reason about under concurrency).
function insert_at_block(index, pos, content)
    local id = get_block_id(index)
    if id then scribe:tree(DOC):text(id):insert(pos, content) end
end

function delete_at_block(index, pos, len)
    local id = get_block_id(index)
    if id then scribe:tree(DOC):text(id):delete(pos, len) end
end
