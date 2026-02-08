-- Canvas App - Graph-Based Data Model
-- ALL app logic lives here - Slint is just a dumb renderer
--
-- ARCHITECTURE:
-- 1. GRAPH (source of truth) - rich relationships, fast queries
-- 2. UI PROJECTION (cached) - flattened arrays with stable indices
-- 3. SURGICAL UPDATES - ui:update(index) for single items

-- =============================================================================
-- GRAPH DATA STRUCTURE
-- =============================================================================

local graph = {
    -- All shapes by ID (source of truth)
    shapes = {},      -- id -> shape object

    -- All connectors by ID
    connectors = {},  -- id -> connector object

    -- Layers (render order)
    layers = {
        [1] = { id = 1, name = "Background", visible = true, locked = false },
        [2] = { id = 2, name = "Default", visible = true, locked = false },
        [3] = { id = 3, name = "Foreground", visible = true, locked = false },
    },

    -- Root shapes (no parent) - entry points for traversal
    roots = {},       -- [shape, shape, ...]
}

-- =============================================================================
-- UI PROJECTION (flattened for Slint, cached for surgical updates)
-- =============================================================================

local ui_projection = {
    -- Shapes array (sorted by layer + z_index)
    shapes_array = {},        -- [shape_for_ui, ...]
    shape_id_to_index = {},   -- id -> 0-based index

    -- Connectors array
    connectors_array = {},
    connector_id_to_index = {},

    -- Dirty flag (true = need full rebuild)
    dirty = true,
}

-- =============================================================================
-- LORO SYNC
-- =============================================================================

local page_id = nil
local shapes_layer = nil       -- Loro map for shapes
local connectors_layer = nil   -- Loro map for connectors

-- =============================================================================
-- SELECTION STATE
-- =============================================================================

local selection = {
    shape_ids = {},           -- Set: { [id] = true }
    bounds = nil,             -- { x, y, width, height } or nil
    active_handle = nil,      -- "nw", "n", "ne", "e", "se", "s", "sw", "w", or nil
}

-- =============================================================================
-- INTERACTION STATE
-- =============================================================================

local viewport = { pan_x = 0, pan_y = 0, zoom = 1.0 }
local current_tool = "select"
local current_layer = 2  -- Default layer
local pointer = { down = false, x = 0, y = 0, start_x = 0, start_y = 0 }
local drag_state = {
    active = false,
    mode = nil,           -- "pan", "move", "resize", "create"
    start_positions = {}, -- { [id] = {x, y} } for undo
    resize_handle = nil,
}
local connector_start = nil  -- Shape when creating connector
local dirty_shapes = {}      -- Shapes modified during drag (commit on up)

-- =============================================================================
-- COLORS
-- =============================================================================

local colors = {
    shape_fill = "#0f3460",
    shape_stroke = "#e94560",
    shape_text = "#eaeaea",
    sticky_yellow = "#ffeb3b",
    sticky_pink = "#f48fb1",
    sticky_blue = "#81d4fa",
    sticky_green = "#a5d6a7",
    sticky_text = "#1a1a2e",
    connector = "#e94560",
}

-- =============================================================================
-- INITIALIZATION
-- =============================================================================

function on_init()
    page_id = scribe:page_id()

    -- Get or create layers
    shapes_layer = scribe:map(page_id .. "/shapes")
    connectors_layer = scribe:map(page_id .. "/connectors")

    -- Load from Loro into graph
    load_from_loro()

    -- Initial UI sync
    rebuild_ui_projection()
    sync_full_ui()
    update_toolbar()
end

function load_from_loro()
    -- Load shapes
    local shape_keys = shapes_layer:keys()
    for _, id in ipairs(shape_keys) do
        local data = shapes_layer:get(id)
        if data then
            create_shape_in_graph(data)
        end
    end

    -- Load connectors
    local conn_keys = connectors_layer:keys()
    for _, id in ipairs(conn_keys) do
        local data = connectors_layer:get(id)
        if data then
            create_connector_in_graph(data)
        end
    end
end

-- =============================================================================
-- LORO CHANGE HANDLER
-- =============================================================================

function on_loro_change(layer_name, ops)
    if layer_name:match("/shapes$") or layer_name:match("/connectors$") then
        graph.shapes = {}
        graph.connectors = {}
        graph.roots = {}
        load_from_loro()
        rebuild_ui_projection()
        sync_full_ui()
    end
end

-- =============================================================================
-- GRAPH OPERATIONS
-- =============================================================================

