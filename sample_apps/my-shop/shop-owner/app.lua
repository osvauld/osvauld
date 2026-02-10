-- Shop Owner App Logic
-- Manages products and processes customer orders
-- Uses Event Bus pattern for unified human/AI interaction
--
-- MIGRATED to scribe:bind() declarative API
-- No more: on_loro_change, on_layer_discovered, refresh_*_ui, order_layers tracking

-- Local state
local page_id = nil
local products_layer = nil       -- synced products list
local orders_summary_layer = nil -- Derived layer (aggregated orders)

-- Form state tracking (synced with UI via Event Bus)
local form_state = {
    product_name = "",
    product_price = "",
    product_desc = "",
    product_stock = ""
}

-- Initialize the app
function on_init()
    page_id = scribe:page_id()

    -- Get layer references for writing
    products_layer = scribe:list(page_id .. "/products")
    orders_summary_layer = scribe:map(page_id .. "/derived/orders_summary")

    -- Declarative bindings: layer data auto-syncs to UI properties
    -- Uses 'key' option for surgical updates (only changed products update)
    scribe:bind("products", "products", {
        key = "id",  -- Stable identity for surgical updates
        transform = function(p)
            if not p then return nil end
            return {
                id = p.id or "",
                name = p.name or "",
                price = p.price or 0,
                description = p.description or "",
                stock = p.stock or 0
            }
        end
    })

    -- Orders binding with key for surgical updates
    -- NOTE: Sorting is done in Slint UI, not here (enables stable indices)
    scribe:bind("orders", "derived/orders_summary", {
        key = "id",
        transform = function(o)
            if not o then return nil end
            return {
                id = o.id or "",
                customer_did = o.customer or "",
                items = "",
                quantity = o.item_count or 0,
                notes = "",
                shipping_address = "",
                status = o.status or "",
                total = o.total or 0,
                created_at = o.created_at or ""
            }
        end
    })
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
    -- Handle order status updates via Event Bus pattern
    -- Format: update_order_status:customer_did:order_id:status
    -- Note: customer_did contains colons (did:key:...), so parse from the end
    if target:match("^update_order_status:") then
        local rest = target:sub(#"update_order_status:" + 1)
        -- Parse from end: last part is status, second-to-last is order_id, rest is customer_did
        local new_status = rest:match(":([^:]+)$")
        local without_status = rest:sub(1, #rest - #new_status - 1)
        local order_id = without_status:match(":([^:]+)$")
        local customer_did = without_status:sub(1, #without_status - #order_id - 1)

        if customer_did and order_id and new_status then
            update_order_status(customer_did, order_id, new_status)
        end
    end
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

    -- Get the orders layer for this customer (on demand, no pre-tracking needed)
    local orders_layer_name = page_id .. "/orders/" .. customer_did
    local orders_layer = scribe:list(orders_layer_name)
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
                log_info("Order " .. order_id .. " status updated to " .. new_status)
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
-- TEST HELPERS
-- ============================================================================

function get_orders_count()
    if orders_summary_layer then
        return #orders_summary_layer:keys()
    end
    return 0
end

-- Debug helper: check internal state
function debug_state()
    return {
        page_id = page_id,
        has_products_layer = products_layer ~= nil,
        has_orders_summary_layer = orders_summary_layer ~= nil,
        all_layers = scribe:list_layers("*")
    }
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

-- Get order status by order ID (from summary layer)
function get_order_status(order_id)
    if not orders_summary_layer then return nil end
    local order = orders_summary_layer:get(order_id)
    if order then
        return order.status
    end
    return nil
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

-- ============================================================================
-- API EXPORTS
-- Register functions for external calling via control server
-- ============================================================================

--- @ai Adds a product to the catalog via UI simulation.
--- @ai Effects: Opens modal, fills form, submits. Product appears in products list.
--- @ai Syncs: products layer propagates to viewers.
api.export("add_product_via_ui", add_product_via_ui)
api.describe("add_product_via_ui", {
    description = "Add a new product via UI flow (modal open, fill, submit)",
    params = {
        {name = "name", type = "string", description = "Product name"},
        {name = "price", type = "number", description = "Price in dollars"},
        {name = "description", type = "string", description = "Product description"},
        {name = "stock", type = "number", description = "Initial stock quantity"},
    },
    effects = {"products layer updated", "UI modal opens and closes", "products list refreshed"},
    syncs = {"products layer to all peers"},
})

--- @ai Updates order status. Only owner can call this.
--- @ai Effects: Order status changes, order_layers updated.
--- @ai Syncs: order layer propagates to customer.
api.export("update_order_status", update_order_status)
api.describe("update_order_status", {
    description = "Update an order's status (pending->confirmed->shipped->delivered)",
    params = {
        {name = "customer_did", type = "string", description = "Customer's DID"},
        {name = "order_id", type = "string", description = "Order ID"},
        {name = "new_status", type = "string", description = "New status: confirmed, shipped, delivered, cancelled"},
    },
    effects = {"order status updated", "orders UI refreshed"},
    syncs = {"order layer to customer"},
})

--- @ai Returns count of products in catalog.
api.export("get_products_count", get_products_count)

--- @ai Returns count of orders (from summary layer).
api.export("get_orders_count", get_orders_count)

--- @ai Returns list of all products.
api.export("get_products", get_products)

--- @ai Returns order status by order ID (from summary layer).
api.export("get_order_status", get_order_status)

-- Aliases for backward compatibility with integration tests
api.export("add_product", add_product_via_ui)
