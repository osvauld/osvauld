# App Development

Building Lua + Slint apps for osvauld.

## Overview

Osvauld apps are built with:
- **Lua**: Business logic, data handling
- **Slint**: UI definition, rendering

```
my-app/
├── manifest.json    # App metadata
├── app.slint        # UI definition
└── app.lua          # Business logic
```

## Quick Start

### 1. Create manifest.json

```json
{
    "name": "My App",
    "version": "1.0.0",
    "description": "A simple osvauld app",
    "entry_ui": "app.slint",
    "entry_logic": "app.lua"
}
```

**Optional fields:**
- `models`: Pre-declare VecModels (optional - created lazily when first used)
- `tick_enabled`: Set to `true` for games/animations that use `tick()` callback

### 2. Define UI (app.slint)

```slint
// Data structure
export struct Item {
    id: string,
    name: string,
    done: bool,
}

// App API - bridge between Lua and Slint
export global AppAPI {
    in-out property<[Item]> items: [];
    callback on_click(string);
}

export component App inherits Rectangle {
    background: #1e293b;

    VerticalLayout {
        padding: 20px;
        spacing: 10px;

        Text {
            text: "My App";
            font-size: 24px;
            color: white;
        }

        for item in AppAPI.items: Rectangle {
            height: 40px;
            background: #334155;
            border-radius: 8px;

            HorizontalLayout {
                padding: 10px;

                Text {
                    text: item.name;
                    color: white;
                }
            }

            TouchArea {
                clicked => { AppAPI.on_click(item.id); }
            }
        }
    }
}
```

### 3. Write Logic (app.lua)

```lua
-- State
local page_id = nil
local items_layer = nil

-- Initialize
function on_init()
    page_id = permit:page_id()
    items_layer = loro:get_or_create_layer(page_id .. "/items", "list")
    refresh_ui()
end

-- Refresh UI from Loro
function refresh_ui()
    local items = {}
    local len = items_layer:length()

    for i = 0, len - 1 do
        local item = items_layer:get(i)
        if item then
            table.insert(items, item)
        end
    end

    ui:set("items", items)
end

-- Handle clicks
function on_click(target)
    -- Toggle item done state
    local len = items_layer:length()
    for i = 0, len - 1 do
        local item = items_layer:get(i)
        if item and item.id == target then
            item.done = not item.done
            items_layer:set(i, item)
            break
        end
    end
    refresh_ui()
end

-- Handle Loro changes (from sync)
function on_loro_change(layer_name, change_type)
    if layer_name:match("/items$") then
        refresh_ui()
    end
end
```

## Lua API Reference

### Permit API

```lua
-- Get page ID
local page_id = permit:page_id()

-- Get user's role
local role = permit:role()  -- "owner", "collaborator", "viewer"

-- Get user's DID
local my_did = permit:my_did()

-- Check write capability
if permit:can_write("layer_name") then
    -- ...
end
```

### Loro API

```lua
-- Get or create layer
local layer = loro:get_or_create_layer(name, type)
-- type: "map", "list", "text"

-- Get existing layer (may be nil)
local layer = loro:get_layer(name, type)

-- List layers matching pattern
local layers = loro:list_layers(page_id .. "/orders/*")
```

### Map Layer Operations

```lua
local map = loro:get_or_create_layer(page_id .. "/data", "map")

-- Set value
map:set("key", { field = "value" })

-- Get value
local value = map:get("key")

-- Delete key
map:delete("key")

-- Get all keys
local keys = map:keys()
```

### List Layer Operations

```lua
local list = loro:get_or_create_layer(page_id .. "/items", "list")

-- Append
list:push({ id = "1", name = "Item" })

-- Get by index (0-based)
local item = list:get(0)

-- Update at index
list:set(0, { id = "1", name = "Updated" })

-- Get length
local count = list:length()
```

### UI API

```lua
-- Set property
ui:set("property_name", value)

-- Get property
local value = ui:get("property_name")

-- Update single array item (efficient)
ui:update("array_name", index, new_item)
```

### Butler API

```lua
-- Send ephemeral message (for cursors, typing)
butler:send_ephemeral('{"type":"typing"}')
```

### Export API

