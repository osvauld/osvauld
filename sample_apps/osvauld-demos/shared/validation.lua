-- Osvauld Demos Validation
-- Shared validation logic for demo apps
--
-- This module provides common validation functions used across demo apps.

local validation = {}

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

return validation