-- Create shape in graph (from data, doesn't write to Loro)
function create_shape_in_graph(data)
    local shape = {
        -- Identity
        id = data.id,
        type = data.type or "rectangle",

        -- Geometry
        x = data.x or 0,
        y = data.y or 0,
        width = data.width or 100,
        height = data.height or 60,

        -- Hierarchy
        parent = nil,           -- Will be resolved after all shapes loaded
        parent_id = data.parent_id,
        children = {},          -- Direct references

        -- Graph edges
        connectors = {},        -- Connectors attached to this shape

        -- Layer (nil = inherit from parent)
        layer = data.layer or 2,
        z_index = data.z_index or 0,

        -- Styling
        fill = data.fill or get_default_fill(data.type),
        stroke = data.stroke or colors.shape_stroke,
        stroke_width = data.stroke_width or 2,
        corner_radius = data.corner_radius or 4,

        -- Text
        text = data.text or "",
        text_size = data.text_size or 14,
        text_color = data.text_color or get_default_text_color(data.type),
    }

    graph.shapes[shape.id] = shape

    -- Track as root if no parent
    if not shape.parent_id then
        table.insert(graph.roots, shape)
    end

    return shape
end

-- Create connector in graph (from data, doesn't write to Loro)
function create_connector_in_graph(data)
    local connector = {
        id = data.id,
        from_id = data.from_id,
        to_id = data.to_id,
        from = nil,  -- Resolved to shape reference
        to = nil,
        from_anchor = data.from_anchor or "auto",
        to_anchor = data.to_anchor or "auto",
        stroke = data.stroke or colors.connector,
        stroke_width = data.stroke_width or 2,

        -- Cached endpoints (recalculated when shapes move)
        from_x = 0, from_y = 0,
        to_x = 0, to_y = 0,
        -- Bezier control points
        ctrl1_x = 0, ctrl1_y = 0,
        ctrl2_x = 0, ctrl2_y = 0,
    }

    -- Resolve references
    connector.from = graph.shapes[connector.from_id]
    connector.to = graph.shapes[connector.to_id]

    if connector.from and connector.to then
        graph.connectors[connector.id] = connector

        -- Add to shapes' connector lists
        table.insert(connector.from.connectors, connector)
        table.insert(connector.to.connectors, connector)

        -- Calculate endpoints
        recalc_connector_endpoints(connector)
    end

    return connector
end

-- Get default fill color based on type
function get_default_fill(shape_type)
    if shape_type == "sticky" then
        return colors.sticky_yellow
    elseif shape_type == "text" then
        return "transparent"
    else
        return colors.shape_fill
    end
end

-- Get default text color based on type
function get_default_text_color(shape_type)
    if shape_type == "sticky" then
        return colors.sticky_text
    else
        return colors.shape_text
    end
end

-- =============================================================================
-- UI PROJECTION - Flatten Graph for Slint
-- =============================================================================

function rebuild_ui_projection()
    ui_projection.shapes_array = {}
    ui_projection.shape_id_to_index = {}
    ui_projection.connectors_array = {}
    ui_projection.connector_id_to_index = {}

    -- Collect all shapes, sorted by layer + z_index
    local sorted_shapes = {}
    for _, shape in pairs(graph.shapes) do
        table.insert(sorted_shapes, shape)
    end

    table.sort(sorted_shapes, function(a, b)
        local layer_a = get_effective_layer(a)
        local layer_b = get_effective_layer(b)
        if layer_a ~= layer_b then
            return layer_a < layer_b
        end
        return (a.z_index or 0) < (b.z_index or 0)
    end)

    -- Build shapes array with index mapping
    for i, shape in ipairs(sorted_shapes) do
        local layer = get_effective_layer(shape)
        local layer_info = graph.layers[layer]
        local visible = layer_info and layer_info.visible or true

        table.insert(ui_projection.shapes_array, shape_to_ui(shape, visible))
        ui_projection.shape_id_to_index[shape.id] = i - 1  -- 0-based for Slint
    end

    -- Build connectors array
    local sorted_connectors = {}
    for _, conn in pairs(graph.connectors) do
        table.insert(sorted_connectors, conn)
    end

    for i, conn in ipairs(sorted_connectors) do
        table.insert(ui_projection.connectors_array, connector_to_ui(conn))
        ui_projection.connector_id_to_index[conn.id] = i - 1
    end

    ui_projection.dirty = false
end

-- Convert shape to UI format
function shape_to_ui(shape, visible)
    return {
        id = shape.id,
        shape_type = shape.type,
        x = shape.x,
        y = shape.y,
        width = shape.width,
        height = shape.height,
        stroke_width = shape.stroke_width or 2,
        text = shape.text or "",
        text_size = shape.text_size or 14,
        z_index = shape.z_index or 0,
        selected = selection.shape_ids[shape.id] == true,
        visible = visible ~= false,
    }
end

-- Convert connector to UI format (with bezier control points)
function connector_to_ui(conn)
    return {
        id = conn.id,
        from_x = conn.from_x or 0,
        from_y = conn.from_y or 0,
        ctrl1_x = conn.ctrl1_x or conn.from_x or 0,
        ctrl1_y = conn.ctrl1_y or conn.from_y or 0,
        ctrl2_x = conn.ctrl2_x or conn.to_x or 0,
        ctrl2_y = conn.ctrl2_y or conn.to_y or 0,
        to_x = conn.to_x or 0,
        to_y = conn.to_y or 0,
        stroke_width = conn.stroke_width or 2,
        selected = false,  -- TODO: connector selection
    }
end

-- Get effective layer (walk up parent chain)
function get_effective_layer(shape)
    if shape.layer then return shape.layer end
    if shape.parent then return get_effective_layer(shape.parent) end
    return 2  -- Default layer
end

-- =============================================================================
-- UI SYNC - Push to Slint
-- =============================================================================

-- Full UI sync (after structural changes)
function sync_full_ui()
    ui:set("shapes", ui_projection.shapes_array)
    ui:set("connectors", ui_projection.connectors_array)
    ui:set("line_points", generate_all_line_points())
    sync_viewport()
    sync_selection()
end

-- Sync just viewport (during pan/zoom)
function sync_viewport()
    ui:set("viewport_pan_x", viewport.pan_x)
    ui:set("viewport_pan_y", viewport.pan_y)
    ui:set("viewport_zoom", viewport.zoom)
end

-- Sync selection box and handles
function sync_selection()
    if next(selection.shape_ids) == nil then
        ui:set("selection_visible", false)
        return
    end

    -- Calculate bounding box of selected shapes
    local min_x, min_y = math.huge, math.huge
    local max_x, max_y = -math.huge, -math.huge

    for id, _ in pairs(selection.shape_ids) do
        local shape = graph.shapes[id]
        if shape then
            min_x = math.min(min_x, shape.x)
            min_y = math.min(min_y, shape.y)
            max_x = math.max(max_x, shape.x + shape.width)
            max_y = math.max(max_y, shape.y + shape.height)
        end
    end

    selection.bounds = {
        x = min_x,
        y = min_y,
        width = max_x - min_x,
        height = max_y - min_y,
    }

    ui:set("selection_visible", true)
    ui:set("sel_x", selection.bounds.x)
    ui:set("sel_y", selection.bounds.y)
    ui:set("sel_width", selection.bounds.width)
    ui:set("sel_height", selection.bounds.height)
    -- Resize handles are rendered inline in Slint based on selection bounds
end

-- =============================================================================
-- SURGICAL UPDATES - Single item updates during drag
-- =============================================================================

-- Update single shape in UI (no full rebuild)
function update_shape_ui(shape)
    local index = ui_projection.shape_id_to_index[shape.id]
    if index == nil then return end

    local layer = get_effective_layer(shape)
    local layer_info = graph.layers[layer]
    local visible = layer_info and layer_info.visible or true

    local ui_shape = shape_to_ui(shape, visible)
    ui_projection.shapes_array[index + 1] = ui_shape  -- Lua is 1-based
    ui:update("shapes", index, ui_shape)
end

-- Update single connector in UI
function update_connector_ui(conn)
    local index = ui_projection.connector_id_to_index[conn.id]
    if index == nil then return end

    local ui_conn = connector_to_ui(conn)
    ui_projection.connectors_array[index + 1] = ui_conn
    ui:update("connectors", index, ui_conn)
end

-- Update all connectors attached to a shape (after shape moves)
function update_shape_connectors(shape)
    for _, conn in ipairs(shape.connectors) do
        recalc_connector_endpoints(conn)
        update_connector_ui(conn)
    end
    -- Regenerate bezier line points
    ui:set("line_points", generate_all_line_points())
end

-- =============================================================================
-- CONNECTOR ENDPOINT CALCULATION (Bezier curves)
-- =============================================================================

function recalc_connector_endpoints(conn)
    if not conn.from or not conn.to then return end

    local from_shape = conn.from
    local to_shape = conn.to

    -- Get shape centers
    local from_cx = from_shape.x + from_shape.width / 2
    local from_cy = from_shape.y + from_shape.height / 2
    local to_cx = to_shape.x + to_shape.width / 2
    local to_cy = to_shape.y + to_shape.height / 2

    -- Calculate edge intersection points (P0 and P3)
    local from_x, from_y, from_edge = get_edge_point(from_shape, to_cx, to_cy)
    local to_x, to_y, to_edge = get_edge_point(to_shape, from_cx, from_cy)

    conn.from_x = from_x
    conn.from_y = from_y
    conn.to_x = to_x
    conn.to_y = to_y

    -- Calculate control points for smooth bezier curve
    -- Control points extend outward from the edge, perpendicular to shape
    local dist = math.sqrt((to_x - from_x)^2 + (to_y - from_y)^2)
    local curve_offset = math.max(30, dist * 0.3)  -- 30% of distance, minimum 30px

    -- Control point 1: extends from source edge
    conn.ctrl1_x, conn.ctrl1_y = get_control_point(from_x, from_y, from_edge, curve_offset)

    -- Control point 2: extends from target edge
    conn.ctrl2_x, conn.ctrl2_y = get_control_point(to_x, to_y, to_edge, curve_offset)
end

-- Returns edge point and which edge it's on ("top", "bottom", "left", "right")
function get_edge_point(shape, target_x, target_y)
    local cx = shape.x + shape.width / 2
    local cy = shape.y + shape.height / 2

    local dx = target_x - cx
    local dy = target_y - cy

    if dx == 0 and dy == 0 then
        return cx, shape.y, "top"
    end

    local abs_dx = math.abs(dx)
    local abs_dy = math.abs(dy)
    local hw = shape.width / 2
    local hh = shape.height / 2

    local scale
    local edge

    -- Determine which edge the line exits from
    if abs_dx / hw > abs_dy / hh then
        -- Exits from left or right edge
        scale = hw / abs_dx
        edge = dx > 0 and "right" or "left"
    else
        -- Exits from top or bottom edge
        scale = hh / abs_dy
        edge = dy > 0 and "bottom" or "top"
    end

    return cx + dx * scale, cy + dy * scale, edge
end

-- Calculate bezier control point extending outward from edge
function get_control_point(x, y, edge, offset)
    if edge == "top" then
        return x, y - offset
    elseif edge == "bottom" then
        return x, y + offset
    elseif edge == "left" then
        return x - offset, y
    elseif edge == "right" then
        return x + offset, y
    end
    return x, y
end

-- Generate points along a cubic bezier curve
-- P0 = start, P1 = ctrl1, P2 = ctrl2, P3 = end
function bezier_points(p0x, p0y, p1x, p1y, p2x, p2y, p3x, p3y, num_points)
    local points = {}
    num_points = num_points or 20

    for i = 0, num_points do
        local t = i / num_points
        local t2 = t * t
        local t3 = t2 * t
        local mt = 1 - t
        local mt2 = mt * mt
        local mt3 = mt2 * mt

        -- Cubic bezier formula: B(t) = (1-t)³P0 + 3(1-t)²tP1 + 3(1-t)t²P2 + t³P3
        local x = mt3 * p0x + 3 * mt2 * t * p1x + 3 * mt * t2 * p2x + t3 * p3x
        local y = mt3 * p0y + 3 * mt2 * t * p1y + 3 * mt * t2 * p2y + t3 * p3y

        table.insert(points, { x = x, y = y })
    end

    return points
end

-- Generate all line points for all connectors
function generate_all_line_points()
    local all_points = {}

    for _, conn in pairs(graph.connectors) do
        local points = bezier_points(
            conn.from_x, conn.from_y,
            conn.ctrl1_x, conn.ctrl1_y,
            conn.ctrl2_x, conn.ctrl2_y,
            conn.to_x, conn.to_y,
            30  -- number of points per connector
        )
        for _, pt in ipairs(points) do
            table.insert(all_points, pt)
        end
    end

    return all_points
end

-- =============================================================================
-- POINTER EVENT HANDLING
-- =============================================================================

function on_pointer_event(event_type, screen_x, screen_y)
    pointer.x = screen_x
    pointer.y = screen_y

    -- Send cursor position to peers (throttled) for collaboration
    local canvas_x, canvas_y = screen_to_canvas(screen_x, screen_y)
    send_cursor_update(canvas_x, canvas_y)

    if event_type == "down" then
        pointer.down = true
        pointer.start_x = screen_x
        pointer.start_y = screen_y
        handle_pointer_down(screen_x, screen_y)

    elseif event_type == "move" and pointer.down then
        handle_pointer_drag(screen_x, screen_y)

    elseif event_type == "up" then
        pointer.down = false
        handle_pointer_up(screen_x, screen_y)
    end
end

function handle_pointer_down(screen_x, screen_y)
    local canvas_x, canvas_y = screen_to_canvas(screen_x, screen_y)

    if current_tool == "select" then
        -- Check for resize handle hit first
        local handle = hit_test_resize_handle(canvas_x, canvas_y)
        if handle then
            drag_state.active = true
            drag_state.mode = "resize"
            drag_state.resize_handle = handle
            save_start_positions()
            return
        end

        -- Check for shape hit
        local hit = hit_test_shape(canvas_x, canvas_y)
        if hit then
            select_shape(hit.id)
            drag_state.active = true
            drag_state.mode = "move"
            save_start_positions()
        else
            clear_selection()
            drag_state.active = true
            drag_state.mode = "pan"
            drag_state.start_pan_x = viewport.pan_x
            drag_state.start_pan_y = viewport.pan_y
        end

    elseif current_tool == "rectangle" or current_tool == "ellipse" or
           current_tool == "diamond" or current_tool == "text" or
           current_tool == "sticky" then
        local shape = create_shape(current_tool, canvas_x - 50, canvas_y - 30, 100, 60)
        select_shape(shape.id)

    elseif current_tool == "connector" then
        local hit = hit_test_shape(canvas_x, canvas_y)
        if hit then
            if connector_start == nil then
                -- First click: select source shape
                connector_start = hit
                update_status("Click target shape...")
            else
                -- Second click: create connector if different shape
                if hit.id ~= connector_start.id then
                    create_connector(connector_start.id, hit.id)
                end
                connector_start = nil
                update_status()
            end
        else
            -- Clicked empty space: cancel connector creation
            if connector_start then
                connector_start = nil
                update_status("Cancelled")
            end
        end
    end
end

function handle_pointer_drag(screen_x, screen_y)
    if not drag_state.active then return end

    if drag_state.mode == "pan" then
        local dx = screen_x - pointer.start_x
        local dy = screen_y - pointer.start_y
        viewport.pan_x = drag_state.start_pan_x + dx
        viewport.pan_y = drag_state.start_pan_y + dy
        sync_viewport()

    elseif drag_state.mode == "move" then
        local canvas_x, canvas_y = screen_to_canvas(screen_x, screen_y)
        local start_canvas_x, start_canvas_y = screen_to_canvas(pointer.start_x, pointer.start_y)
        local dx = canvas_x - start_canvas_x
        local dy = canvas_y - start_canvas_y

        for id, start_pos in pairs(drag_state.start_positions) do
            local shape = graph.shapes[id]
            if shape then
                shape.x = start_pos.x + dx
                shape.y = start_pos.y + dy
                dirty_shapes[id] = true

                -- Surgical update: just this shape + its connectors
                update_shape_ui(shape)
                update_shape_connectors(shape)
            end
        end
        sync_selection()

    elseif drag_state.mode == "resize" then
        handle_resize_drag(screen_x, screen_y)
    end
end

function handle_pointer_up(screen_x, screen_y)
    local canvas_x, canvas_y = screen_to_canvas(screen_x, screen_y)

    -- Complete connector creation
    if current_tool == "connector" and connector_start then
        local hit = hit_test_shape(canvas_x, canvas_y)
        if hit and hit.id ~= connector_start.id then
            create_connector(connector_start.id, hit.id)
        end
        connector_start = nil
        update_status()
    end

    -- Commit dirty shapes to Loro
    for id, _ in pairs(dirty_shapes) do
        local shape = graph.shapes[id]
        if shape then
            shapes_layer:set(id, shape_to_loro(shape))
        end
    end
    dirty_shapes = {}

    -- Reset drag state
    drag_state.active = false
    drag_state.mode = nil
    drag_state.start_positions = {}
    drag_state.resize_handle = nil
end

-- =============================================================================
-- RESIZE HANDLING
-- =============================================================================

function handle_resize_drag(screen_x, screen_y)
    local canvas_x, canvas_y = screen_to_canvas(screen_x, screen_y)
    local start_canvas_x, start_canvas_y = screen_to_canvas(pointer.start_x, pointer.start_y)
    local dx = canvas_x - start_canvas_x
    local dy = canvas_y - start_canvas_y

    local handle = drag_state.resize_handle

    for id, start in pairs(drag_state.start_positions) do
        local shape = graph.shapes[id]
        if shape then
            local new_x, new_y = start.x, start.y
            local new_w, new_h = start.width, start.height

            -- Apply resize based on handle
            if handle == "nw" or handle == "w" or handle == "sw" then
                new_x = start.x + dx
                new_w = start.width - dx
            end
            if handle == "ne" or handle == "e" or handle == "se" then
                new_w = start.width + dx
            end
            if handle == "nw" or handle == "n" or handle == "ne" then
                new_y = start.y + dy
                new_h = start.height - dy
            end
            if handle == "sw" or handle == "s" or handle == "se" then
                new_h = start.height + dy
            end

            -- Minimum size
            if new_w < 20 then new_w = 20 end
            if new_h < 20 then new_h = 20 end

            shape.x = new_x
            shape.y = new_y
            shape.width = new_w
            shape.height = new_h
            dirty_shapes[id] = true

            update_shape_ui(shape)
            update_shape_connectors(shape)
        end
    end
    sync_selection()
end

function save_start_positions()
    drag_state.start_positions = {}
    for id, _ in pairs(selection.shape_ids) do
        local shape = graph.shapes[id]
        if shape then
            drag_state.start_positions[id] = {
                x = shape.x,
                y = shape.y,
                width = shape.width,
                height = shape.height,
            }
        end
    end
end

-- =============================================================================
-- SCROLL (ZOOM)
-- =============================================================================

function on_scroll(delta, screen_x, screen_y)
    local zoom_factor = 1.1
    local old_zoom = viewport.zoom

    if delta > 0 then
        viewport.zoom = viewport.zoom * zoom_factor
    else
        viewport.zoom = viewport.zoom / zoom_factor
    end

    viewport.zoom = math.max(0.1, math.min(5.0, viewport.zoom))

    -- Zoom toward mouse position
    local zoom_change = viewport.zoom / old_zoom
    viewport.pan_x = screen_x - (screen_x - viewport.pan_x) * zoom_change
    viewport.pan_y = screen_y - (screen_y - viewport.pan_y) * zoom_change

    sync_viewport()
    update_status()
end

-- =============================================================================
-- CLICK HANDLER (toolbar)
-- =============================================================================

function on_click(target)
    -- Tool selection
    if target:match("^tool_") then
        current_tool = target:sub(6)
        ui:set("current_tool", current_tool)
        connector_start = nil
        update_status()
        return
    end

    -- Layer selection
    if target:match("^layer_") then
        current_layer = tonumber(target:sub(7))
        ui:set("current_layer", current_layer)
        update_status()
        return
    end

    -- Actions
    if target:match("^action_") then
        local action = target:sub(8)
        if action == "auto_layout" then
            auto_layout()
        elseif action == "clear" then
            clear_canvas()
        end
        return
    end
end

-- =============================================================================
-- SHAPE OPERATIONS
-- =============================================================================

function create_shape(shape_type, x, y, width, height)
    local id = generate_id()

    local data = {
        id = id,
        type = shape_type,
        x = x,
        y = y,
        width = width or 100,
        height = height or 60,
        layer = current_layer,
        z_index = get_next_z_index(),
        text = get_default_text(shape_type),
    }

    -- Create in graph
    local shape = create_shape_in_graph(data)

    -- Save to Loro
    shapes_layer:set(id, shape_to_loro(shape))

    -- Rebuild UI (structural change)
    rebuild_ui_projection()
    sync_full_ui()
    update_status()

    return shape
end

function delete_selected()
    for id, _ in pairs(selection.shape_ids) do
        local shape = graph.shapes[id]
        if shape then
            -- Delete connected connectors first
            for _, conn in ipairs(shape.connectors) do
                connectors_layer:delete(conn.id)
                graph.connectors[conn.id] = nil
            end

            -- Delete shape
            shapes_layer:delete(id)
            graph.shapes[id] = nil

            -- Remove from roots
            for i, root in ipairs(graph.roots) do
                if root.id == id then
                    table.remove(graph.roots, i)
                    break
                end
            end
        end
    end

    clear_selection()
    rebuild_ui_projection()
    sync_full_ui()
    update_status()
end

function create_connector(from_id, to_id)
    local id = generate_id()

    local data = {
        id = id,
        from_id = from_id,
        to_id = to_id,
    }

    local connector = create_connector_in_graph(data)
    if connector then
        connectors_layer:set(id, connector_to_loro(connector))
        rebuild_ui_projection()
        sync_full_ui()
    end
    update_status()
end

-- =============================================================================
-- SELECTION
-- =============================================================================

function select_shape(id)
    selection.shape_ids = { [id] = true }

    -- Update UI for previously selected and newly selected
    rebuild_ui_projection()  -- Selection state changed
    sync_full_ui()
end

function clear_selection()
    selection.shape_ids = {}
    selection.bounds = nil
    sync_selection()

    -- Update shapes to show deselected state
    rebuild_ui_projection()
    sync_full_ui()
end

-- =============================================================================
-- HIT TESTING
-- =============================================================================

function hit_test_shape(canvas_x, canvas_y)
    -- Check shapes in reverse z-order (top first)
    local sorted = {}
    for _, shape in pairs(graph.shapes) do
        table.insert(sorted, shape)
    end
    table.sort(sorted, function(a, b)
        local layer_a = get_effective_layer(a)
        local layer_b = get_effective_layer(b)
        if layer_a ~= layer_b then
            return layer_a > layer_b  -- Higher layer first
        end
        return (a.z_index or 0) > (b.z_index or 0)
    end)

    for _, shape in ipairs(sorted) do
        if point_in_rect(canvas_x, canvas_y, shape) then
            return shape
        end
    end
    return nil
end

function hit_test_resize_handle(canvas_x, canvas_y)
    if not selection.bounds then return nil end

    local b = selection.bounds
    local handle_size = 8 / viewport.zoom  -- Adjust for zoom

    local handles = {
        { x = b.x, y = b.y, cursor = "nw" },
        { x = b.x + b.width / 2, y = b.y, cursor = "n" },
        { x = b.x + b.width, y = b.y, cursor = "ne" },
        { x = b.x + b.width, y = b.y + b.height / 2, cursor = "e" },
        { x = b.x + b.width, y = b.y + b.height, cursor = "se" },
        { x = b.x + b.width / 2, y = b.y + b.height, cursor = "s" },
        { x = b.x, y = b.y + b.height, cursor = "sw" },
        { x = b.x, y = b.y + b.height / 2, cursor = "w" },
    }

    for _, h in ipairs(handles) do
        local dx = canvas_x - h.x
        local dy = canvas_y - h.y
        if math.abs(dx) <= handle_size and math.abs(dy) <= handle_size then
            return h.cursor
        end
    end

    return nil
end

function point_in_rect(x, y, shape)
    return x >= shape.x and x <= shape.x + shape.width and
           y >= shape.y and y <= shape.y + shape.height
end

-- =============================================================================
-- COORDINATE TRANSFORMS
-- =============================================================================

function screen_to_canvas(screen_x, screen_y)
    local canvas_x = (screen_x - viewport.pan_x) / viewport.zoom
    local canvas_y = (screen_y - viewport.pan_y) / viewport.zoom
    return canvas_x, canvas_y
end

function canvas_to_screen(canvas_x, canvas_y)
    local screen_x = canvas_x * viewport.zoom + viewport.pan_x
    local screen_y = canvas_y * viewport.zoom + viewport.pan_y
    return screen_x, screen_y
end

-- =============================================================================
-- LORO SERIALIZATION
-- =============================================================================

function shape_to_loro(shape)
    return {
        id = shape.id,
        type = shape.type,
        x = shape.x,
        y = shape.y,
        width = shape.width,
        height = shape.height,
        parent_id = shape.parent_id,
        layer = shape.layer,
        z_index = shape.z_index,
        fill = shape.fill,
        stroke = shape.stroke,
        stroke_width = shape.stroke_width,
        corner_radius = shape.corner_radius,
        text = shape.text,
        text_size = shape.text_size,
        text_color = shape.text_color,
    }
end

function connector_to_loro(conn)
    return {
        id = conn.id,
        from_id = conn.from_id,
        to_id = conn.to_id,
        from_anchor = conn.from_anchor,
        to_anchor = conn.to_anchor,
        stroke = conn.stroke,
        stroke_width = conn.stroke_width,
    }
end

-- =============================================================================
-- AUTO LAYOUT
-- =============================================================================

function auto_layout()
    update_status("Calculating layout...")

    local nodes = {}
    local edges = {}

    for id, shape in pairs(graph.shapes) do
        nodes[id] = { width = shape.width, height = shape.height }
    end

    for _, conn in pairs(graph.connectors) do
        table.insert(edges, { from = conn.from_id, to = conn.to_id })
    end

    local positions
    if #edges > 0 then
        positions = layout:flowchart(nodes, edges)
    else
        positions = layout:grid(nodes, 4)
    end

    for id, pos in pairs(positions) do
        local shape = graph.shapes[id]
        if shape then
            shape.x = pos.x + 100
            shape.y = pos.y + 100
            shapes_layer:set(id, shape_to_loro(shape))
        end
    end

    rebuild_ui_projection()
    sync_full_ui()
    update_status("Layout applied")
end

-- =============================================================================
-- CLEAR CANVAS
-- =============================================================================

function clear_canvas()
    for id, _ in pairs(graph.shapes) do
        shapes_layer:delete(id)
    end
    for id, _ in pairs(graph.connectors) do
        connectors_layer:delete(id)
    end

    graph.shapes = {}
    graph.connectors = {}
    graph.roots = {}

    clear_selection()
    rebuild_ui_projection()
    sync_full_ui()
    update_status("Canvas cleared")
end

-- =============================================================================
-- UI HELPERS
-- =============================================================================

function update_toolbar()
    ui:set("current_tool", current_tool)
    ui:set("current_layer", current_layer)
    update_status()
end

function update_status(msg)
    if msg then
        ui:set("status_text", msg)
    else
        local shape_count = 0
        for _ in pairs(graph.shapes) do shape_count = shape_count + 1 end

        local zoom_pct = math.floor(viewport.zoom * 100)
        local tool_name = current_tool:sub(1, 1):upper() .. current_tool:sub(2)
        ui:set("status_text", tool_name .. " | L" .. current_layer .. " | " .. zoom_pct .. "%")
    end
end

-- =============================================================================
-- REMOTE CURSORS (Live Collaboration)
-- =============================================================================

local remote_cursors = {}  -- user_did -> { x, y, last_seen, color_index }

-- Number of cursor colors available in Slint Theme (0-7)
local NUM_CURSOR_COLORS = 8

-- Simple JSON encoder for ephemeral messages
-- Note: Lua doesn't have built-in JSON, so we use a minimal encoder
local function encode_cursor_json(x, y)
    return string.format('{"type":"cursor","x":%f,"y":%f}', x, y)
end

-- Simple JSON decoder for ephemeral messages
-- Returns table or nil if parsing fails
local function decode_json(str)
    -- Very basic JSON parsing (handles simple objects only)
    if not str or str == "" then return nil end

    local result = {}
    -- Match "key":value patterns
    for key, value in str:gmatch('"([^"]+)":([^,}]+)') do
        -- Remove quotes from string values
        local unquoted = value:match('^"(.*)"$')
        if unquoted then
            result[key] = unquoted
        else
            -- Try to parse as number
            local num = tonumber(value)
            if num then
                result[key] = num
            elseif value == "true" then
                result[key] = true
            elseif value == "false" then
                result[key] = false
            else
                result[key] = value
            end
        end
    end
    return result
end

-- Assign consistent color index based on user DID (0-7)
function color_index_for_user(did)
    local hash = 0
    for i = 1, #did do
        hash = (hash * 31 + string.byte(did, i)) % NUM_CURSOR_COLORS
    end
    return hash
end

-- Handle incoming ephemeral data (generic)
-- Dispatches based on message type
function on_ephemeral(user_did, payload)
    local msg = decode_json(payload)
    if not msg then return end

    if msg.type == "cursor" then
        handle_remote_cursor(user_did, msg.x, msg.y)
    end
end

-- Handle cursor update from remote peer
function handle_remote_cursor(user_did, x, y)
    remote_cursors[user_did] = {
        x = x,
        y = y,
        last_seen = os.time(),
        color_index = color_index_for_user(user_did),
    }
    refresh_remote_cursors()
end

-- Update UI with remote cursor positions
-- Pass canvas coordinates directly - Slint applies viewport transform (same as shapes)
function refresh_remote_cursors()
    local cursors = {}
    local now = os.time()
    local stale_users = {}

    for did, cursor in pairs(remote_cursors) do
        -- Remove stale cursors (>3 seconds without update)
        if now - cursor.last_seen > 3 then
            table.insert(stale_users, did)
        else
            -- Use canvas coords directly (not screen coords)
            -- Slint will apply viewport transform in the same container as shapes
            table.insert(cursors, {
                id = did,
                x = cursor.x,   -- canvas coords
                y = cursor.y,   -- canvas coords
                color_index = cursor.color_index,
                label = string.sub(did, -8),  -- Last 8 chars of DID
            })
        end
    end

    -- Remove stale cursors
    for _, did in ipairs(stale_users) do
        remote_cursors[did] = nil
    end

    ui:set("remote_cursors", cursors)
end

-- Track last sent cursor position to avoid spamming identical updates
local last_sent_cursor = { x = nil, y = nil }

-- Send cursor position to peers (with deduplication)
function send_cursor_update(canvas_x, canvas_y)
    -- Skip if position hasn't changed (within 1 pixel tolerance)
    if last_sent_cursor.x and last_sent_cursor.y then
        local dx = math.abs(canvas_x - last_sent_cursor.x)
        local dy = math.abs(canvas_y - last_sent_cursor.y)
        if dx < 1 and dy < 1 then
            return  -- No change, skip send
        end
    end

    last_sent_cursor.x = canvas_x
    last_sent_cursor.y = canvas_y

    butler:send_ephemeral(encode_cursor_json(canvas_x, canvas_y))
end

-- Handle hover events (mouse move without drag)
-- Called by timer-based polling when mouse is over canvas but not pressed
function on_hover(screen_x, screen_y)
    local canvas_x, canvas_y = screen_to_canvas(screen_x, screen_y)
    send_cursor_update(canvas_x, canvas_y)
end

-- Periodic tick for cursor cleanup (called if app supports tick)
function tick()
    refresh_remote_cursors()
end

-- =============================================================================
-- UTILITIES
-- =============================================================================

function generate_id()
    return string.format("%x", os.time()) .. "-" .. string.format("%04x", math.random(0, 65535))
end

function get_next_z_index()
    local max_z = 0
    for _, shape in pairs(graph.shapes) do
        if (shape.z_index or 0) > max_z then
            max_z = shape.z_index
        end
    end
    return max_z + 1
end

function get_default_text(shape_type)
    if shape_type == "text" then
        return "Text"
    elseif shape_type == "sticky" then
        return "Note"
    else
        return shape_type:sub(1, 1):upper() .. shape_type:sub(2)
    end
end

-- =============================================================================
-- API FOR TESTING / AI
-- =============================================================================

function get_shapes()
    local result = {}
    for _, shape in pairs(graph.shapes) do
        table.insert(result, shape)
    end
    return result
end

function get_connectors()
    local result = {}
    for _, conn in pairs(graph.connectors) do
        table.insert(result, {
            id = conn.id,
            from_id = conn.from_id,
            to_id = conn.to_id,
            from_x = conn.from_x,
            from_y = conn.from_y,
            ctrl1_x = conn.ctrl1_x,
            ctrl1_y = conn.ctrl1_y,
            ctrl2_x = conn.ctrl2_x,
            ctrl2_y = conn.ctrl2_y,
            to_x = conn.to_x,
            to_y = conn.to_y,
        })
    end
    return result
end

function force_refresh()
    rebuild_ui_projection()
    sync_full_ui()
    return "refreshed"
end

function get_shape_count()
    local count = 0
    for _ in pairs(graph.shapes) do count = count + 1 end
    return count
end

function get_connector_count()
    local count = 0
    for _ in pairs(graph.connectors) do count = count + 1 end
    return count
end

function get_ui_connectors()
    return ui_projection.connectors_array
end

function debug_connector_sync()
    rebuild_ui_projection()
    local conns = ui_projection.connectors_array
    sync_full_ui()
    return {
        count = #conns,
        connectors = conns
    }
end

function add_shape(shape_type, x, y, width, height)
    return create_shape(shape_type, x, y, width, height)
end

function add_shapes_grid(shape_type, count, cols, spacing)
    cols = cols or 10
    spacing = spacing or 20
    local width, height = 80, 50

    for i = 0, count - 1 do
        local row = math.floor(i / cols)
        local col = i % cols
        local x = 50 + col * (width + spacing)
        local y = 50 + row * (height + spacing)
        create_shape(shape_type, x, y, width, height)
    end
end
