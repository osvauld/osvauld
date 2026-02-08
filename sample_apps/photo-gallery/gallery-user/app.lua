-- Photo Gallery App Logic
-- Simple photo upload and display using secure file picker
-- Uses scribe:bind() for declarative layer-UI sync

local page_id = nil
local photos_layer = nil

function on_init()
    page_id = scribe:page_id()
    photos_layer = scribe:list(page_id .. "/photos")

    -- Declarative binding: photos auto-sync to UI
    scribe:bind("photos", "photos", {
        transform = function(photo)
            return {
                hash = photo.hash or "",
                filename = photo.filename or "Unknown",
                mime_type = photo.mime_type or "image/unknown",
                size = photo.size or 0
            }
        end
    })

    ui:set("status", "Gallery loaded - " .. get_photo_count() .. " photos")
end

-- Asset upload handler
function on_asset_uploaded(asset)
    print("Photo uploaded: " .. asset.filename .. " (" .. asset.hash .. ")")

    if photos_layer then
        local photo_entry = {
            hash = asset.hash,
            filename = asset.filename,
            mime_type = asset.mime_type,
            size = asset.size,
            uploaded_at = os.time(),
            uploaded_by = scribe:my_did()
        }
        photos_layer:push(photo_entry)
        ui:set("status", "Uploaded: " .. asset.filename)
    else
        ui:set("status", "Error: Photos layer not initialized")
    end
end

function on_click(action)
    if action == "refresh" then
        ui:set("status", "Refreshed - " .. get_photo_count() .. " photos")
    end
end

function get_photo_count()
    return photos_layer and photos_layer:length() or 0
end

-- Test helpers
function add_photo_for_test(hash, filename, mime_type, size)
    if not photos_layer then return false end
    local photo_entry = {
        hash = hash or ("test-hash-" .. os.time()),
        filename = filename or "test-photo.jpg",
        mime_type = mime_type or "image/jpeg",
        size = size or 1024,
        uploaded_at = os.time(),
        uploaded_by = scribe:my_did()
    }
    photos_layer:push(photo_entry)
    return true
end

function get_photos()
    if not photos_layer then return {} end
    local photos = {}
    local len = photos_layer:length()
    for i = 0, len - 1 do
        local photo = photos_layer:get(i)
        if photo then table.insert(photos, photo) end
    end
    return photos
end
