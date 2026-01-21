-- Photo Gallery Validation
-- Validates photo metadata

function validate_photo(photo)
    if type(photo) ~= "table" then
        return false, "photo must be a table"
    end
    if type(photo.hash) ~= "string" or photo.hash == "" then
        return false, "photo.hash must be non-empty string"
    end
    if type(photo.filename) ~= "string" or photo.filename == "" then
        return false, "photo.filename must be non-empty string"
    end
    if type(photo.mime_type) ~= "string" then
        return false, "photo.mime_type must be string"
    end
    -- Validate mime_type is an image
    if not photo.mime_type:match("^image/") then
        return false, "photo.mime_type must start with 'image/'"
    end
    if type(photo.size) ~= "number" or photo.size <= 0 then
        return false, "photo.size must be positive number"
    end
    return true
end
