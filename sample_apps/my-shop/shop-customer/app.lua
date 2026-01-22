-- Shop Customer App Logic
-- Browse products and manage orders (multiple orders as LIST)
-- Uses Event Bus pattern for unified human/AI interaction

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

-- Form state tracking (synced with UI via Event Bus)
local form_state = {
    order_quantity = "1",
    order_notes = "",
    order_address = ""
}

-- Currently selected product (stored when user clicks Select)
local selected_product = nil

-- Currently selected order (for editing/submitting)
local selected_order_id = nil

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

-- ============================================================================
-- EVENT BUS HANDLERS
-- Unified event flow for human UI and AI automation
-- ============================================================================

-- Handle button clicks
function on_click(target)
    -- Parse target: "action:id" or just "action"
    local action, id = target:match("^([^:]+):?(.*)$")

    if action == "select_product" then
        do_select_product(id)
    elseif action == "submit_order" then
        do_submit_order(id)
    elseif action == "cancel_order" then
        do_cancel_order(id)
    end
end

-- Handle field changes from UI
function on_field_changed(field_name, value)
    form_state[field_name] = value
end

-- Handle modal actions (open/close/submit)
function on_modal_action(modal_name, action)
    if modal_name == "order" then
        if action == "submit" then
            do_create_order()
        end
    end
end

-- ============================================================================
-- CORE LOGIC
-- Business logic called by event handlers
-- ============================================================================

-- Select product when user clicks Select button
function do_select_product(product_id)
    local product = find_product(product_id)
    if product then
        selected_product = {
            id = product.id,
            name = product.name,
            price = product.price
        }
        log_info("Selected product: " .. product.name)
    end
end