```lua
-- Export function for testing/AI
api.export("function_name", function_ref)

-- Describe function (optional, for AI)
api.describe("function_name", {
    description = "What it does",
    params = {
        { name = "arg1", type = "string", description = "First arg" }
    },
    effects = { "what changes" },
    syncs = { "what syncs to peers" }
})
```

## Lifecycle Callbacks

```lua
-- Called once when app starts
function on_init()
end

-- Called every frame (~60fps) for games/animations
-- Requires "tick_enabled": true in manifest.json
function tick()
end

-- Called when Loro layer changes
function on_loro_change(layer_name, change_type)
end

-- Called when new layer discovered
function on_layer_discovered(layer_name)
end

-- Called when ephemeral message received
function on_ephemeral(user_did, payload)
end

-- Called when peer joins
function on_peer_joined(user_did)
end

-- Called when peer leaves
function on_peer_left(user_did)
end
```

## Slint Patterns

### AppAPI Global

Every app should have an AppAPI global for Lua communication:

```slint
export global AppAPI {
    // Data (Lua writes, Slint reads)
    in-out property<[Item]> items: [];

    // UI state (two-way binding)
    in-out property<string> input_text: "";
    in-out property<bool> modal_visible: false;

    // Callbacks (Slint calls, Lua handles)
    callback on_click(string);
    callback on_field_changed(string, string);  // field, value
}
```

### Event Bus Pattern

Route all events through typed callbacks:

```slint
// In Slint
callback on_click(string);
callback on_field_changed(string, string);
callback on_modal_action(string, string);  // modal, action

// Button click
TouchArea {
    clicked => { AppAPI.on_click("add_item"); }
}

// Form input
TextInput {
    edited => { AppAPI.on_field_changed("name", self.text); }
}

// Modal submit
TouchArea {
    clicked => { AppAPI.on_modal_action("add_dialog", "submit"); }
}
```

```lua
-- In Lua
function on_click(target)
    if target == "add_item" then
        add_item()
    end
end

function on_field_changed(field, value)
    form_state[field] = value
end

function on_modal_action(modal, action)
    if modal == "add_dialog" and action == "submit" then
        do_add()
    end
end
```

### Theme Global

Define colors in a Theme global:

```slint
global Theme {
    out property<color> bg-primary: #0f172a;
    out property<color> bg-secondary: #1e293b;
    out property<color> text-primary: #f1f5f9;
    out property<color> accent: #3b82f6;
}

export component App {
    background: Theme.bg-primary;

    Text {
        color: Theme.text-primary;
    }
}
```

### Structs for Data

Define structs for complex data:

```slint
export struct Product {
    id: string,
    name: string,
    price: float,
    stock: int,
}

export global AppAPI {
    in-out property<[Product]> products: [];
}
```

## Common Patterns

### Refresh UI from Loro

```lua
function refresh_items_ui()
    local items = {}
    local len = items_layer:length()

    for i = 0, len - 1 do
        local item = items_layer:get(i)
        if item then
            table.insert(items, {
                id = item.id or "",
                name = item.name or "",
                -- ... map fields
            })
        end
    end

    ui:set("items", items)
end
```

### Generate IDs

```lua
function generate_id()
    return string.format("%x-%04x", os.time(), math.random(0, 65535))
end
```

### JSON Encoding (Simple)

```lua
-- Encode simple object
local function encode_json(t)
    local parts = {}
    for k, v in pairs(t) do
        if type(v) == "string" then
            table.insert(parts, '"' .. k .. '":"' .. v .. '"')
        elseif type(v) == "number" then
            table.insert(parts, '"' .. k .. '":' .. v)
        elseif type(v) == "boolean" then
            table.insert(parts, '"' .. k .. '":' .. (v and "true" or "false"))
        end
    end
    return "{" .. table.concat(parts, ",") .. "}"
end
```

### Throttled Actions

```lua
local last_action = 0

function throttled_action()
    local now = os.time()
    if now - last_action < 2 then
        return  -- Skip if called within 2 seconds
    end
    last_action = now

    -- Do action
end
```

## Testing

### Export Test Helpers

```lua
-- Export functions for integration tests
api.export("get_items_count", function()
    return items_layer:length()
end)

api.export("add_item_via_ui", function(name)
    on_field_changed("name", name)
    on_modal_action("add_dialog", "submit")
end)
```

