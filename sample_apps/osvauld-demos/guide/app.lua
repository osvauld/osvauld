-- Sthalam Guide App Logic
-- Welcome to the Extended Web - documentation and introduction

-- Initialize the app
function on_init()
    -- Start on Welcome tab
    ui:set("active_tab", 0)
end

-- Handle click events (tab navigation)
function on_click(target)
    if target:match("^tab:") then
        local tab_id = tonumber(target:sub(5))
        if tab_id then
            ui:set("active_tab", tab_id)
        end
    end
end
