---@diagnostic disable: undefined-global, lowercase-global
-- Drag-reorderable list, phase 3 demo for renderer_egui.
--
-- Each row is BOTH a drag source (so you can pick it up) and a drop zone
-- (so you can drop other rows onto it). Dropping row A onto row B moves A
-- to B's slot — the simplest reorder semantics. egui paints the dragged
-- row as a ghost at the cursor for free.
--
-- No scribe: state is a plain Lua list, lost on relaunch. Phase 5 will
-- port this against a Loro tree.

local items = {
    "Write the plan",
    "Bench the bridge",
    "Build the skeleton",
    "Wire sthalam dispatch",
    "Hello-egui smoke test",
    "Layout primitives",
    "Drag & drop",
    "Port text editor",
}

-- Move item at `from` to position `to` in-place. Both 1-indexed.
local function move_item(from, to)
    if from == to or from < 1 or to < 1 or from > #items or to > #items then
        return
    end
    local v = table.remove(items, from)
    table.insert(items, to, v)
end

-- Find the index of an item by its stable id ("item-N" where N is its
-- *original* slot — we use these as payloads so they survive reordering
-- across frames). For this demo, we use the current index as the id;
-- since we read it fresh each frame, "item-3" always means "row 3 right
-- now". Good enough for v1.
local function index_of(id_str)
    local n = tonumber(id_str:match("^item%-(%d+)$"))
    return n
end

function on_frame(ui)
    ui:label("Drag any row onto another to reorder.")
    ui:separator()

    ui:scroll_area(function()
        for i, text in ipairs(items) do
            local id = "item-" .. i

            -- The drop zone is the outer wrapper: dropping on this row
            -- moves the source into this row's slot.
            local dropped = ui:drop_zone(function()
                -- The drag source is the row body. We frame it for
                -- visual presence and to give the ghost something to
                -- carry while dragging.
                ui:drag_source(id, id, function()
                    ui:frame({ bg = "#2a2a3a", rounding = 6, padding = 10 }, function()
                        ui:horizontal(function()
                            ui:label("⋮⋮ ")
                            ui:label(i .. ".  " .. text)
                        end)
                    end)
                end)
            end)

            if dropped then
                local from = index_of(dropped)
                if from then
                    move_item(from, i)
                end
            end
        end
    end)
end