### Integration Test Example

```rust
#[tokio::test]
async fn test_add_item() {
    let runtime = setup_test_runtime("my-app").await;

    // Call exported function
    runtime.call_lua("add_item_via_ui", &["Test Item"]).await;

    // Verify
    let count: i32 = runtime.call_lua("get_items_count", &[]).await;
    assert_eq!(count, 1);
}
```

## Keyboard Input

For games and interactive apps that need keyboard input, use the `on_key_pressed` callback.

### Slint Setup

```slint
export global AppAPI {
    // Keyboard callback - receives key name as string
    callback on_key_pressed(string);
}

export component App inherits Rectangle {
    // Make app focusable to receive keyboard events
    focus-scope := FocusScope {
        key-pressed(event) => {
            AppAPI.on_key_pressed(event.text);
            accept
        }
    }
}
```

### Lua Handler

```lua
function on_key_pressed(key)
    if key == "w" or key == "W" then
        move_up()
    elseif key == "a" or key == "A" then
        move_left()
    elseif key == "s" or key == "S" then
        move_down()
    elseif key == "d" or key == "D" then
        move_right()
    elseif key == " " then  -- Space
        toggle_pause()
    end
end
```

### Special Keys

Common key names from Slint:
- Arrow keys: `"↑"`, `"↓"`, `"←"`, `"→"`
- Space: `" "`
- Enter: `"\n"`
- Escape: `"\u{1b}"`
- Letters: `"a"`, `"A"`, etc.

## Emoji Support

The `emoji` binding provides emoji lookup by shortcode.

### Emoji API

```lua
-- Get emoji by shortcode
local smile = emoji:get("smile")  -- Returns "😄" or nil

-- Get emoji name from Unicode
local name = emoji:name("😄")  -- Returns "grinning face with smiling eyes"

-- Search emojis (returns up to 10 matches)
local results = emoji:search("heart")
-- Returns: [{ emoji = "❤️", name = "red heart", shortcode = "heart" }, ...]
```

### Display in Slint

For proper emoji rendering, use emoji-capable fonts:

```slint
Text {
    text: "Hello 😊";
    font-family: "Noto Color Emoji, sans-serif";
}
```

### System Requirements

**Linux**: Install emoji fonts
```bash
sudo apt install fonts-noto-color-emoji  # Debian/Ubuntu
sudo pacman -S noto-fonts-emoji          # Arch
```

**macOS/Windows**: Emoji fonts are included by default.

## Internationalization (i18n)

Use Slint's built-in `@tr()` macro for translatable strings.

### Marking Strings for Translation

```slint
Button {
    text: @tr("Send Message");
}

Text {
    // With context for translators
    text: @tr("Welcome, {}" => user_name);
}

Text {
    // Plural forms
    text: @tr("{} item" | "{} items" % count);
}
```

### Translation Workflow

1. **Mark strings** with `@tr()` in your `.slint` files

2. **Extract strings** to a `.pot` template:
   ```bash
   slint-tr-extractor app.slint -o translations/messages.pot
   ```

3. **Create translations** for each language (`.po` files):
   ```bash
   msginit -i translations/messages.pot -o translations/es.po -l es
   ```

4. **Translate** the `.po` files using any PO editor (Poedit, Lokalize, etc.)

5. **Compile** to binary `.mo` files:
   ```bash
   msgfmt translations/es.po -o translations/es/LC_MESSAGES/messages.mo
   ```

### Directory Structure

```
my-app/
├── manifest.json
├── app.slint
├── app.lua
└── translations/
    ├── messages.pot          # Template
    ├── es/LC_MESSAGES/messages.mo  # Spanish
    └── fr/LC_MESSAGES/messages.mo  # French
```

### Runtime Language Selection

Language is typically set via system locale. Apps can also provide a language picker that stores preference in a Loro layer.

## Best Practices

1. **Separate Concerns**: UI in Slint, logic in Lua
2. **Use Layers for Persistence**: All mutable state in Loro layers
3. **Handle Sync**: Implement `on_loro_change` for real-time updates
4. **Validate Early**: Check permit capabilities before mutations
5. **Efficient Updates**: Use `ui:update()` for single item changes
6. **Export for Testing**: Make functions testable via `api.export()`
