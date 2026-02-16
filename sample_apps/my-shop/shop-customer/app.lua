-- Shop Customer App Logic
-- Browse products and manage orders (multiple orders as LIST)
-- Uses scribe:bind() for declarative layer-UI sync

-- Order State Machine
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
    confirmed = { writable = {}, can_delete = false, customer_transitions = {} },
    shipped = { writable = {}, can_delete = false, customer_transitions = {} },
    delivered = { writable = {}, can_delete = false, customer_transitions = {} },
    cancelled = { writable = {}, can_delete = false, customer_transitions = {} }
}

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
local orders_layer = nil
local drafts_layer = nil
local my_did = nil

-- Product cache (for lookups)
local products_cache = {}

-- Form state
local form_state = {
    order_quantity = "1",
    order_notes = "",
    order_address = ""
}

local selected_product = nil
local selected_order_id = nil

-- Initialize the app
function on_init()
    page_id = scribe:page_id()
    my_did = scribe:my_did()

    -- Get layer references for writing
    products_layer = scribe:list(page_id .. "/products")
    drafts_layer = scribe:map(page_id .. "/drafts")
    orders_layer = scribe:list(page_id .. "/orders/" .. my_did)

    -- Declarative bindings with surgical updates via 'key' option
    scribe:bind("products", "products", {
        key = "id",  -- Stable identity for surgical updates
        transform = function(p)
            -- Guard against nil or malformed data
            if not p then return nil end
            -- Update cache for lookups (guard against nil id)
            if p.id then
                products_cache[p.id] = p
            end
            return {
                id = p.id or "",
                name = p.name or "",
                price = p.price or 0,
                description = p.description or "",
                stock = p.stock or 0
            }
        end
    })

    -- Drafts (local-only, map layer) with surgical updates
    scribe:bind("drafts", "drafts", {
        key = "id",  -- Stable identity for surgical updates
        transform = function(d)
            if not d then return nil end
            return {
                id = d.id or "",
                items = d.items or "",
                quantity = d.quantity or 0,
                notes = d.notes or "",
                shipping_address = d.shipping_address or "",
                status = "draft",
                total = d.total or 0,
                created_at = d.created_at or ""
            }
        end
    })

    -- Submitted orders (synced) with key for surgical updates
    -- NOTE: Sorting is done in Slint UI, not here (enables stable indices)
    scribe:bind("my_orders", "orders/{me}", {
        key = "id",
        transform = function(o)
            if not o then return nil end
            return {
                id = o.id or "",
                items = o.items or "",
                quantity = o.quantity or 0,
                notes = o.notes or "",
                shipping_address = o.shipping_address or "",
                status = o.status or "pending",
                total = o.total or 0,
                created_at = o.created_at or ""
            }
        end
    })
end

-- ============================================================================
-- EVENT BUS HANDLERS
-- ============================================================================

function on_click(target)
    local action, id = target:match("^([^:]+):?(.*)$")
    if action == "select_product" then
        do_select_product(id)
    elseif action == "submit_order" then
        do_submit_order(id)
    elseif action == "cancel_order" then
        do_cancel_order(id)
    end
end

function on_field_changed(field_name, value)
    form_state[field_name] = value
end

function on_modal_action(modal_name, action)
    if modal_name == "order" and action == "submit" then
        do_create_order()
    end
end

-- ============================================================================
-- CORE LOGIC
-- ============================================================================

function do_select_product(product_id)
    local product = products_cache[product_id]
    if product then
        selected_product = {
            id = product.id,
            name = product.name,
            price = product.price
        }
        log_info("Selected product: " .. product.name)
    end
end

function do_create_order()
    if permit:role() ~= "customer" then
        log_warn("Only customers can create orders")
        return
    end
    if not drafts_layer then return end

    local product_id = ui:get("selected_product_id")
    if not product_id or product_id == "" then
        log_warn("No product selected")
        return
    end

    if not selected_product or selected_product.id ~= product_id then
        selected_product = products_cache[product_id]
        if not selected_product then
            log_warn("Product not found: " .. product_id)
            return
        end
    end

    local quantity = tonumber(form_state.order_quantity) or 1
    if quantity < 1 then quantity = 1 end

    local order = {
        id = generate_id(),
        items = selected_product.name,
        quantity = quantity,
        notes = form_state.order_notes or "",
        shipping_address = form_state.order_address or "",
        status = "draft",
        total = selected_product.price * quantity,
        created_at = os.date("%Y-%m-%d %H:%M:%S"),
        product_id = selected_product.id
    }

    drafts_layer:set(order.id, order)
    selected_order_id = order.id
    log_info("Order draft created: " .. order.id)

    clear_order_form()
    ui:set("current_view", 1)
    -- UI auto-updates via scribe:bind("drafts", ...)
end

