-- Photo Gallery App Logic
-- Simple photo upload and display using secure file picker
-- Assets are encrypted and stored via Butler's AssetStore

-- Local state
local page_id = nil
local photos_layer = nil  -- Loro list layer storing photo metadata

-- ============================================================================
-- INITIALIZATION
-- ============================================================================

function on_init()
    page_id = permit:page_id()

    -- Get or create the photos layer (list of photo metadata)
    photos_layer = loro:get_or_create_layer(page_id .. "/photos", "list")

    -- Initial UI refresh
    refresh_photos_ui()

    ui:set("status", "Gallery loaded - " .. get_photo_count() .. " photos")
end

-- ============================================================================
-- ASSET UPLOAD HANDLER
-- Called when user uploads a photo via file picker
-- ============================================================================

function on_asset_uploaded(asset)
    -- asset = { hash, filename, mime_type, size }
    print("Photo uploaded: " .. asset.filename .. " (" .. asset.hash .. ")")

    -- Add photo metadata to Loro layer
    if photos_layer then
        local photo_entry = {
            hash = asset.hash,
            filename = asset.filename,
            mime_type = asset.mime_type,
            size = asset.size,
            uploaded_at = os.time(),
            uploaded_by = permit:my_did()
        }

        photos_layer:push(photo_entry)
        -- Note: commit is automatic via scribe

        ui:set("status", "Uploaded: " .. asset.filename)
    else
        ui:set("status", "Error: Photos layer not initialized")
    end

    -- Refresh UI
    refresh_photos_ui()
end

-- ============================================================================
-- LORO CHANGE HANDLERS
-- ============================================================================

function on_loro_change(layer_name, change_type)
    local photos_layer_name = page_id .. "/photos"
    if layer_name == photos_layer_name then
        refresh_photos_ui()
        ui:set("status", "Photos synced - " .. get_photo_count() .. " total")
    end
end

function on_layer_discovered(layer_name)
    local photos_layer_name = page_id .. "/photos"
    if layer_name == photos_layer_name and not photos_layer then
        photos_layer = loro:get_layer(layer_name, "list")
        refresh_photos_ui()
    end
end

-- ============================================================================
-- UI HELPERS
-- ============================================================================

function refresh_photos_ui()
    if not photos_layer then
        ui:set("photos", {})
        return
    end

    -- Convert to UI format using length() and get()
    local ui_photos = {}
    local len = photos_layer:length()
    for i = 0, len - 1 do
        local photo = photos_layer:get(i)
        if photo then
            table.insert(ui_photos, {
                hash = photo.hash or "",
                filename = photo.filename or "Unknown",
                mime_type = photo.mime_type or "image/unknown",
                size = photo.size or 0
            })
        end
    end

    ui:set("photos", ui_photos)
end

function get_photo_count()
    if not photos_layer then
        return 0
    end
    return photos_layer:length()
end

-- ============================================================================
-- CLICK HANDLERS
-- ============================================================================

function on_click(action)
    if action == "refresh" then
        refresh_photos_ui()
        ui:set("status", "Refreshed - " .. get_photo_count() .. " photos")
    end
end

-- ============================================================================
-- TEST HELPERS
-- ============================================================================

-- Add a photo for testing (simulates upload without actual file)
function add_photo_for_test(hash, filename, mime_type, size)
    if not photos_layer then
        return false
    end

    local photo_entry = {
        hash = hash or ("test-hash-" .. os.time()),
        filename = filename or "test-photo.jpg",
        mime_type = mime_type or "image/jpeg",
        size = size or 1024,
        uploaded_at = os.time(),
        uploaded_by = permit:my_did()
    }

    photos_layer:push(photo_entry)
    refresh_photos_ui()
    return true
end

-- Get all photos for verification
function get_photos()
    if not photos_layer then
        return {}
    end

    local photos = {}
    local len = photos_layer:length()
    for i = 0, len - 1 do
        local photo = photos_layer:get(i)
        if photo then
            table.insert(photos, photo)
        end
    end
    return photos
end
