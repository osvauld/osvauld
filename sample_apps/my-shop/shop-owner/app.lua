-- Shop Owner App Logic
-- Manages products and processes customer orders (LIST model)

-- Local state
local products_layer = nil
local order_layers = {}  -- Map of user_did -> orders_layer (LIST)
local page_id = nil

-- Initialize the app
function on_init()
    -- Get page_id for layer pattern matching
    page_id = permit:page_id()

    -- Get or create products layer
    products_layer = loro:get_or_create_layer("products", "list")

    -- Load existing products into UI
    refresh_products_ui()

    -- Scan for existing customer orders layers
    -- Pattern: {page_id}/orders/*
    local pattern = page_id .. "/orders/*"
    local layers = loro:list_layers(pattern)
    for _, layer_name in ipairs(layers) do
        local orders_layer = loro:get_layer(layer_name, "list")
        if orders_layer then
            -- Extract user_did from layer name: {page_id}/orders/{user_did}
            local user_did = layer_name:match("/orders/(.+)$")
            if user_did then
                order_layers[user_did] = orders_layer
                log_info("Loaded orders layer for: " .. user_did)
            end
        end
    end
    refresh_orders_ui()
end

-- Called when any Loro layer changes
function on_loro_change(layer_name, change_type)
    if layer_name == "products" then
        refresh_products_ui()
    elseif layer_name:match("/orders/") then
        refresh_orders_ui()
    end
end

-- Called when a new layer is discovered (e.g., customer created orders layer)
function on_layer_discovered(layer_name)
    -- Pattern: {page_id}/orders/{user_did}
    local expected_prefix = page_id .. "/orders/"
    if layer_name:sub(1, #expected_prefix) == expected_prefix then
        local orders_layer = loro:get_layer(layer_name, "list")
        if orders_layer then
            local user_did = layer_name:match("/orders/(.+)$")
            if user_did then
                order_layers[user_did] = orders_layer
                log_info("Discovered new orders layer for: " .. user_did)
                refresh_orders_ui()
            end
        end
    end
end

-- UI callback: Add a new product
function add_product(name, price, description, stock)
    -- Only owner can add products
    if permit:role() ~= "owner" then
        log_warn("Only owner can add products")
        return
    end

    if not products_layer then return end
    if name == "" then return end

    local price_num = tonumber(price) or 0
    local stock_num = tonumber(stock) or 0

    local product = {
        id = generate_id(),
        name = name,
        price = price_num,
        description = description or "",
        stock = stock_num
    }

    products_layer:push(product)
    log_info("Product added: " .. name)
end

-- UI callback: Update order status (owner transitions)
-- Owner can: pending -> confirmed, confirmed -> shipped, shipped -> delivered
-- Owner can also: pending -> cancelled
function update_order_status(customer_did, order_id, new_status)
    -- Only owner can update order status
    if permit:role() ~= "owner" then
        log_warn("Only owner can update order status")
        return
    end

    local orders_layer = order_layers[customer_did]
    if not orders_layer then
        log_warn("Orders layer not found for customer: " .. customer_did)
        return
    end

    -- Find order by ID in the customer's orders list
    local order_index = nil
    local order = nil
    local len = orders_layer:length()
    for i = 0, len - 1 do
        local o = orders_layer:get(i)
        if o and o.id == order_id then
            order_index = i
            order = o
            break
        end
    end

    if not order_index then
        log_warn("Order not found: " .. order_id)
        return
    end

    local current_status = order.status or "draft"

    -- Validate owner transitions
    local valid_transitions = {
        pending = { "confirmed", "cancelled" },
        confirmed = { "shipped" },
        shipped = { "delivered" }
    }

    local allowed = valid_transitions[current_status] or {}
    local is_valid = false
    for _, status in ipairs(allowed) do
        if status == new_status then
            is_valid = true
            break
        end
    end

    if not is_valid then
        log_warn("Invalid transition from " .. current_status .. " to " .. new_status)
        return
    end

    -- Apply the status change
    order.status = new_status
    order.updated_at = os.date("%Y-%m-%d %H:%M:%S")
    orders_layer:set(order_index, order)

    log_info("Order " .. order_id .. " status updated to " .. new_status)
    refresh_orders_ui()
end

-- Refresh products in UI
function refresh_products_ui()
    if not products_layer then return end

    local products = {}
    local len = products_layer:length()
    for i = 0, len - 1 do
        local product = products_layer:get(i)
        if product then
            table.insert(products, {
                id = product.id or "",
                name = product.name or "",
                price = product.price or 0,
                description = product.description or "",
                stock = product.stock or 0
            })
        end
    end

    ui:set("products", products)
end

-- Refresh orders in UI (aggregates from all customer orders lists)
function refresh_orders_ui()
    local all_orders = {}

    for user_did, orders_layer in pairs(order_layers) do
        local len = orders_layer:length()
        for i = 0, len - 1 do
            local order = orders_layer:get(i)
            -- Only show non-draft orders to owner
            if order and order.status and order.status ~= "draft" then
                table.insert(all_orders, {
                    id = order.id or "",
                    customer_did = user_did,
                    items = order.items or "",
                    quantity = order.quantity or 0,
                    notes = order.notes or "",
                    shipping_address = order.shipping_address or "",
                    status = order.status,
                    total = order.total or 0,
                    created_at = order.created_at or ""
                })
            end
        end
    end

    -- Sort by created_at descending (newest first)
    table.sort(all_orders, function(a, b)
        return a.created_at > b.created_at
    end)

    ui:set("orders", all_orders)
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
