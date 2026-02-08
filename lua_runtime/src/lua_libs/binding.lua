-- Reactive Binding System for Fine-Grained UI Updates
--
-- Provides BindingState class for surgical UI updates based on ops.
-- Instead of replacing entire VecModels on every change, this tracks
-- key-to-index mappings and applies ops individually.
--
-- Usage:
--   local state = BindingState.new("my_model", "id")
--   state:process_ops(ops, transform_fn)
--
-- Or use scribe:bind() with the `key` option for automatic integration.

local BindingState = {}
BindingState.__index = BindingState

--- Create a new binding state for tracking model updates
-- @param model_name string The UI model name to sync to
-- @param key_field string|nil Field to use as stable key (default: "id")
-- @return BindingState
function BindingState.new(model_name, key_field)
    return setmetatable({
        model_name = model_name,
        key_field = key_field or "id",
        -- Map from source key to UI index (1-based for Lua)
        key_to_index = {},
        -- Ordered list of (key, transformed_value) pairs
        items = {},
    }, BindingState)
end

--- Process ops from Loro change and apply surgical UI updates
-- @param ops table Array of JsonOp from on_loro_change
-- @param transform function Transform function: (item) -> transformed_item or nil
-- @return number Number of ops processed
function BindingState:process_ops(ops, transform)
    if not ops or #ops == 0 then
        return 0
    end

    local processed = 0

    for _, op in ipairs(ops) do
        if op.op == "set" or op.op == "insert" then
            -- Map set/insert: key is in op.key, value is in op.value
            local key = op.key or (op.value and op.value[self.key_field])
            if key and op.value then
                local transformed = transform and transform(op.value) or op.value

                if transformed then
                    local idx = self.key_to_index[key]
                    if idx then
                        -- Update existing: surgical set
                        self.items[idx] = { key = key, value = transformed }
                        ui:update(self.model_name, idx - 1, transformed)  -- 0-based for Slint
                    else
                        -- Insert new: append
                        local new_idx = #self.items + 1
                        self.key_to_index[key] = new_idx
                        self.items[new_idx] = { key = key, value = transformed }
                        ui:push(self.model_name, transformed)
                    end
                    processed = processed + 1
                else
                    -- Transform returned nil: filter out, remove if exists
                    self:remove_by_key(key)
                    processed = processed + 1
                end
            end

        elseif op.op == "delete" then
            local key = op.key
            if key then
                self:remove_by_key(key)
                processed = processed + 1
            end

        elseif op.op == "update" then
            -- Map update: same as set for our purposes
            local key = op.key
            if key and op.value then
                local transformed = transform and transform(op.value) or op.value
                local idx = self.key_to_index[key]
                if idx and transformed then
                    self.items[idx] = { key = key, value = transformed }
                    ui:update(self.model_name, idx - 1, transformed)
                    processed = processed + 1
                elseif not transformed then
                    self:remove_by_key(key)
                    processed = processed + 1
                end
            end
        end
    end

    return processed
end

--- Remove item by key, shifting subsequent indices
-- @param key string The key to remove
function BindingState:remove_by_key(key)
    local idx = self.key_to_index[key]
    if not idx then return end

    -- Remove from key map
    self.key_to_index[key] = nil

    -- Remove from items array
    table.remove(self.items, idx)

    -- Remove from UI
    ui:remove(self.model_name, idx - 1)  -- 0-based for Slint

    -- Update indices for shifted items
    for k, i in pairs(self.key_to_index) do
        if i > idx then
            self.key_to_index[k] = i - 1
        end
    end
end

--- Initialize state from full data (for initial load or fallback)
-- @param full_data table Map or array of items
-- @param transform function Transform function: (item) -> transformed_item or nil
function BindingState:init_from_data(full_data, transform)
    -- Clear existing state
    self.key_to_index = {}
    self.items = {}

    local transformed_items = {}

    -- Handle both map and array formats
    if type(full_data) == "table" then
        for k, v in pairs(full_data) do
            local key = type(k) == "string" and k or (v and v[self.key_field]) or tostring(k)
            local transformed = transform and transform(v) or v

            if transformed then
                local idx = #self.items + 1
                self.key_to_index[key] = idx
                self.items[idx] = { key = key, value = transformed }
                table.insert(transformed_items, transformed)
            end
        end
    end

    -- Replace entire model with transformed items
    ui:set(self.model_name, transformed_items)
end

--- Get current item count
-- @return number
function BindingState:count()
    return #self.items
end

--- Get all items as array
-- @return table Array of values
function BindingState:values()
    local result = {}
    for _, item in ipairs(self.items) do
        table.insert(result, item.value)
    end
    return result
end

--- Check if key exists
-- @param key string
-- @return boolean
function BindingState:has(key)
    return self.key_to_index[key] ~= nil
end

--- Get item by key
-- @param key string
-- @return any|nil
function BindingState:get(key)
    local idx = self.key_to_index[key]
    if idx then
        return self.items[idx].value
    end
    return nil
end

-- Export module
return {
    BindingState = BindingState,
}
