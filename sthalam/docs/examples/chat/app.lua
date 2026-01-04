-- Chat App Logic
-- This Lua script runs inside the sandboxed app runtime

-- Local state (mutable)
local state = {
    messages = {},
    draft = "",
    username = "You",
    message_counter = 0
}

-- Called on app init, returns initial state as JSON
function init()
    return '{"messages": [], "draft": "", "username": "You", "message_counter": 0}'
end

-- Called when send button is clicked or Enter is pressed
function send()
    if state.draft == "" then
        return state_to_json()
    end

    -- Increment counter for unique ID
    state.message_counter = state.message_counter + 1

    -- Send to Scribe (this will sync to other peers)
    scribe.list_push("messages", "items", {
        id = "msg_" .. state.message_counter,
        author = state.username,
        content = state.draft
    })

    -- Clear draft
    state.draft = ""

    return state_to_json()
end

-- Called when Scribe layer is updated
function on_data(state_json, layer, data_json)
    -- Parse incoming state
    local s = json_decode(state_json)

    -- Update messages from the layer data
    if layer == "messages" then
        s.messages = json_decode(data_json)
    end

    -- Encode and return updated state
    return json_encode(s)
end

-- Helper: Convert state to JSON
function state_to_json()
    return json_encode(state)
end

-- Simple JSON encoder (for basic types)
function json_encode(obj)
    if type(obj) == "nil" then
        return "null"
    elseif type(obj) == "boolean" then
        return tostring(obj)
    elseif type(obj) == "number" then
        return tostring(obj)
    elseif type(obj) == "string" then
        return '"' .. obj:gsub('\\', '\\\\'):gsub('"', '\\"'):gsub('\n', '\\n') .. '"'
    elseif type(obj) == "table" then
        -- Check if array or object
        local is_array = true
        local max_idx = 0
        for k, _ in pairs(obj) do
            if type(k) == "number" and k > 0 and math.floor(k) == k then
                max_idx = math.max(max_idx, k)
            else
                is_array = false
                break
            end
        end

        if is_array then
            local parts = {}
            for i = 1, max_idx do
                parts[i] = json_encode(obj[i])
            end
            return "[" .. table.concat(parts, ",") .. "]"
        else
            local parts = {}
            for k, v in pairs(obj) do
                table.insert(parts, json_encode(tostring(k)) .. ":" .. json_encode(v))
            end
            return "{" .. table.concat(parts, ",") .. "}"
        end
    else
        return "null"
    end
end

-- Simple JSON decoder (for basic types)
function json_decode(str)
    -- Use Lua's load to parse JSON-like syntax
    -- This is a simplified decoder for demonstration
    local func = load("return " .. str:gsub('%[', '{'):gsub('%]', '}'):gsub('null', 'nil'):gsub('true', 'true'):gsub('false', 'false'))
    if func then
        return func()
    end
    return nil
end
