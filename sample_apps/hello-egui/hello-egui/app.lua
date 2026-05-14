---@diagnostic disable: undefined-global, lowercase-global
-- Phase 1 smoke test for renderer_egui.
--
-- Exercises every binding currently exposed: label, button, separator,
-- horizontal, vertical, text_edit. No scribe state — just local Lua state
-- that proves on_frame is being called and re-renders pick up changes.

local counter = 0
local name = "world"

function on_init()
    print("hello-egui: on_init called")
end

function on_frame(ui)
    ui:label("hello-egui — renderer_egui phase 1 smoke test")
    ui:separator()

    ui:horizontal(function()
        ui:label("Counter:")
        ui:label(tostring(counter))
        if ui:button("+") then
            counter = counter + 1
        end
        if ui:button("-") then
            counter = counter - 1
        end
        if ui:button("reset") then
            counter = 0
        end
    end)

    ui:separator()

    ui:vertical(function()
        ui:label("Name:")
        local new_name, changed = ui:text_edit(name)
        if changed then
            name = new_name
        end
        ui:label("Hello, " .. name .. "!")
    end)
end
