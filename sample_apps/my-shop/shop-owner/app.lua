-- Shop Owner App Logic
-- Manages products and processes customer orders
-- Uses Event Bus pattern for unified human/AI interaction

-- Local state
local page_id = nil
local products_layer = nil       -- synced products list
local orders_summary_layer = nil -- Derived layer (aggregated orders)
local order_layers = {}          -- Map of user_did -> orders_layer (for writing)

-- Form state tracking (synced with UI via Event Bus)
local form_state = {
    product_name = "",
    product_price = "",
    product_desc = "",
    product_stock = ""
}

-- Initialize the app
function on_init()
    page_id = permit:page_id()
    products_layer = loro:get_or_create_layer(page_id .. "/products", "list")
    refresh_products_ui()

    -- Subscribe to derived orders_summary layer
    local summary_layer_name = page_id .. "/derived/orders_summary"
    orders_summary_layer = loro:get_layer(summary_layer_name, "map")

    -- Track source layers for writing (when owner updates status)
    local pattern = page_id .. "/orders/*"
    local layers = loro:list_layers(pattern)
    for _, layer_name in ipairs(layers) do
        local orders_layer = loro:get_layer(layer_name, "list")
        if orders_layer then
            local user_did = layer_name:match("/orders/(.+)$")
            if user_did then
                order_layers[user_did] = orders_layer
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
        refresh_orders_ui()
    elseif layer_name:match("/orders/") then
        refresh_orders_ui()
    end
end

-- Called when a new layer is discovered
function on_layer_discovered(layer_name)
    local summary_layer_name = page_id .. "/derived/orders_summary"
    if layer_name == summary_layer_name then
        orders_summary_layer = loro:get_layer(layer_name, "map")
        if orders_summary_layer then
            refresh_orders_ui()
        end
        return
    end

    local expected_prefix = page_id .. "/orders/"
    if layer_name:sub(1, #expected_prefix) == expected_prefix then
        local orders_layer = loro:get_layer(layer_name, "list")
        if orders_layer then
            local user_did = layer_name:match("/orders/(.+)$")
            if user_did then
                order_layers[user_did] = orders_layer
            end
        end
    end
end

-- ============================================================================
-- EVENT BUS HANDLERS
-- Unified event flow for human UI and AI automation
-- ============================================================================

-- Handle field changes from UI
function on_field_changed(field_name, value)
    form_state[field_name] = value
end

-- Handle modal actions (open/close/submit)
function on_modal_action(modal_name, action)
    if modal_name == "add_product" then
        if action == "open" then
            ui:set("form_modal_visible", true)
        elseif action == "submit" then
            do_add_product()
            clear_product_form()
        elseif action == "cancel" or action == "close" then
            clear_product_form()
        end
    end
end

-- Handle button clicks
function on_click(target)
    -- Extensible: add click handlers as needed
end

-- ============================================================================
-- CORE LOGIC
-- Business logic called by event handlers
-- ============================================================================

-- Add product using current form state
function do_add_product()
    if permit:role() ~= "owner" then
        log_warn("Only owner can add products")
        return
    end

    if not products_layer then return end

    local name = form_state.product_name or ""
    if name == "" then return end

    local product = {
        id = generate_id(),
        name = name,
        price = tonumber(form_state.product_price) or 0,
        description = form_state.product_desc or "",
        stock = tonumber(form_state.product_stock) or 0
    }

    products_layer:push(product)
    log_info("Product added: " .. name)
end

-- Clear product form and close modal
function clear_product_form()
    form_state.product_name = ""
    form_state.product_price = ""
    form_state.product_desc = ""
    form_state.product_stock = ""

    ui:set("form_product_name", "")
    ui:set("form_product_price", "")
    ui:set("form_product_desc", "")
    ui:set("form_product_stock", "")
    ui:set("form_modal_visible", false)
end

-- Update order status (called from Slint callback)
function update_order_status(customer_did, order_id, new_status)
    if permit:role() ~= "owner" then
        return
    end

    local orders_layer = order_layers[customer_did]
    if not orders_layer then return end

    local len = orders_layer:length()
    for i = 0, len - 1 do
        local order = orders_layer:get(i)
        if order and order.id == order_id then
            local current = order.status or "draft"
            local valid = {
                pending = { confirmed = true, cancelled = true },
                confirmed = { shipped = true },
                shipped = { delivered = true }
            }
            if valid[current] and valid[current][new_status] then
                order.status = new_status
                order.updated_at = os.date("%Y-%m-%d %H:%M:%S")
                orders_layer:set(i, order)
            end
            break
        end
    end
end

-- ============================================================================
-- AI AUTOMATION API
-- Same code path as human interaction
-- ============================================================================

-- Add product via UI simulation (for AI/testing)
-- Uses exact same callbacks as human interaction
function add_product_via_ui(name, price, description, stock)
    -- 1. Open modal (same as human clicking "Add Product" button)
    on_modal_action("add_product", "open")

    -- 2. Fill form fields (same as human typing)
    on_field_changed("product_name", name or "")
    on_field_changed("product_price", tostring(price or ""))
    on_field_changed("product_desc", description or "")
    on_field_changed("product_stock", tostring(stock or ""))

    -- Also update UI display (two-way binding)
    ui:set("form_product_name", name or "")
    ui:set("form_product_price", tostring(price or ""))
    ui:set("form_product_desc", description or "")
    ui:set("form_product_stock", tostring(stock or ""))

    -- 3. Submit (same as human clicking "Add Product" in modal)
    on_modal_action("add_product", "submit")
end

-- ============================================================================
-- UI REFRESH
-- ============================================================================

function refresh_products_ui()
    local products = {}
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
                    stock = product.stock or 0
                })
            end
        end
    end
    ui:set("products", products)
end

function refresh_orders_ui()
    local all_orders = {}
    if orders_summary_layer then
        local keys = orders_summary_layer:keys()
        for _, order_id in ipairs(keys) do
            local order = orders_summary_layer:get(order_id)
            if order then
                table.insert(all_orders, {
                    id = order.id or "",
                    customer_did = order.customer or "",
                    items = "",
                    quantity = order.item_count or 0,
                    notes = "",
                    shipping_address = "",
                    status = order.status or "",
                    total = order.total or 0,
                    created_at = order.created_at or ""
                })
            end
        end
    end
    table.sort(all_orders, function(a, b)
        return a.created_at > b.created_at
    end)
    ui:set("orders", all_orders)
end

-- ============================================================================
-- TEST HELPERS
-- ============================================================================

function get_orders_count()
    if orders_summary_layer then
        return #orders_summary_layer:keys()
    end
    return 0
end

function get_products_count()
    if products_layer then
        return products_layer:length()
    end
    return 0
end

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

-- ============================================================================
-- UTILITIES
-- ============================================================================

function generate_id()
    return string.format("%x", os.time()) .. "-" .. string.format("%04x", math.random(0, 65535))
end

function log_info(msg)
    print("[INFO] " .. msg)
end

function log_warn(msg)
    print("[WARN] " .. msg)
end
