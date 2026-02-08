# Renderers

Osvauld supports two renderers: **Slint** (declarative UI) and **Raylib** (immediate-mode graphics).

## When to Use Which

| Use Slint | Use Raylib |
|-----------|------------|
| Forms, dashboards, data-heavy apps | Games, simulations, custom rendering |
| Collaboration tools, chat | Anything needing pixel-level control |
| Standard widget layouts | Real-time animation at high FPS |

---

## Slint Renderer

### AppAPI Global

Every Slint app must have an `AppAPI` global. This is the bridge between Lua and Slint:

```slint
export global AppAPI {
    // Data (Lua writes, Slint reads)
    in-out property<[Product]> products: [];

    // UI state (two-way binding)
    in-out property<string> input_text: "";
    in-out property<bool> modal_visible: false;
    in-out property<int> current_tab: 0;

    // Callbacks (Slint calls, Lua handles)
    callback on_click(string);
    callback on_field_changed(string, string);  // field, value
    callback on_modal_action(string, string);   // modal, action
    callback on_key_pressed(string);
}
```

### Component Naming

The main component **must** be named `App`:

```slint
export component App inherits Rectangle {
    background: #1e293b;
    // ...
}
```

### Structs

Define structs for complex data passed between Lua and Slint:

```slint
export struct Product {
    id: string,
    name: string,
    price: float,
    stock: int,
}
```

### Two-Way Binding

```slint
export component App inherits Rectangle {
    property<string> new-name <=> AppAPI.input_text;

    TextInput {
        text <=> new-name;
        edited => { AppAPI.on_field_changed("name", self.text); }
    }
}
```

### Theme Global

```slint
global Theme {
    out property<color> bg-primary: #0f172a;
    out property<color> bg-secondary: #1e293b;
    out property<color> text-primary: #f1f5f9;
    out property<color> accent: #3b82f6;
    out property<color> success: #22c55e;
    out property<color> warning: #f59e0b;
    out property<color> danger: #ef4444;
}
```

### Keyboard Input

Requires a `FocusScope` to capture key events:

```slint
export component App inherits Rectangle {
    FocusScope {
        key-pressed(event) => {
            AppAPI.on_key_pressed(event.text);
            accept
        }
    }
}
```

Special key names from Slint:
- Arrow keys: `"↑"`, `"↓"`, `"←"`, `"→"`
- Space: `" "`
- Enter: `"\n"`
- Escape: `"\u{1b}"`
- Letters: `"a"`, `"A"`, etc.

### VecModel Pattern

Arrays set via `ui:set` become VecModels. Use `for` loops in Slint to render them:

```slint
for item in AppAPI.products: Rectangle {
    height: 40px;
    Text { text: item.name; }
    TouchArea {
        clicked => { AppAPI.on_click(item.id); }
    }
}
```

### Event Bus Pattern

Route all UI events through typed callbacks:

```slint
// Button click
TouchArea {
    clicked => { AppAPI.on_click("add_item"); }
}

// Form input
TextInput {
    edited => { AppAPI.on_field_changed("name", self.text); }
}

// Modal action
TouchArea {
    clicked => { AppAPI.on_modal_action("add_dialog", "submit"); }
}
```

```lua
function on_click(target)
    if target == "add_item" then add_item() end
end

function on_field_changed(field, value)
    form_state[field] = value
end

function on_modal_action(modal, action)
    if modal == "add_dialog" and action == "submit" then do_add() end
end
```

### Complete Slint Example