-- Create a new order draft (stored in local drafts map, never syncs)
function do_create_order()
    -- Only customers can create orders
    if permit:role() ~= "customer" then
        log_warn("Only customers can create orders")
        return
    end

    if not drafts_layer then
        log_warn("Drafts layer not available")
        return
    end

    -- Get selected product from UI state
    local product_id = ui:get("selected_product_id")
    if not product_id or product_id == "" then
        log_warn("No product selected")
        return
    end

    -- Use stored selected product
    if not selected_product or selected_product.id ~= product_id then
        local product = find_product(product_id)
        if not product then
            log_warn("Product not found: " .. product_id)
            return
        end
        selected_product = product
    end

    local quantity = tonumber(form_state.order_quantity) or 1
    if quantity < 1 then quantity = 1 end

    -- Calculate total
    local total = selected_product.price * quantity

    -- Create order in draft state
    local order = {
        id = generate_id(),
        items = selected_product.name,
        quantity = quantity,
        notes = form_state.order_notes or "",
        shipping_address = form_state.order_address or "",
        status = "draft",
        total = total,
        created_at = os.date("%Y-%m-%d %H:%M:%S"),
        product_id = selected_product.id
    }

    -- Store draft in map by order ID (local only, won't sync)
    drafts_layer:set(order.id, order)
    selected_order_id = order.id

    log_info("Order draft created (local only): " .. order.id)

    -- Clear form and switch to orders view
    clear_order_form()
    ui:set("current_view", 1)

    -- Update UI immediately
    refresh_orders_ui()
end

-- Submit order (move from drafts map to orders list, draft -> pending)
function do_submit_order(order_id)
    -- Only customers can submit orders
    if permit:role() ~= "customer" then
        log_warn("Only customers can submit orders")
        return
    end

    if not orders_layer or not drafts_layer then return end

    -- Use provided order_id or selected_order_id
    local target_id = order_id
    if not target_id or target_id == "" then
        target_id = selected_order_id
    end
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

-- Cancel order (customer can cancel draft or pending)
function do_cancel_order(order_id)
    -- Only customers can cancel their orders
    if permit:role() ~= "customer" then
        log_warn("Only customers can cancel orders")
        return
    end

    -- Use provided order_id or selected_order_id
    local target_id = order_id
    if not target_id or target_id == "" then
        target_id = selected_order_id
    end
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

-- Clear order form
function clear_order_form()
    form_state.order_quantity = "1"
    form_state.order_notes = ""
    form_state.order_address = ""

    ui:set("selected_product_id", "")
    ui:set("selected_product_name", "")
    ui:set("selected_product_price", 0)
    ui:set("order_quantity", "1")
    ui:set("order_notes", "")
    ui:set("order_address", "")

    selected_product = nil
end

-- ============================================================================
-- AI AUTOMATION API
-- Same code path as human interaction
-- ============================================================================

-- Select product via UI simulation (for AI/testing)
function select_product_via_ui(product_id)
    local product = find_product(product_id)
    if not product then
        log_warn("Product not found: " .. product_id)
        return false
    end

    -- Update internal state first (no UI updates yet)
    selected_product = {
        id = product.id,
        name = product.name,
        price = product.price
    }

    -- Batch UI updates: set current_view first if needed,
    -- THEN set product details (to avoid recursion when form becomes visible)
    local current_view = ui:get("current_view")
    if current_view ~= 0 then
        ui:set("current_view", 0)
    end

    -- Now set product details (form should already be visible or about to be)
    ui:set("selected_product_id", product.id)
    ui:set("selected_product_name", product.name)
    ui:set("selected_product_price", product.price)

    log_info("Selected product: " .. product.name)
    return true
end

-- Place order via UI simulation (for AI/testing)
-- This is the complete flow: select product, fill form, create order, submit
function place_order_via_ui(product_id, quantity, notes, address)
    -- 1. Select product
    if not select_product_via_ui(product_id) then
        return false
    end

    -- 2. Fill form fields
    quantity = tostring(quantity or 1)
    notes = notes or ""
    address = address or ""

    ui:set("order_quantity", quantity)
    ui:set("order_notes", notes)
    ui:set("order_address", address)

    form_state.order_quantity = quantity
    form_state.order_notes = notes
    form_state.order_address = address

    -- 3. Create order (submit modal)
    on_modal_action("order", "submit")

    -- 4. Submit the order (draft -> pending)
    if selected_order_id then
        on_click("submit_order:" .. selected_order_id)
    end

    return true
end

-- Cancel order via UI simulation (for AI/testing)
function cancel_order_via_ui(order_id)
    on_click("cancel_order:" .. order_id)
end

-- Switch views (for AI/testing)
function show_products_view()
    ui:set("current_view", 0)
end

function show_orders_view()
    ui:set("current_view", 1)
end

-- ============================================================================
-- UI REFRESH
-- ============================================================================

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

-- ============================================================================
-- HELPERS
-- ============================================================================

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

-- Generate a simple ID
function generate_id()
    return string.format("%x", os.time()) .. "-" .. string.format("%04x", math.random(0, 65535))
end

-- ============================================================================
-- TEST HELPERS
-- ============================================================================

-- Get total count of products (for testing)
function get_products_count()
    if products_layer then
        return products_layer:length()
    end
    return 0
end

-- Get first product ID (for testing)
function get_first_product_id()
    if products_cache and #products_cache > 0 then
        return products_cache[1].id or ""
    end
    return ""
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

-- ============================================================================
-- UTILITIES
-- ============================================================================

function log_info(msg)
    print("[INFO] " .. msg)
end

function log_warn(msg)
    print("[WARN] " .. msg)
end

-- ============================================================================
-- API EXPORTS
-- Register functions for external calling via control server
-- ============================================================================

--- @ai Creates an order (selects product, fills form, creates draft).
--- @ai For tests - alias to internal create flow.
api.export("create_order", function(product_id, quantity, notes, address)
    -- Ensure products cache is populated (may not be if called right after sync)
    refresh_products_ui()

    -- Select the product
    if not select_product_via_ui(product_id) then
        log_warn("create_order: failed to select product " .. (product_id or "nil"))
        return false
    end
    -- Fill form
    form_state.order_quantity = tostring(quantity or 1)
    form_state.order_notes = notes or ""
    form_state.order_address = address or ""
    ui:set("order_quantity", form_state.order_quantity)
    ui:set("order_notes", form_state.order_notes)
    ui:set("order_address", form_state.order_address)
    -- Create the order (draft state)
    do_create_order()
    return true
end)

--- @ai Selects a product for ordering via UI simulation.
--- @ai Effects: Sets selected_product state, updates UI.
api.export("select_product", select_product_via_ui)
api.describe("select_product", {
    description = "Select a product for ordering via UI flow",
    params = {
        {name = "product_id", type = "string", description = "Product ID to select"},
    },
    returns = "boolean - true if product found and selected",
    effects = {"selected_product state updated", "UI shows selected product"},
})

--- @ai Places an order via UI simulation (select, fill form, create draft, submit).
--- @ai Effects: Creates order, moves to pending status, syncs to owner.
api.export("place_order", place_order_via_ui)
api.describe("place_order", {
    description = "Place a complete order via UI flow",
    params = {
        {name = "product_id", type = "string", description = "Product ID"},
        {name = "quantity", type = "number", description = "Quantity"},
        {name = "notes", type = "string", description = "Order notes"},
        {name = "address", type = "string", description = "Shipping address"},
    },
    returns = "boolean - true if order placed successfully",
    effects = {"order created in draft", "order submitted", "synced to owner"},
    syncs = {"orders layer to owner/node"},
})

--- @ai Submits a draft order (draft -> pending).
api.export("submit_order", do_submit_order)
api.describe("submit_order", {
    description = "Submit a draft order",
    params = {
        {name = "order_id", type = "string", description = "Order ID to submit"},
    },
    effects = {"order status changes to pending", "order syncs to owner"},
})

--- @ai Cancels an order.
api.export("cancel_order", cancel_order_via_ui)

--- @ai Returns count of products available.
api.export("get_products_count", get_products_count)

--- @ai Returns all products as table.
api.export("get_products", get_products)

--- @ai Returns first product ID (for testing).
api.export("get_first_product_id", get_first_product_id)

--- @ai Returns count of synced orders.
api.export("get_orders_count", get_orders_count)

--- @ai Returns count of local draft orders.
api.export("get_drafts_count", get_drafts_count)

--- @ai Returns all orders (drafts + synced).
api.export("get_all_orders", get_all_orders)

--- @ai Returns last created order ID.
api.export("get_last_order_id", get_last_order_id)

--- @ai Checks if orders_summary derived layer exists.
api.export("has_orders_summary", has_orders_summary)
