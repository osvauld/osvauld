-- Osvauld Demos Validation
-- Shared validation logic for demo apps
--
-- This module provides common validation functions used across demo apps.
--
-- **Presence Validation**: Uses presence_lib (provided by LuaRuntime)
-- to enforce that users can only update their own presence entry.

local validation = {}

-- =============================================================================
-- Helper Functions
-- =============================================================================

-- Validate that a value is a non-empty string
function validation.is_non_empty_string(value)
    return type(value) == "string" and value ~= ""
end

-- Validate that a value is a positive number
function validation.is_positive_number(value)
    return type(value) == "number" and value > 0
end

-- Validate that a value is a non-negative number
function validation.is_non_negative_number(value)
    return type(value) == "number" and value >= 0
end

-- Validate a message structure for group-chat
function validation.is_valid_message(msg)
    if type(msg) ~= "table" then return false end
    if not validation.is_non_empty_string(msg.id) then return false end
    if not validation.is_non_empty_string(msg.sender_did) then return false end
    if not validation.is_non_empty_string(msg.text) then return false end
    if not validation.is_positive_number(msg.timestamp) then return false end
    return true
end

-- Validate a score entry for snake-game
function validation.is_valid_score(score)
    if type(score) ~= "table" then return false end
    if not validation.is_non_empty_string(score.player) then return false end
    if not validation.is_non_negative_number(score.score) then return false end
    if not validation.is_positive_number(score.timestamp) then return false end
    return true
end

-- =============================================================================
-- Layer-Specific Validation
-- =============================================================================

--- Check if a layer is a presence layer
function validation.is_presence_layer(layer_name)
    return layer_name and layer_name:match("/presence$") ~= nil
end

--- Validate presence layer operations
---
--- **Context**: Uses presence_lib.validate_write to enforce presence rules
--- **Rule**: Users can only write to their own DID key in the presence map
---
--- @param ops table List of operations
--- @param from_did string DID of the writer
--- @return boolean, string? valid, error_message
function validation.validate_presence_ops(ops, from_did)
    -- presence_lib is a global provided by LuaRuntime
    if not presence_lib or not presence_lib.validate_write then
        -- No presence_lib available (shouldn't happen in kunki mode)
        return true, nil
    end

    for _, op in ipairs(ops) do
        local valid, err = presence_lib.validate_write(op, from_did)
        if not valid then
            return false, err
        end
    end

    return true, nil
end

-- =============================================================================
-- Global validate_ops Function (called by LuaRuntime)
-- =============================================================================

--- Main validation entry point
---
--- **Context**: Called by LuaRuntime.validate_ops() for each remote update
--- **Signature**: validate_ops(layer_name, ops, from_did, role, page_id) -> boolean
---
--- @param layer_name string Layer being updated
--- @param ops table List of operations (JsonOp format)
--- @param from_did string DID of the remote peer
--- @param role string Role of the peer ("viewer", "owner", "peer", etc.)
--- @param page_id string Page ID for context
--- @return boolean valid
function validate_ops(layer_name, ops, from_did, role, page_id)
    -- 1. Presence layer validation - enforce "own DID only" rule
    if validation.is_presence_layer(layer_name) then
        local valid, err = validation.validate_presence_ops(ops, from_did)
        if not valid then
            print(string.format("[validation] Rejected presence op from %s: %s",
                from_did:sub(-8), err or "unknown"))
            return false
        end
        return true
    end

    -- 2. App-specific layer validation can be added here
    -- Example: Validate chat messages have correct sender_did
    -- if layer_name:match("/messages$") then
    --     return validation.validate_chat_ops(ops, from_did)
    -- end

    -- 3. Default: allow all other operations
    return true
end

return validation