```slint
export struct Item {
    id: string,
    name: string,
    done: bool,
}

export global AppAPI {
    in-out property<[Item]> items: [];
    in-out property<string> input_text: "";
    callback on_click(string);
    callback on_field_changed(string, string);
}

global Theme {
    out property<color> bg: #1e293b;
    out property<color> text: #f1f5f9;
    out property<color> accent: #3b82f6;
}

export component App inherits Rectangle {
    background: Theme.bg;

    VerticalLayout {
        padding: 20px;
        spacing: 10px;

        Text {
            text: "Todo List";
            color: Theme.text;
            font-size: 24px;
        }

        HorizontalLayout {
            spacing: 8px;
            TextInput {
                text <=> AppAPI.input_text;
                edited => { AppAPI.on_field_changed("input", self.text); }
            }
            TouchArea {
                clicked => { AppAPI.on_click("add"); }
                Rectangle {
                    background: Theme.accent;
                    Text { text: "Add"; color: white; }
                }
            }
        }

        for item in AppAPI.items: Rectangle {
            height: 40px;
            background: #334155;
            border-radius: 4px;
            HorizontalLayout {
                padding: 10px;
                Text { text: item.name; color: Theme.text; }
            }
            TouchArea {
                clicked => { AppAPI.on_click(item.id); }
            }
        }
    }
}
```

### Slint Gotchas

- Component **must** be named `App`
- **Must** have `export global AppAPI`
- VecModels can be declared in manifest `models` field OR created lazily via `ui:set`
- Keyboard input requires a `FocusScope` wrapping the area
- `tick_enabled: true` in manifest needed for `tick()` callback

---

## Raylib Renderer

Raylib apps use immediate-mode rendering -- all drawing happens in Lua each frame.

### How It Works

- No `.slint` file needed -- all rendering in Lua
- The game loop runs automatically (no `tick_enabled` needed)
- Drawing happens via raylib API calls in `tick()` or the draw callback

### Manifest

```json
{
    "name": "Snake Raylib",
    "version": "1.0.0",
    "description": "Snake game with Raylib",
    "entry_logic": "app.lua",
    "renderer": "raylib",
    "width": 800,
    "height": 600,
    "target_fps": 60
}
```

Note: No `entry_ui` field -- Raylib apps don't use .slint files.

### Raylib-Specific Manifest Fields

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `width` | number | 800 | Window width in pixels |
| `height` | number | 600 | Window height in pixels |
| `target_fps` | number | 60 | Target frame rate |

### Complete Raylib Example

```lua
-- Game state
local snake = {{x = 10, y = 10}}
local direction = {x = 1, y = 0}
local cell_size = 20

function on_init()
    -- Initialize game state
end

function tick()
    -- Update game logic
    move_snake()
    check_collisions()
end

function draw()
    -- Clear screen
    rl.clear_background(0x1e293bFF)

    -- Draw snake
    for _, segment in ipairs(snake) do
        rl.draw_rectangle(
            segment.x * cell_size,
            segment.y * cell_size,
            cell_size - 1,
            cell_size - 1,
            0x22c55eFF
        )
    end

    -- Draw text
    rl.draw_text("Score: " .. #snake, 10, 10, 20, 0xFFFFFFFF)
end

function on_key_pressed(key)
    if key == "w" then direction = {x = 0, y = -1}
    elseif key == "s" then direction = {x = 0, y = 1}
    elseif key == "a" then direction = {x = -1, y = 0}
    elseif key == "d" then direction = {x = 1, y = 0}
    end
end
```

### Raylib Gotchas

- Requires `--features raylib` compile flag when building sthalam
- No `.slint` file -- all rendering in Lua
- Always has a game loop (no `tick_enabled` needed)
- `renderer: "raylib"` must be set in manifest
- If `renderer` is omitted, it defaults to `"slint"`

---

## Emoji Support

For Slint apps using emoji, use emoji-capable fonts:

```slint
Text {
    text: "Hello 😊";
    font-family: "Noto Color Emoji, sans-serif";
}
```

**Linux**: Install emoji fonts: `sudo pacman -S noto-fonts-emoji` (Arch) or `sudo apt install fonts-noto-color-emoji` (Debian/Ubuntu)

**macOS/Windows**: Emoji fonts included by default.
