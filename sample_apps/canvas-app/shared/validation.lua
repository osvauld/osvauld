-- Canvas App Validation
-- Validates shape and connector data

function validate_shape(shape)
    if type(shape) ~= "table" then
        return false, "shape must be a table"
    end
    if type(shape.id) ~= "string" or shape.id == "" then
        return false, "shape.id must be non-empty string"
    end
    if type(shape.x) ~= "number" or type(shape.y) ~= "number" then
        return false, "shape.x and shape.y must be numbers"
    end
    if type(shape.width) ~= "number" or shape.width <= 0 then
        return false, "shape.width must be positive number"
    end
    if type(shape.height) ~= "number" or shape.height <= 0 then
        return false, "shape.height must be positive number"
    end
    return true
end

function validate_connector(connector)
    if type(connector) ~= "table" then
        return false, "connector must be a table"
    end
    if type(connector.id) ~= "string" or connector.id == "" then
        return false, "connector.id must be non-empty string"
    end
    if type(connector.from_shape) ~= "string" then
        return false, "connector.from_shape must be string"
    end
    if type(connector.to_shape) ~= "string" then
        return false, "connector.to_shape must be string"
    end
    return true
end
