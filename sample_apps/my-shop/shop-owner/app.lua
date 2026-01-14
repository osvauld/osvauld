-- Shop Owner App Logic
-- Manages products and processes customer orders
-- Uses derived orders_summary layer for aggregated view

-- Local state
local page_id = nil
local products_layer = nil       -- synced products list
local drafts_layer = nil         -- local-only drafts map (sync:false)
local orders_summary_layer = nil -- Derived layer (aggregated orders)
local order_layers = {}          -- Map of user_did -> orders_layer (for writing)

-- Initialize the app
function on_init()
    -- Get page_id for layer pattern matching
    page_id = permit:page_id()

    -- Get or create products layer - namespaced under page_id
    products_layer = loro:get_or_create_layer(page_id .. "/products", "list")

    -- Load existing products into UI
    refresh_products_ui()

    -- Local drafts layer: {page_id}/drafts (sync:false in permit - never syncs)
    drafts_layer = loro:get_or_create_layer(page_id .. "/drafts", "map")

    -- Subscribe to derived orders_summary layer (read-only for owner)
    -- This layer is maintained by node's derivation engine
    local summary_layer_name = page_id .. "/derived/orders_summary"
    orders_summary_layer = loro:get_layer(summary_layer_name, "map")
    if orders_summary_layer then
        log_info("Subscribed to orders_summary layer")
    else
        log_info("orders_summary layer not yet available")
    end

    -- Also track source layers for writing (when owner updates status)
    local pattern = page_id .. "/orders/*"
    local layers = loro:list_layers(pattern)
    for _, layer_name in ipairs(layers) do
        local orders_layer = loro:get_layer(layer_name, "list")
        if orders_layer then
            local user_did = layer_name:match("/orders/(.+)$")
            if user_did then
                order_layers[user_did] = orders_layer
                log_info("Loaded source orders layer for: " .. user_did)
            end
        end
    end

    refresh_orders_ui()
end

-- Called when any Loro layer changes
function on_loro_change(layer_name, change_type)
    if layer_name == page_id .. "/products" then
        refresh_products_ui()
    elseif layer_name:match("/derived/orders_summary$") then
        -- Derived layer updated - refresh orders UI
        log_info("orders_summary updated via derivation")
        refresh_orders_ui()
    elseif layer_name:match("/orders/") then
        -- Source layer changed - also refresh (for local changes)
        refresh_orders_ui()
    elseif layer_name == page_id .. "/drafts" then
        -- Drafts changed - refresh products UI to show drafts
        refresh_products_ui()
    end
end