function do_submit_order(order_id)
    if permit:role() ~= "customer" then return end
    if not orders_layer or not drafts_layer then return end

    local target_id = order_id
    if not target_id or target_id == "" then
        target_id = selected_order_id
    end
    if not target_id then return end

    local order = drafts_layer:get(target_id)
    if not order then return end

    if not can_transition_to(order.status, "pending") then
        log_warn("Cannot submit order in " .. (order.status or "unknown") .. " state")
        return
    end

    if not order.items or order.items == "" then
        log_warn("Order must have items")
        return
    end

    if not order.shipping_address or order.shipping_address == "" then
        log_warn("Order must have shipping address")
        return
    end

    order.status = "pending"
    order.submitted_at = os.date("%Y-%m-%d %H:%M:%S")
    orders_layer:push(order)
    drafts_layer:delete(target_id)

    log_info("Order submitted: " .. target_id)
    -- UI auto-updates via scribe:bind() for both drafts and my_orders
end

function do_cancel_order(order_id)
    if permit:role() ~= "customer" then return end

    local target_id = order_id
    if not target_id or target_id == "" then
        target_id = selected_order_id
    end
    if not target_id then return end

    -- Check drafts first
    if drafts_layer then
        local draft = drafts_layer:get(target_id)
        if draft then
            drafts_layer:delete(target_id)
            log_info("Draft deleted: " .. target_id)
            -- UI auto-updates via scribe:bind("drafts", ...)
            return
        end
    end

    -- Check synced orders
    if not orders_layer then return end
    local order_index = find_order_index(target_id)
    if not order_index then return end

    local order = orders_layer:get(order_index)
    local state = ORDER_STATES[order.status or 'draft']

    if not state or not state.can_delete then
        log_warn("Cannot cancel order in " .. (order.status or "unknown") .. " state")
        return
    end

    order.status = "cancelled"
    order.cancelled_at = os.date("%Y-%m-%d %H:%M:%S")
    orders_layer:set(order_index, order)

    log_info("Order cancelled: " .. target_id)
    -- UI auto-updates via scribe:bind("my_orders", ...)
end

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
-- HELPERS
-- ============================================================================

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

function generate_id()
    return string.format("%x", os.time()) .. "-" .. string.format("%04x", math.random(0, 65535))
end

function log_info(msg) print("[INFO] " .. msg) end
function log_warn(msg) print("[WARN] " .. msg) end

-- ============================================================================
-- TEST HELPERS
-- ============================================================================

function get_products_count()
    return products_layer and products_layer:length() or 0
end

function get_first_product_id()
    if products_layer and products_layer:length() > 0 then
        local p = products_layer:get(0)
        return p and p.id or ""
    end
    return ""
end

function get_orders_count()
    return orders_layer and orders_layer:length() or 0
end

function get_drafts_count()
    if drafts_layer then
        local keys = drafts_layer:keys()
        return keys and #keys or 0
    end
    return 0
end

function get_last_order_id()
    return selected_order_id
end

function get_all_orders()
    local orders = {}
    if drafts_layer then
        local keys = drafts_layer:keys()
        if keys then
            for _, key in ipairs(keys) do
                local draft = drafts_layer:get(key)
                if draft then table.insert(orders, draft) end
            end
        end
    end
    if orders_layer then
        local len = orders_layer:length()
        for i = 0, len - 1 do
            local order = orders_layer:get(i)
            if order then table.insert(orders, order) end
        end
    end
    return orders
end

-- ============================================================================
-- API EXPORTS
-- ============================================================================

api.export("select_product", function(product_id)
    do_select_product(product_id)
    ui:set("selected_product_id", product_id)
    local p = products_cache[product_id]
    if p then
        ui:set("selected_product_name", p.name)
        ui:set("selected_product_price", p.price)
    end
    return p ~= nil
end)

api.export("create_order", function(product_id, quantity, notes, address)
    do_select_product(product_id)
    ui:set("selected_product_id", product_id)
    local p = products_cache[product_id]
    if not p then return false end
    ui:set("selected_product_name", p.name)
    ui:set("selected_product_price", p.price)
    form_state.order_quantity = tostring(quantity or 1)
    form_state.order_notes = notes or ""
    form_state.order_address = address or ""
    do_create_order()
    return true
end)

api.export("place_order", function(product_id, quantity, notes, address)
    do_select_product(product_id)
    ui:set("selected_product_id", product_id)
    local p = products_cache[product_id]
    if not p then return false end
    ui:set("selected_product_name", p.name)
    ui:set("selected_product_price", p.price)
    form_state.order_quantity = tostring(quantity or 1)
    form_state.order_notes = notes or ""
    form_state.order_address = address or ""
    do_create_order()
    if selected_order_id then
        do_submit_order(selected_order_id)
    end
    return true
end)

api.export("submit_order", do_submit_order)
api.export("cancel_order", do_cancel_order)
api.export("get_products_count", get_products_count)
api.export("get_first_product_id", get_first_product_id)
api.export("get_orders_count", get_orders_count)
api.export("get_drafts_count", get_drafts_count)
api.export("get_all_orders", get_all_orders)
api.export("get_last_order_id", get_last_order_id)
