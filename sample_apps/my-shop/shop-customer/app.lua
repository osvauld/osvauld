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
local products_layer = nil
local orders_layer = nil  -- LIST of orders
local my_did = nil

-- Product cache (read from owner's layer)
local products_cache = {}

-- Currently selected product (stored when user clicks Select)
local selected_product = nil

-- Currently selected order (for editing/submitting)
local selected_order_id = nil

-- Initialize the app
function on_init()
    -- Get my DID from permit
    my_did = permit:my_did()

    -- Get products layer (read-only, from owner) - specify "list" type
    products_layer = loro:get_layer("products", "list")
    if products_layer then
        refresh_products_ui()
    end

    -- Get or create my personal orders layer (LIST)
    -- Layer name: {page_id}/orders/{my_did}
    local orders_layer_name = permit:my_layer("orders")
    orders_layer = loro:get_or_create_layer(orders_layer_name, "list")

    refresh_orders_ui()
end

-- Called when any Loro layer changes
function on_loro_change(layer_name, change_type)
    if layer_name == "products" then
        refresh_products_ui()
    elseif layer_name == permit:my_layer("orders") then
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

-- UI callback: Create a new order (adds to orders list)
function create_order(product_id, quantity_str, notes, shipping_address)
    -- Only customers can create orders
    if permit:role() ~= "customer" then
        log_warn("Only customers can create orders")
        return
    end

    if not orders_layer then
        log_warn("Orders layer not available")
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

    -- Push to orders list
    orders_layer:push(order)
    selected_order_id = order.id

    log_info("Order created in draft state: " .. order.id)

    -- Update UI immediately
    refresh_orders_ui()
end

-- UI callback: Submit order (draft -> pending)
function submit_order(order_id)
    -- Only customers can submit orders
    if permit:role() ~= "customer" then
        log_warn("Only customers can submit orders")
        return
    end

    if not orders_layer then return end

    -- Use provided order_id or selected_order_id
    local target_id = order_id or selected_order_id
    if not target_id then
        log_warn("No order selected to submit")
        return
    end

    -- Find order by ID
    local order_index = find_order_index(target_id)
    if not order_index then
        log_warn("Order not found: " .. target_id)
        return
    end

    local order = orders_layer:get(order_index)
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

    -- Update order status
    order.status = "pending"
    order.submitted_at = os.date("%Y-%m-%d %H:%M:%S")
    orders_layer:set(order_index, order)

    log_info("Order submitted: " .. target_id)

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

    if not orders_layer then return end

    -- Use provided order_id or selected_order_id
    local target_id = order_id or selected_order_id
    if not target_id then
        log_warn("No order selected to cancel")
        return
    end

    -- Find order by ID
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

-- Find order index by ID
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

-- Refresh orders UI (shows all orders)
function refresh_orders_ui()
    if not orders_layer then
        ui:set("my_orders", {})
        return
    end

    local orders = {}
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
                status = order.status or "draft",
                total = order.total or 0,
                created_at = order.created_at or ""
            })
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

-- Logging helpers
function log_info(msg)
    print("[INFO] " .. msg)
end

function log_warn(msg)
    print("[WARN] " .. msg)
end
