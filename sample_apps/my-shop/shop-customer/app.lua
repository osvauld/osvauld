-- Shop Customer App Logic
-- Browse products and manage orders (multiple orders as LIST)

-- Order State Machine
-- Defines what customers can do in each order state
local ORDER_STATES = {
    draft = {
        writable = {'items', 'quantity', 'notes', 'shipping_address'},
        can_delete = true,
        customer_transitions = {'pending'}
    },
    pending = {
        writable = {'notes'},
        can_delete = true,
        customer_transitions = {'cancelled'}
    },
    confirmed = {
        writable = {},
        can_delete = false,
        customer_transitions = {}
    },
    shipped = {
        writable = {},
        can_delete = false,
        customer_transitions = {}
    },
    delivered = {
        writable = {},
        can_delete = false,
        customer_transitions = {}
    },
    cancelled = {
        writable = {},
        can_delete = false,
        customer_transitions = {}
    }
}

-- Check if customer can transition to new status
local function can_transition_to(current_status, new_status)
    local state = ORDER_STATES[current_status or 'draft']
    if not state then return false end
    for _, s in ipairs(state.customer_transitions) do
        if s == new_status then return true end
    end
    return false
end

-- Local state
local page_id = nil
local products_layer = nil
local orders_layer = nil  -- LIST of orders (synced)
local drafts_layer = nil  -- MAP of draft orders (local-only, sync:false)
local my_did = nil