-- Called when a new layer is discovered (e.g., customer created orders layer)
function on_layer_discovered(layer_name)
    -- Check for derived layer
    local summary_layer_name = page_id .. "/derived/orders_summary"
    if layer_name == summary_layer_name then
        orders_summary_layer = loro:get_layer(layer_name, "map")
        if orders_summary_layer then
            log_info("Discovered orders_summary layer")
            refresh_orders_ui()
        end
        return
    end

    -- Check for source order layers (for writing)
    local expected_prefix = page_id .. "/orders/"
    if layer_name:sub(1, #expected_prefix) == expected_prefix then
        local orders_layer = loro:get_layer(layer_name, "list")
        if orders_layer then
            local user_did = layer_name:match("/orders/(.+)$")
            if user_did then
                order_layers[user_did] = orders_layer
                log_info("Discovered source orders layer for: " .. user_did)
            end
        end
    end
end

-- UI callback: Add a new product draft (stored locally, not synced)
function add_product_draft(name, price, description, stock)
    -- Only owner can add products
    if permit:role() ~= "owner" then
        log_warn("Only owner can add products")
        return
    end

    if not drafts_layer then return end
    if name == "" then return end

    local price_num = tonumber(price) or 0
    local stock_num = tonumber(stock) or 0

    local product = {
        id = generate_id(),
        name = name,
        price = price_num,
        description = description or "",
        stock = stock_num,
        is_draft = true
    }

    -- Store draft in map by product ID (local only, won't sync)
    drafts_layer:set(product.id, product)
    log_info("Product draft created (local only): " .. name)

    refresh_products_ui()
end

-- UI callback: Publish a product (move from drafts to products list)
function publish_product(product_id)
    -- Only owner can publish products
    if permit:role() ~= "owner" then
        log_warn("Only owner can publish products")
        return
    end

    if not products_layer or not drafts_layer then return end

    -- Get draft
    local product = drafts_layer:get(product_id)
    if not product then
        log_warn("Draft not found: " .. product_id)
        return
    end

    -- Remove draft flag and push to synced products list
    product.is_draft = nil
    products_layer:push(product)

    -- Remove from drafts
    drafts_layer:delete(product_id)

    log_info("Product published: " .. (product.name or product_id))
    refresh_products_ui()
end

-- UI callback: Add a new product directly (for backward compatibility)
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

    -- Add directly to synced products list
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

    -- Apply the status change to SOURCE layer
    -- Derivation engine will update orders_summary automatically
    order.status = new_status
    order.updated_at = os.date("%Y-%m-%d %H:%M:%S")
    orders_layer:set(order_index, order)

    log_info("Order " .. order_id .. " status updated to " .. new_status)
end

-- Refresh products in UI (includes both drafts and published products)
function refresh_products_ui()
    local products = {}

    -- Add published products
    if products_layer then
        local len = products_layer:length()
        for i = 0, len - 1 do
            local product = products_layer:get(i)
            if product then
                table.insert(products, {
                    id = product.id or "",
                    name = product.name or "",
                    price = product.price or 0,
                    description = product.description or "",
                    stock = product.stock or 0,
                    is_draft = false
                })
            end
        end
    end

    -- Add drafts
    if drafts_layer then
        local keys = drafts_layer:keys()
        if keys then
            for _, key in ipairs(keys) do
                local draft = drafts_layer:get(key)
                if draft and draft.name then  -- Only product drafts have name
                    table.insert(products, {
                        id = draft.id or "",
                        name = draft.name or "",
                        price = draft.price or 0,
                        description = draft.description or "",
                        stock = draft.stock or 0,
                        is_draft = true
                    })
                end
            end
        end
    end

    ui:set("products", products)
end

-- Refresh orders in UI from derived orders_summary layer
function refresh_orders_ui()
    local all_orders = {}

    -- Read from derived orders_summary layer
    if orders_summary_layer then
        -- orders_summary is a Map: order_id -> summary
        local keys = orders_summary_layer:keys()
        for _, order_id in ipairs(keys) do
            local order = orders_summary_layer:get(order_id)
            if order then
                table.insert(all_orders, {
                    id = order.id or "",
                    customer_did = order.customer or "",
                    items = "",  -- Summary doesn't have items, just item_count
                    quantity = order.item_count or 0,
                    notes = "",
                    shipping_address = "",
                    status = order.status or "",
                    total = order.total or 0,
                    created_at = order.created_at or ""
                })
            end
        end
        log_info("Loaded " .. #all_orders .. " orders from summary layer")
    else
        log_info("orders_summary layer not available yet")
    end

    -- Sort by created_at descending (newest first)
    table.sort(all_orders, function(a, b)
        return a.created_at > b.created_at
    end)

    ui:set("orders", all_orders)
end

-- Get total count of submitted orders (for testing)
function get_orders_count()
    if orders_summary_layer then
        local keys = orders_summary_layer:keys()
        return #keys
    end
    return 0
end

-- Get total count of products (for testing)
function get_products_count()
    if products_layer then
        return products_layer:length()
    end
    return 0
end

-- Get total count of product drafts (for testing)
function get_drafts_count()
    if drafts_layer then
        local keys = drafts_layer:keys()
        local count = 0
        if keys then
            for _, key in ipairs(keys) do
                local draft = drafts_layer:get(key)
                if draft and draft.name then  -- Only count product drafts
                    count = count + 1
                end
            end
        end
        return count
    end
    return 0
end

-- Get ID of last created product draft (for testing publish workflow)
function get_last_draft_id()
    if drafts_layer then
        local keys = drafts_layer:keys()
        if keys and #keys > 0 then
            -- Return the last key (most recently added)
            return keys[#keys]
        end
    end
    return nil
end

-- Get all products as a table (for testing)
function get_products()
    local products = {}
    if products_layer then
        local len = products_layer:length()
        for i = 0, len - 1 do
            local product = products_layer:get(i)
            if product then
                table.insert(products, product)
            end
        end
    end
    return products
end

-- Get order status from derived layer (for testing)
function get_order_status(order_id)
    if orders_summary_layer then
        local order = orders_summary_layer:get(order_id)
        if order then
            return order.status
        end
    end
    return nil
end

-- Get all orders from derived layer (for testing)
function get_all_orders()
    local orders = {}
    if orders_summary_layer then
        local keys = orders_summary_layer:keys()
        for _, key in ipairs(keys) do
            local order = orders_summary_layer:get(key)
            if order then
                table.insert(orders, order)
            end
        end
    end
    return orders
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
