--[[
Osvauld App API Module

Provides utilities for apps to expose functions for external calling (via control server).

Usage in apps:
    -- Register a function for external calling
    api.export("add_product", add_product)

    -- Add description for AI assistance (optional)
    api.describe("add_product", {
        description = "Add a new product to the catalog",
        params = {
            {name = "name", type = "string", description = "Product name"},
            {name = "price", type = "number", description = "Price in dollars"},
        },
        returns = "product_id",
        effects = {"products layer updated", "UI refreshes product list"},
        syncs = {"products layer propagates to all peers"},
    })

The @ai annotations in LDoc comments are also supported:
    --- @ai Adds product to catalog.
    --- @ai Effects: products layer updated, product_count incremented
    --- @ai Syncs: products layer to all peers
    function add_product(name, price)
        -- ...
    end
--]]

local api = {}

-- Registry of exported functions
-- Maps function_name -> function
api._exports = {}

-- Registry of function descriptions (for AI assistance)
-- Maps function_name -> description table
api._descriptions = {}

--- Export a function for external calling
--- @param name string The name to expose the function as
--- @param fn function The function to export
--- @param description table|nil Optional description table
function api.export(name, fn, description)
    if type(name) ~= "string" then
        error("api.export: name must be a string, got " .. type(name))
    end
    if type(fn) ~= "function" then
        error("api.export: fn must be a function, got " .. type(fn))
    end

    api._exports[name] = fn

    -- Also set as global for backward compatibility with direct calls
    _G[name] = fn

    if description then
        api._descriptions[name] = description
    end
end

--- Add or update description for an exported function
--- @param name string Function name
--- @param description table Description table with fields:
---   - description: string - What the function does
---   - params: table[] - Parameter descriptions [{name, type, description}, ...]
---   - returns: string - What the function returns
---   - effects: string[] - Side effects (UI changes, layer updates)
---   - syncs: string[] - What syncs to other peers
function api.describe(name, description)
    if type(name) ~= "string" then
        error("api.describe: name must be a string")
    end
    if type(description) ~= "table" then
        error("api.describe: description must be a table")
    end

    api._descriptions[name] = description
end

--- Call an exported function by name
--- Used by the control server to invoke functions
--- @param name string Function name
--- @param ... any Arguments to pass
--- @return any Result from the function
function api.call(name, ...)
    local fn = api._exports[name]
    if not fn then
        error("api.call: function '" .. name .. "' not exported")
    end
    return fn(...)
end

--- List all exported function names
--- @return string[] List of exported function names
function api.list()
    local names = {}
    for name, _ in pairs(api._exports) do
        table.insert(names, name)
    end
    table.sort(names)
    return names
end

--- Get description for a function
--- @param name string Function name
--- @return table|nil Description table or nil if not described
function api.get_description(name)
    return api._descriptions[name]
end

--- Get all descriptions
--- @return table Map of function_name -> description
function api.get_all_descriptions()
    return api._descriptions
end

--- Check if a function is exported
--- @param name string Function name
--- @return boolean
function api.has(name)
    return api._exports[name] ~= nil
end

return api