-- Product cache (read from owner's layer)
local products_cache = {}

-- Currently selected product (stored when user clicks Select)
local selected_product = nil

-- Currently selected order (for editing/submitting)
local selected_order_id = nil

-- UI form state (tracked in Lua for submit_order_ui)
local ui_order_quantity = "1"
local ui_order_notes = ""
local ui_order_address = ""

-- Initialize the app
function on_init()
    -- Get page_id and my DID
    page_id = permit:page_id()
    my_did = permit:my_did()

    -- Get products layer (read-only, from owner) - namespaced under page_id
    products_layer = loro:get_layer(page_id .. "/products", "list")
    if products_layer then
        refresh_products_ui()
    end

    -- Local drafts layer: {page_id}/drafts (sync:false in permit - never syncs)
    drafts_layer = loro:get_or_create_layer(page_id .. "/drafts", "map")

    -- Synced orders layer: {page_id}/orders/{my_did} (sync:true in permit)
    local orders_layer_name = permit:my_layer("orders")
    orders_layer = loro:get_or_create_layer(orders_layer_name, "list")

    refresh_orders_ui()
end

-- Called when any Loro layer changes
function on_loro_change(layer_name, change_type)
    if layer_name == page_id .. "/products" then
        refresh_products_ui()
    elseif layer_name == permit:my_layer("orders") then
        refresh_orders_ui()
    elseif layer_name == page_id .. "/drafts" then
        refresh_orders_ui()
    end
end

-- UI callback: Select a product to order
function select_product(product_id, product_name, product_price)
    -- Store selected product for order form
    selected_product = {
        id = product_id,
        name = product_name,
        price = product_price
    }
    log_info("Selected product: " .. product_name)
end

-- UI-driven product selection (shows form via debug socket)
function select_product_ui(product_id, name, price)
    select_product(product_id, name, price)
    ui:set("selected_product_id", product_id)
    ui:set("selected_product_name", name)
    ui:set("selected_product_price", price)
    ui:set("current_view", 0)  -- stay on products view to show form
    log_info("UI: Selected product " .. name)
end

-- Set form fields from Lua (for debug socket control)
function set_order_quantity(qty)
    ui_order_quantity = tostring(qty)
    ui:set("order_quantity", tostring(qty))
end

function set_order_notes(notes)
    ui_order_notes = notes
    ui:set("order_notes", notes)
end

function set_order_address(addr)
    ui_order_address = addr
    ui:set("order_address", addr)
end

-- Submit order and clear form (for debug socket)
function submit_order_ui()
    if not selected_product then
        log_warn("No product selected")
        return
    end
    -- Get form values from Lua state (set by set_* functions)
    local qty = ui_order_quantity or "1"
    local notes = ui_order_notes or ""
    local addr = ui_order_address or ""

    create_order(selected_product.id, qty, notes, addr)
    submit_order()

    -- Clear form
    ui:set("selected_product_id", "")
    ui:set("selected_product_name", "")
    ui:set("selected_product_price", 0)
    ui:set("order_quantity", "1")
    ui:set("order_notes", "")
    ui:set("order_address", "")
    ui:set("current_view", 1)  -- switch to orders view
    log_info("UI: Order submitted")
end

-- Switch views (for debug socket)
function show_products_view()
    ui:set("current_view", 0)
end

function show_orders_view()
    ui:set("current_view", 1)
end

-- UI callback: Create a new order draft (stored in local drafts map, never syncs)
function create_order(product_id, quantity_str, notes, shipping_address)
    -- Only customers can create orders
    if permit:role() ~= "customer" then
        log_warn("Only customers can create orders")
        return
    end

    if not drafts_layer then
        log_warn("Drafts layer not available")
        return
    end

    local quantity = tonumber(quantity_str) or 1
    if quantity < 1 then quantity = 1 end

    -- Use stored selected product (from select_product callback)
    if not selected_product or selected_product.id ~= product_id then
        -- Fallback: try to find in cache
        local product = find_product(product_id)
        if not product then
            log_warn("Product not found: " .. product_id)
            return
        end
        selected_product = product
    end

    -- Calculate total
    local total = selected_product.price * quantity

    -- Create order in draft state
    local order = {
        id = generate_id(),
        items = selected_product.name,
        quantity = quantity,
        notes = notes or "",
        shipping_address = shipping_address or "",
        status = "draft",
        total = total,
        created_at = os.date("%Y-%m-%d %H:%M:%S"),
        product_id = selected_product.id
    }

    -- Store draft in map by order ID (local only, won't sync)
    drafts_layer:set(order.id, order)
    selected_order_id = order.id

    log_info("Order draft created (local only): " .. order.id)

    -- Update UI immediately
    refresh_orders_ui()
end

-- UI callback: Submit order (move from drafts map to orders list, draft -> pending)
function submit_order(order_id)
    -- Only customers can submit orders
    if permit:role() ~= "customer" then
        log_warn("Only customers can submit orders")
        return
    end

    if not orders_layer or not drafts_layer then return end

    -- Use provided order_id or selected_order_id
    local target_id = order_id or selected_order_id
    if not target_id then
        log_warn("No order selected to submit")
        return
    end

    -- Get draft from drafts map
    local order = drafts_layer:get(target_id)
    if not order then
        log_warn("Draft not found: " .. target_id)
        return
    end

    local status = order.status

    -- Use state machine to check transition
    if not can_transition_to(status, "pending") then
        log_warn("Cannot submit order in " .. (status or "unknown") .. " state")
        return
    end

    -- Validate required fields
    if not order.items or order.items == "" then
        log_warn("Order must have items")
        return
    end

    if not order.shipping_address or order.shipping_address == "" then
        log_warn("Order must have shipping address")
        return
    end

    -- Update order status and move to synced orders layer
    order.status = "pending"
    order.submitted_at = os.date("%Y-%m-%d %H:%M:%S")

    -- Push to synced orders list (this will sync to node/owner)
    orders_layer:push(order)

    -- Remove from local drafts map
    drafts_layer:delete(target_id)

    log_info("Order submitted and syncing: " .. target_id)

    -- Update UI immediately
    refresh_orders_ui()
end

-- UI callback: Cancel order (customer can cancel draft or pending)
function cancel_order(order_id)
    -- Only customers can cancel their orders
    if permit:role() ~= "customer" then
        log_warn("Only customers can cancel orders")
        return
    end

    -- Use provided order_id or selected_order_id
    local target_id = order_id or selected_order_id
    if not target_id then
        log_warn("No order selected to cancel")
        return
    end

    -- First check if it's a draft (in drafts map)
    if drafts_layer then
        local draft = drafts_layer:get(target_id)
        if draft then
            -- Just delete from drafts (local only)
            drafts_layer:delete(target_id)
            log_info("Draft order deleted: " .. target_id)
            refresh_orders_ui()
            return
        end
    end

    -- Otherwise check synced orders
    if not orders_layer then return end

    -- Find order by ID in synced orders
    local order_index = find_order_index(target_id)
    if not order_index then
        log_warn("Order not found: " .. target_id)
        return
    end

    local order = orders_layer:get(order_index)
    local status = order.status
    local state = ORDER_STATES[status or 'draft']

    -- Check if deletion/cancellation is allowed in this state
    if not state or not state.can_delete then
        log_warn("Cannot cancel order in " .. (status or "unknown") .. " state")
        return
    end

    -- Update order to cancelled status (keep in list for history)
    order.status = "cancelled"
    order.cancelled_at = os.date("%Y-%m-%d %H:%M:%S")
    orders_layer:set(order_index, order)

    log_info("Order cancelled: " .. target_id)

    -- Update UI immediately
    refresh_orders_ui()
end

-- Find order index by ID in synced orders
function find_order_index(order_id)
    if not orders_layer then return nil end
    local len = orders_layer:length()
    for i = 0, len - 1 do
        local order = orders_layer:get(i)
        if order and order.id == order_id then
            return i
        end
    end
    return nil
end

-- Find product by ID in cache
function find_product(product_id)
    for _, product in ipairs(products_cache) do
        if product.id == product_id then
            return product
        end
    end
    return nil
end

-- Refresh products from owner's layer
function refresh_products_ui()
    if not products_layer then return end

    products_cache = {}
    local len = products_layer:length()
    for i = 0, len - 1 do
        local product = products_layer:get(i)
        if product then
            table.insert(products_cache, {
                id = product.id or "",
                name = product.name or "",
                price = product.price or 0,
                description = product.description or "",
                stock = product.stock or 0
            })
        end
    end

    ui:set("products", products_cache)
end

-- Refresh orders UI (shows drafts + synced orders)
function refresh_orders_ui()
    local orders = {}

    -- First add drafts (local only)
    if drafts_layer then
        local keys = drafts_layer:keys()
        if keys then
            for _, key in ipairs(keys) do
                local draft = drafts_layer:get(key)
                if draft then
                    table.insert(orders, {
                        id = draft.id or "",
                        items = draft.items or "",
                        quantity = draft.quantity or 0,
                        notes = draft.notes or "",
                        shipping_address = draft.shipping_address or "",
                        status = draft.status or "draft",
                        total = draft.total or 0,
                        created_at = draft.created_at or ""
                    })
                end
            end
        end
    end

    -- Then add synced orders
    if orders_layer then
        local len = orders_layer:length()
        for i = 0, len - 1 do
            local order = orders_layer:get(i)
            if order then
                table.insert(orders, {
                    id = order.id or "",
                    items = order.items or "",
                    quantity = order.quantity or 0,
                    notes = order.notes or "",
                    shipping_address = order.shipping_address or "",
                    status = order.status or "pending",
                    total = order.total or 0,
                    created_at = order.created_at or ""
                })
            end
        end
    end

    -- Sort by created_at descending (newest first)
    table.sort(orders, function(a, b)
        return a.created_at > b.created_at
    end)

    ui:set("my_orders", orders)
end

-- Generate a simple ID
function generate_id()
    return string.format("%x", os.time()) .. "-" .. string.format("%04x", math.random(0, 65535))
end

-- Get total count of products (for testing)
function get_products_count()
    if products_layer then
        return products_layer:length()
    end
    return 0
end

-- Get all products as a table (for testing)
function get_products()
    return products_cache
end

-- Get total count of synced orders (for testing)
function get_orders_count()
    if orders_layer then
        return orders_layer:length()
    end
    return 0
end

-- Get total count of drafts (for testing)
function get_drafts_count()
    if drafts_layer then
        local keys = drafts_layer:keys()
        return keys and #keys or 0
    end
    return 0
end

-- Check if orders_summary derived layer exists (for testing)
-- Customer should NOT have this layer (not in their permit)
function has_orders_summary()
    local summary_layer_name = page_id .. "/derived/orders_summary"
    local layer = loro:get_layer(summary_layer_name, "map")
    return layer ~= nil
end

-- Get the last created order ID (for testing)
function get_last_order_id()
    return selected_order_id
end

-- Get all orders (drafts + synced) as a table (for testing)
function get_all_orders()
    local orders = {}

    -- Add drafts
    if drafts_layer then
        local keys = drafts_layer:keys()
        if keys then
            for _, key in ipairs(keys) do
                local draft = drafts_layer:get(key)
                if draft then
                    table.insert(orders, draft)
                end
            end
        end
    end

    -- Add synced orders
    if orders_layer then
        local len = orders_layer:length()
        for i = 0, len - 1 do
            local order = orders_layer:get(i)
            if order then
                table.insert(orders, order)
            end
        end
    end
    return orders
end

-- Logging helpers
function log_info(msg)
    print("[INFO] " .. msg)
end

function log_warn(msg)
    print("[WARN] " .. msg)
end
