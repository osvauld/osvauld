-- Threads — open/close thread, send reply

local helpers = require("ui_helpers")
local messages = require("messages")

local M = {}

-- Module state
local my_did = nil
local my_name = nil
local thread_parent_id = nil

function M.init(did, name)
    my_did = did
    my_name = name
end

-- Open thread panel for a given message
function M.open(messages_layer, msg_id)
    if not messages_layer or not msg_id then return end

    local parent_msg = messages_layer:get(msg_id)
    if not parent_msg then return end

    thread_parent_id = msg_id

    -- Set parent message in UI
    ui:set("thread_parent", messages.to_ui_message(parent_msg))
    ui:set("thread_open", true)
    ui:set("thread_draft", "")

    -- Collect thread replies
    M.refresh_replies(messages_layer)
end

-- Refresh thread replies from layer
function M.refresh_replies(messages_layer)
    if not messages_layer or not thread_parent_id then return end

    local replies = {}
    local keys = messages_layer:keys()
    if keys then
        for _, key in ipairs(keys) do
            local msg = messages_layer:get(key)
            if msg and msg.thread_parent_id == thread_parent_id then
                table.insert(replies, msg)
            end
        end
    end

    -- Sort by timestamp
    table.sort(replies, function(a, b)
        return (a.timestamp or 0) < (b.timestamp or 0)
    end)

    -- Transform to UI format
    local ui_replies = {}
    for _, msg in ipairs(replies) do
        table.insert(ui_replies, messages.to_ui_message(msg))
    end

    ui:set("thread_messages", ui_replies)

    -- Also refresh parent (reply count may have changed)
    local parent = messages_layer:get(thread_parent_id)
    if parent then
        ui:set("thread_parent", messages.to_ui_message(parent))
    end
end

-- Send a reply in the current thread
function M.send_reply(messages_layer)
    if not messages_layer or not thread_parent_id then return end

    local text = ui:get("thread_draft")
    if not text or text == "" then return end

    local msg_id = helpers.generate_id()
    local msg = {
        id = msg_id,
        sender_did = my_did,
        sender_name = my_name,
        text = text,
        timestamp = os.time(),
        deleted = false,
        edited = false,
        thread_parent_id = thread_parent_id,
        thread_reply_count = 0,
        reply_to = "",
        reply_preview = "",
        reactions = {},
        attachment_hash = "",
        attachment_name = "",
    }

    messages_layer:set(msg_id, msg)

    -- Increment parent's reply count
    local parent = messages_layer:get(thread_parent_id)
    if parent then
        parent.thread_reply_count = (parent.thread_reply_count or 0) + 1
        messages_layer:set(thread_parent_id, parent)
    end

    ui:set("thread_draft", "")

    -- Refresh thread view
    M.refresh_replies(messages_layer)
end

-- Close thread panel
function M.close()
    thread_parent_id = nil
    ui:set("thread_open", false)
    ui:set("thread_messages", {})
    ui:set("thread_draft", "")
end

-- Check if thread is open
function M.is_open()
    return thread_parent_id ~= nil
end

-- Get current thread parent ID
function M.get_parent_id()
    return thread_parent_id
end

return M
