-- UI Helpers — shared utility functions

local M = {}

function M.format_time(ts)
    if not ts then return "" end
    return os.date("%H:%M", ts)
end

function M.generate_id()
    return string.format("%x-%04x", os.time(), math.random(0, 65535))
end

function M.truncate_text(text, max_len)
    if not text then return "" end
    if #text > max_len then return text:sub(1, max_len) .. "..." end
    return text
end

function M.contains(tbl, value)
    if not tbl then return false end
    for _, v in ipairs(tbl) do
        if v == value then return true end
    end
    return false
end

function M.find_index(tbl, value)
    if not tbl then return nil end
    for i, v in ipairs(tbl) do
        if v == value then return i end
    end
    return nil
end

-- Get sender initials for avatar (first letter of each word, max 2)
function M.sender_initials(name)
    if not name or name == "" then return "?" end
    local initials = ""
    for word in name:gmatch("%S+") do
        initials = initials .. word:sub(1, 1):upper()
        if #initials >= 2 then break end
    end
    return initials
end

return M
